// Literal port-side composition of IRQL safe points with its bound IRQ queue.
use crate::irq_delivery;
use crate::irql;
use crate::memory;
use crate::registers;
fn hook_queue(ram: &memory::Memory, task: &irql::Task) -> Result<irq_delivery::Interrupts, i32> {
    irq_delivery::Interrupts::irq_borrow(ram, task.irql_hook_context(ram)?)
}
pub fn safe_point(
    task: &mut irql::Task,
    ram: &mut memory::Memory,
    cpu: &mut registers::Registers,
) -> Result<(), i32> {
    hook_queue(ram, task)?.irq_safe_point(ram, task, cpu)
}
pub fn release_point(
    task: &mut irql::Task,
    ram: &mut memory::Memory,
    cpu: &mut registers::Registers,
) -> Result<(), i32> {
    hook_queue(ram, task)?.irq_release_point(ram, task, cpu)
}
pub fn release_window(
    task: &mut irql::Task,
    ram: &mut memory::Memory,
    cpu: &mut registers::Registers,
) -> Result<(), i32> {
    crate::irq_observer::release_window(&hook_queue(ram, task)?, task, ram, cpu)
}
pub fn spin_wake(task: &mut irql::Task, ram: &mut memory::Memory) -> Result<(), i32> {
    crate::irq_observer::spin_wake(&hook_queue(ram, task)?, task, ram)
}
pub fn warn(
    task: &mut irql::Task,
    ram: &mut memory::Memory,
    kind: u32,
    old: u8,
    level_next: u8,
) -> Result<(), i32> {
    crate::irq_observer::warn(&hook_queue(ram, task)?, task, ram, kind, old, level_next)
}
