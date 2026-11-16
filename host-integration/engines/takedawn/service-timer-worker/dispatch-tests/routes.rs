// Two controlled direct native routines, not production title handlers.
use crate::gamepkg;
use crate::irql;
use crate::memory;
use crate::registers;
use crate::timers;
fn pass_need(value: bool) -> Result<(), i32> {
    if value {
        Ok(())
    } else {
        Err(-3)
    }
}
pub fn dpc_route_supported(routine: u32) -> bool {
    routine == 0x1100 || routine == 0x2200
}
pub fn dpc_route_call(
    task: &mut irql::Task,
    ram: &mut memory::Memory,
    cpu: &mut registers::Registers,
    routine: u32,
) -> Result<(), i32> {
    pass_need(dpc_route_supported(routine) && task.irql_current(ram)? == 2)?;
    pass_need(
        cpu.fs_base == 0x1000000
            && ram.read_u32(cpu.fs_base)? == u32::MAX
            && ram.read_u8(cpu.fs_base + 0x24)? == 2,
    )?;
    let is_timer = routine == 0x2200;
    let dpc = if is_timer { 0x740120u32 } else { 0x740100 };
    pass_need(
        cpu.esp == 0x7bffdc
            && ram.read_u32(cpu.esp)? == 0
            && ram.read_u32(cpu.esp + 4)? == dpc
            && ram.read_u32(cpu.esp + 8)? == 0xa54e12b8,
    )?;
    for (offset, value) in [
        (12u32, if is_timer { 0 } else { 0x11223344u32 }),
        (16u32, if is_timer { 0 } else { 0x55667788u32 }),
    ] {
        pass_need(ram.read_u32(cpu.esp + offset)? == value)?;
    }
    let count = ram.read_u32(0x742000)?;
    pass_need(count < 2)?;
    ram.write_u32(0x742004 + count * 4, if is_timer { 2 } else { 1 })?;
    ram.write_u32(0x742000, count + 1)?;
    ram.write_u32(0x742010, cpu.fs_base)?;
    ram.write_u32(0x742014, cpu.esp)?;
    let mode = ram.read_u32(0x741000)?;
    if !is_timer {
        if mode == 2 {
            return Err(-5);
        }
        if mode == 3 || mode == 4 {
            gamepkg::local_mailbox_send(ram.read_u64(0x741008)?, &[1u8])?;
            let mut byte = [0u8; 1];
            gamepkg::local_mailbox_receive(ram.read_u64(0x741010)?, 1000, &mut byte)?;
        }
        // Force time to advance before publishing a new immediate timer. A
        // dispatcher sampling its millisecond clock before drain misses it.
        let pause = gamepkg::local_mailbox_create(1, 1)?;
        let mut byte = [0u8; 1];
        let waited = gamepkg::local_mailbox_receive(pause, 3, &mut byte);
        gamepkg::local_mailbox_release(pause)?;
        pass_need(waited == Err(-2))?;
        let table = timers::Timers::timers_borrow(ram, ram.read_u64(0x741018)?)?;
        table.timer_set(ram, 0x745000, 0, 0, 0x740120)?;
    } else if mode == 1 {
        return Err(-5);
    }
    cpu.eax = cpu.eax.wrapping_add(routine);
    cpu.esp = cpu.esp.wrapping_add(20);
    Ok(())
}
