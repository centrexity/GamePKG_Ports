// Controlled profiling boundary, not the production profiler.
use crate::irql;
use crate::isr_routes;
use crate::memory;
use crate::registers;
pub fn isr_profile_begin(
    task: &mut irql::Task,
    ram: &mut memory::Memory,
    cpu: &mut registers::Registers,
    _: u32,
) -> Result<u64, i32> {
    if ram.read_u32(0x741000)? == 0 {
        return Ok(0);
    }
    if task.irql_current(ram)? != 16 || cpu.esp != 0x7bffe4 {
        return Err(-3);
    }
    isr_routes::isr_route_note(ram, 1)?;
    if ram.read_u32(0x741020)? == 3 {
        return Err(-1);
    }
    Ok(1)
}
pub fn isr_profile_end(
    task: &mut irql::Task,
    ram: &mut memory::Memory,
    cpu: &mut registers::Registers,
    ticket: u64,
    completed: bool,
) -> Result<(), i32> {
    if ticket == 0 {
        return Ok(());
    }
    if task.irql_current(ram)? != 16 || completed && cpu.esp != 0x7bfff0 {
        return Err(-3);
    }
    isr_routes::isr_route_note(ram, if completed { 3 } else { 4 })?;
    if ram.read_u32(0x741020)? == 4 {
        return Err(-2);
    }
    Ok(())
}
