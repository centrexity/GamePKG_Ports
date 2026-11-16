// Fixture only: no pending IRQ, DPC or spin waiter. NOT production device hooks.
use crate::irql;
use crate::memory;
use crate::registers;
pub fn safe_point(
    _: &mut irql::Task,
    _: &mut memory::Memory,
    _: &mut registers::Registers,
) -> Result<(), i32> {
    Ok(())
}
pub fn release_point(
    _: &mut irql::Task,
    _: &mut memory::Memory,
    _: &mut registers::Registers,
) -> Result<(), i32> {
    Ok(())
}
pub fn release_window(
    _: &mut irql::Task,
    _: &mut memory::Memory,
    _: &mut registers::Registers,
) -> Result<(), i32> {
    Ok(())
}
pub fn spin_wake(_: &mut irql::Task, _: &mut memory::Memory) -> Result<(), i32> {
    Ok(())
}
pub fn warn(_: &mut irql::Task, _: &mut memory::Memory, _: u32, _: u8, _: u8) -> Result<(), i32> {
    Ok(())
}
