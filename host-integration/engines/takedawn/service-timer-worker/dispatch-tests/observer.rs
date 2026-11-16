// Controlled inert profiler only; real title profiling remains required.
use crate::irql;
use crate::memory;
use crate::registers;
pub fn dpc_profile_begin(
    _: &mut irql::Task,
    _: &mut memory::Memory,
    _: &mut registers::Registers,
    _: i32,
    _: u32,
) -> Result<u64, i32> {
    Ok(0)
}
pub fn dpc_profile_start(
    _: &mut irql::Task,
    _: &mut memory::Memory,
    _: &mut registers::Registers,
    _: u64,
) -> Result<(), i32> {
    Ok(())
}
pub fn dpc_profile_end(
    _: &mut irql::Task,
    _: &mut memory::Memory,
    _: &mut registers::Registers,
    _: u64,
    _: bool,
) -> Result<(), i32> {
    Ok(())
}
pub fn dpc_drain_busy(
    _: &mut irql::Task,
    _: &mut memory::Memory,
    _: &mut registers::Registers,
) -> Result<(), i32> {
    Ok(())
}
pub fn dpc_drain_depth(
    _: &mut irql::Task,
    _: &mut memory::Memory,
    _: &mut registers::Registers,
    _: u32,
) -> Result<(), i32> {
    Ok(())
}
pub fn dpc_drain_latency(
    _: &mut irql::Task,
    _: &mut memory::Memory,
    _: &mut registers::Registers,
    _: u64,
) -> Result<(), i32> {
    Ok(())
}
