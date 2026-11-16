// Admitted fixture: actual reviewed workers, records, gates and clock wake lane.
use crate::gamepkg;
use crate::irql;
use crate::kernel_pool;
use crate::memory;
use crate::notify;
use crate::queue;
use crate::registers;
use crate::wait_service;
fn wake_need(value: bool) -> Result<(), i32> {
    if value {
        Ok(())
    } else {
        Err(-3)
    }
}
fn wake_word(bytes: &[u8], at: usize) -> Result<u64, i32> {
    let b = bytes.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}
fn wake_exchange(commands: u64, replies: u64, cmd: u64) -> Result<[u64; 3], i32> {
    gamepkg::local_mailbox_send(commands, &u64::to_le_bytes(cmd))?;
    let mut bytes = [0u8; 24];
    wake_need(gamepkg::local_mailbox_receive(replies, 1000, &mut bytes)? == 24)?;
    Ok([
        wake_word(&bytes, 0)?,
        wake_word(&bytes, 8)?,
        wake_word(&bytes, 16)?,
    ])
}
fn wake_case(
    ram: &mut memory::Memory,
    task: &mut irql::Task,
    pending: &queue::Queue,
    notifier: &notify::Notifier,
    index: u32,
) -> Result<[u64; 3], i32> {
    let mut cpu = registers::Registers::new();
    let level = [0u8, 1, 2, 3, 4, 16, 255, 128][((index >> 3) & 7) as usize];
    // Original mistaken lower-to-higher publishes a level without adding holds.
    task.irql_lower(ram, &mut cpu, level)?;
    let depth = (index >> 1) & 3;
    for _ in 0..depth {
        task.irql_gate_enter(ram, 0)?;
    }
    task.irql_set_isr(ram, index & 64 != 0)?;
    if index & 128 != 0 {
        wake_need(pending.queue_insert(ram, 0x740100, 11, 22)? == [1, 0])?;
    }
    let before = gamepkg::local_clock_state()?.generation;
    let queued = notifier.notify_queued(ram, task, index & 1 != 0)?;
    let released = notifier.notify_release(ram, task, index & 1 != 0)?;
    let after = gamepkg::local_clock_state()?.generation;
    task.irql_set_isr(ram, false)?;
    for _ in 0..depth {
        task.irql_gate_leave(ram, &mut cpu)?;
    }
    task.irql_lower(ram, &mut cpu, 0)?;
    Ok([
        u64::from(queued),
        u64::from(released),
        after.checked_sub(before).ok_or(-3)?,
    ])
}
fn wake_worker_run(context: u64) -> Result<u64, i32> {
    let (_, bytes) = gamepkg::local_record_snapshot(context, 48)?;
    wake_need(bytes.len() == 48)?;
    let mut ram = memory::Memory::with_call_handle(wake_word(&bytes, 0)?)?;
    ram.attach(0x740000, 65536, wake_word(&bytes, 8)?, 0, true)?;
    let pending = queue::Queue::queue_borrow(&ram, wake_word(&bytes, 16)?)?;
    let controller = irql::Controller::controller_borrow(&ram, wake_word(&bytes, 24)?)?;
    let mut task = controller.controller_task(&ram)?;
    let notifier = notify::Notifier::notify_new(&ram, &pending, &controller)?;
    let commands = wake_word(&bytes, 32)?;
    let replies = wake_word(&bytes, 40)?;
    let mut sleeper = wait_service::Sleeper::sleep_new(&ram)?;
    gamepkg::local_mailbox_send(replies, &[0u8; 24])?;
    for _ in 0..2048u32 {
        let mut bytes = [0u8; 8];
        wake_need(gamepkg::local_mailbox_receive(commands, 1000, &mut bytes)? == 8)?;
        let cmd = u64::from_le_bytes(bytes);
        if cmd == 513 {
            return Ok(0);
        }
        let mut result = [0u64; 3];
        if cmd < 512 {
            wake_need(cmd >= 256)?;
            result = wake_case(&mut ram, &mut task, &pending, &notifier, cmd as u32)?;
        } else if cmd == 515 {
            task.irql_gate_enter(&ram, 0)?;
        } else if cmd == 516 {
            task.irql_gate_leave(&mut ram, &mut registers::Registers::new())?;
        } else {
            wake_need(cmd == 512)?;
        }
        if cmd < 513 {
            while pending.queue_pop(&mut ram)?.is_some() {}
            // A wake delivered before waiting must remain pending, then coalesce.
            let had = sleeper.sleep_pending(&ram)?;
            wake_need(sleeper.sleep_ns(&ram, 0)? == u32::from(had))?;
            wake_need(!sleeper.sleep_pending(&ram)?)?;
        }
        let mut output = std::vec::Vec::new();
        for value in result {
            output.extend(u64::to_le_bytes(value));
        }
        gamepkg::local_mailbox_send(replies, &output)?;
    }
    Err(-2)
}
pub fn wake_worker(context: u64) -> u64 {
    match wake_worker_run(context) {
        Ok(value) => value,
        Err(code) => code as i64 as u64,
    }
}
pub fn wake_probe() -> Result<std::vec::Vec<u8>, i32> {
    let mut bytes = [0u8; 16];
    wake_need(gamepkg::local_call(1, &[], &mut bytes)? == 16)?;
    let window = wake_word(&bytes, 0)?;
    let request = [
        u64::to_le_bytes(window).as_slice(),
        &u64::to_le_bytes(0),
        &u64::to_le_bytes(65536),
        &u32::to_le_bytes(2),
    ]
    .concat();
    gamepkg::local_call(2, &request, &mut [])?;
    let mut ram = memory::Memory::new()?;
    ram.attach(0x740000, 65536, window, 0, true)?;
    let services = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2)?;
    let mut pending = queue::Queue::queue_new(&ram, false)?;
    let mut controller = irql::Controller::controller_new(&ram)?;
    let mut task = controller.controller_task(&ram)?;
    wake_need(notify::Notifier::notify_new(&ram, &pending, &controller).err() == Some(-5))?;
    let commands = gamepkg::local_mailbox_create(8, 1)?;
    let replies = gamepkg::local_mailbox_create(24, 1)?;
    let context = gamepkg::local_record_create(
        &[
            u64::to_le_bytes(ram.call_handle()),
            u64::to_le_bytes(window),
            u64::to_le_bytes(pending.queue_id()),
            u64::to_le_bytes(controller.controller_id()),
            u64::to_le_bytes(commands),
            u64::to_le_bytes(replies),
        ]
        .concat(),
    )?;
    let mut worker = [0u8; 8];
    gamepkg::local_call(
        10,
        &[
            u32::to_le_bytes(0).as_slice(),
            &u64::to_le_bytes(context),
            &u32::to_le_bytes(1),
        ]
        .concat(),
        &mut worker,
    )?;
    let worker = u64::from_le_bytes(worker);
    pending.queue_assign(&ram, worker)?;
    let notifier = notify::Notifier::notify_new(&ram, &pending, &controller)?;
    let mut resumed = [0u8; 4];
    wake_need(
        gamepkg::local_call(13, &u64::to_le_bytes(worker), &mut resumed)? == 4
            && resumed == u32::to_le_bytes(1),
    )?;
    let mut ready = [0u8; 24];
    wake_need(
        gamepkg::local_mailbox_receive(replies, 1000, &mut ready)? == 24 && ready == [0u8; 24],
    )?;
    let mut output = std::vec::Vec::new();
    for index in 0..512u32 {
        let result = if index < 256 {
            let value = wake_case(&mut ram, &mut task, &pending, &notifier, index)?;
            wake_exchange(commands, replies, 512)?;
            value
        } else {
            wake_exchange(commands, replies, u64::from(index))?
        };
        for value in result {
            output.extend(u64::to_le_bytes(value));
        }
    }
    // Empty queued hook is intentionally different from pending release hook.
    wake_need(
        notifier.notify_queued(&ram, &task, true)?
            && !notifier.notify_release(&ram, &task, true)?,
    )?;
    wake_exchange(commands, replies, 512)?;
    // Another actual task's dispatch hold must not suppress this task's kick.
    wake_exchange(commands, replies, 515)?;
    wake_need(notifier.notify_queued(&ram, &task, true)?)?;
    wake_exchange(commands, replies, 516)?;
    wake_exchange(commands, replies, 512)?;
    let before = gamepkg::local_clock_state()?.generation;
    let foreign = memory::Memory::new()?;
    wake_need(notifier.notify_queued(&foreign, &task, true) == Err(-4))?;
    gamepkg::local_record_release(foreign.call_handle())?;
    let mut other = irql::Controller::controller_new(&ram)?;
    let other_task = other.controller_task(&ram)?;
    wake_need(notifier.notify_release(&ram, &other_task, true) == Err(-4))?;
    other.controller_close(&ram)?;
    let mut readonly = ram.call_view();
    readonly.attach(0x740000, 65536, window, 0, false)?;
    wake_need(notifier.notify_insert(&mut readonly, &task, true, 0x740100, 1, 2) == Err(-4))?;
    wake_need(notifier.notify_insert(&mut ram, &task, true, 0, 1, 2)? == [0, 0, 0])?;
    wake_need(gamepkg::local_clock_state()?.generation == before)?;
    wake_need(notifier.notify_insert(&mut ram, &task, true, 0x740100, 1, 2)? == [1, 0, 1])?;
    let before = gamepkg::local_clock_state()?.generation;
    wake_need(notifier.notify_insert(&mut ram, &task, true, 0x740100, 3, 4)? == [0, 0, 0])?;
    wake_need(gamepkg::local_clock_state()?.generation == before)?;
    wake_exchange(commands, replies, 512)?;
    for slot in 0..63u32 {
        wake_need(
            notifier.notify_insert(&mut ram, &task, false, 0x740100 + slot * 32, 1, 2)?
                == [1, 0, 0],
        )?;
    }
    let before = gamepkg::local_clock_state()?.generation;
    wake_need(notifier.notify_insert(&mut ram, &task, true, 0x740900, 1, 2)? == [0, 1, 0])?;
    wake_need(gamepkg::local_clock_state()?.generation == before)?;
    wake_exchange(commands, replies, 512)?;
    let (version, mut malformed) = gamepkg::local_record_snapshot(pending.queue_id(), 1616)?;
    malformed[72] = 1;
    let version = gamepkg::local_record_compare_exchange(pending.queue_id(), version, &malformed)?;
    let before = gamepkg::local_clock_state()?.generation;
    wake_need(notifier.notify_queued(&ram, &task, true) == Err(-1))?;
    wake_need(gamepkg::local_clock_state()?.generation == before)?;
    malformed[72] = 0;
    gamepkg::local_record_compare_exchange(pending.queue_id(), version, &malformed)?;
    // Temporarily revoke the root's KSV association while the timer stays live.
    let (version, original) = gamepkg::local_record_snapshot(ram.call_handle(), 288)?;
    let mut revoked = original.clone();
    for lane in 280..288usize {
        revoked[lane] = 0;
    }
    let version = gamepkg::local_record_compare_exchange(ram.call_handle(), version, &revoked)?;
    let before = gamepkg::local_clock_state()?.generation;
    wake_need(notifier.notify_queued(&ram, &task, true) == Err(-4))?;
    wake_need(notifier.notify_release(&ram, &task, true) == Err(-4))?;
    wake_need(gamepkg::local_clock_state()?.generation == before)?;
    gamepkg::local_record_compare_exchange(ram.call_handle(), version, &original)?;
    // Notification needs no new sync object/record, even at both hard quotas.
    let mut records = std::vec::Vec::new();
    let mut sync = std::vec::Vec::new();
    for _ in 0..65u32 {
        match gamepkg::local_record_create(&[0u8; 8]) {
            Ok(id) => records.push(id),
            Err(-2) => break,
            Err(code) => return Err(code),
        }
    }
    wake_need(!records.is_empty() && gamepkg::local_record_create(&[0u8; 8]) == Err(-2))?;
    for _ in 0..129u32 {
        let mut out = [0u8; 8];
        match gamepkg::local_call(20, &[0u8; 8], &mut out) {
            Ok(8) => sync.push(u64::from_le_bytes(out)),
            Err(-2) => break,
            Err(code) => return Err(code),
            _ => return Err(-3),
        }
    }
    let mut out = [0u8; 8];
    wake_need(!sync.is_empty() && gamepkg::local_call(20, &[0u8; 8], &mut out) == Err(-2))?;
    wake_need(notifier.notify_queued(&ram, &task, true)?)?;
    wake_need(!notifier.notify_release(&ram, &task, true)?)?;
    for id in sync {
        gamepkg::local_call(25, &u64::to_le_bytes(id), &mut [])?;
    }
    for id in records {
        gamepkg::local_record_release(id)?;
    }
    wake_exchange(commands, replies, 512)?;
    // Worker exit/join and stale copied notifier never manufacture a wake.
    gamepkg::local_mailbox_send(commands, &u64::to_le_bytes(513))?;
    for _ in 0..1000u32 {
        if gamepkg::local_task_finished(worker)? {
            break;
        }
        memory::Memory::call_retry_pause()?;
    }
    wake_need(gamepkg::local_task_finished(worker)?)?;
    let before = gamepkg::local_clock_state()?.generation;
    wake_need(notifier.notify_release(&ram, &task, true) == Err(-4))?;
    wake_need(pending.queue_join(&ram)? == 0)?;
    pending.queue_close(&ram)?;
    wake_need(notifier.notify_queued(&ram, &task, true) == Err(-4))?;
    wake_need(gamepkg::local_clock_state()?.generation == before)?;
    controller.controller_close(&ram)?;
    for id in [commands, replies] {
        gamepkg::local_mailbox_release(id)?;
    }
    gamepkg::local_record_release(context)?;
    services.service_close(&ram)?;
    gamepkg::local_record_release(ram.call_handle())?;
    gamepkg::local_call(
        4,
        &[
            u64::to_le_bytes(window),
            u64::to_le_bytes(0),
            u64::to_le_bytes(65536),
        ]
        .concat(),
        &mut [],
    )?;
    Ok(output)
}
