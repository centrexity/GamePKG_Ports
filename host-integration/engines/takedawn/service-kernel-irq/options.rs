// Actual holder worker; protocol options and exact threshold/rate-limit edges.
use crate::gamepkg;
use crate::irq_delivery;
use crate::irql;
use crate::memory;
use crate::registers;
use crate::stacks;
fn option_need(ok: bool) -> Result<(), i32> {
    if ok {
        Ok(())
    } else {
        Err(-3)
    }
}
fn option_word(b: &[u8], at: usize) -> Result<u64, i32> {
    let v = b.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7],
    ]))
}
fn option_reply(id: u64, phase: u64) -> Result<(), i32> {
    let mut b = [0u8; 24];
    option_need(
        gamepkg::local_mailbox_receive(id, 1000, &mut b)? == 24 && option_word(&b, 0)? == phase,
    )
}
pub fn irq_options(ram: &mut memory::Memory, window: u64) -> Result<(), i32> {
    for mode in 5..=10u32 {
        gamepkg::local_memory_fill(window, 0x740000, 65536, 0)?;
        ram.write_u64(0x741008, 1000000000)?;
        let mut controller = irql::Controller::controller_new(ram)?;
        let mut pool = stacks::Pool::stacks_new(ram, 0x780000, 1048576)?;
        let safe = mode != 6 && mode != 7;
        let force = mode == 5 || mode == 6 || mode == 10;
        let configured_wait = if mode == 6 {
            0
        } else if mode == 10 {
            -1
        } else {
            250
        };
        let mut queue = irq_delivery::Interrupts::irq_new(
            ram,
            &controller,
            &pool,
            7,
            safe,
            force,
            configured_wait,
            false,
        )?;
        let mut task = controller.controller_task(ram)?;
        queue.irq_attach_task(ram, &mut task)?;
        let mut cpu = registers::Registers::new();
        cpu.fs_base = 0x740000;
        let command = gamepkg::local_mailbox_create(8, 1)?;
        let response = gamepkg::local_mailbox_create(24, 1)?;
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
        option_need(
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
        option_reply(response, 1)?;
        let (_, state) = gamepkg::local_record_snapshot(queue.irq_id(), 384)?;
        option_need(
            option_word(&state, 80)? == (configured_wait as u64).wrapping_mul(1000000)
                && option_word(&state, 88)? == 0,
        )?;
        if mode == 5 {
            option_need(
                queue.irq_post(ram, &mut task, &mut cpu, 0)? == 0
                    && queue.irq_post(ram, &mut task, &mut cpu, 2)? == 0,
            )?;
            ram.write_u64(0x741008, 1249999999)?;
            option_need(
                queue.irq_post(ram, &mut task, &mut cpu, 0)? == 0
                    && queue.irq_pending(ram)? == 5
                    && queue.irq_stats(ram, false)?[18] == 0,
            )?;
            ram.write_u64(0x741008, 1250000000)?;
            option_need(
                queue.irq_post(ram, &mut task, &mut cpu, 0)? == 2 && queue.irq_pending(ram)? == 4,
            )?;
            queue.irq_note_ungated(ram, 0)?;
            let stats = queue.irq_stats(ram, false)?;
            option_need(
                stats[9] == 1 && stats[12] == 1 && stats[18] == 1 && stats[0] == 1 && stats[2] == 1,
            )?;
        } else if mode == 6 {
            option_need(
                controller.controller_counts(ram)?[1] == 1
                    && queue.irq_post(ram, &mut task, &mut cpu, 1)? == 0,
            )?;
            ram.write_u64(0x741008, 1250000000)?;
            option_need(
                queue.irq_post(ram, &mut task, &mut cpu, 1)? == 0
                    && queue.irq_pending(ram)? == 2
                    && queue.irq_stats(ram, false)?[10] == 0,
            )?;
        } else if mode == 7 {
            option_need(
                queue.irq_post(ram, &mut task, &mut cpu, 0)? == 2
                    && queue.irq_pending(ram)? == 0
                    && queue.irq_stats(ram, false)?[0] == 0,
            )?;
            queue.irq_note_ungated(ram, 0)?;
            option_need(queue.irq_stats(ram, false)?[12] == 1)?;
        } else if mode == 8 {
            option_need(queue.irq_post(ram, &mut task, &mut cpu, 0)? == 0)?;
            for pair in [
                [1249999999u64, 0],
                [1250000000, 1],
                [1500000000, 1],
                [2249999999, 1],
                [2250000000, 2],
            ] {
                ram.write_u64(0x741008, pair[0])?;
                option_need(
                    queue.irq_post(ram, &mut task, &mut cpu, 0)? == 0
                        && u64::from(queue.irq_stats(ram, false)?[18]) == pair[1]
                        && u64::from(ram.read_u32(0x741020)?) == pair[1],
                )?;
            }
        } else if mode == 9 {
            option_need(queue.irq_post(ram, &mut task, &mut cpu, 0)? == 0)?;
            ram.write_u64(0x741008, 1000000000 + (i32::MAX as u64 + 1) * 1000)?;
        } else {
            option_need(queue.irq_post(ram, &mut task, &mut cpu, 0)? == 0)?;
            ram.write_u64(0x741008, 2000000000)?;
            option_need(
                queue.irq_post(ram, &mut task, &mut cpu, 0)? == 0
                    && queue.irq_pending(ram)? == 1
                    && queue.irq_stats(ram, false)?[18] == 0
                    && queue.irq_stats(ram, false)?[9] == 0,
            )?;
        }
        gamepkg::local_mailbox_send(command, &u64::to_le_bytes(2))?;
        option_reply(response, 4)?;
        let mut joined = false;
        for _ in 0..4096u32 {
            match gamepkg::local_call(11, &u64::to_le_bytes(worker), &mut out) {
                Ok(8) => {
                    option_need(u64::from_le_bytes(out) == 0)?;
                    joined = true;
                    break;
                }
                Err(-2) => memory::Memory::call_retry_pause()?,
                Err(code) => return Err(code),
                _ => return Err(-3),
            }
        }
        option_need(joined && pool.stacks_usage(ram)? == [0, 4])?;
        let stats = queue.irq_stats(ram, false)?;
        if mode == 9 {
            option_need(
                queue.irq_pending(ram)? == 0
                    && stats[3] == 1
                    && stats[15] == 0
                    && ram.read_u32(0x741000)? == 1,
            )?;
            queue.irq_close(ram, false)?;
        } else {
            option_need(queue.irq_pending(ram)? == 0)?;
            if mode == 5 {
                option_need(stats[5] == 1 && stats[3] == 0 && stats[9] == 1)?;
            }
            if mode == 6 {
                option_need(stats[4] == 1 && controller.controller_counts(ram)?[1] == 0)?;
            }
            if mode == 7 {
                option_need(stats[3..6] == [0, 0, 0])?;
            }
            if mode == 8 {
                option_need(stats[3] == 1 && stats[15] == 1250000 && stats[18] == 2)?;
            }
            if mode == 10 {
                option_need(stats[3] == 1 && stats[15] == 1000000)?;
            }
            queue.irq_close(ram, false)?;
        }
        pool.stacks_close(ram)?;
        controller.controller_close(ram)?;
        gamepkg::local_record_release(context)?;
        gamepkg::local_mailbox_release(command)?;
        gamepkg::local_mailbox_release(response)?;
    }
    Ok(())
}
