use crate::gamepkg;
use crate::kernel_pool;
use crate::kernel_startup;
use crate::memory;
use crate::registers;
fn cs_image_need(value: bool) -> Result<(), i32> {
    if value {
        Ok(())
    } else {
        Err(-891)
    }
}
pub fn critical_image_calls(
    ram: &mut memory::Memory,
    owner: &kernel_startup::Startup,
) -> Result<[u64; 5], i32> {
    let ids = owner.startup_ids(ram)?;
    let (_, context) = gamepkg::local_record_snapshot(ids[2], 96)?;
    cs_image_need(
        context.len() == 96
            && u64::from_le_bytes([
                context[0], context[1], context[2], context[3], context[4], context[5], context[6],
                context[7],
            ]) == 0x3543494b,
    )?;
    let (map, domain) = owner.startup_criticals(ram)?;
    let base = ram.read_u32(0x4008)?;
    let canary = base.checked_add(0x5000).ok_or(-1)?;
    let original = ram.read_u32(canary)?;
    let guest = 0xfffffff0u32;
    for slot in [10u32, 11, 11, 10, 12, 12] {
        let sp = base.checked_add(0x2000).ok_or(-1)?;
        ram.write_u32(sp, 0x87654321)?;
        ram.write_u32(sp + 4, guest)?;
        let mut state = registers::Registers::new();
        state.esp = sp;
        state.eax = 0xdeadbeef;
        state.ebx = 0x12345678;
        state.ecx = 0x33333333;
        state.edx = 0x44444444;
        state.esi = 0x55555555;
        state.edi = 0x66666666;
        state.ebp = 0x77777777;
        let mut expected = registers::Registers::new();
        expected.decode(&state.encode())?;
        expected.eax = 0;
        expected.esp = sp + 8;
        kernel_pool::kernel_invoke(&mut state, ram, slot)?;
        cs_image_need(state.encode() == expected.encode())?;
    }
    cs_image_need(
        ram.read_u32(canary)? == original && domain.criticals_counts(ram, &map)? == [2, 2],
    )?;
    let mut guard = map.shadow_lock(ram)?;
    let native = guard.shadow_lookup(ram, guest)?.ok_or(-4)?;
    guard.shadow_guard_close(ram)?;
    cs_image_need(!gamepkg::local_mutex_state(native)?.locked)?;
    // Null is a no-op only in a fully bound context, as in the pinned bridge.
    for slot in [10u32, 11, 12] {
        let sp = base.checked_add(0x2000).ok_or(-1)?;
        ram.write_u32(sp, 0x87654321)?;
        ram.write_u32(sp + 4, 0)?;
        let mut state = registers::Registers::new();
        state.esp = sp;
        state.eax = 0xdeadbeef;
        kernel_pool::kernel_invoke(&mut state, ram, slot)?;
        cs_image_need(state.eax == 0 && state.esp == sp + 8)?;
    }
    cs_image_need(domain.criticals_counts(ram, &map)? == [2, 2])?;
    Ok([2, 2, native, map.shadow_id(), domain.criticals_id()])
}
