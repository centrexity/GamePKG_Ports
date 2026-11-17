use crate::gamepkg;
use crate::kernel_pool;
use crate::memory;
use crate::registers;
fn event_import_need(stage: i32, ok: bool) -> Result<(), i32> {
    if ok {
        Ok(())
    } else {
        Err(-stage)
    }
}
fn event_import_word(b: &[u8], at: usize) -> Result<u64, i32> {
    let p = b.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        p[0], p[1], p[2], p[3], p[4], p[5], p[6], p[7],
    ]))
}
fn event_import_frame(ram: &mut memory::Memory, args: &[u32]) -> Result<registers::Registers, i32> {
    let mut cpu = registers::Registers::new();
    cpu.esp = ram.read_u32(0x4008)?.checked_add(0x2000).ok_or(-1)?;
    cpu.eax = 0xdeadbeef;
    cpu.ebx = 0xabcdef01;
    cpu.ecx = 0x87654321;
    cpu.eflags = 0x202;
    cpu.fs_base = 0x4000;
    cpu.mm3 = [0x11223344, 0xfedcba98];
    ram.write_u32(cpu.esp, 0x12345678)?;
    for (index, value) in args.iter().enumerate() {
        ram.write_u32(cpu.esp + 4 + index as u32 * 4, *value)?;
    }
    Ok(cpu)
}
fn event_import_call(
    ram: &mut memory::Memory,
    slot: u32,
    args: &[u32],
    status: u32,
) -> Result<(), i32> {
    let mut cpu = event_import_frame(ram, args)?;
    let mut expected = registers::Registers::new();
    expected.decode(&cpu.encode())?;
    expected.eax = status;
    expected.esp += 4 + args.len() as u32 * 4;
    kernel_pool::kernel_invoke(&mut cpu, ram, slot)?;
    event_import_need(
        401,
        cpu.encode() == expected.encode() && ram.kernel_caller() == 0x12345678,
    )
}
fn event_import_error(
    ram: &mut memory::Memory,
    slot: u32,
    args: &[u32],
    code: i32,
) -> Result<(), i32> {
    let mut cpu = event_import_frame(ram, args)?;
    let before = cpu.encode();
    let actual = kernel_pool::kernel_invoke(&mut cpu, ram, slot);
    let same = cpu.encode() == before;
    if actual != Err(code) || !same {
        let observed = match actual {
            Ok(()) => 0,
            Err(value) => value,
        };
        let _ = gamepkg::diagnostic(
            2,
            0x544b3900,
            &[
                u64::from(slot),
                u64::from(args[0]),
                code as i64 as u64,
                observed as i64 as u64,
                u64::from(same),
            ],
        );
        return Err(-402);
    }
    Ok(())
}
fn event_import_id(
    ram: &memory::Memory,
    table: &kernel_pool::Handles,
    token: u32,
) -> Result<u64, i32> {
    let mut guard = table.handles_lock(ram)?;
    let result = guard.handle_guard_lookup(ram, token);
    let closed = guard.handles_guard_close(ram);
    let value = result?.ok_or(-4)?;
    closed?;
    event_import_need(403, value[1] == 1)?;
    Ok(value[0])
}
fn event_import_put(out: &mut [u8], cursor: &mut usize, value: u32) -> Result<(), i32> {
    let encoded = u32::to_le_bytes(value);
    for lane in 0..4usize {
        *out.get_mut(*cursor + lane).ok_or(-1)? = encoded[lane];
    }
    *cursor += 4;
    Ok(())
}
pub fn event_import_vectors(
    ram: &mut memory::Memory,
    table: &kernel_pool::Handles,
    domain: &kernel_pool::Events,
    slots: [u32; 2],
) -> Result<std::vec::Vec<u8>, i32> {
    let base = ram.read_u32(0x4008)?;
    let output = base + 0x100;
    let previous = base + 0x108;
    let mut out = std::vec::Vec::new();
    out.resize(1764, 0);
    let mut cursor = 0usize;
    for event_type in [0u32, 1, 2, 0xffffffff] {
        for initial in [0u32, 1, 0x100, 0x101, 0xffffff00, 0xffffffff] {
            ram.write_u32(output, 0x33333333)?;
            ram.write_u32(output + 4, 0xabcdef01)?;
            event_import_call(ram, slots[0], &[output, 0xffffffff, event_type, initial], 0)?;
            let token = ram.read_u32(output)?;
            event_import_need(
                404,
                token == 0x48000001 && ram.read_u32(output + 4)? == 0xabcdef01,
            )?;
            let state = domain.events_state(ram, table, token)?;
            event_import_need(405, state == [event_type == 0, initial & 255 != 0])?;
            ram.write_u32(previous, 0x55555555)?;
            event_import_call(ram, slots[1], &[token, previous], 0)?;
            let after = domain.events_state(ram, table, token)?;
            event_import_need(406, after[1] && ram.read_u32(previous)? == 0)?;
            for value in [
                0,
                token,
                u32::from(state[0]),
                u32::from(state[1]),
                ram.read_u32(previous)?,
                0,
                u32::from(after[1]),
            ] {
                event_import_put(&mut out, &mut cursor, value)?;
            }
            let id = event_import_id(ram, table, token)?;
            domain.events_remove(ram, table, token)?;
            event_import_need(407, gamepkg::local_event_state(id).err() == Some(-4))?;
        }
    }
    for token in [
        0u32, 0xffffffff, 0xfffffffe, 0xdead0001, 0xbeef0010, 0x48000000, 0x48004000, 0x48003fff,
        0x49000001,
    ] {
        ram.write_u32(previous, 0x55555555)?;
        event_import_call(ram, slots[1], &[token, previous], 0xc0000008)?;
        event_import_need(408, ram.read_u32(previous)? == 0)?;
        event_import_put(&mut out, &mut cursor, 0xc0000008)?;
        event_import_put(&mut out, &mut cursor, 0)?;
    }
    event_import_call(ram, slots[0], &[0, 0xffffffff, 1, 0x100], 0)?;
    event_import_call(ram, slots[0], &[output, 0xffffffff, 0, 1], 0)?;
    let token = ram.read_u32(output)?;
    event_import_need(409, token == 0x48000001)?;
    for value in [0, 0, token] {
        event_import_put(&mut out, &mut cursor, value)?;
    }
    domain.events_remove(ram, table, token)?;
    for index in 0..126u32 {
        ram.write_u32(output, 0x33333333)?;
        let status = if index < 125 { 0 } else { 0xc000009a };
        event_import_call(ram, slots[0], &[output, 0xffffffff, 0, 0], status)?;
        let token = ram.read_u32(output)?;
        event_import_need(
            410,
            token == if index < 125 { 0x48000001 + index } else { 0 },
        )?;
        event_import_put(&mut out, &mut cursor, status)?;
        event_import_put(&mut out, &mut cursor, token)?;
    }
    event_import_need(411, cursor == out.len())?;
    for index in 0..125u32 {
        domain.events_remove(ram, table, 0x48000001 + index)?;
    }
    Ok(out)
}
pub fn event_import_controls(
    ram: &mut memory::Memory,
    table: &kernel_pool::Handles,
    domain: &kernel_pool::Events,
    slots: [u32; 2],
) -> Result<(), i32> {
    let base = ram.read_u32(0x4008)?;
    let output = base + 0x100;
    let previous = base + 0x108;
    // Accepted creation/tag remain owned even when the guest output write fails.
    event_import_error(ram, slots[0], &[0x90000000, 0xffffffff, 0, 0], -4)?;
    let id = event_import_id(ram, table, 0x48000001)?;
    event_import_need(412, !gamepkg::local_event_state(id)?.signaled)?;
    domain.events_remove(ram, table, 0x48000001)?;
    let backing = ram.memory_writable_word_backing(output)?;
    ram.write_u32(output, 0x11223344)?;
    event_import_protect(backing, 1)?;
    event_import_error(ram, slots[0], &[output, 0xffffffff, 0, 0], -4)?;
    event_import_need(413, ram.read_u32(output)? == 0x11223344)?;
    event_import_protect(backing, 2)?;
    domain.events_remove(ram, table, 0x48000001)?;
    event_import_call(ram, slots[0], &[output, 0xffffffff, 0, 0], 0)?;
    let token = ram.read_u32(output)?;
    let id = event_import_id(ram, table, token)?;
    event_import_error(ram, slots[1], &[token, 0x90000000], -4)?;
    event_import_need(414, !gamepkg::local_event_state(id)?.signaled)?;
    ram.write_u32(previous, 0x55555555)?;
    event_import_protect(backing, 1)?;
    event_import_error(ram, slots[1], &[token, previous], -4)?;
    event_import_need(
        415,
        !gamepkg::local_event_state(id)?.signaled && ram.read_u32(previous)? == 0x55555555,
    )?;
    event_import_protect(backing, 2)?;
    // Valid numeric tags with inaccessible/corrupt table pages are service
    // failures, not ordinary invalid handles. PreviousState is already accepted.
    let scratch = crate::layout::Layout::layout_services(ram)?.layout_scratch()?;
    event_import_protect([scratch, 0], 0)?;
    event_import_error(ram, slots[1], &[token, previous], -4)?;
    event_import_need(423, ram.read_u32(previous)? == 0 && !gamepkg::local_event_state(id)?.signaled)?;
    event_import_protect([scratch, 0], 2)?;
    let mut magic = [0u8; 1];
    gamepkg::local_call(5, &[u64::to_le_bytes(scratch).as_slice(), &u64::to_le_bytes(0), &u32::to_le_bytes(1)].concat(), &mut magic)?;
    gamepkg::local_call(6, &[u64::to_le_bytes(scratch).as_slice(), &u64::to_le_bytes(0), &[0u8]].concat(), &mut [])?;
    event_import_error(ram, slots[1], &[token, previous], -1)?;
    event_import_need(424, ram.read_u32(previous)? == 0 && !gamepkg::local_event_state(id)?.signaled)?;
    gamepkg::local_call(6, &[u64::to_le_bytes(scratch).as_slice(), &u64::to_le_bytes(0), &magic].concat(), &mut [])?;
    let mut guard = table.handles_lock(ram)?;
    let wrong = guard.handle_guard_insert(ram, id, 2, true)?;
    guard.handles_guard_close(ram)?;
    event_import_call(ram, slots[1], &[wrong, previous], 0xc0000008)?;
    event_import_need(
        416,
        ram.read_u32(previous)? == 0 && !gamepkg::local_event_state(id)?.signaled,
    )?;
    let mut guard = table.handles_lock(ram)?;
    guard.handle_guard_remove(ram, wrong)?;
    guard.handles_guard_close(ram)?;
    let mut bytes = [0u8; 8];
    gamepkg::local_call(20, &[0u8; 8], &mut bytes)?;
    let foreign = u64::from_le_bytes(bytes);
    let mut guard = table.handles_lock(ram)?;
    let fake = guard.handle_guard_insert(ram, foreign, 1, true)?;
    guard.handles_guard_close(ram)?;
    event_import_call(ram, slots[1], &[fake, previous], 0xc0000008)?;
    event_import_need(417, !gamepkg::local_event_state(foreign)?.signaled)?;
    let mut guard = table.handles_lock(ram)?;
    guard.handle_guard_remove(ram, fake)?;
    guard.handles_guard_close(ram)?;
    gamepkg::local_call(25, &u64::to_le_bytes(foreign), &mut [])?;
    // A valid legacy context cannot make these new object imports ready.
    let map_id = ram.kernel_handle();
    let (_, original) = gamepkg::local_record_snapshot(map_id, 1548)?;
    let context =
        kernel_pool::KernelContext::kernel_context_borrow(ram, event_import_word(&original, 24)?)?;
    let registry = context.kernel_context_registry(ram)?;
    let event_shadow = context.kernel_context_shadow(ram)?;
    let mut legacy =
        kernel_pool::KernelContext::kernel_context_new_with_shadow(ram, &registry, &event_shadow)?;
    event_import_need(
        422,
        kernel_pool::kernel_requires_context(189)
            && kernel_pool::kernel_requires_context(225)
            && kernel_pool::kernel_context_ready(ram, legacy.kernel_context_id(), 189) == Err(-5)
            && kernel_pool::kernel_context_ready(ram, legacy.kernel_context_id(), 225) == Err(-5),
    )?;
    let (version, mut changed) = gamepkg::local_record_snapshot(map_id, 1548)?;
    let encoded = u64::to_le_bytes(legacy.kernel_context_id());
    for lane in 0..8usize {
        changed[24 + lane] = encoded[lane];
    }
    gamepkg::local_record_compare_exchange(map_id, version, &changed)?;
    ram.write_u32(previous, 0x55555555)?;
    event_import_error(ram, slots[1], &[token, previous], -5)?;
    event_import_need(418, ram.read_u32(previous)? == 0x55555555)?;
    let (version, _) = gamepkg::local_record_snapshot(map_id, 1548)?;
    gamepkg::local_record_compare_exchange(map_id, version, &original)?;
    legacy.kernel_context_close(ram)?;
    let services = ram.services_handle()?;
    ram.services_link(services, 0)?;
    event_import_error(ram, slots[1], &[token, previous], -4)?;
    event_import_need(
        419,
        ram.read_u32(previous)? == 0x55555555 && !gamepkg::local_event_state(id)?.signaled,
    )?;
    ram.services_link(0, services)?;
    let alias = domain.events_alias(ram, table, token, true)?;
    event_import_call(ram, slots[1], &[alias, 0], 0)?;
    event_import_need(420, gamepkg::local_event_state(id)?.signaled)?;
    domain.events_remove(ram, table, token)?;
    event_import_need(421, gamepkg::local_event_state(id).is_ok())?;
    domain.events_remove(ram, table, alias)?;
    Ok(())
}
fn event_import_protect(backing: [u64; 2], mode: u32) -> Result<(), i32> {
    gamepkg::local_call(
        3,
        &[
            u64::to_le_bytes(backing[0]).as_slice(),
            &u64::to_le_bytes(backing[1] & !4095),
            &u64::to_le_bytes(4096),
            &u32::to_le_bytes(mode),
        ]
        .concat(),
        &mut [],
    )?;
    Ok(())
}
