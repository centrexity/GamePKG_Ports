// Checked authenticated startup integration fixtures; controlled finite routes,
// not production device/profiler handlers, vblank delivery or a full timer loop.
use crate::dispatching;
use crate::events;
use crate::gamepkg;
use crate::irql;
use crate::kernel_data;
use crate::kernel_startup;
use crate::memory;
use crate::queue;
use crate::schedule;
use crate::shadow;
use crate::stacks;
use crate::tick_service;
use crate::tick_startup;
use crate::timer_startup;
use crate::timers;
use crate::worker_setup;
fn launch_need(ok: bool) -> Result<(), i32> {
    if ok {
        Ok(())
    } else {
        Err(-3)
    }
}
fn launch_u64(bytes: &[u8], at: usize) -> Result<u64, i32> {
    let b = bytes.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}
fn launch_cleanup(result: Result<(), i32>, revoked: bool) -> Result<(), i32> {
    launch_need(result == Ok(()) || revoked && result == Err(-4))
}
fn launch_pause() -> Result<(), i32> {
    memory::Memory::call_retry_pause()
}
fn timer_actor(argument: u64) -> Result<(), i32> {
    let (_, bytes) = gamepkg::local_record_snapshot(argument, 112)?;
    launch_need(bytes.len() == 112)?;
    let mut context = [0u64; 4];
    for at in 0..4usize {
        context[at] = launch_u64(&bytes, at * 8)?;
    }
    let pending = launch_u64(&bytes, 32)?;
    let control = launch_u64(&bytes, 40)?;
    let pool = launch_u64(&bytes, 48)?;
    let table = launch_u64(&bytes, 56)?;
    let map = launch_u64(&bytes, 64)?;
    let mut active_worker = worker_setup::Worker::worker_start(
        context,
        pool,
        launch_u64(&bytes, 96)? as u32,
        launch_u64(&bytes, 104)? as u32,
    )?;
    let (view, cpu, _) = active_worker.worker_parts()?;
    gamepkg::diagnostic(
        1,
        0x544b1801,
        &[
            100,
            u64::from(cpu.esp),
            u64::from(cpu.fs_base),
            u64::from(view.read_u32(cpu.fs_base)?),
        ],
    )?;
    let mut dispatcher = dispatching::Dispatch::dispatch_new(
        &mut active_worker,
        pending,
        control,
        pool,
        table,
        map,
        1200,
        0,
        3,
    )?;
    let (ram, _, _) = active_worker.worker_parts()?;
    let display_pacing = schedule::Schedule::schedule_borrow(ram, ram.read_u64(0x741020)?)?;
    launch_need(display_pacing.schedule_worker(ram)? == gamepkg::local_task_status()?.0)?;
    let result = dispatcher.dispatch_pass(&mut active_worker, 0, false);
    let work_code = match result {
        Ok(_) => 0u64,
        Err(value) => value as i64 as u64,
    };
    let (view, _) = active_worker.worker_cleanup()?;
    gamepkg::diagnostic(
        1,
        0x544b1801,
        &[120, work_code, u64::from(view.read_u32(0x742000)?)],
    )?;
    let finished = dispatcher.dispatch_finish(&mut active_worker);
    launch_need(dispatcher.dispatch_closed() && active_worker.worker_finished())?;
    result?;
    finished
}
pub fn timer_worker(argument: u64) -> u64 {
    match timer_actor(argument) {
        Ok(()) => 0,
        Err(code) => code as i64 as u64,
    }
}
fn timer_hold(argument: u64) -> Result<(), i32> {
    let (_, bytes) = gamepkg::local_record_snapshot(argument, 24)?;
    launch_need(bytes.len() == 24)?;
    let mutex = launch_u64(&bytes, 0)?;
    let ready = launch_u64(&bytes, 8)?;
    let ack = launch_u64(&bytes, 16)?;
    gamepkg::local_mutex_lock(mutex, 1000)?;
    gamepkg::local_mailbox_send(ready, &[1u8])?;
    let mut received = Err(-2);
    let mut byte = [0u8; 1];
    for _ in 0..4u32 {
        match gamepkg::local_mailbox_receive(ack, 1000, &mut byte) {
            Err(-2) => {}
            value => {
                received = value;
                break;
            }
        }
    }
    let unlocked = gamepkg::local_call(24, &u64::to_le_bytes(mutex), &mut []);
    launch_need(received? == 1 && byte[0] == 1)?;
    unlocked?;
    Ok(())
}
pub fn timer_holder(argument: u64) -> u64 {
    match timer_hold(argument) {
        Ok(()) => 0,
        Err(code) => code as i64 as u64,
    }
}
fn launch_wait(task: u64) -> Result<(), i32> {
    for _ in 0..4096u32 {
        if gamepkg::local_task_finished(task)? {
            return Ok(());
        }
        launch_pause()?;
    }
    Err(-2)
}
fn launch_join(
    owner: &mut timer_startup::Launch,
    ram: &memory::Memory,
) -> Result<gamepkg::LocalTaskJoin, i32> {
    for _ in 0..4096u32 {
        match owner.launch_join_task(ram) {
            Err(-2) => launch_pause()?,
            other => return other,
        }
    }
    Err(-2)
}
fn tick_join(
    clock: &mut tick_service::Tick,
    ram: &memory::Memory,
) -> Result<gamepkg::LocalTaskJoin, i32> {
    for _ in 0..4096u32 {
        match clock.tick_join_task(ram) {
            Err(-2) => launch_pause()?,
            other => return other,
        }
    }
    Err(-2)
}
fn timer_unrelated_join(argument: u64) -> Result<gamepkg::LocalTaskJoin, i32> {
    let mut request = std::vec::Vec::new();
    request.extend(u32::to_le_bytes(2));
    request.extend(u64::to_le_bytes(argument));
    request.extend(u32::to_le_bytes(0));
    let mut bytes = [0u8; 8];
    launch_need(gamepkg::local_call(10, &request, &mut bytes)? == 8)?;
    let task = u64::from_le_bytes(bytes);
    launch_wait(task)?;
    gamepkg::local_task_join(task)
}
pub fn startup_timer_case(
    owner: &mut kernel_startup::Startup,
    idle_startup: &mut kernel_startup::Startup,
    ram: &mut memory::Memory,
    data: &kernel_data::Data,
    mode: u8,
    shape: [u32; 14],
) -> Result<(), i32> {
    gamepkg::diagnostic(
        1,
        0x544b1800,
        &[
            u64::from(mode),
            0,
            u64::from(shape[0]),
            u64::from(shape[4]),
            u64::from(shape[7]),
            u64::from(shape[8]),
        ],
    )?;
    let original = owner.startup_ids(ram)?;
    ram.fill(0x740000, 0, 32768)?;
    let mut pending = queue::Queue::queue_new(ram, false)?;
    let mut table = timers::Timers::timers_new(ram, &pending)?;
    let mut display_pacing = schedule::Schedule::schedule_new(ram, -1)?;
    let mut control = irql::Controller::controller_new(ram)?;
    let mut pool = stacks::Pool::stacks_new(ram, shape[0], shape[2])?;
    let mut map = shadow::Shadow::shadow_new(ram)?;
    events::timer_initialize(&map, ram, 0x745000, 0)?;
    let event = events::event_lookup(&map, ram, 0x745000)?.ok_or(-3)?;
    let reply = gamepkg::local_mailbox_create(1, 1)?;
    let ack = gamepkg::local_mailbox_create(1, 1)?;
    ram.write_u32(
        0x741000,
        if mode == 38 {
            2
        } else if mode == 39 {
            3
        } else if mode == 41 {
            4
        } else {
            0
        },
    )?;
    ram.write_u64(0x741008, reply)?;
    ram.write_u64(0x741010, ack)?;
    ram.write_u64(0x741018, table.timers_id())?;
    ram.write_u64(0x741020, display_pacing.schedule_id())?;
    for (dpc, routine) in [(0x740100u32, 0x1100u32), (0x740120, 0x2200)] {
        ram.write_u32(dpc + 12, routine)?;
        ram.write_u32(dpc + 16, 0xa54e12b8)?;
    }
    let mut bytes = std::vec::Vec::new();
    for id in ram.thread_context() {
        bytes.extend(u64::to_le_bytes(id));
    }
    for value in [
        pending.queue_id(),
        control.controller_id(),
        pool.stacks_id(),
        table.timers_id(),
        map.shadow_id(),
        0,
        reply,
        ack,
        u64::from(shape[7]),
        u64::from(shape[8]),
    ] {
        bytes.extend(u64::to_le_bytes(value));
    }
    let argument = gamepkg::local_record_create(&bytes)?;
    let mut clock = tick_service::Tick::tick_new();
    tick_startup::startup_tick(owner, &mut clock, ram, data, 2)?;
    let mut launch = timer_startup::Launch::launch_new();
    let mut holder = 0u64;
    let mut hold_argument = 0u64;
    if mode == 37 {
        let mut borrowed = queue::Queue::queue_borrow(ram, pending.queue_id())?;
        let mut empty_launch = timer_startup::Launch::launch_new();
        launch_need(
            empty_launch.launch_prepare(
                owner,
                ram,
                &mut borrowed,
                &mut table,
                &mut display_pacing,
                3,
                argument,
            ) == Err(-4)
                && owner.startup_jobs() == 1,
        )?;
        launch_need(
            empty_launch.launch_prepare(
                owner,
                ram,
                &mut pending,
                &mut table,
                &mut display_pacing,
                u32::MAX,
                argument,
            ) == Err(-4)
                && empty_launch.launch_task(ram)? == 0
                && owner.startup_jobs() == 1,
        )?;
        empty_launch.launch_close(ram)?;
        pending.queue_start_ready(ram)?;
        table.timers_start_ready(ram, &pending)?;
        display_pacing.schedule_start_ready(ram)?;
    }
    if mode == 37 {
        let (_, state) = gamepkg::local_record_snapshot(table.timers_id(), 832)?;
        let mutex = launch_u64(&state, 24)?;
        let mut payload = std::vec::Vec::new();
        for id in [mutex, reply, ack] {
            payload.extend(u64::to_le_bytes(id));
        }
        hold_argument = gamepkg::local_record_create(&payload)?;
        holder = owner.startup_spawn(ram, 4, hold_argument)?;
        let mut out = [0u8; 4];
        gamepkg::local_call(13, &u64::to_le_bytes(holder), &mut out)?;
        let mut byte = [0u8; 1];
        launch_need(gamepkg::local_mailbox_receive(reply, 1000, &mut byte)? == 1)?;
    }
    let prepared = launch.launch_prepare(
        owner,
        ram,
        &mut pending,
        &mut table,
        &mut display_pacing,
        3,
        argument,
    );
    let task = launch.launch_task(ram)?;
    let code = match prepared {
        Ok(()) => 0u64,
        Err(value) => value as i64 as u64,
    };
    gamepkg::diagnostic(
        1,
        0x544b1800,
        &[u64::from(mode), 20, code, task, owner.startup_jobs() as u64],
    )?;
    launch_need(
        task != 0
            && !gamepkg::local_task_finished(task)?
            && owner.startup_close(ram) == Err(-2)
            && launch.launch_close(ram) == Err(-2),
    )?;
    if mode == 37 {
        launch_need(
            prepared == Err(-2)
                && launch.launch_progress()
                    == [true, true, false, false, false, false, false, false, false],
        )?;
        gamepkg::local_mailbox_send(ack, &[1u8])?;
        launch_wait(holder)?;
        let joined = owner.startup_join_task(ram, holder)?;
        launch_need(joined.result() == 0)?;
        gamepkg::local_record_release(hold_argument)?;
        launch_need(
            launch.launch_resume(owner, ram, &pending, &table, &display_pacing) == Err(-1),
        )?;
    } else {
        prepared?;
        launch_need(
            launch.launch_progress() == [true, true, true, true, false, false, false, false, false],
        )?;
        if mode == 36 {
            let (version, mut state) =
                gamepkg::local_record_snapshot(display_pacing.schedule_id(), 144)?;
            let saved = state.clone();
            state[31] ^= 1;
            gamepkg::local_record_compare_exchange(display_pacing.schedule_id(), version, &state)?;
            launch_need(
                launch.launch_resume(owner, ram, &pending, &table, &display_pacing) == Err(-4)
                    && !gamepkg::local_task_finished(task)?,
            )?;
            let (version, _) = gamepkg::local_record_snapshot(display_pacing.schedule_id(), 144)?;
            gamepkg::local_record_compare_exchange(display_pacing.schedule_id(), version, &saved)?;
        }
        launch_need(
            launch.launch_resume(idle_startup, ram, &pending, &table, &display_pacing) == Err(-4),
        )?;
        table.timer_set(ram, 0x745000, 0, 0, 0x740120)?;
        launch_need(pending.queue_insert(ram, 0x740100, 0x11223344, 0x55667788)?[0] == 1)?;
        launch.launch_resume(owner, ram, &pending, &table, &display_pacing)?;
        launch_need(
            launch.launch_resume(owner, ram, &pending, &table, &display_pacing) == Err(-1),
        )?;
        if mode == 39 || mode == 41 {
            let mut byte = [0u8; 1];
            launch_need(
                gamepkg::local_mailbox_receive(reply, 1000, &mut byte)? == 1
                    && pool.stacks_usage(ram)? == [1, 16],
            )?;
            if mode == 39 {
                gamepkg::local_call(12, &u64::to_le_bytes(task), &mut [])?;
            } else {
                ram.services_link(original[0], 0)?;
                gamepkg::local_mailbox_send(ack, &[1u8])?;
            }
        }
        launch_wait(task)?;
        launch_need(
            launch.launch_close(ram) == Err(-2)
                && pending.queue_close(ram) == Err(-2)
                && display_pacing.schedule_close(ram) == Err(-2)
                && table.timers_close(ram, None) == Err(-2),
        )?;
    }
    let unrelated = timer_unrelated_join(0)?;
    launch_need(unrelated.result() == 4 && launch.launch_accept_join(ram, &unrelated) == Err(-4))?;
    let joined = if mode == 33 {
        pending.queue_join_task(ram)?
    } else if mode == 34 {
        display_pacing.schedule_join_task(ram)?
    } else if mode == 35 {
        owner.startup_join_task(ram, task)?
    } else {
        launch_join(&mut launch, ram)?
    };
    if mode == 33 || mode == 34 || mode == 35 {
        launch.launch_accept_join(ram, &joined)?;
    }
    gamepkg::diagnostic(
        1,
        0x544b1800,
        &[
            u64::from(mode),
            40,
            joined.result(),
            u64::from(ram.read_u32(0x742000)?),
        ],
    )?;
    launch_need(
        joined.result()
            == if mode == 38 {
                -5i64 as u64
            } else if mode == 39 || mode == 41 {
                -4i64 as u64
            } else if mode == 40 {
                -3i64 as u64
            } else {
                0
            }
            && launch.launch_close(ram) == Err(-2)
            && launch.launch_accept_join(ram, &joined) == Err(-4),
    )?;
    let progress = launch.launch_progress();
    launch_need(
        launch.launch_share_join(
            idle_startup,
            ram,
            &mut pending,
            &table,
            &mut display_pacing,
            &joined,
        ) == Err(-4)
            && launch.launch_progress() == progress,
    )?;
    launch.launch_share_join(
        owner,
        ram,
        &mut pending,
        &table,
        &mut display_pacing,
        &joined,
    )?;
    launch.launch_share_join(
        owner,
        ram,
        &mut pending,
        &table,
        &mut display_pacing,
        &joined,
    )?;
    launch_need(owner.startup_jobs() == 1 && owner.startup_close(ram) == Err(-2))?;
    launch.launch_close(ram)?;
    launch.launch_close(ram)?;
    let witness = pending.queue_join_witness(ram)?;
    let count = ram.read_u32(0x742000)?;
    launch_need(
        count
            == if mode == 37 || mode == 40 {
                0
            } else if mode == 38 || mode == 39 || mode == 41 {
                1
            } else {
                2
            },
    )?;
    if count == 2 {
        launch_need(
            ram.read_u32(0x742004)? == 1
                && ram.read_u32(0x742008)? == 2
                && gamepkg::local_event_state(event)?.signaled,
        )?;
    }
    let revoked = mode == 41;
    if revoked {
        launch_need(pool.stacks_usage(ram) == Err(-4))?;
        let (_, state) = gamepkg::local_record_snapshot(pool.stacks_id(), 576)?;
        for slot in 0..16usize {
            launch_need(launch_u64(&state, 64 + slot * 32)? == 0)?;
        }
    } else {
        launch_need(pool.stacks_usage(ram)? == [0, 16])?;
    }
    launch_cleanup(table.timers_close(ram, Some(&witness)), revoked)?;
    launch_cleanup(display_pacing.schedule_close(ram), revoked)?;
    launch_cleanup(pending.queue_close(ram), revoked)?;
    launch_cleanup(control.controller_close(ram), revoked)?;
    launch_cleanup(pool.stacks_close(ram), revoked)?;
    launch_cleanup(events::event_close_all(&map, ram), revoked)?;
    launch_cleanup(map.shadow_close(ram), revoked)?;
    launch_need(
        table.timers_closed()
            && display_pacing.schedule_closed()
            && pending.queue_closed()
            && control.controller_closed()
            && pool.stacks_closed()
            && map.shadow_closed(),
    )?;
    if revoked {
        for _ in 0..4096u32 {
            if clock.tick_finished(ram)? {
                break;
            }
            launch_pause()?;
        }
    }
    let tick_proof = tick_join(&mut clock, ram)?;
    launch_need(tick_proof.result() == if revoked { 4 } else { 0 })?;
    owner.startup_accept_join(ram, &tick_proof)?;
    launch_cleanup(clock.tick_close(ram), revoked)?;
    launch_need(clock.tick_closed())?;
    launch_need(owner.startup_jobs() == 0 && launch.launch_closed())?;
    gamepkg::local_record_release(argument)?;
    gamepkg::local_mailbox_release(reply)?;
    gamepkg::local_mailbox_release(ack)?;
    launch_cleanup(owner.startup_close(ram), revoked)?;
    launch_need(owner.startup_closed())?;
    for id in original {
        match gamepkg::local_record_snapshot(id, 4096) {
            Err(-4) => {}
            _ => return Err(-3),
        }
    }
    gamepkg::diagnostic(1, 0x544b0f00, &[u64::from(mode), 0, 5, 0])?;
    Ok(())
}
