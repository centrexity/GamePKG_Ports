// Real reviewed workers, deterministic mailbox-controlled post/release windows.
use crate::gamepkg;
use crate::gate;
use crate::irq_delivery;
use crate::irql;
use crate::memory;
use crate::registers;
use crate::stacks;
fn race_need(ok: bool) -> Result<(), i32> {
    if ok {
        Ok(())
    } else {
        Err(-3)
    }
}
fn race_word(b: &[u8], at: usize) -> Result<u64, i32> {
    let v = b.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7],
    ]))
}
fn race_send(id: u64, phase: u64, result: u64, depth: u64) -> Result<(), i32> {
    gamepkg::local_mailbox_send(
        id,
        &[
            u64::to_le_bytes(phase),
            u64::to_le_bytes(result),
            u64::to_le_bytes(depth),
        ]
        .concat(),
    )
}
fn race_reply(id: u64, phase: u64) -> Result<[u64; 3], i32> {
    let mut b = [0u8; 24];
    race_need(
        gamepkg::local_mailbox_receive(id, 1000, &mut b)? == 24 && race_word(&b, 0)? == phase,
    )?;
    Ok([phase, race_word(&b, 8)?, race_word(&b, 16)?])
}
fn race_worker_run(id: u64) -> Result<(), i32> {
    let (_, b) = gamepkg::local_record_snapshot(id, 64)?;
    race_need(b.len() == 64)?;
    let mut ram = memory::Memory::with_call_handle(race_word(&b, 0)?)?;
    ram.attach(0, 0x4000000, race_word(&b, 24)?, 0, true)?;
    let queue = irq_delivery::Interrupts::irq_borrow(&ram, race_word(&b, 8)?)?;
    let controller = irql::Controller::controller_borrow(&ram, race_word(&b, 16)?)?;
    let mut task = controller.controller_task(&ram)?;
    queue.irq_attach_task(&ram, &mut task)?;
    let command = race_word(&b, 32)?;
    let response = race_word(&b, 40)?;
    let mut cpu = registers::Registers::new();
    cpu.fs_base = 0x740000;
    let mode = race_word(&b, 48)?;
    if mode == 11 || mode == 12 {
        race_send(response, 1, 0, 0)?;
        let mut message = [0u8; 8];
        race_need(gamepkg::local_mailbox_receive(command, 1000, &mut message)? == 8)?;
        if mode == 11 {
            race_need(
                queue.irq_post(&mut ram, &mut task, &mut cpu, 0)? == 0
                    && queue.irq_post(&mut ram, &mut task, &mut cpu, 1)? == 0,
            )?;
            race_send(response, 3, 0, 0)?;
            race_need(gamepkg::local_mailbox_receive(command, 1000, &mut message)? == 8)?;
            race_send(response, 4, 0, 0)?;
            return Ok(());
        }
        let original_cpu = cpu.encode();
        task.irql_disable(&mut ram, &mut cpu)?;
        race_need(queue.irq_post(&mut ram, &mut task, &mut cpu, 0)? == 0)?;
        let result = task.irql_enable_after_shadow(&mut ram, &mut cpu);
        race_need(
            (result == Err(-4) || result == Err(-3))
                && cpu.encode() == original_cpu
                && gamepkg::local_task_status()?.1,
        )?;
        race_send(response, 3, result.err().unwrap_or(0) as i64 as u64, 0)?;
        return Ok(());
    }
    if mode >= 5 {
        if mode == 6 {
            task.irql_disable(&mut ram, &mut cpu)?;
        } else {
            task.irql_gate_enter(&ram, 0)?;
        }
        race_send(response, 1, 0, 0)?;
        let mut message = [0u8; 8];
        race_need(gamepkg::local_mailbox_receive(command, 1000, &mut message)? == 8)?;
        let outcome = if mode == 6 {
            task.irql_enable_after_shadow(&mut ram, &mut cpu)
        } else {
            task.irql_gate_leave(&mut ram, &mut cpu)
        };
        outcome?;
        race_send(response, 4, 0, 0)?;
        return Ok(());
    }
    race_send(response, 1, 0, 0)?;
    let mut message = [0u8; 8];
    race_need(gamepkg::local_mailbox_receive(command, 1000, &mut message)? == 8)?;
    let result = queue.irq_post(&mut ram, &mut task, &mut cpu, race_word(&b, 56)? as u32)?;
    let lock = gate::Gate::gate_borrow(&ram, controller.controller_gate())?;
    race_send(response, 3, result as u64, lock.gate_state(&ram)?[1])?;
    race_need(gamepkg::local_mailbox_receive(command, 1000, &mut message)? == 8)?;
    if result == 1 {
        task.irql_gate_leave(&mut ram, &mut cpu)?;
    }
    race_send(response, 4, 0, 0)?;
    Ok(())
}
pub fn irq_race_worker(id: u64) -> u64 {
    match race_worker_run(id) {
        Ok(()) => 0,
        Err(code) => code as i64 as u64,
    }
}
pub fn irq_races(ram: &mut memory::Memory, window: u64) -> Result<(), i32> {
    for mode in 1..=4u32 {
        gamepkg::local_memory_fill(window, 0x740000, 65536, 0)?;
        ram.write_u64(0x741008, 1000000000)?;
        ram.write_u32(0x741028, mode)?;
        let mut controller = irql::Controller::controller_new(ram)?;
        let mut pool = stacks::Pool::stacks_new(ram, 0x780000, 1048576)?;
        let mut queue =
            irq_delivery::Interrupts::irq_new(ram, &controller, &pool, 7, true, false, 250, true)?;
        let mut task = controller.controller_task(ram)?;
        queue.irq_attach_task(ram, &mut task)?;
        let mut cpu = registers::Registers::new();
        cpu.fs_base = 0x740000;
        if mode == 4 {
            task.irql_raise(ram, &mut cpu, 2)?;
        } else {
            task.irql_gate_enter(ram, 0)?;
        }
        let command = gamepkg::local_mailbox_create(8, 1)?;
        let response = gamepkg::local_mailbox_create(24, 1)?;
        ram.write_u64(0x741040, command)?;
        ram.write_u64(0x741048, response)?;
        let line = if mode == 4 { 2 } else { mode - 1 };
        let context = gamepkg::local_record_create(
            &[
                u64::to_le_bytes(ram.call_handle()),
                u64::to_le_bytes(queue.irq_id()),
                u64::to_le_bytes(controller.controller_id()),
                u64::to_le_bytes(window),
                u64::to_le_bytes(command),
                u64::to_le_bytes(response),
                u64::to_le_bytes(u64::from(mode)),
                u64::to_le_bytes(u64::from(line)),
            ]
            .concat(),
        )?;
        let mut out = [0u8; 8];
        race_need(
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
        race_reply(response, 1)?;
        if mode == 4 {
            task.irql_lower(ram, &mut cpu, 0)?;
            race_need(
                ram.read_u32(0x741038)? == 0
                    && ram.read_u32(0x741034)? == 0
                    && ram.read_u32(0x741000)? == 1
                    && ram.read_u32(0x744000)? == 2
                    && queue.irq_pending(ram)? == 0
                    && queue.irq_stats(ram, false)?[5] == 1,
            )?;
        } else {
            gamepkg::local_mailbox_send(command, &u64::to_le_bytes(0))?;
            race_reply(response, 2)?;
            race_need(queue.irq_pending(ram)? == if mode == 1 { 0 } else { 1u32 << line })?;
            if mode == 3 {
                gate::Gate::gate_borrow(ram, controller.controller_gate())?.gate_leave_raw(ram)?;
            } else {
                task.irql_gate_leave(ram, &mut cpu)?;
            }
            gamepkg::local_mailbox_send(command, &u64::to_le_bytes(1))?;
            let result = race_reply(response, 3)?;
            let expected = if mode == 2 { 0 } else { 1 };
            race_need(
                result[1] == expected && result[2] == expected && queue.irq_pending(ram)? == 0,
            )?;
            let stats = queue.irq_stats(ram, false)?;
            race_need(
                stats[6 + line as usize] == expected as u32
                    && stats[3 + line as usize] == u32::from(mode == 2),
            )?;
        }
        gamepkg::local_mailbox_send(command, &u64::to_le_bytes(2))?;
        race_reply(response, 4)?;
        let mut joined = false;
        for _ in 0..4096u32 {
            match gamepkg::local_call(11, &u64::to_le_bytes(worker), &mut out) {
                Ok(8) => {
                    race_need(u64::from_le_bytes(out) == 0)?;
                    joined = true;
                    break;
                }
                Err(-2) => memory::Memory::call_retry_pause()?,
                Err(code) => return Err(code),
                _ => return Err(-3),
            }
        }
        race_need(joined)?;
        race_need(queue.irq_close(ram, false)? == 0 && pool.stacks_usage(ram)? == [0, 4])?;
        pool.stacks_close(ram)?;
        controller.controller_close(ram)?;
        gamepkg::local_record_release(context)?;
        gamepkg::local_mailbox_release(command)?;
        gamepkg::local_mailbox_release(response)?;
    }
    Ok(())
}
