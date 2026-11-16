use crate::cases;
use crate::gamepkg;
use crate::gate;
use crate::irq_delivery;
use crate::irql;
use crate::memory;
use crate::stacks;
fn irq_probe_need(ok: bool) -> Result<(), i32> {
    if ok {
        Ok(())
    } else {
        Err(-3)
    }
}
pub fn irq_probe_case(
    ram: &mut memory::Memory,
    window: u64,
    index: u32,
) -> Result<std::vec::Vec<u8>, i32> {
    gamepkg::local_memory_fill(window, 0x740000, 65536, 0)?;
    ram.write_u64(0x741008, 1000000000)?;
    let mut controller = irql::Controller::controller_new(ram)?;
    let mut pool = stacks::Pool::stacks_new(ram, 0x780000, 1048576)?;
    let mut queue =
        irq_delivery::Interrupts::irq_new(ram, &controller, &pool, 7, true, false, 250, true)?;
    let mut task = controller.controller_task(ram)?;
    queue.irq_attach_task(ram, &mut task)?;
    let mut cpu = cases::irq_cpu(index)?;
    let original = cpu.encode();
    task.irql_disable(ram, &mut cpu)?;
    let lines = [2u32, 0, 1, 1];
    for lane in 0..4usize {
        ram.write_u64(0x741008, 1000000000 + lane as u64 * 1000)?;
        irq_probe_need(queue.irq_post(ram, &mut task, &mut cpu, lines[lane])? == 0)?;
    }
    irq_probe_need(queue.irq_pending(ram)? == 7)?;
    queue.irq_safe_point(ram, &mut task, &mut cpu)?;
    irq_probe_need(ram.read_u32(0x741000)? == 0)?;
    task.irql_enable_after_shadow(ram, &mut cpu)?;
    irq_probe_need(
        cpu.encode() == original
            && queue.irq_pending(ram)? == 0
            && ram.read_u32(0x741000)? == 3
            && ram.read_u32(0x741010)? == 3
            && ram.read_u32(0x741014)? == 3,
    )?;
    for line in 0..3u32 {
        let base = 0x744000 + line * 32;
        irq_probe_need(
            ram.read_u32(base)? == line
                && ram.read_u32(base + 4)? == 0x7bfff0
                && ram.read_u32(base + 8)? == 1
                && ram.read_u32(base + 12)? == 16
                && ram.read_u32(base + 16)? == 1
                && ram.read_u32(base + 20)? == 1
                && ram.read_u32(base + 24)? == 16,
        )?;
    }
    let stats = queue.irq_stats(ram, false)?;
    irq_probe_need(
        stats[..6] == [1, 1, 1, 1, 1, 1]
            && stats[15..18] == [2, 1, 3]
            && stats[19] == 1
            && pool.stacks_usage(ram)? == [0, 4],
    )?;
    let mut zero = pool.stacks_allocate(ram)?.ok_or(-3)?;
    let mut one = pool.stacks_allocate(ram)?.ok_or(-3)?;
    let mut two = pool.stacks_allocate(ram)?.ok_or(-3)?;
    let mut three = pool.stacks_allocate(ram)?.ok_or(-3)?;
    task.irql_gate_enter(ram, 0)?;
    task.irql_disable(ram, &mut cpu)?;
    for line in 0..3u32 {
        irq_probe_need(queue.irq_post(ram, &mut task, &mut cpu, line)? == 0)?;
    }
    task.irql_enable_after_shadow(ram, &mut cpu)?;
    irq_probe_need(
        queue.irq_pending(ram)? == 7
            && ram.read_u32(0x741000)? == 3
            && pool.stacks_usage(ram)? == [4, 4]
            && cpu.encode() == original,
    )?;
    zero.stack_release(ram)?;
    queue.irq_safe_point(ram, &mut task, &mut cpu)?;
    irq_probe_need(
        queue.irq_pending(ram)? == 0
            && ram.read_u32(0x741000)? == 6
            && pool.stacks_usage(ram)? == [3, 4]
            && cpu.encode() == original,
    )?;
    one.stack_release(ram)?;
    two.stack_release(ram)?;
    three.stack_release(ram)?;
    task.irql_gate_leave(ram, &mut cpu)?;
    let stats = queue.irq_stats(ram, true)?;
    irq_probe_need(stats[..6] == [2, 2, 2, 2, 2, 2] && queue.irq_stats(ram, false)? == [0; 20])?;
    let mut image = std::vec::Vec::new();
    image.resize(65536, 0u8);
    ram.read_bytes(0x740000, &mut image)?;
    let hash = cases::irq_hash(&image);
    let v = task.task_values(ram)?;
    let counts = controller.controller_counts(ram)?;
    let lock = gate::Gate::gate_borrow(ram, controller.controller_gate())?;
    let fields = [
        v[0] as u32,
        v[1] as u32,
        v[2] as u32,
        v[3] as u32,
        v[4] as u32,
        v[5] as u32,
        counts[0] as u32,
        counts[1] as u32,
        counts[2] as u32,
        lock.gate_state(ram)?[1] as u32,
        queue.irq_pending(ram)?,
        pool.stacks_usage(ram)?[0],
        hash as u32,
        (hash >> 32) as u32,
    ];
    let mut output = std::vec::Vec::new();
    output.resize(452, 0u8);
    let encoded = cpu.encode();
    for lane in 0..316usize {
        output[lane] = encoded[lane];
    }
    for lane in 0..20usize {
        let word = u32::to_le_bytes(stats[lane]);
        for byte in 0..4usize {
            output[316 + lane * 4 + byte] = word[byte];
        }
    }
    for lane in 0..14usize {
        let word = u32::to_le_bytes(fields[lane]);
        for byte in 0..4usize {
            output[396 + lane * 4 + byte] = word[byte];
        }
    }
    irq_probe_need(queue.irq_close(ram, false)? == 0 && queue.irq_closed())?;
    pool.stacks_close(ram)?;
    controller.controller_close(ram)?;
    Ok(output)
}
pub fn irq_probe(ram: &mut memory::Memory, window: u64) -> Result<(), i32> {
    irq_probe_case(ram, window, 0)?;
    Ok(())
}
