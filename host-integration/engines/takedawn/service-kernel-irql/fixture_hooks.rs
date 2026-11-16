// Checked fixture only: controlled pending interrupt and native callback.
// Production device handlers and profiling/diagnostics are not supplied here.
use crate::gate;
use crate::irql;
use crate::memory;
use crate::registers;
fn fixture_trace(
    task: &irql::Task,
    ram: &mut memory::Memory,
    kind: u64,
    flags: u32,
    level_next: u8,
) -> Result<(), i32> {
    let count = ram.read_u32(0x741000)?;
    if count >= 256 {
        return Err(-1);
    }
    let v = task.task_values(ram)?;
    let owner = irql::Controller::controller_borrow(ram, task.task_key()[0])?;
    let counts = owner.controller_counts(ram)?;
    let lock = gate::Gate::gate_borrow(ram, owner.controller_gate())?;
    let values = [
        kind,
        v[0],
        v[1],
        v[2],
        v[3],
        v[4],
        v[5],
        counts[0],
        counts[1],
        counts[2],
        lock.gate_state(ram)?[1],
        u64::from(flags),
        u64::from(ram.read_u8(0x740024)?),
        u64::from(level_next),
    ];
    let mut bytes = [0u8; 128];
    for lane in 0..14usize {
        let value = u64::to_le_bytes(values[lane]);
        for byte in 0..8usize {
            bytes[lane * 8 + byte] = value[byte];
        }
    }
    ram.write_bytes(0x744000 + count * 128, &bytes)?;
    ram.write_u32(0x741000, count + 1)
}
pub fn safe_point(
    task: &mut irql::Task,
    ram: &mut memory::Memory,
    cpu: &mut registers::Registers,
) -> Result<(), i32> {
    fixture_trace(task, ram, 1, cpu.eflags, 0)?;
    let v = task.task_values(ram)?;
    let owner = irql::Controller::controller_borrow(ram, task.task_key()[0])?;
    let lock = gate::Gate::gate_borrow(ram, owner.controller_gate())?;
    if ram.read_u8(0x741008)? == 0
        || lock.gate_state(ram)?[1] == 0
        || v[3] != 0
        || v[4] != 0
        || v[0] > 2
    {
        return Ok(());
    }
    ram.write_u8(0x741008, 0)?;
    let saved_cpu = cpu.encode();
    task.irql_set_isr(ram, true)?;
    let saved = task.irql_enter_interrupt(ram, cpu, 16)?;
    cpu.eax = 0x31415926;
    cpu.xmm7 = [9, 8, 7, 6];
    cpu.mm3 = [0xffffffff, 0x80000000];
    cpu.eflags ^= 0x1234;
    // The native callback returns at DISPATCH; the IRQ epilogue must restore
    // whichever level was interrupted, without leaking another counted raise.
    task.irql_raise(ram, cpu, 2)?;
    task.irql_leave_interrupt(ram, cpu, saved)?;
    task.irql_set_isr(ram, false)?;
    cpu.irq_restore(&saved_cpu)?;
    Ok(())
}
pub fn release_window(
    task: &mut irql::Task,
    ram: &mut memory::Memory,
    cpu: &mut registers::Registers,
) -> Result<(), i32> {
    fixture_trace(task, ram, 2, cpu.eflags, 0)
}
pub fn release_point(
    task: &mut irql::Task,
    ram: &mut memory::Memory,
    cpu: &mut registers::Registers,
) -> Result<(), i32> {
    fixture_trace(task, ram, 3, cpu.eflags, 0)
}
pub fn spin_wake(task: &mut irql::Task, ram: &mut memory::Memory) -> Result<(), i32> {
    fixture_trace(task, ram, 4, 0, 0)
}
pub fn warn(
    task: &mut irql::Task,
    ram: &mut memory::Memory,
    kind: u32,
    _old: u8,
    level_next: u8,
) -> Result<(), i32> {
    fixture_trace(task, ram, 5 + u64::from(kind), 0, level_next)
}
