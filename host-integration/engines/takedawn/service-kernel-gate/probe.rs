use crate::gamepkg;
use crate::gate;
use crate::kernel_pool;
use crate::memory;
fn probe_word(b: &[u8], at: usize) -> Result<u64, i32> {
    let b = b.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}
fn probe_need(ok: bool) -> Result<(), i32> {
    if ok {
        Ok(())
    } else {
        Err(-3)
    }
}
fn probe_run(arg: u64) -> Result<u64, i32> {
    let (_, ctx) = gamepkg::local_record_snapshot(arg, 32)?;
    probe_need(ctx.len() == 32)?;
    let ram = memory::Memory::with_call_handle(probe_word(&ctx, 0)?)?;
    let view = gate::Gate::gate_borrow(&ram, probe_word(&ctx, 8)?)?;
    let command = probe_word(&ctx, 16)?;
    let reply = probe_word(&ctx, 24)?;
    gamepkg::local_mailbox_send(reply, &u64::to_le_bytes(1))?;
    for _ in 0..16u32 {
        let mut bytes = [0u8; 8];
        probe_need(gamepkg::local_mailbox_receive(command, 1000, &mut bytes)? == 8)?;
        let operation = u64::from_le_bytes(bytes);
        if operation == 3 {
            return Ok(0);
        }
        if operation == 0 {
            probe_need(
                view.gate_enter(&ram, 0) == Err(-2) && view.gate_leave_raw(&ram) == Err(-4),
            )?;
            gamepkg::local_mailbox_send(reply, &u64::to_le_bytes(2))?;
        } else if operation == 1 {
            view.gate_enter(&ram, 1000)?;
            let task = gamepkg::local_task_status()?.0;
            probe_need(view.gate_state(&ram)? == [task, 1])?;
            gamepkg::local_mailbox_send(reply, &u64::to_le_bytes(3))?;
            let received = gamepkg::local_mailbox_receive(command, 1000, &mut bytes);
            let cancelled = gamepkg::local_task_status()?.1;
            // A cancelled receive is a cleanup request, never a skipped unlock.
            if cancelled {
                probe_need(received == Err(-4))?;
            } else {
                probe_need(received? == 8 && u64::from_le_bytes(bytes) == 2)?;
            }
            view.gate_leave_raw(&ram)?;
            if cancelled {
                probe_need(
                    view.gate_enter(&ram, 0) == Err(-4) && view.gate_state(&ram) == Err(-4),
                )?;
            }
            gamepkg::local_mailbox_send(reply, &u64::to_le_bytes(4))?;
            if cancelled {
                return Ok(0);
            }
        } else {
            return Err(-1);
        }
    }
    Err(-2)
}
pub fn gate_worker(arg: u64) -> u64 {
    match probe_run(arg) {
        Ok(v) => v,
        Err(c) => c as i64 as u64,
    }
}
fn probe_reply(id: u64, value: u64) -> Result<(), i32> {
    let mut b = [0u8; 8];
    probe_need(
        gamepkg::local_mailbox_receive(id, 1000, &mut b)? == 8 && u64::from_le_bytes(b) == value,
    )
}
fn gate_case(cancel: bool) -> Result<(), i32> {
    let ram = memory::Memory::new()?;
    let services = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2)?;
    let mut owner = gate::Gate::gate_new(&ram)?;
    probe_need(owner.gate_state(&ram)? == [0, 0] && owner.gate_leave_raw(&ram) == Err(-4))?;
    let foreign = memory::Memory::new()?;
    probe_need(owner.gate_state(&foreign) == Err(-4))?;
    gamepkg::local_record_release(foreign.call_handle())?;
    probe_need(owner.gate_enter(&ram, 1001) == Err(-1) && owner.gate_state(&ram)? == [0, 0])?;
    let mut signal = [0u8; 8];
    probe_need(
        gamepkg::local_call(
            20,
            &[u32::to_le_bytes(0), u32::to_le_bytes(1)].concat(),
            &mut signal,
        )? == 8,
    )?;
    let (_, mut fake) = gamepkg::local_record_snapshot(owner.gate_id(), 64)?;
    for lane in 0..8usize {
        fake[24 + lane] = signal[lane];
    }
    let fake_id = gamepkg::local_record_create(&fake)?;
    probe_need(gate::Gate::gate_borrow(&ram, fake_id).err() == Some(-1))?;
    gamepkg::local_call(
        23,
        &[signal.as_slice(), &u32::to_le_bytes(0)].concat(),
        &mut [],
    )?;
    probe_need(
        gamepkg::local_call(
            23,
            &[signal.as_slice(), &u32::to_le_bytes(0)].concat(),
            &mut [],
        ) == Err(-2),
    )?;
    gamepkg::local_call(25, &signal, &mut [])?;
    gamepkg::local_record_release(fake_id)?;
    let foreign = memory::Memory::new()?;
    probe_need(
        owner.gate_state(&foreign) == Err(-4)
            && owner.gate_enter(&foreign, 0) == Err(-4)
            && owner.gate_leave_raw(&foreign) == Err(-4),
    )?;
    gamepkg::local_record_release(foreign.call_handle())?;
    let mut view = gate::Gate::gate_borrow(&ram, owner.gate_id())?;
    probe_need(view.gate_close(&ram) == Err(-4))?;
    owner.gate_enter(&ram, 0)?;
    view.gate_enter(&ram, 0)?;
    probe_need(
        owner.gate_state(&ram)? == [0, 2]
            && owner.gate_close(&ram) == Err(-2)
            && owner.gate_state(&ram)? == [0, 2],
    )?;
    let command = gamepkg::local_mailbox_create(8, 1)?;
    let reply = gamepkg::local_mailbox_create(8, 1)?;
    let context = gamepkg::local_record_create(
        &[
            u64::to_le_bytes(ram.call_handle()),
            u64::to_le_bytes(owner.gate_id()),
            u64::to_le_bytes(command),
            u64::to_le_bytes(reply),
        ]
        .concat(),
    )?;
    let mut out = [0u8; 8];
    probe_need(
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
    let task = u64::from_le_bytes(out);
    probe_reply(reply, 1)?;
    gamepkg::local_mailbox_send(command, &u64::to_le_bytes(0))?;
    probe_reply(reply, 2)?;
    view.gate_leave_raw(&ram)?;
    probe_need(owner.gate_state(&ram)? == [0, 1])?;
    gamepkg::local_mailbox_send(command, &u64::to_le_bytes(0))?;
    probe_reply(reply, 2)?;
    owner.gate_leave_raw(&ram)?;
    gamepkg::local_mailbox_send(command, &u64::to_le_bytes(1))?;
    probe_reply(reply, 3)?;
    probe_need(
        owner.gate_state(&ram)? == [task, 1]
            && owner.gate_enter(&ram, 0) == Err(-2)
            && owner.gate_close(&ram) == Err(-2)
            && owner.gate_leave_raw(&ram) == Err(-4),
    )?;
    if cancel {
        gamepkg::local_call(12, &u64::to_le_bytes(task), &mut [])?;
    } else {
        gamepkg::local_mailbox_send(command, &u64::to_le_bytes(2))?;
    }
    probe_reply(reply, 4)?;
    if !cancel {
        gamepkg::local_mailbox_send(command, &u64::to_le_bytes(3))?;
    }
    let mut joined = false;
    for _ in 0..4096u32 {
        match gamepkg::local_call(11, &u64::to_le_bytes(task), &mut out) {
            Ok(8) => {
                probe_need(u64::from_le_bytes(out) == 0)?;
                joined = true;
                break;
            }
            Err(-2) => memory::Memory::call_retry_pause()?,
            Err(c) => return Err(c),
            _ => return Err(-3),
        }
    }
    probe_need(joined)?;
    gamepkg::local_record_release(context)?;
    gamepkg::local_mailbox_release(command)?;
    gamepkg::local_mailbox_release(reply)?;
    let stale = owner.gate_id();
    owner.gate_enter(&ram, 0)?;
    owner.gate_enter(&ram, 0)?;
    services.service_close(&ram)?;
    probe_need(owner.gate_enter(&ram, 0) == Err(-4))?;
    owner.gate_leave_raw(&ram)?;
    probe_need(owner.gate_enter(&ram, 0) == Err(-4))?;
    owner.gate_leave_raw(&ram)?;
    probe_need(
        owner.gate_state(&ram) == Err(-4)
            && owner.gate_close(&ram) == Err(-4)
            && owner.gate_closed()
            && gate::Gate::gate_borrow(&ram, stale).err() == Some(-4),
    )?;
    gamepkg::local_record_release(ram.call_handle())?;
    Ok(())
}

pub fn gate_probe() -> Result<(), i32> {
    for cancel in [false, true] {
        gate_case(cancel)?;
    }
    Ok(())
}
