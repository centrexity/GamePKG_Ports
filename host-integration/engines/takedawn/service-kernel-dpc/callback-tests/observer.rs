// Controlled profiling observer, not a production profiler or a host callback.
use crate::irql;
use crate::memory;
use crate::registers;
pub fn callback_trace(
    task: &irql::Task,
    ram: &mut memory::Memory,
    cpu: &registers::Registers,
    kind: u32,
) -> Result<(), i32> {
    let count = ram.read_u32(0x741000)?;
    if count >= 8 {
        return Err(-1);
    }
    let mut bytes = std::vec::Vec::new();
    for word in [
        kind,
        u32::from(task.irql_current(ram)?),
        cpu.esp,
        cpu.fs_base,
        ram.read_u32(0x741020)?,
        ram.read_u32(0x741024)?,
    ] {
        bytes.extend(u32::to_le_bytes(word));
    }
    ram.write_bytes(0x743000 + count * 24, &bytes)?;
    ram.write_u32(0x741000, count + 1)
}
pub fn dpc_profile_begin(
    task: &mut irql::Task,
    ram: &mut memory::Memory,
    cpu: &mut registers::Registers,
    profile: i32,
    routine: u32,
) -> Result<u64, i32> {
    if ram.read_u32(0x741004)? == 0 {
        return Ok(0);
    }
    ram.write_u32(0x741020, profile as u32)?;
    ram.write_u32(0x741024, routine)?;
    callback_trace(task, ram, cpu, 1)?;
    Ok(1)
}
pub fn dpc_profile_start(
    task: &mut irql::Task,
    ram: &mut memory::Memory,
    cpu: &mut registers::Registers,
    ticket: u64,
) -> Result<(), i32> {
    if ticket == 0 {
        return Ok(());
    }
    callback_trace(task, ram, cpu, 2)?;
    if ram.read_u32(0x741030)? == 6 {
        return Err(-1);
    }
    Ok(())
}
pub fn dpc_profile_end(
    task: &mut irql::Task,
    ram: &mut memory::Memory,
    cpu: &mut registers::Registers,
    ticket: u64,
    completed: bool,
) -> Result<(), i32> {
    if ticket == 0 {
        return Ok(());
    }
    callback_trace(task, ram, cpu, if completed { 4 } else { 5 })?;
    if ram.read_u32(0x741030)? == 3 {
        return Err(-2);
    }
    Ok(())
}
