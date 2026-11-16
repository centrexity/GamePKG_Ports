use crate::dispatch;
use crate::events;
use crate::gamepkg;
use crate::heap_service;
use crate::irql;
use crate::kernel_pool;
use crate::layout;
use crate::memory;
use crate::queue;
use crate::shadow;
use crate::stacks;
use crate::timers;
use crate::worker_setup;
fn step_need(ok: bool) -> Result<(), i32> {
    if ok {
        Ok(())
    } else {
        Err(-3)
    }
}
fn step_word(b: &[u8], at: usize) -> Result<u64, i32> {
    let s = b.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
    ]))
}
fn step_actor(record: u64) -> Result<(), i32> {
    let (_, b) = gamepkg::local_record_snapshot(record, 112)?;
    step_need(b.len() == 112)?;
    let mut context = [0u64; 4];
    for at in 0..4usize {
        context[at] = step_word(&b, at * 8)?;
    }
    let pending = step_word(&b, 32)?;
    let controller = step_word(&b, 40)?;
    let pool = step_word(&b, 48)?;
    let table = step_word(&b, 56)?;
    let map = step_word(&b, 64)?;
    let mode = step_word(&b, 72)?;
    let mut running = worker_setup::Worker::worker_start(
        context,
        pool,
        step_word(&b, 96)? as u32,
        step_word(&b, 104)? as u32,
    )?;
    if mode == 6 {
        let (ram, _, _) = running.worker_parts()?;
        let view = timers::Timers::timers_borrow(ram, table)?;
        let mut bytes = view.timers_state(ram)?;
        let wrong = u64::to_le_bytes(pending ^ u64::MAX);
        for lane in 0..8usize {
            bytes[32 + lane] = wrong[lane];
        }
        let fake = gamepkg::local_record_create(&bytes)?;
        step_need(
            dispatch::Dispatch::dispatch_new(
                &mut running,
                pending,
                controller,
                pool,
                fake,
                map,
                1200,
                0,
                3,
            )
            .err()
                == Some(-4),
        )?;
        gamepkg::local_record_release(fake)?;
    }
    let mut dispatcher = dispatch::Dispatch::dispatch_new(
        &mut running,
        pending,
        controller,
        pool,
        table,
        map,
        1200,
        0,
        3,
    )?;
    if mode == 5 {
        let mut alternate = worker_setup::Worker::worker_start(
            context,
            pool,
            step_word(&b, 96)? as u32,
            step_word(&b, 104)? as u32,
        )?;
        step_need(dispatcher.dispatch_actor(&mut alternate).err() == Some(-4))?;
        step_need(dispatcher.dispatch_pass(&mut alternate, 0, false) == Err(-4))?;
        step_need(dispatcher.dispatch_finish(&mut alternate) == Err(-4))?;
        alternate.worker_finish()?;
    }
    let (ram, task) = dispatcher.dispatch_actor(&mut running)?;
    step_need(
        ram.call_handle() == context[0]
            && task.task_key() == [controller, gamepkg::local_task_status()?.0],
    )?;
    let result = dispatcher.dispatch_pass(&mut running, 0, false);
    let failure = match mode {
        1 | 2 => -5,
        3 | 4 | 7 => -4,
        _ => 0,
    };
    if failure == 0 {
        step_need(result == Ok([0, 1, 1, u64::MAX]))?;
        let (ram, cpu, lease) = running.worker_parts()?;
        step_need(
            cpu.esp == lease.stack_guest_top(ram)?
                && cpu.fs_base == 0x1000000
                && ram.read_u8(cpu.fs_base + 0x24)? == 0,
        )?;
    } else {
        step_need(result == Err(failure))?;
        step_need(dispatcher.dispatch_pass(&mut running, 0, false) == Err(-4))?;
    }
    let finish = dispatcher.dispatch_finish(&mut running);
    let expected = if mode == 3 || mode == 4 {
        Err(-4)
    } else {
        Ok(())
    };
    step_need(finish == expected && running.worker_finished() && dispatcher.dispatch_closed())?;
    step_need(
        dispatcher.dispatch_finish(&mut running) == expected
            && running.worker_parts().err() == Some(-4),
    )?;
    // Cleanup access remains actual-owner-only after cancellation/local finish;
    // it exposes no usable CPU, mutable lease or native pointer.
    step_need(dispatcher.dispatch_actor(&mut running).err() == Some(-4))?;
    let (ram, key) = running.worker_cleanup()?;
    step_need(key[0] == pool && key[1] == 0 && key[2] == 1 && ram.call_handle() == context[0])?;
    Ok(())
}
pub fn dispatch_worker(record: u64) -> u64 {
    match step_actor(record) {
        Ok(()) => 0,
        Err(code) => code as i64 as u64,
    }
}
fn step_spawn(entry: u32, arg: u64, suspended: bool) -> Result<u64, i32> {
    let mut b = std::vec::Vec::new();
    b.extend(u32::to_le_bytes(entry));
    b.extend(u64::to_le_bytes(arg));
    b.extend(u32::to_le_bytes(u32::from(suspended)));
    let mut out = [0u8; 8];
    step_need(gamepkg::local_call(10, &b, &mut out)? == 8)?;
    Ok(u64::from_le_bytes(out))
}
fn step_join(job: u64) -> Result<gamepkg::LocalTaskJoin, i32> {
    let pause = gamepkg::local_mailbox_create(1, 1)?;
    let mut byte = [0u8; 1];
    loop {
        match gamepkg::local_task_join(job) {
            Ok(proof) => {
                gamepkg::local_mailbox_release(pause)?;
                step_need(proof.result() == 0)?;
                return Ok(proof);
            }
            Err(-2) => step_need(gamepkg::local_mailbox_receive(pause, 1, &mut byte) == Err(-2))?,
            Err(code) => return Err(code),
        }
    }
}
fn step_protect(window: u64, mode: u32) -> Result<(), i32> {
    let mut b = std::vec::Vec::new();
    b.extend(u64::to_le_bytes(window));
    b.extend(u64::to_le_bytes(0x745000));
    b.extend(u64::to_le_bytes(4096));
    b.extend(u32::to_le_bytes(mode));
    gamepkg::local_call(3, &b, &mut [])?;
    Ok(())
}
fn step_cleanup_result(result: Result<(), i32>, revoked: bool) -> Result<(), i32> {
    step_need(result == Ok(()) || revoked && result == Err(-4))
}
fn step_sample(mode: u64) -> Result<std::vec::Vec<u8>, i32> {
    let root = memory::Memory::new()?;
    let mapping = layout::Layout::layout_current(&root)?;
    let main = mapping.layout_banks()[0];
    let mut b = std::vec::Vec::new();
    b.extend(u64::to_le_bytes(main));
    b.extend(u64::to_le_bytes(0));
    b.extend(u64::to_le_bytes(67108864));
    gamepkg::local_call(4, &b, &mut [])?;
    b.extend(u32::to_le_bytes(2));
    gamepkg::local_call(2, &b, &mut [])?;
    let mut ram = mapping.layout_view(&root)?;
    ram.fill(0, 0, 67108864)?;
    ram.write_u32(0x4000, u32::MAX)?;
    ram.write_u32(0x4004, 0xf7fff0)?;
    ram.write_u32(0x4008, 0x780000)?;
    let services = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2)?;
    let mut pending = queue::Queue::queue_new(&ram, false)?;
    let mut table = timers::Timers::timers_new(&ram, &pending)?;
    let mut controller = irql::Controller::controller_new(&ram)?;
    let mut pool = stacks::Pool::stacks_new(&ram, 0x780000, 8 * 1048576)?;
    let mut map = shadow::Shadow::shadow_new(&ram)?;
    let inbox = heap_service::heap_inbox()?;
    let heap_job = step_spawn(0, inbox, false)?;
    let mut client = heap_service::HeapClient::heap_client(inbox, heap_job)?;
    client.heap_initialize(ram.call_handle(), main, 0x1000000, 0x2000000)?;
    let mut primary = Some(client);
    ram.heap_install(&mut primary)?;
    events::timer_initialize(&map, &mut ram, 0x745000, 0)?;
    ram.write_u32(0x745004, 9)?;
    let event = events::event_lookup(&map, &ram, 0x745000)?.ok_or(-3)?;
    let reply = gamepkg::local_mailbox_create(1, 1)?;
    let ack = gamepkg::local_mailbox_create(1, 1)?;
    ram.write_u32(0x741000, mode as u32)?;
    ram.write_u64(0x741008, reply)?;
    ram.write_u64(0x741010, ack)?;
    ram.write_u64(0x741018, table.timers_id())?;
    for (dpc, routine) in [(0x740100u32, 0x1100u32), (0x740120, 0x2200)] {
        ram.write_u32(dpc + 12, routine)?;
        ram.write_u32(dpc + 16, 0xa54e12b8)?;
    }
    let mut setup = std::vec::Vec::new();
    for id in ram.thread_context() {
        setup.extend(u64::to_le_bytes(id));
    }
    for value in [
        pending.queue_id(),
        controller.controller_id(),
        pool.stacks_id(),
        table.timers_id(),
        map.shadow_id(),
        mode,
        reply,
        ack,
        0x770000,
        148,
    ] {
        setup.extend(u64::to_le_bytes(value));
    }
    step_need(setup.len() == 112)?;
    let record = gamepkg::local_record_create(&setup)?;
    let job = step_spawn(1, record, true)?;
    pending.queue_assign(&ram, job)?;
    table.timers_start(&ram, &pending)?;
    table.timer_set(&mut ram, 0x745000, 0, 0, 0x740120)?;
    step_need(pending.queue_insert(&mut ram, 0x740100, 0x11223344, 0x55667788)?[0] == 1)?;
    let (_, iql) = gamepkg::local_record_snapshot(controller.controller_id(), 2176)?;
    let raw_gate = step_word(&iql, 24)?;
    let (_, gate_bytes) = gamepkg::local_record_snapshot(raw_gate, 64)?;
    let mutex = step_word(&gate_bytes, 24)?;
    if mode == 7 {
        step_protect(main, 1)?;
    }
    let mut resumed = [0u8; 4];
    gamepkg::local_call(13, &u64::to_le_bytes(job), &mut resumed)?;
    if mode == 3 || mode == 4 {
        let mut byte = [0u8; 1];
        step_need(gamepkg::local_mailbox_receive(reply, 1000, &mut byte)? == 1)?;
        step_need(
            pool.stacks_usage(&ram)? == [1, 16]
                && pool.stacks_close(&ram) == Err(-2)
                && gamepkg::local_mutex_lock(mutex, 0) == Err(-2),
        )?;
        if mode == 3 {
            gamepkg::local_call(12, &u64::to_le_bytes(job), &mut [])?;
        } else {
            services.service_close(&ram)?;
            gamepkg::local_mailbox_send(ack, &[1u8])?;
        }
    }
    let proof = step_join(job)?;
    pending.queue_accept_join(&ram, &proof)?;
    let joined = pending.queue_join_witness(&ram)?;
    gamepkg::local_mutex_lock(mutex, 0)?;
    gamepkg::local_call(24, &u64::to_le_bytes(mutex), &mut [])?;
    if mode == 7 {
        step_protect(main, 2)?;
    }
    let count = ram.read_u32(0x742000)?;
    let first = ram.read_u32(0x742004)?;
    let second = ram.read_u32(0x742008)?;
    step_need(
        count
            == if mode == 2 || mode == 3 || mode == 4 {
                1
            } else {
                2
            }
            && first == 1
            && second == if count == 2 { 2 } else { 0 },
    )?;
    step_need(ram.read_u32(0x742010)? == 0x1000000 && ram.read_u32(0x742014)? == 0x7bffdc)?;
    let state = gamepkg::local_event_state(event)?;
    step_need(state.signaled == (mode == 0 || mode == 5 || mode == 6 || mode == 7))?;
    let (_, slots) = gamepkg::local_record_snapshot(table.timers_id(), 832)?;
    let present = slots[64..68] == u32::to_le_bytes(0x745000);
    step_need(present == (mode == 2 || mode == 3 || mode == 4))?;
    let signal = ram.read_u32(0x745004)?;
    step_need(
        signal
            == if mode == 0 || mode == 5 || mode == 6 {
                1
            } else {
                9
            },
    )?;
    let (_, iql) = gamepkg::local_record_snapshot(controller.controller_id(), 2176)?;
    step_need(step_word(&iql, 32)? == 0 && step_word(&iql, 40)? == 0)?;
    let mut out = std::vec::Vec::new();
    for value in [
        mode,
        u64::from(count),
        u64::from(first),
        u64::from(second),
        u64::from(ram.read_u32(0x742010)?),
        u64::from(ram.read_u32(0x742014)?),
        u64::from(state.signaled),
        u64::from(present),
        u64::from(signal),
    ] {
        out.extend(u64::to_le_bytes(value));
    }
    step_cleanup_result(table.timers_close(&ram, Some(&joined)), mode == 4)?;
    step_need(table.timers_closed())?;
    step_cleanup_result(pending.queue_close(&ram), mode == 4)?;
    step_need(pending.queue_closed())?;
    step_cleanup_result(controller.controller_close(&ram), mode == 4)?;
    step_need(controller.controller_closed())?;
    step_cleanup_result(pool.stacks_close(&ram), mode == 4)?;
    step_need(pool.stacks_closed())?;
    step_cleanup_result(events::event_close_all(&map, &ram), mode == 4)?;
    step_cleanup_result(map.shadow_close(&ram), mode == 4)?;
    step_need(map.shadow_closed())?;
    ram.heap_close()?;
    step_join(heap_job)?;
    if mode != 4 {
        services.service_close(&ram)?;
    }
    gamepkg::local_record_release(record)?;
    gamepkg::local_mailbox_release(reply)?;
    gamepkg::local_mailbox_release(ack)?;
    gamepkg::local_record_release(ram.call_handle())?;
    Ok(out)
}
pub fn dispatch_probe() -> Result<std::vec::Vec<u8>, i32> {
    let mut out = std::vec::Vec::new();
    for mode in 0..8u64 {
        out.extend(step_sample(mode)?);
    }
    Ok(out)
}
