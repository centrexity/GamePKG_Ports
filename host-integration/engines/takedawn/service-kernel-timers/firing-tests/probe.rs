use crate::callback;
use crate::callback_cases;
use crate::events;
use crate::firing;
use crate::gamepkg;
use crate::irql;
use crate::kernel_pool;
use crate::memory;
use crate::queue;
use crate::shadow;
use crate::stacks;
use crate::timers;
fn fire_need(value: bool) -> Result<(), i32> {
    if value {
        Ok(())
    } else {
        Err(-3)
    }
}
fn fire_word(bytes: &[u8], at: usize) -> Result<u64, i32> {
    let b = bytes.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}
fn fire_actor(context: u64) -> Result<std::vec::Vec<u8>, i32> {
    let (_, b) = gamepkg::local_record_snapshot(context, 80)?;
    fire_need(b.len() == 80)?;
    let mut ram = memory::Memory::with_call_handle(fire_word(&b, 0)?)?;
    ram.attach(0x740000, 524288, fire_word(&b, 8)?, 0, true)?;
    let pending = queue::Queue::queue_borrow(&ram, fire_word(&b, 16)?)?;
    let controller = irql::Controller::controller_borrow(&ram, fire_word(&b, 24)?)?;
    let pool = stacks::Pool::stacks_borrow(&ram, fire_word(&b, 32)?)?;
    let table = timers::Timers::timers_borrow(&ram, fire_word(&b, 48)?)?;
    let map_view = shadow::Shadow::shadow_borrow(&ram, fire_word(&b, 56)?)?;
    let mode = fire_word(&b, 64)? as u32;
    let invoker = callback::Callback::callback_new(&ram, &pending, &controller, &pool)?;
    let mut task = controller.controller_task(&ram)?;
    let mut lease = pool.stacks_allocate(&ram)?.ok_or(-2)?;
    let mut cpu = callback_cases::callback_initial(lease.stack_guest_top(&ram)?);
    if mode == 7 {
        let mut bytes = table.timers_state(&ram)?;
        let wrong = u64::to_le_bytes(pending.queue_id() ^ u64::MAX);
        for lane in 0..8usize {
            bytes[32 + lane] = wrong[lane];
        }
        let record = gamepkg::local_record_create(&bytes)?;
        let fake = timers::Timers::timers_borrow(&ram, record)?;
        fire_need(
            firing::timer_fire_pass(
                &fake, &map_view, &invoker, &mut ram, &mut task, &mut cpu, &lease, 1000, 3,
            ) == Err(-4),
        )?;
        gamepkg::local_record_release(record)?;
        fire_need(task.irql_current(&ram)? == 0)?;
    }
    let mut protected = ram.call_view();
    if mode == 4 {
        let window = fire_word(&b, 8)?;
        protected.attach(0x740000, 0x504, window, 0, true)?;
        protected.attach(0x740504, 4, window, 0x504, false)?;
        protected.attach(0x740508, 524288 - 0x508, window, 0x508, true)?;
    }
    let result = if mode == 4 {
        firing::timer_fire_pass(
            &table,
            &map_view,
            &invoker,
            &mut protected,
            &mut task,
            &mut cpu,
            &lease,
            1000,
            3,
        )
    } else {
        firing::timer_fire_pass(
            &table, &map_view, &invoker, &mut ram, &mut task, &mut cpu, &lease, 1000, 3,
        )
    };
    let expected_error = match mode {
        2 | 3 => -5,
        4 => -4,
        _ => 0,
    };
    if expected_error == 0 {
        fire_need(result == Ok([if mode == 1 { 2 } else { 1 }, u64::MAX]))?;
        fire_need(task.irql_current(&ram)? == 0)?;
        if mode == 5 || mode == 8 {
            fire_need(
                events::event_lookup(&map_view, &ram, 0x740500)? == None
                    && ram.read_u32(0x740504)? == 9,
            )?;
        } else {
            let id = events::event_lookup(&map_view, &ram, 0x740500)?.ok_or(-3)?;
            fire_need(gamepkg::local_event_state(id)?.signaled && ram.read_u32(0x740504)? == 1)?;
        }
        if mode == 1 {
            let id = events::event_lookup(&map_view, &ram, 0x740540)?.ok_or(-3)?;
            let b = table.timers_state(&ram)?;
            fire_need(
                gamepkg::local_event_state(id)?.signaled
                    && ram.read_u32(0x740544)? == 1
                    && fire_word(&b, 72)? == 1010
                    && b[64..68] == u32::to_le_bytes(0x740500)
                    && b[88..92] == [0u8; 4]
                    && b[112..116] == [0u8; 4],
            )?;
        }
    } else {
        fire_need(result == Err(expected_error) && task.irql_current(&ram) == Err(-4))?;
        fire_need(
            firing::timer_fire_pass(
                &table, &map_view, &invoker, &mut ram, &mut task, &mut cpu, &lease, 1000, 3,
            ) == Err(-4),
        )?;
        let id = events::event_lookup(&map_view, &ram, 0x740500)?.ok_or(-3)?;
        fire_need(
            gamepkg::local_event_state(id)?.signaled == (mode == 4) && ram.read_u32(0x740504)? == 9,
        )?;
    }
    let mut witness = cpu.encode();
    witness.extend(u64::to_le_bytes(mode as u64));
    witness.extend(u64::to_le_bytes(expected_error as i64 as u64));
    lease.stack_release(&ram)?;
    fire_need(lease.stack_released())?;
    Ok(witness)
}
pub fn firing_worker(context: u64) -> u64 {
    let result = fire_actor(context);
    if let Ok((_, b)) = gamepkg::local_record_snapshot(context, 80) {
        if let Ok(replies) = fire_word(&b, 40) {
            let mut message = std::vec::Vec::new();
            message.extend(u64::to_le_bytes(2));
            match result {
                Ok(bytes) => {
                    message.extend(u64::to_le_bytes(0));
                    message.extend(bytes);
                }
                Err(code) => {
                    message.extend(u64::to_le_bytes(code as i64 as u64));
                }
            }
            message.resize(512, 0);
            let _ = gamepkg::local_mailbox_send(replies, &message);
        }
    }
    0
}
pub fn firing_probe() -> Result<std::vec::Vec<u8>, i32> {
    let mut info = [0u8; 16];
    fire_need(gamepkg::local_call(1, &[], &mut info)? == 16)?;
    let window = fire_word(&info, 0)?;
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
    for mode in 0..9u64 {
        let mut ram = memory::Memory::new()?;
        ram.attach(0x740000, 524288, window, 0, true)?;
        ram.fill(0x740000, 0, 524288)?;
        let services = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2)?;
        let mut pending = queue::Queue::queue_new(&ram, false)?;
        let mut table = timers::Timers::timers_new(&ram, &pending)?;
        let mut controller = irql::Controller::controller_new(&ram)?;
        let mut pool = stacks::Pool::stacks_new(&ram, 0x780000, 262144)?;
        let mut map_view = shadow::Shadow::shadow_new(&ram)?;
        events::timer_initialize(&map_view, &mut ram, 0x740500, 0)?;
        ram.write_u32(0x740504, 9)?;
        let dpc = if mode == 1 || mode == 2 || mode == 3 || mode == 4 || mode == 6 || mode == 8 {
            0x740100u32
        } else {
            0
        };
        ram.write_u32(
            0x74010c,
            if mode == 2 {
                0x3300
            } else if mode == 6 {
                0
            } else {
                0x1100
            },
        )?;
        ram.write_u32(0x740110, 0xa54e12b8)?;
        let replies = gamepkg::local_mailbox_create(512, 4)?;
        let commands = gamepkg::local_mailbox_create(8, 4)?;
        ram.write_u64(0x741080, table.timers_id())?;
        ram.write_u64(0x741088, map_view.shadow_id())?;
        ram.write_u64(0x7410a0, commands)?;
        ram.write_u64(0x7410a8, replies)?;
        ram.write_u32(0x7410b0, mode as u32)?;
        ram.write_u32(0x741004, (mode & 1) as u32)?;
        let context = gamepkg::local_record_create(
            &[
                u64::to_le_bytes(ram.call_handle()),
                u64::to_le_bytes(window),
                u64::to_le_bytes(pending.queue_id()),
                u64::to_le_bytes(controller.controller_id()),
                u64::to_le_bytes(pool.stacks_id()),
                u64::to_le_bytes(replies),
                u64::to_le_bytes(table.timers_id()),
                u64::to_le_bytes(map_view.shadow_id()),
                u64::to_le_bytes(mode),
                u64::to_le_bytes(commands),
            ]
            .concat(),
        )?;
        let mut job = [0u8; 8];
        gamepkg::local_call(
            10,
            &[
                u32::to_le_bytes(0).as_slice(),
                &u64::to_le_bytes(context),
                &u32::to_le_bytes(1),
            ]
            .concat(),
            &mut job,
        )?;
        let job = u64::from_le_bytes(job);
        pending.queue_assign(&ram, job)?;
        table.timers_start(&ram, &pending)?;
        table.timer_set(&mut ram, 0x740500, 0, 0, dpc)?;
        if mode == 1 {
            events::timer_initialize(&map_view, &mut ram, 0x740540, 0)?;
            events::timer_initialize(&map_view, &mut ram, 0x740580, 0)?;
            table.timer_set(&mut ram, 0x740540, -1000000, 0, 0x740120)?;
            table.timer_set(&mut ram, 0x740580, 0, 0, 0)?;
        }
        if mode == 5 {
            events::event_remove(&map_view, &ram, 0x740500)?;
        }
        let mut resumed = [0u8; 4];
        gamepkg::local_call(13, &u64::to_le_bytes(job), &mut resumed)?;
        let mut calls = 0u32;
        loop {
            let mut packet = [0u8; 512];
            fire_need(gamepkg::local_mailbox_receive(replies, 1000, &mut packet)? == 512)?;
            if fire_word(&packet, 0)? == 2 {
                fire_need(fire_word(&packet, 8)? == 0)?;
                output.extend(packet);
                break;
            }
            fire_need(fire_word(&packet, 0)? == 1)?;
            let b = table.timers_state(&ram)?;
            let lock = fire_word(&b, 24)?;
            gamepkg::local_mutex_lock(lock, 0)?;
            gamepkg::local_call(24, &u64::to_le_bytes(lock), &mut [])?;
            gamepkg::local_mailbox_send(commands, &[0u8; 8])?;
            calls += 1;
        }
        fire_need(
            calls
                == if mode == 1 || mode == 3 || mode == 4 || mode == 8 {
                    1
                } else {
                    0
                },
        )?;
        for _ in 0..1000u32 {
            if gamepkg::local_task_finished(job)? {
                break;
            }
            memory::Memory::call_retry_pause()?;
        }
        fire_need(pending.queue_join(&ram)? == 0)?;
        let joined = pending.queue_join_witness(&ram)?;
        table.timers_close(&ram, Some(&joined))?;
        pending.queue_close(&ram)?;
        fire_need(pool.stacks_usage(&ram)? == [0, 1])?;
        controller.controller_close(&ram)?;
        pool.stacks_close(&ram)?;
        events::event_close_all(&map_view, &ram)?;
        map_view.shadow_close(&ram)?;
        gamepkg::local_record_release(context)?;
        gamepkg::local_mailbox_release(replies)?;
        gamepkg::local_mailbox_release(commands)?;
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
