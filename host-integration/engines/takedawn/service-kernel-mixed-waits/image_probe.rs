// Authenticated KIC4 consumer using actual create/set/release imports and owners.
use crate::kernel_pool;
use crate::memory;
use crate::registers;
fn mixed_image_need(value: bool) -> Result<(), i32> {
    if value {
        Ok(())
    } else {
        Err(-761)
    }
}
fn mixed_image_call(ram: &mut memory::Memory, slot: u32, args: &[u32]) -> Result<(), i32> {
    let mut cpu = registers::Registers::new();
    cpu.esp = ram.read_u32(0x4008)?.checked_add(0x2000).ok_or(-1)?;
    cpu.eax = 0xdeadbeef;
    cpu.ebx = 0x89abcdef;
    cpu.edx = 0x76543210;
    cpu.eflags = 0x202;
    cpu.mm7 = [0xabcdef12, 0xfedcba98];
    ram.write_u32(cpu.esp, 0x87654321)?;
    for (index, value) in args.iter().enumerate() {
        ram.write_u32(cpu.esp + 4 + index as u32 * 4, *value)?;
    }
    let mut expected = registers::Registers::new();
    expected.decode(&cpu.encode())?;
    expected.eax = 0;
    expected.esp += 4 + args.len() as u32 * 4;
    kernel_pool::kernel_invoke(&mut cpu, ram, slot)?;
    mixed_image_need(cpu.encode() == expected.encode() && ram.kernel_caller() == 0x87654321)
}
pub fn mixed_image_calls(
    ram: &mut memory::Memory,
    table: &kernel_pool::Handles,
    native_events: &kernel_pool::Events,
    semaphores: &kernel_pool::Semaphores,
    slots: [u32; 4],
) -> Result<[u64; 8], i32> {
    let base = ram.read_u32(0x4008)?;
    base.checked_add(0x7200).ok_or(-1)?;
    let event_out = base + 0x7000;
    let sem_out = base + 0x7010;
    mixed_image_call(ram, slots[0], &[event_out, 0, 1, 0])?;
    mixed_image_call(ram, slots[2], &[sem_out, 0, 1, 2])?;
    let event = ram.read_u32(event_out)?;
    let sem = ram.read_u32(sem_out)?;
    {
        let mut mixed =
            kernel_pool::MixedWait::mixed_wait_new(ram, table, native_events, semaphores)?;
        mixed.mixed_wait_prepare(ram, &[event, sem])?;
        let first = mixed.mixed_wait(ram, true, 0);
        mixed_image_need(first == Err(-2) && mixed.mixed_wait_native_result() == Err(-2))?;
        mixed.mixed_wait_close(ram)?;
        mixed_image_need(
            mixed.mixed_wait_closed() && semaphores.semaphores_state(ram, table, sem)?[0] == 1,
        )?;
    }
    mixed_image_call(ram, slots[1], &[event, 0])?;
    let second = {
        let mut mixed =
            kernel_pool::MixedWait::mixed_wait_new(ram, table, native_events, semaphores)?;
        mixed.mixed_wait_prepare(ram, &[event, sem])?;
        let second = mixed.mixed_wait(ram, true, 0)?;
        mixed_image_need(second.index == 0 && !second.abandoned)?;
        mixed.mixed_wait_close(ram)?;
        second
    };
    mixed_image_need(
        native_events.events_state(ram, table, event)? == [false, false]
            && semaphores.semaphores_state(ram, table, sem)?[0] == 0,
    )?;
    mixed_image_call(ram, slots[3], &[sem, 2, 0])?;
    mixed_image_call(ram, slots[1], &[event, 0])?;
    let third = {
        let mut mixed =
            kernel_pool::MixedWait::mixed_wait_new(ram, table, native_events, semaphores)?;
        mixed.mixed_wait_prepare(ram, &[sem, event])?;
        let third = mixed.mixed_wait(ram, false, 0)?;
        mixed_image_need(third.index == 0 && !third.abandoned)?;
        mixed.mixed_wait_close(ram)?;
        third
    };
    let state = native_events.events_state(ram, table, event)?;
    let count = semaphores.semaphores_state(ram, table, sem)?[0];
    mixed_image_need(state == [false, true] && count == 1)?;
    native_events.events_remove(ram, table, event)?;
    semaphores.semaphores_remove(ram, table, sem)?;
    let mut guard = table.handles_lock(ram)?;
    let event_gone = guard.handle_guard_lookup(ram, event)?.is_none();
    let sem_gone = guard.handle_guard_lookup(ram, sem)?.is_none();
    guard.handles_guard_close(ram)?;
    mixed_image_need(event_gone && sem_gone)?;
    Ok([
        (-2i64) as u64,
        u64::from(second.index),
        u64::from(third.index),
        u64::from(state[0]),
        u64::from(state[1]),
        u64::from(count),
        u64::from(event_gone),
        u64::from(sem_gone),
    ])
}
