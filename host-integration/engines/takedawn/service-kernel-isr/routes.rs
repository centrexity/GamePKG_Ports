// Controlled finite native routines for qualification, not title handlers.
use crate::gamepkg;
use crate::irql;
use crate::memory;
use crate::registers;
fn isr_route_need(ok: bool) -> Result<(), i32> {
    if ok {
        Ok(())
    } else {
        Err(-3)
    }
}
pub fn isr_route_supported(routine: u32) -> bool {
    routine == 0x1100 || routine == 0x2200
}
pub fn isr_route_note(ram: &mut memory::Memory, value: u32) -> Result<(), i32> {
    let count = ram.read_u32(0x741004)?;
    isr_route_need(count < 3)?;
    ram.write_u32(0x741008 + count * 4, value)?;
    ram.write_u32(0x741004, count + 1)
}
pub fn isr_route_call(
    task: &mut irql::Task,
    ram: &mut memory::Memory,
    cpu: &mut registers::Registers,
    routine: u32,
) -> Result<(), i32> {
    isr_route_need(
        isr_route_supported(routine)
            && task.irql_current(ram)? == 16
            && ram.read_u8(cpu.fs_base + 0x24)? == 16,
    )?;
    isr_route_need(cpu.fs_base == 0x1000000 && cpu.esp == 0x7bffe4 && ram.read_u32(cpu.esp)? == 0)?;
    let object = ram.read_u32(cpu.esp + 4)?;
    isr_route_need(object == 0x740100 || object == 0x740200)?;
    let context = ram.read_u32(cpu.esp + 8)?;
    isr_route_need(context == ram.read_u32(object + 4)?)?;
    isr_route_note(ram, 2)?;
    cpu.eax = context;
    let fault = ram.read_u32(0x741020)?;
    if fault == 1 {
        return Err(-5);
    }
    if fault == 8 || fault == 9 {
        gamepkg::local_mailbox_send(ram.read_u64(0x741028)?, &[1u8])?;
        let mut byte = [0u8; 1];
        gamepkg::local_mailbox_receive(ram.read_u64(0x741030)?, 1000, &mut byte)?;
        task.task_values(ram)?;
    }
    cpu.esp = cpu.esp.wrapping_add(if fault == 2 { 8 } else { 12 });
    Ok(())
}
