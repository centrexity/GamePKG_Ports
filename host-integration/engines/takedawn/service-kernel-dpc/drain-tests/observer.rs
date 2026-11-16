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
    if count >= 64 {
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
        ram.read_u32(0x741038)?,
        ram.read_u32(0x74103c)?,
    ] {
        bytes.extend(u32::to_le_bytes(word));
    }
    ram.write_bytes(0x743000 + count * 32, &bytes)?;
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

fn dpc_metric(
    task: &mut irql::Task,
    ram: &mut memory::Memory,
    cpu: &mut registers::Registers,
    kind: u32,
    value: u64,
) -> Result<(), i32> {
    ram.write_u64(0x741038, value)?;
    callback_trace(task, ram, cpu, kind)
}
pub fn dpc_drain_busy(
    task: &mut irql::Task,
    ram: &mut memory::Memory,
    cpu: &mut registers::Registers,
) -> Result<(), i32> {
    if ram.read_u32(0x7410a0)? == 4 {
        return Err(-5);
    }
    if ram.read_u32(0x741004)? == 0 {
        return Ok(());
    }
    dpc_metric(task, ram, cpu, 6, 1)
}
pub fn dpc_drain_depth(
    task: &mut irql::Task,
    ram: &mut memory::Memory,
    cpu: &mut registers::Registers,
    depth: u32,
) -> Result<(), i32> {
    if ram.read_u32(0x7410a0)? == 1 {
        return Err(-5);
    }
    if ram.read_u32(0x741004)? == 0 {
        return Ok(());
    }
    let pending = crate::queue::Queue::queue_borrow(ram, task.irql_hook_context(ram)?)?;
    let b = pending.queue_state(ram)?;
    let head = u64::from_le_bytes([b[56], b[57], b[58], b[59], b[60], b[61], b[62], b[63]]);
    let at = 80 + head as usize * 24;
    let dpc = u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]]);
    let flag = if dpc == 0 {
        0
    } else {
        u64::from(ram.read_u8(dpc + 2)?)
    };
    dpc_metric(
        task,
        ram,
        cpu,
        7,
        u64::from(depth) | ((head * 256 + flag) << 32),
    )
}
pub fn dpc_drain_latency(
    task: &mut irql::Task,
    ram: &mut memory::Memory,
    cpu: &mut registers::Registers,
    timestamp: u64,
) -> Result<(), i32> {
    if ram.read_u32(0x7410a0)? == 2 {
        return Err(-2);
    }
    dpc_metric(task, ram, cpu, 8, timestamp)
}
