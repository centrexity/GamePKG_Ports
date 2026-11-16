// Checked fixtures: malformed records, explicit failure cleanup and real task ownership.
use crate::gamepkg;
use crate::gate;
use crate::irql;
use crate::kernel_pool;
use crate::memory;
use crate::registers;
fn control_need(ok: bool) -> Result<(), i32> {
    if ok {
        Ok(())
    } else {
        Err(-3)
    }
}
fn control_word(b: &[u8], at: usize) -> Result<u64, i32> {
    let s = b.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
    ]))
}
fn control_put(b: &mut [u8], at: usize, v: u64) {
    let bytes = u64::to_le_bytes(v);
    for i in 0..8usize {
        b[at + i] = bytes[i];
    }
}
fn control_reply(id: u64, v: u64) -> Result<(), i32> {
    let mut b = [0u8; 8];
    control_need(
        gamepkg::local_mailbox_receive(id, 1000, &mut b)? == 8 && u64::from_le_bytes(b) == v,
    )
}
fn control_worker_run(id: u64) -> Result<(), i32> {
    let (_, b) = gamepkg::local_record_snapshot(id, 40)?;
    control_need(b.len() == 40)?;
    let mut ram = memory::Memory::with_call_handle(control_word(&b, 0)?)?;
    ram.attach(0x740000, 65536, control_word(&b, 32)?, 0, true)?;
    let owner = irql::Controller::controller_borrow(&ram, control_word(&b, 8)?)?;
    let mut task = owner.controller_task(&ram)?;
    let mut cpu = registers::Registers::new();
    control_need(task.task_key()[1] == gamepkg::local_task_status()?.0)?;
    control_need(task.irql_raise(&mut ram, &mut cpu, 2)? == 0)?;
    task.irql_disable(&mut ram, &mut cpu)?;
    gamepkg::local_mailbox_send(control_word(&b, 24)?, &u64::to_le_bytes(1))?;
    let mut message = [0u8; 8];
    let received = gamepkg::local_mailbox_receive(control_word(&b, 16)?, 1000, &mut message);
    let cancelled = gamepkg::local_task_status()?.1;
    if cancelled {
        control_need(received == Err(-4))?;
        task.irql_abort(&ram)?;
        control_need(task.irql_current(&ram) == Err(-4))?;
    } else {
        control_need(received? == 8)?;
        task.irql_lower(&mut ram, &mut cpu, 0)?;
        task.irql_enable_after_shadow(&mut ram, &mut cpu)?;
    }
    gamepkg::local_mailbox_send(control_word(&b, 24)?, &u64::to_le_bytes(2))?;
    Ok(())
}
pub fn irql_worker(id: u64) -> u64 {
    match control_worker_run(id) {
        Ok(()) => 0,
        Err(c) => c as i64 as u64,
    }
}
pub fn controls(ram: &mut memory::Memory, window: u64) -> Result<(), i32> {
    ram.write_u32(0x741000, 0)?;
    ram.write_u8(0x741008, 0)?;
    let foreign = memory::Memory::new()?;
    let mut owner = irql::Controller::controller_new(ram)?;
    let mut task = owner.controller_task(ram)?;
    control_need(task.irql_hook_context(ram) == Err(-5) && task.irql_hook_bind(ram, 0) == Err(-1))?;
    task.irql_hook_bind(ram, owner.controller_id())?;
    task.irql_hook_bind(ram, owner.controller_id())?;
    control_need(
        task.irql_hook_context(ram)? == owner.controller_id()
            && task.irql_hook_bind(ram, foreign.call_handle()) == Err(-4),
    )?;
    control_need(owner.controller_task(ram)?.irql_hook_context(ram) == Err(-5))?;
    control_need(task.task_values(&foreign) == Err(-4))?;
    gamepkg::local_record_release(foreign.call_handle())?;
    let mut view = irql::Controller::controller_borrow(ram, owner.controller_id())?;
    control_need(view.controller_close(ram) == Err(-4))?;
    // All malformed snapshots must fail without consuming/changing the valid record.
    let (_, original) = gamepkg::local_record_snapshot(owner.controller_id(), 2176)?;
    for pair in [
        [0u64, 0],
        [56, 2],
        [32, 1],
        [40, 1],
        [48, u32::MAX as u64 + 1],
        [72, 2],
        [80, 256],
        [88, 2],
        [144, 1],
    ] {
        let mut bad = original.clone();
        control_put(&mut bad, pair[0] as usize, pair[1]);
        let (v, _) = gamepkg::local_record_snapshot(owner.controller_id(), 2176)?;
        gamepkg::local_record_compare_exchange(owner.controller_id(), v, &bad)?;
        control_need(owner.controller_counts(ram) == Err(-1))?;
        let (v, after) = gamepkg::local_record_snapshot(owner.controller_id(), 2176)?;
        control_need(after == bad)?;
        gamepkg::local_record_compare_exchange(owner.controller_id(), v, &original)?;
    }
    // Synthetic full table with 33 unique occupied IDs: never allocate a 34th slot.
    let mut full = original.clone();
    for slot in 0..33usize {
        control_put(&mut full, 64 + slot * 64, slot as u64 + 1);
        control_put(&mut full, 72 + slot * 64, 1);
    }
    let (v, _) = gamepkg::local_record_snapshot(owner.controller_id(), 2176)?;
    gamepkg::local_record_compare_exchange(owner.controller_id(), v, &full)?;
    control_need(owner.controller_task(ram).err() == Some(-2))?;
    let (v, after) = gamepkg::local_record_snapshot(owner.controller_id(), 2176)?;
    control_need(after == full)?;
    gamepkg::local_record_compare_exchange(owner.controller_id(), v, &original)?;
    let mut closing = original.clone();
    control_put(&mut closing, 56, 1);
    let (v, _) = gamepkg::local_record_snapshot(owner.controller_id(), 2176)?;
    gamepkg::local_record_compare_exchange(owner.controller_id(), v, &closing)?;
    control_need(task.task_values(ram) == Err(-4) && owner.controller_task(ram).err() == Some(-4))?;
    let (v, _) = gamepkg::local_record_snapshot(owner.controller_id(), 2176)?;
    gamepkg::local_record_compare_exchange(owner.controller_id(), v, &original)?;
    // A PCR address error occurs after the original state prefix; abort is required.
    let mut cpu = registers::Registers::new();
    cpu.fs_base = u32::MAX;
    control_need(task.irql_raise(ram, &mut cpu, 2) == Err(-1))?;
    control_need(
        owner.controller_counts(ram)? == [1, 0, 1] && owner.controller_close(ram) == Err(-2),
    )?;
    task.irql_abort(ram)?;
    control_need(
        owner.controller_counts(ram)? == [0, 0, 1] && owner.controller_task(ram).err() == Some(-4),
    )?;
    owner.controller_close(ram)?;
    let mut read_only = ram.call_view();
    read_only.attach(0x740000, 65536, window, 0, false)?;
    let mut owner = irql::Controller::controller_new(&read_only)?;
    let mut task = owner.controller_task(&read_only)?;
    let mut cpu = registers::Registers::new();
    cpu.fs_base = 0x740000;
    let before = ram.read_u8(0x740024)?;
    control_need(task.irql_raise(&mut read_only, &mut cpu, 2) == Err(-4))?;
    control_need(ram.read_u8(0x740024)? == before && owner.controller_counts(ram)? == [1, 0, 1])?;
    task.irql_abort(&read_only)?;
    owner.controller_close(&read_only)?;
    // Actual KSV revocation denies new work but still permits bounded owned cleanup.
    let mut revoked = memory::Memory::new()?;
    let services = kernel_pool::kernel_services_initialize(&revoked, 0x10000, 2)?;
    let mut owner = irql::Controller::controller_new(&revoked)?;
    let mut task = owner.controller_task(&revoked)?;
    let mut cpu = registers::Registers::new();
    task.irql_raise(&mut revoked, &mut cpu, 2)?;
    task.irql_disable(&mut revoked, &mut cpu)?;
    services.service_close(&revoked)?;
    control_need(
        task.irql_current(&revoked) == Err(-4) && task.irql_gate_enter(&revoked, 0) == Err(-4),
    )?;
    task.irql_abort(&revoked)?;
    control_need(owner.controller_close(&revoked) == Err(-4) && owner.controller_closed())?;
    gamepkg::local_record_release(revoked.call_handle())?;
    // More than 64 recursive holds reports incomplete cleanup, then resumes safely.
    let mut owner = irql::Controller::controller_new(ram)?;
    let task = owner.controller_task(ram)?;
    for _ in 0..65u32 {
        task.irql_gate_enter(ram, 0)?;
    }
    control_need(task.irql_abort(ram) == Err(-2))?;
    task.irql_abort(ram)?;
    owner.controller_close(ram)?;
    for cancel in [false, true] {
        let mut owner = irql::Controller::controller_new(ram)?;
        let parent = owner.controller_task(ram)?;
        let command = gamepkg::local_mailbox_create(8, 1)?;
        let response = gamepkg::local_mailbox_create(8, 1)?;
        let context = gamepkg::local_record_create(
            &[
                u64::to_le_bytes(ram.call_handle()),
                u64::to_le_bytes(owner.controller_id()),
                u64::to_le_bytes(command),
                u64::to_le_bytes(response),
                u64::to_le_bytes(window),
            ]
            .concat(),
        )?;
        let mut out = [0u8; 8];
        control_need(
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
        control_reply(response, 1)?;
        control_need(owner.controller_counts(ram)? == [1, 1, 1])?;
        let lock = gate::Gate::gate_borrow(ram, owner.controller_gate())?;
        control_need(
            lock.gate_state(ram)? == [worker, 2]
                && lock.gate_leave_raw(ram) == Err(-4)
                && parent.irql_gate_enter(ram, 0) == Err(-2)
                && owner.controller_close(ram) == Err(-2),
        )?;
        if cancel {
            gamepkg::local_call(12, &u64::to_le_bytes(worker), &mut [])?;
        } else {
            gamepkg::local_mailbox_send(command, &u64::to_le_bytes(1))?;
        }
        control_reply(response, 2)?;
        let mut joined = false;
        for _ in 0..4096u32 {
            match gamepkg::local_call(11, &u64::to_le_bytes(worker), &mut out) {
                Ok(8) => {
                    control_need(u64::from_le_bytes(out) == 0)?;
                    joined = true;
                    break;
                }
                Err(-2) => memory::Memory::call_retry_pause()?,
                Err(c) => return Err(c),
                _ => return Err(-3),
            }
        }
        control_need(
            joined && owner.controller_counts(ram)? == [0, 0, if cancel { 1 } else { 2 }],
        )?;
        owner.controller_close(ram)?;
        control_need(parent.irql_current(ram) == Err(-4))?;
        gamepkg::local_record_release(context)?;
        gamepkg::local_mailbox_release(command)?;
        gamepkg::local_mailbox_release(response)?;
    }
    Ok(())
}
