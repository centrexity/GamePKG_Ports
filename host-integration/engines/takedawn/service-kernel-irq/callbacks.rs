// New posts during a native callback and actual cancellation while inside it.
use crate::gamepkg;
use crate::gate;
use crate::irq_delivery;
use crate::irql;
use crate::memory;
use crate::registers;
use crate::stacks;
fn callback_need(ok: bool) -> Result<(), i32> {
    if ok {
        Ok(())
    } else {
        Err(-3)
    }
}
fn callback_word(b: &[u8], at: usize) -> Result<u64, i32> {
    let v = b.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7],
    ]))
}
fn callback_reply(id: u64, phase: u64) -> Result<[u64; 3], i32> {
    let mut b = [0u8; 24];
    callback_need(
        gamepkg::local_mailbox_receive(id, 1000, &mut b)? == 24 && callback_word(&b, 0)? == phase,
    )?;
    Ok([phase, callback_word(&b, 8)?, callback_word(&b, 16)?])
}
pub fn irq_thread_patch_probe(ram: &mut memory::Memory, window: u64) -> Result<(), i32> {
    for mode in [0u32, 1, 2, 3] {
        gamepkg::local_memory_fill(window, 0x740000, 65536, 0)?;
        ram.write_u64(0x741008, 1000000000)?;
        ram.write_u32(0x741024, 13)?;
        let mut controller = irql::Controller::controller_new(ram)?;
        let mut pool = stacks::Pool::stacks_new(ram, 0x780000, 1048576)?;
        let mut queue =
            irq_delivery::Interrupts::irq_new(ram, &controller, &pool, 1, true, false, 250, true)?;
        let mut task = controller.controller_task(ram)?;
        let mut cpu = registers::Registers::new();
        cpu.fs_base = 0x740000;
        cpu.ebx = 0xa1b2c3d4;
        cpu.esi = if mode == 1 { 0x750000 } else { 0x741080 };
        cpu.esp = if mode >= 2 { 0x750004 } else { 0x741088 };
        let call_esp = cpu.esp;
        let untouched = cpu.encode();
        callback_need(queue.irq_thread_patch(ram, &mut task, &mut cpu) == Err(-5))?;
        callback_need(irq_delivery::irq_thread_patch_current(ram, &mut cpu) == Err(-5))?;
        callback_need(
            cpu.encode() == untouched && controller.controller_counts(ram)? == [0, 0, 0],
        )?;
        queue.irq_attach_task(ram, &mut task)?;
        let mut dispatch_ram = ram.call_view();
        dispatch_ram.attach(0, 0x4000000, window, 0, true)?;
        queue.irq_bind_dispatch(&mut dispatch_ram, &task)?;
        queue.irq_bind_dispatch(&mut dispatch_ram, &task)?;
        let actor = task.task_key()[1];
        callback_need(dispatch_ram.dispatch_irq_record(actor.wrapping_add(1)) == Err(-4))?;
        let mut copied_ram = memory::Memory::with_context(dispatch_ram.thread_context())?;
        callback_need(
            irq_delivery::irq_thread_patch_current(&mut copied_ram, &mut cpu) == Err(-5),
        )?;
        let mut foreign =
            irq_delivery::Interrupts::irq_new(ram, &controller, &pool, 1, true, false, 250, true)?;
        let mut foreign_task = controller.controller_task(ram)?;
        foreign.irq_attach_task(ram, &mut foreign_task)?;
        callback_need(foreign.irq_bind_dispatch(&mut dispatch_ram, &foreign_task) == Err(-4))?;
        callback_need(cpu.encode() == untouched && foreign.irq_close(ram, false)? == 0)?;
        let pending = u32::from(mode != 3);
        if pending != 0 {
            task.irql_disable(ram, &mut cpu)?;
            queue.irq_post(ram, &mut task, &mut cpu, 0)?;
        }
        callback_need(queue.irq_pending(ram)? == pending && ram.read_u32(0x741000)? == 0)?;
        let outcome = irq_delivery::irq_thread_patch_current(&mut dispatch_ram, &mut cpu);
        if mode == 0 {
            callback_need(
                outcome == Ok(call_esp)
                    && cpu.esp == call_esp - 4
                    && cpu.eflags & 0x200 != 0
                    && queue.irq_pending(ram)? == 0
                    && ram.read_u32(0x741000)? == 1
                    && ram.read_u32(0x741060)? == 0x28c5048d
                    && ram.read_u32(0x741064)? == cpu.ebx
                    && ram.read_u32(0x741084)? == cpu.ebx
                    && controller.controller_counts(ram)?[..2] == [0, 0],
            )?;
        } else {
            callback_need(
                outcome == Err(-4)
                    && cpu.eflags & 0x200 == 0
                    && cpu.esp == if mode == 1 { call_esp } else { call_esp - 4 }
                    && queue.irq_pending(ram)? == pending
                    && ram.read_u32(0x741000)? == 0
                    && ram.read_u32(0x741080)? == if mode == 1 { 0 } else { 0x28c5048d }
                    && controller.controller_counts(ram)?[..2] == [0, 0],
            )?;
        }
        let discarded = if mode == 0 { 0 } else { pending };
        callback_need(queue.irq_close(ram, discarded != 0)? == discarded)?;
        let after_close = cpu.encode();
        callback_need(
            irq_delivery::irq_thread_patch_current(&mut dispatch_ram, &mut cpu).is_err(),
        )?;
        callback_need(cpu.encode() == after_close)?;
        pool.stacks_close(ram)?;
        controller.controller_close(ram)?;
    }
    Ok(())
}
pub fn irq_callbacks_probe(ram: &mut memory::Memory, window: u64) -> Result<(), i32> {
    for mode in [11u32, 12] {
        let mut sync_ids = [0u64; 128];
        let mut sync_count = 0usize;
        gamepkg::local_memory_fill(window, 0x740000, 65536, 0)?;
        ram.write_u64(0x741008, 1000000000)?;
        ram.write_u32(0x741024, if mode == 11 { 7 } else { 12 })?;
        let mut controller = irql::Controller::controller_new(ram)?;
        let mut pool = stacks::Pool::stacks_new(ram, 0x780000, 1048576)?;
        let mut queue =
            irq_delivery::Interrupts::irq_new(ram, &controller, &pool, 7, true, false, 250, true)?;
        let mut task = controller.controller_task(ram)?;
        queue.irq_attach_task(ram, &mut task)?;
        let mut cpu = registers::Registers::new();
        cpu.fs_base = 0x740000;
        let original_cpu = cpu.encode();
        let command = gamepkg::local_mailbox_create(8, 1)?;
        let response = gamepkg::local_mailbox_create(24, 1)?;
        ram.write_u64(0x741040, command)?;
        ram.write_u64(0x741048, response)?;
        let context = gamepkg::local_record_create(
            &[
                u64::to_le_bytes(ram.call_handle()),
                u64::to_le_bytes(queue.irq_id()),
                u64::to_le_bytes(controller.controller_id()),
                u64::to_le_bytes(window),
                u64::to_le_bytes(command),
                u64::to_le_bytes(response),
                u64::to_le_bytes(u64::from(mode)),
                u64::to_le_bytes(0),
            ]
            .concat(),
        )?;
        let mut out = [0u8; 8];
        callback_need(
            gamepkg::local_call(
                10,
                &[
                    u32::to_le_bytes(0).as_slice(),
                    &u64::to_le_bytes(context),
                    &u32::to_le_bytes(0),
                ]
                .concat(),
                &mut out,
            )? == 8,
        )?;
        let worker = u64::from_le_bytes(out);
        callback_reply(response, 1)?;
        if mode == 11 {
            task.irql_disable(ram, &mut cpu)?;
            queue.irq_post(ram, &mut task, &mut cpu, 0)?;
            queue.irq_post(ram, &mut task, &mut cpu, 2)?;
            task.irql_enable_after_shadow(ram, &mut cpu)?;
            callback_need(
                cpu.encode() == original_cpu
                    && queue.irq_pending(ram)? == 0
                    && ram.read_u32(0x741000)? == 4,
            )?;
            for (lane, line) in [(0u32, 0u32), (1, 2), (2, 0), (3, 1)] {
                callback_need(ram.read_u32(0x744000 + lane * 32)? == line)?;
            }
            callback_need(queue.irq_stats(ram, false)?[..6] == [2, 1, 1, 2, 1, 1])?;
            gamepkg::local_mailbox_send(command, &u64::to_le_bytes(2))?;
            let worker_patch = callback_reply(response, 4)?;
            callback_need(
                worker != 0
                    && worker_patch[1] == worker
                    && ram.read_u32(0x741090)? == 0x28c5048d
                    && ram.read_u32(0x741094)? == 0xc1d2e3f4,
            )?;
        } else {
            gamepkg::local_mailbox_send(command, &u64::to_le_bytes(0))?;
            callback_reply(response, 2)?;
            let (_, state) = gamepkg::local_record_snapshot(queue.irq_id(), 384)?;
            callback_need(
                callback_word(&state, 128)? == 1
                    && pool.stacks_usage(ram)? == [1, 4]
                    && controller.controller_counts(ram)?[0] == 1
                    && queue.irq_close(ram, false) == Err(-2)
                    && queue.irq_close(ram, true) == Err(-2),
            )?;
            for _ in 0..128usize {
                let mut id = [0u8; 8];
                match gamepkg::local_call(20, &[0u8; 8], &mut id) {
                    Ok(8) => {
                        sync_ids[sync_count] = u64::from_le_bytes(id);
                        sync_count += 1;
                    }
                    Err(-2) => break,
                    Err(code) => return Err(code),
                    _ => return Err(-3),
                }
            }
            callback_need(sync_count > 0 && sync_count < 128)?;
            gamepkg::local_call(12, &u64::to_le_bytes(worker), &mut [])?;
            let cleanup_report = callback_reply(response, 3)?;
            let (_, state) = gamepkg::local_record_snapshot(queue.irq_id(), 384)?;
            if controller.controller_counts(ram)?[0] != 0 {
                return Err(-3131);
            }
            callback_need(cleanup_report[1] == (-4i64 as u64))?;

            callback_need(
                callback_word(&state, 96)? == 1
                    && callback_word(&state, 128)? == 0
                    && queue.irq_pending(ram)? == 1
                    && queue.irq_stats(ram, false)?[3] == 0
                    && controller.controller_counts(ram)?[0..2] == [0, 0]
                    && pool.stacks_usage(ram)? == [0, 4]
                    && gate::Gate::gate_borrow(ram, controller.controller_gate())?
                        .gate_state(ram)?[1]
                        == 0,
            )?;
        }
        for lane in 0..sync_count {
            gamepkg::local_call(25, &u64::to_le_bytes(sync_ids[lane]), &mut [])?;
        }
        let mut joined = false;
        for _ in 0..4096u32 {
            match gamepkg::local_call(11, &u64::to_le_bytes(worker), &mut out) {
                Ok(8) => {
                    callback_need(u64::from_le_bytes(out) == 0)?;
                    joined = true;
                    break;
                }
                Err(-2) => memory::Memory::call_retry_pause()?,
                Err(code) => return Err(code),
                _ => return Err(-3),
            }
        }
        callback_need(joined)?;
        callback_need(queue.irq_close(ram, mode == 12)? == u32::from(mode == 12))?;
        pool.stacks_close(ram)?;
        controller.controller_close(ram)?;
        gamepkg::local_record_release(context)?;
        gamepkg::local_mailbox_release(command)?;
        gamepkg::local_mailbox_release(response)?;
    }
    Ok(())
}
