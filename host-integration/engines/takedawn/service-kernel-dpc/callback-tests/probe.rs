use crate::callback;
use crate::cases;
use crate::gamepkg;
use crate::irql;
use crate::kernel_pool;
use crate::memory;
use crate::queue;
use crate::stacks;
fn cb_need(value: bool) -> Result<(), i32> {
    if value {
        Ok(())
    } else {
        Err(-3)
    }
}
// The depth-one witness channel has retryable backpressure. Preserve every
// witness instead of terminating the worker (and losing its error report) if
// the parent is briefly descheduled. This is test transport, not a game limit.
pub fn callback_send(replies: u64, bytes: &[u8]) -> Result<(), i32> {
    let start = gamepkg::local_performance_sample()?;
    loop {
        match gamepkg::local_mailbox_send(replies, bytes) {
            Ok(()) => return Ok(()),
            Err(-2) => {
                let now = gamepkg::local_performance_sample()?;
                if now.frequency != start.frequency || now.counter < start.counter {
                    return Err(-3);
                }
                if now.counter - start.counter >= start.frequency {
                    return Err(-2);
                }
                memory::Memory::call_retry_pause()?;
            }
            Err(code) => return Err(code),
        }
    }
}
fn cb_word(bytes: &[u8], at: usize) -> Result<u64, i32> {
    let b = bytes.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}
fn cb_worker(context: u64) -> Result<u64, i32> {
    let (_, bytes) = gamepkg::local_record_snapshot(context, 64)?;
    cb_need(bytes.len() == 64)?;
    let mut ram = memory::Memory::with_call_handle(cb_word(&bytes, 0)?)?;
    ram.attach(0x740000, 524288, cb_word(&bytes, 8)?, 0, true)?;
    let pending = queue::Queue::queue_borrow(&ram, cb_word(&bytes, 16)?)?;
    let controller = irql::Controller::controller_borrow(&ram, cb_word(&bytes, 24)?)?;
    let pool = stacks::Pool::stacks_borrow(&ram, cb_word(&bytes, 32)?)?;
    let replies = cb_word(&bytes, 48)?;
    let scenario = cb_word(&bytes, 56)?;
    let mut task = controller.controller_task(&ram)?;
    let mut lease = pool.stacks_allocate(&ram)?.ok_or(-2)?;
    let invoker = callback::Callback::callback_new(&ram, &pending, &controller, &pool)?;
    let result = cb_cases(&mut ram, &mut task, &lease, &invoker, replies, scenario);
    let aborted = if result.is_err() {
        task.irql_abort(&ram)
    } else {
        Ok(())
    };
    let freed = lease.stack_release(&ram);
    result?;
    aborted?;
    freed?;
    cb_need(lease.stack_released())?;
    Ok(0)
}
fn cb_cases(
    ram: &mut memory::Memory,
    task: &mut irql::Task,
    lease: &stacks::Lease,
    invoker: &callback::Callback,
    replies: u64,
    scenario: u64,
) -> Result<(), i32> {
    let top = lease.stack_guest_top(ram)?;
    if scenario != 0 {
        let mut cpu = cases::callback_seed(0, ram, top)?;
        ram.write_u32(0x741004, 1)?;
        ram.write_u32(0x741030, scenario as u32)?;
        task.irql_gate_enter(ram, 0)?;
        let params = cases::callback_params(0);
        let result = if scenario == 4 || scenario == 5 {
            let (_, root) = gamepkg::local_record_snapshot(ram.call_handle(), 288)?;
            cb_need(root.len() == 288)?;
            let mut readonly = ram.call_view();
            let backing = ram.memory_page_backing(0x740000)?;
            if scenario == 4 {
                readonly.attach(0x740000, 524288, backing[0], backing[1], false)?;
            } else {
                cpu.fs_base = 0x742000;
                readonly.attach(0x740000, 8192, backing[0], backing[1], true)?;
                readonly.attach(0x742000, 4096, backing[0], backing[1] + 8192, false)?;
                readonly.attach(
                    0x743000,
                    524288 - 12288,
                    backing[0],
                    backing[1] + 12288,
                    true,
                )?;
            }
            invoker.callback_run(
                &mut readonly,
                task,
                &mut cpu,
                lease,
                params[0],
                params[3],
                params[4],
                params[5] as i32,
            )
        } else {
            invoker.callback_run(
                ram,
                task,
                &mut cpu,
                lease,
                params[0],
                params[3],
                params[4],
                params[5] as i32,
            )
        };
        cb_need(
            result
                == Err(if scenario == 1 {
                    -5
                } else if scenario == 2 {
                    -3
                } else if scenario == 3 {
                    -2
                } else if scenario == 6 {
                    -1
                } else {
                    -4
                }),
        )?;
        // The actor is permanently aborted, both counts and raw holds are gone.
        cb_need(task.irql_current(ram) == Err(-4))?;
        let controller = irql::Controller::controller_borrow(ram, task.task_key()[0])?;
        cb_need(controller.controller_counts(ram)?[0..2] == [0, 0])?;
        let lock = crate::gate::Gate::gate_borrow(ram, controller.controller_gate())?;
        cb_need(lock.gate_state(ram)?[1] == 0)?;
        if scenario == 5 {
            cb_need(
                ram.read_u32(0x741000)? == 2
                    && ram.read_u32(0x743000)? == 1
                    && ram.read_u32(0x743018)? == 5,
            )?;
        }
        if scenario == 6 {
            cb_need(ram.read_u32(0x741000)? == 3 && ram.read_u32(0x743030)? == 5)?;
        }
        callback_send(replies, &[0u8; 336])?;
        return Ok(());
    }
    for index in 0..256u32 {
        let params = cases::callback_params(index);
        let mut cpu = cases::callback_seed(index, ram, top)?;
        task.irql_raise(ram, &mut cpu, cases::callback_old(index))?;
        let ran = invoker.callback_run(
            ram,
            task,
            &mut cpu,
            lease,
            params[0],
            params[3],
            params[4],
            params[5] as i32,
        )?;
        let mut witness = cpu.encode();
        let mut bytes = std::vec::Vec::new();
        bytes.resize(524288, 0);
        ram.read_bytes(0x740000, &mut bytes)?;
        witness.extend(u64::to_le_bytes(cases::callback_hash(&bytes)));
        witness.extend(u32::to_le_bytes(u32::from(ran)));
        witness.extend(u32::to_le_bytes(u32::from(task.irql_current(ram)?)));
        witness.extend(u32::to_le_bytes(ram.read_u32(0x741000)?));
        cb_need(witness.len() == 336)?;
        let controller = irql::Controller::controller_borrow(ram, task.task_key()[0])?;
        let counts = controller.controller_counts(ram)?;
        cb_need(counts[0] == u64::from(cases::callback_old(index) >= 2) && counts[1] == 0)?;
        callback_send(replies, &witness)?;
        task.irql_lower(ram, &mut cpu, 0)?;
    }
    // Preflight errors are explicit and leave the live worker reusable.
    let params = cases::callback_params(0);
    let mut cpu = cases::callback_seed(0, ram, top)?;
    ram.write_u32(0x74010c, 0x3300)?;
    let before = cpu.encode();
    cb_need(invoker.callback_run(ram, task, &mut cpu, lease, params[0], 1, 2, 0) == Err(-5))?;
    cb_need(
        cpu.encode() == before && ram.read_u32(0x740114)? == 0 && task.irql_current(ram)? == 0,
    )?;
    cpu.esp = top + 4;
    cb_need(invoker.callback_run(ram, task, &mut cpu, lease, params[0], 1, 2, 0) == Err(-1))?;
    cpu.esp = top;
    cb_need(invoker.callback_run(ram, task, &mut cpu, lease, u32::MAX, 1, 2, 0) == Err(-1))?;
    Ok(())
}
pub fn callback_worker(context: u64) -> u64 {
    match cb_worker(context) {
        Ok(value) => value,
        Err(code) => {
            // Keep mutation controls diagnostic: report the worker's actual
            // failure rather than making the parent infer it from a timeout.
            if let Ok((_, bytes)) = gamepkg::local_record_snapshot(context, 64) {
                if let Ok(replies) = cb_word(&bytes, 48) {
                    let mut message = [0u8; 336];
                    message[0..4].copy_from_slice(&u32::to_le_bytes(0x52454243));
                    message[4..8].copy_from_slice(&i32::to_le_bytes(code));
                    let _ = callback_send(replies, &message);
                }
            }
            code as i64 as u64
        }
    }
}
pub fn callback_probe() -> Result<std::vec::Vec<u8>, i32> {
    let mut window = [0u8; 16];
    cb_need(gamepkg::local_call(1, &[], &mut window)? == 16)?;
    let window = cb_word(&window, 0)?;
    gamepkg::local_call(
        2,
        &[
            u64::to_le_bytes(window).as_slice(),
            &u64::to_le_bytes(0),
            &u64::to_le_bytes(524288),
            &u32::to_le_bytes(2),
        ]
        .concat(),
        &mut [],
    )?;
    let mut output = std::vec::Vec::new();
    for scenario in 0..7u64 {
        let mut ram = memory::Memory::new()?;
        ram.attach(0x740000, 524288, window, 0, true)?;
        let services = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2)?;
        let mut pending = queue::Queue::queue_new(&ram, false)?;
        let mut controller = irql::Controller::controller_new(&ram)?;
        let mut pool = stacks::Pool::stacks_new(&ram, 0x780000, 262144)?;
        cb_need(
            callback::Callback::callback_new(&ram, &pending, &controller, &pool).err() == Some(-5),
        )?;
        let replies = gamepkg::local_mailbox_create(336, 1)?;
        let context = gamepkg::local_record_create(
            &[
                u64::to_le_bytes(ram.call_handle()),
                u64::to_le_bytes(window),
                u64::to_le_bytes(pending.queue_id()),
                u64::to_le_bytes(controller.controller_id()),
                u64::to_le_bytes(pool.stacks_id()),
                u64::to_le_bytes(0),
                u64::to_le_bytes(replies),
                u64::to_le_bytes(scenario),
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
        let invoker = callback::Callback::callback_new(&ram, &pending, &controller, &pool)?;
        let mut main_task = controller.controller_task(&ram)?;
        let mut main_lease = pool.stacks_allocate(&ram)?.ok_or(-2)?;
        let mut cpu = cases::callback_initial(main_lease.stack_guest_top(&ram)?);
        cb_need(
            invoker.callback_run(&mut ram, &mut main_task, &mut cpu, &main_lease, 0, 0, 0, 0)
                == Err(-4),
        )?;
        main_lease.stack_release(&ram)?;
        let mut resumed = [0u8; 4];
        gamepkg::local_call(13, &u64::to_le_bytes(worker), &mut resumed)?;
        for _ in 0..if scenario == 0 { 256u32 } else { 1 } {
            let mut bytes = [0u8; 336];
            cb_need(gamepkg::local_mailbox_receive(replies, 1000, &mut bytes)? == 336)?;
            if u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) == 0x52454243 {
                return Err(i32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]));
            }
            if scenario == 0 {
                output.extend(bytes);
            } else {
                cb_need(bytes == [0u8; 336])?;
            }
        }
        for _ in 0..1000u32 {
            if gamepkg::local_task_finished(worker)? {
                break;
            }
            memory::Memory::call_retry_pause()?;
        }
        cb_need(gamepkg::local_task_finished(worker)? && pending.queue_join(&ram)? == 0)?;
        cb_need(pool.stacks_usage(&ram)? == [0, 1])?;
        pending.queue_close(&ram)?;
        controller.controller_close(&ram)?;
        pool.stacks_close(&ram)?;
        gamepkg::local_mailbox_release(replies)?;
        gamepkg::local_record_release(context)?;
        services.service_close(&ram)?;
        gamepkg::local_record_release(ram.call_handle())?;
    }
    gamepkg::local_call(
        4,
        &[
            u64::to_le_bytes(window),
            u64::to_le_bytes(0),
            u64::to_le_bytes(524288),
        ]
        .concat(),
        &mut [],
    )?;
    Ok(output)
}
