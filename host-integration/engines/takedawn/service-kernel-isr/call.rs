// Original kernel_call_isr convention, using only finite reviewed Rust routes.
use crate::connections;
use crate::gamepkg;
use crate::irql;
use crate::isr_observer;
use crate::isr_routes;
use crate::memory;
use crate::registers;
use crate::stacks;
pub fn isr_call(
    registry: &connections::Connections,
    ram: &mut memory::Memory,
    task: &mut irql::Task,
    cpu: &mut registers::Registers,
    lease: &stacks::Lease,
    vector: u32,
) -> Result<i32, i32> {
    let object = registry.connection_get(ram, vector)?;
    if object == 0 {
        return Ok(-1);
    }
    let routine = ram.read_u32(object)?;
    let context = ram.read_u32(object.checked_add(4).ok_or(-1)?)?;
    if routine == 0 || !isr_routes::isr_route_supported(routine) {
        return Ok(-1);
    }
    let identity = lease.stack_identity(ram)?;
    let top = lease.stack_guest_top(ram)?;
    let base = top
        .checked_add(16)
        .ok_or(-1)?
        .checked_sub(262144)
        .ok_or(-1)?;
    if !cpu.valid()
        || cpu.fs_base == 0
        || cpu.esp > top
        || cpu.esp < base.checked_add(12).ok_or(-1)?
    {
        return Err(-1);
    }
    if gamepkg::local_task_status()? != (task.task_key()[1], false) || identity[2] == 0 {
        return Err(-4);
    }
    task.task_values(ram)?;
    let old = task.irql_current(ram)?;
    let returned = cpu.esp;
    let result = isr_invoke(ram, task, cpu, object, context, routine, returned, old);
    if let Err(code) = result {
        if task.irql_abort(ram).is_err() {
            return Err(-3);
        }
        return Err(code);
    }
    result
}
fn isr_invoke(
    ram: &mut memory::Memory,
    task: &mut irql::Task,
    cpu: &mut registers::Registers,
    object: u32,
    context: u32,
    routine: u32,
    returned: u32,
    old: u8,
) -> Result<i32, i32> {
    for value in [context, object, 0u32] {
        cpu.esp = cpu.esp.wrapping_sub(4);
        ram.write_u32(cpu.esp, value)?;
    }
    let entered = task.irql_enter_interrupt(ram, cpu, 16);
    let ticket = match entered {
        Ok(_) => isr_observer::isr_profile_begin(task, ram, cpu, routine),
        Err(code) => Err(code),
    };
    let called = match ticket {
        Ok(token) => {
            let native = isr_routes::isr_route_call(task, ram, cpu, routine);
            let completed = native.is_ok() && cpu.esp == returned;
            let ended = isr_observer::isr_profile_end(task, ram, cpu, token, completed);
            match native {
                Err(code) => Err(code),
                Ok(()) if !completed => Err(-3),
                Ok(()) => ended,
            }
        }
        Err(code) => Err(code),
    };
    let restored = task.irql_leave_interrupt(ram, cpu, i32::from(old));
    called?;
    restored?;
    Ok((cpu.eax & 1) as i32)
}
