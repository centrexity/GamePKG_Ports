use crate::gamepkg;
use crate::irq_delivery;
use crate::irql;
use crate::kernel_pool;
use crate::memory;
use crate::registers;
use crate::stacks;
fn irq_control_need(ok: bool) -> Result<(), i32> {
    if ok {
        Ok(())
    } else {
        Err(-3)
    }
}
fn irq_control_put(b: &mut [u8], at: usize, value: u64) {
    let word = u64::to_le_bytes(value);
    for lane in 0..8usize {
        b[at + lane] = word[lane];
    }
}
fn irq_control_word(b: &[u8], at: usize) -> Result<u64, i32> {
    let v = b.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7],
    ]))
}
pub fn irq_controls(ram: &mut memory::Memory, window: u64) -> Result<(), i32> {
    for (elapsed, expected) in [
        (0u64, 0u32),
        (2147483647, 2147483647),
        (2147483648, 0),
        (4294967295, 0),
        (4294967296, 0),
        (4294967297, 1),
    ] {
        irq_control_need(
            irq_delivery::irq_wait_maximum(1000000000 + elapsed * 1000, 1000000000, 0)? == expected,
        )?;
    }
    irq_control_need(irq_delivery::irq_wait_maximum(0, 0, u32::MAX) == Err(-1))?;

    gamepkg::local_memory_fill(window, 0x740000, 65536, 0)?;
    ram.write_u64(0x741008, 1000000000)?;
    let mut controller = irql::Controller::controller_new(ram)?;
    let mut pool = stacks::Pool::stacks_new(ram, 0x780000, 1048576)?;
    irq_control_need(
        irq_delivery::Interrupts::irq_new(ram, &controller, &pool, 8, true, false, 250, true).err()
            == Some(-1),
    )?;
    let mut queue =
        irq_delivery::Interrupts::irq_new(ram, &controller, &pool, 7, true, false, 250, true)?;
    let mut borrowed = irq_delivery::Interrupts::irq_borrow(ram, queue.irq_id())?;
    irq_control_need(borrowed.irq_close(ram, false) == Err(-4))?;
    let foreign = memory::Memory::new()?;
    irq_control_need(
        queue.irq_pending(&foreign) == Err(-4)
            && irq_delivery::Interrupts::irq_borrow(&foreign, queue.irq_id()).err() == Some(-4),
    )?;
    gamepkg::local_record_release(foreign.call_handle())?;
    let mut other = irql::Controller::controller_new(ram)?;
    let mut wrong = other.controller_task(ram)?;
    irq_control_need(queue.irq_attach_task(ram, &mut wrong) == Err(-4))?;
    other.controller_close(ram)?;
    let (_, original) = gamepkg::local_record_snapshot(queue.irq_id(), 384)?;
    for pair in [
        [0u64, 0],
        [48, 8],
        [56, 8],
        [64, 2],
        [72, 2],
        [88, 2],
        [104, u32::MAX as u64 + 1],
        [112, 2],
        [120, u32::MAX as u64 + 1],
        [128, 34],
        [136, 1],
        [200, u32::MAX as u64 + 1],
        [240, i32::MAX as u64 + 1],
        [248, 1],
    ] {
        let mut bad = original.clone();
        irq_control_put(&mut bad, pair[0] as usize, pair[1]);
        let (v, _) = gamepkg::local_record_snapshot(queue.irq_id(), 384)?;
        gamepkg::local_record_compare_exchange(queue.irq_id(), v, &bad)?;
        let (before, _) = gamepkg::local_record_snapshot(queue.irq_id(), 384)?;
        irq_control_need(
            queue.irq_pending(ram) == Err(-1) && queue.irq_stats(ram, true) == Err(-1),
        )?;
        let (after, bytes) = gamepkg::local_record_snapshot(queue.irq_id(), 384)?;
        irq_control_need(before == after && bytes == bad)?;
        gamepkg::local_record_compare_exchange(queue.irq_id(), after, &original)?;
    }
    let (v, mut closing) = gamepkg::local_record_snapshot(queue.irq_id(), 384)?;
    irq_control_put(&mut closing, 112, 1);
    gamepkg::local_record_compare_exchange(queue.irq_id(), v, &closing)?;
    irq_control_need(
        queue.irq_pending(ram) == Err(-4)
            && queue.irq_close(ram, false)? == 0
            && queue.irq_closed(),
    )?;
    irq_control_need(borrowed.irq_pending(ram) == Err(-4))?;
    pool.stacks_close(ram)?;
    controller.controller_close(ram)?;
    // A missing registered route consumes a pending line without pretending a handler ran.
    let mut controller = irql::Controller::controller_new(ram)?;
    let mut pool = stacks::Pool::stacks_new(ram, 0x780000, 0)?;
    let mut queue =
        irq_delivery::Interrupts::irq_new(ram, &controller, &pool, 0, true, false, 250, true)?;
    let mut task = controller.controller_task(ram)?;
    queue.irq_attach_task(ram, &mut task)?;
    let mut cpu = registers::Registers::new();
    cpu.fs_base = 0x740000;
    task.irql_disable(ram, &mut cpu)?;
    for line in 0..3u32 {
        irq_control_need(queue.irq_post(ram, &mut task, &mut cpu, line)? == 0)?;
    }
    task.irql_enable_after_shadow(ram, &mut cpu)?;
    irq_control_need(
        queue.irq_pending(ram)? == 0
            && queue.irq_stats(ram, false)?[3..6] == [0, 0, 0]
            && ram.read_u32(0x741000)? == 0
            && pool.stacks_usage(ram)? == [0, 0],
    )?;
    queue.irq_close(ram, false)?;
    pool.stacks_close(ram)?;
    controller.controller_close(ram)?;
    // Higher priority and in-ISR refusals, followed by the proper release point.
    for in_isr in [false, true] {
        gamepkg::local_memory_fill(window, 0x740000, 65536, 0)?;
        ram.write_u64(0x741008, 1000000000)?;
        let mut controller = irql::Controller::controller_new(ram)?;
        let mut pool = stacks::Pool::stacks_new(ram, 0x780000, 1048576)?;
        let mut queue =
            irq_delivery::Interrupts::irq_new(ram, &controller, &pool, 7, true, false, 250, true)?;
        let mut task = controller.controller_task(ram)?;
        queue.irq_attach_task(ram, &mut task)?;
        let mut cpu = registers::Registers::new();
        cpu.fs_base = 0x740000;
        let original_cpu = cpu.encode();
        if in_isr {
            task.irql_gate_enter(ram, 0)?;
            task.irql_set_isr(ram, true)?;
        } else {
            task.irql_raise(ram, &mut cpu, 4)?;
        }
        task.irql_disable(ram, &mut cpu)?;
        queue.irq_post(ram, &mut task, &mut cpu, 1)?;
        task.irql_enable_after_shadow(ram, &mut cpu)?;
        queue.irq_safe_point(ram, &mut task, &mut cpu)?;
        irq_control_need(queue.irq_pending(ram)? == 2 && ram.read_u32(0x741000)? == 0)?;
        if in_isr {
            task.irql_set_isr(ram, false)?;
            task.irql_gate_leave(ram, &mut cpu)?;
        } else {
            task.irql_lower(ram, &mut cpu, 0)?;
        }
        irq_control_need(
            queue.irq_pending(ram)? == 0
                && ram.read_u32(0x741000)? == 1
                && cpu.encode() == original_cpu
                && queue.irq_stats(ram, false)?[19] == if in_isr { 2 } else { 3 },
        )?;
        queue.irq_close(ram, false)?;
        pool.stacks_close(ram)?;
        controller.controller_close(ram)?;
    }
    // Original u32 counter wrapping/reset, coalesced first timestamp and wait-log rate limit.
    gamepkg::local_memory_fill(window, 0x740000, 65536, 0)?;
    ram.write_u64(0x741008, 1000000000)?;
    let mut controller = irql::Controller::controller_new(ram)?;
    let mut pool = stacks::Pool::stacks_new(ram, 0x780000, 1048576)?;
    let mut queue =
        irq_delivery::Interrupts::irq_new(ram, &controller, &pool, 7, true, false, 250, true)?;
    let mut task = controller.controller_task(ram)?;
    queue.irq_attach_task(ram, &mut task)?;
    let mut cpu = registers::Registers::new();
    cpu.fs_base = 0x740000;
    let (v, mut b) = gamepkg::local_record_snapshot(queue.irq_id(), 384)?;
    for at in [200usize, 208, 232, 104, 120] {
        irq_control_put(&mut b, at, u32::MAX as u64);
    }
    gamepkg::local_record_compare_exchange(queue.irq_id(), v, &b)?;
    task.irql_disable(ram, &mut cpu)?;
    queue.irq_post(ram, &mut task, &mut cpu, 0)?;
    queue.irq_safe_point(ram, &mut task, &mut cpu)?;
    queue.irq_note_ungated(ram, 0)?;
    ram.write_u64(0x741008, 1250000000)?;
    queue.irq_post(ram, &mut task, &mut cpu, 0)?;
    queue.irq_post(ram, &mut task, &mut cpu, 0)?;
    let stats = queue.irq_stats(ram, false)?;
    irq_control_need(
        stats[0] == 0
            && stats[12] == 0
            && stats[18] == 0
            && stats[19] == 0
            && ram.read_u32(0x741020)? == 1,
    )?;
    task.irql_enable_after_shadow(ram, &mut cpu)?;
    let stats = queue.irq_stats(ram, true)?;
    irq_control_need(
        stats[3] == 0 && stats[15] == 250000 && queue.irq_stats(ram, false)? == [0; 20],
    )?;
    queue.irq_close(ram, false)?;
    pool.stacks_close(ram)?;
    controller.controller_close(ram)?;
    // Callback FS retention and explicit failure cleanup. Each uses a fresh isolated root.
    for scenario in [1u32, 2, 3, 6] {
        gamepkg::local_memory_fill(window, 0x740000, 65536, 0)?;
        let mut private = memory::Memory::new()?;
        private.attach(0, 0x4000000, window, 0, true)?;
        let services = kernel_pool::kernel_services_initialize(&private, 0x10000, 2)?;
        private.write_u64(0x741008, 1000000000)?;
        private.write_u32(0x741024, scenario)?;
        let mut controller = irql::Controller::controller_new(&private)?;
        let mut pool = stacks::Pool::stacks_new(&private, 0x780000, 1048576)?;
        let mut queue = irq_delivery::Interrupts::irq_new(
            &private,
            &controller,
            &pool,
            7,
            true,
            false,
            250,
            true,
        )?;
        let mut task = controller.controller_task(&private)?;
        queue.irq_attach_task(&private, &mut task)?;
        let mut cpu = registers::Registers::new();
        cpu.fs_base = 0x740000;
        cpu.eax = 77;
        let mut original_cpu = cpu.encode();
        task.irql_disable(&mut private, &mut cpu)?;
        for line in 0..3u32 {
            queue.irq_post(&mut private, &mut task, &mut cpu, line)?;
        }
        if scenario == 3 {
            gamepkg::local_call(
                3,
                &[
                    u64::to_le_bytes(window).as_slice(),
                    &u64::to_le_bytes(0x740000),
                    &u64::to_le_bytes(4096),
                    &u32::to_le_bytes(1),
                ]
                .concat(),
                &mut [],
            )?;
        }
        let outcome = task.irql_enable_after_shadow(&mut private, &mut cpu);
        if scenario == 3 {
            gamepkg::local_call(
                3,
                &[
                    u64::to_le_bytes(window).as_slice(),
                    &u64::to_le_bytes(0x740000),
                    &u64::to_le_bytes(4096),
                    &u32::to_le_bytes(2),
                ]
                .concat(),
                &mut [],
            )?;
        }
        if scenario == 1 {
            let fs_bytes = u32::to_le_bytes(0x740100);
            for lane in 0..4usize {
                original_cpu[240 + lane] = fs_bytes[lane];
            }
            irq_control_need(
                outcome == Ok(())
                    && cpu.encode() == original_cpu
                    && private.read_u8(0x740024)? == 16
                    && private.read_u8(0x740124)? == 0
                    && queue.irq_pending(&private)? == 0,
            )?;
            queue.irq_close(&private, false)?;
            pool.stacks_close(&private)?;
            controller.controller_close(&private)?;
        } else {
            irq_control_need(
                outcome == Err(if scenario == 2 { -5 } else { -4 }) && cpu.encode() == original_cpu,
            )?;
            let (_, state) = gamepkg::local_record_snapshot(queue.irq_id(), 384)?;
            irq_control_need(
                irq_control_word(&state, 56)? == 7
                    && irq_control_word(&state, 128)? == 0
                    && irq_control_word(&state, 208)? == 0,
            )?;
            if scenario == 6 {
                irq_control_need(
                    queue.irq_close(&private, true) == Err(-4)
                        && queue.irq_closed()
                        && pool.stacks_close(&private) == Err(-4)
                        && pool.stacks_closed()
                        && controller.controller_close(&private) == Err(-4)
                        && controller.controller_closed(),
                )?;
            } else {
                irq_control_need(
                    controller.controller_counts(&private)?[0..2] == [0, 0]
                        && task.irql_current(&private) == Err(-4)
                        && pool.stacks_usage(&private)? == [0, 4]
                        && queue.irq_close(&private, false) == Err(-2)
                        && queue.irq_close(&private, true)? == 7,
                )?;
                pool.stacks_close(&private)?;
                controller.controller_close(&private)?;
            }
        }
        services.service_close(&private)?;
        gamepkg::local_record_release(private.call_handle())?;
    }
    Ok(())
}
