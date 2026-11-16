// Checked startup composition with actual SDK worker and separate heap actor.
use crate::gamepkg;
use crate::heap_service;
use crate::kernel_pool;
use crate::layout;
use crate::memory;
use crate::registers;
use crate::stacks;
use crate::worker_setup;
fn startup_need(value: bool) -> Result<(), i32> {
    if value {
        Ok(())
    } else {
        Err(-3)
    }
}
fn startup_word(b: &[u8], at: usize) -> Result<u64, i32> {
    let s = b.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
    ]))
}
fn startup_expected(mode: u32) -> i32 {
    match mode {
        0 | 8 | 9 => 0,
        1 | 2 => -3,
        3 => -5,
        4 => -2,
        5 | 7 => -4,
        6 => -1,
        _ => -1,
    }
}
fn startup_send(reply: u64, values: [u64; 8]) -> Result<(), i32> {
    let mut b = [0u8; 64];
    for at in 0..8usize {
        let word = u64::to_le_bytes(values[at]);
        for lane in 0..8usize {
            b[at * 8 + lane] = word[lane];
        }
    }
    gamepkg::local_mailbox_send(reply, &b)
}
fn startup_actor(record: u64) -> Result<(), i32> {
    let (_, b) = gamepkg::local_record_snapshot(record, 80)?;
    startup_need(b.len() == 80)?;
    let mut context = [0u64; 4];
    for at in 0..4usize {
        context[at] = startup_word(&b, at * 8)?;
    }
    let pool_id = startup_word(&b, 32)?;
    let mode = startup_word(&b, 48)? as u32;
    let reply = startup_word(&b, 56)?;
    let ack = startup_word(&b, 72)?;
    let template = startup_word(&b, 40)? as u32;
    let total = (startup_word(&b, 40)? >> 32) as u32;
    if mode == 3 {
        context[2] = 0;
        context[3] = 0;
    }
    if mode == 6 {
        context[2] = 0;
    }
    let configured = worker_setup::Worker::worker_start(context, pool_id, template, total);
    let mut running = match configured {
        Err(code) => {
            startup_need(code == startup_expected(mode))?;
            startup_send(
                reply,
                [u64::from(mode), code as i64 as u64, 0, 0, 0, 0, 0, 0],
            )?;
            return Ok(());
        }
        Ok(state) => {
            startup_need(startup_expected(mode) == 0)?;
            state
        }
    };
    let (ram, cpu, lease) = running.worker_parts()?;
    startup_need(cpu.esp == 0x7bfff0 && cpu.fs_base == 0x1000000)?;
    let mut expected = registers::Registers::new();
    expected.esp = 0x7bfff0;
    expected.fs_base = 0x1000000;
    startup_need(cpu.encode() == expected.encode())?;
    let identity = lease.stack_identity(ram)?;
    startup_need(identity == [pool_id, 0, 1])?;
    let view = stacks::Pool::stacks_borrow(ram, pool_id)?;
    startup_need(view.stacks_usage(ram)? == [1, 16])?;
    let tib = cpu.fs_base;
    let data = tib + 64 + total;
    startup_need(
        ram.read_u32(tib)? == u32::MAX
            && ram.read_u32(tib + 4)? == data
            && ram.read_u32(tib + 64)? == data,
    )?;
    let mut thread_data = [0u8; 64];
    ram.read_bytes(data, &mut thread_data)?;
    startup_need(thread_data == [0u8; 64])?;
    startup_need(ram.read_u8(tib + 64 + total - 1)? == 0x6d)?;
    let ready = [
        u64::from(mode),
        0,
        u64::from(cpu.esp),
        u64::from(tib),
        identity[1],
        identity[2],
        u64::from(ram.read_u32(tib)?),
        u64::from(ram.read_u8(tib + 64 + total - 1)?),
    ];
    // Repeated access retains the same actual owner's stack across iterations.
    let (ram, cpu, lease) = running.worker_parts()?;
    startup_need(lease.stack_guest_top(ram)? == cpu.esp)?;
    startup_send(reply, ready)?;
    let mut message = [0u8; 1];
    let wake = gamepkg::local_mailbox_receive(ack, 1000, &mut message);
    if mode == 9 {
        startup_need(wake == Err(-4))?;
    } else {
        startup_need(wake == Ok(1))?;
    }
    if mode == 8 || mode == 9 {
        startup_need(running.worker_parts().err() == Some(-4))?;
        startup_need(running.worker_finish() == Err(-4))?;
    } else {
        running.worker_finish()?;
    }
    startup_need(running.worker_finished())?;
    startup_need(
        running.worker_finish()
            == if mode == 8 || mode == 9 {
                Err(-4)
            } else {
                Ok(())
            },
    )?;
    startup_need(running.worker_parts().err() == Some(-4))?;
    Ok(())
}
pub fn startup_worker(record: u64) -> u64 {
    match startup_actor(record) {
        Ok(()) => 0,
        Err(code) => code as i64 as u64,
    }
}
fn startup_spawn(entry: u32, arg: u64) -> Result<u64, i32> {
    let mut b = std::vec::Vec::new();
    b.extend(u32::to_le_bytes(entry));
    b.extend(u64::to_le_bytes(arg));
    b.extend(u32::to_le_bytes(0));
    let mut out = [0u8; 8];
    startup_need(gamepkg::local_call(10, &b, &mut out)? == 8)?;
    Ok(u64::from_le_bytes(out))
}
fn startup_join(job: u64) -> Result<(), i32> {
    let pause = gamepkg::local_mailbox_create(1, 1)?;
    let mut byte = [0u8; 1];
    loop {
        match gamepkg::local_task_join(job) {
            Ok(proof) => {
                gamepkg::local_mailbox_release(pause)?;
                return startup_need(proof.result() == 0);
            }
            Err(-2) => {
                startup_need(gamepkg::local_mailbox_receive(pause, 1, &mut byte) == Err(-2))?
            }
            Err(code) => return Err(code),
        }
    }
}
fn startup_sample(mode: u32) -> Result<std::vec::Vec<u8>, i32> {
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
    ram.fill(0, 0xa5, 67108864)?;
    ram.write_u8(0x770000 + 147, 0x6d)?;
    let service_owner = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2)?;
    let services = ram.services_handle()?;
    let mut pool =
        stacks::Pool::stacks_new(&ram, 0x780000, if mode == 4 { 0 } else { 8 * 1048576 })?;
    let inbox = heap_service::heap_inbox()?;
    let heap_job = startup_spawn(0, inbox)?;
    let mut client = heap_service::HeapClient::heap_client(inbox, heap_job)?;
    client.heap_initialize(
        ram.call_handle(),
        main,
        0x1000000,
        if mode == 2 { 0x1000010 } else { 0x2000000 },
    )?;
    let mut primary = Some(client);
    ram.heap_install(&mut primary)?;
    startup_need(
        worker_setup::Worker::worker_start(ram.thread_context(), pool.stacks_id(), 0x770000, 148)
            .err()
            == Some(-4),
    )?;
    let foreign = memory::Memory::new()?;
    let foreign_services = kernel_pool::kernel_services_initialize(&foreign, 0x10000, 2)?;
    let mut foreign_pool = stacks::Pool::stacks_new(&foreign, 0x780000, 8 * 1048576)?;
    let reply = gamepkg::local_mailbox_create(64, 1)?;
    let ack = gamepkg::local_mailbox_create(1, 1)?;
    let mut setup = std::vec::Vec::new();
    for id in ram.thread_context() {
        setup.extend(u64::to_le_bytes(id));
    }
    for value in [
        if mode == 5 {
            foreign_pool.stacks_id()
        } else {
            pool.stacks_id()
        },
        u64::from(0x770000u32) | (u64::from(if mode == 1 { 0u32 } else { 148u32 }) << 32),
        u64::from(mode),
        reply,
        services,
        ack,
    ] {
        setup.extend(u64::to_le_bytes(value));
    }
    startup_need(setup.len() == 80)?;
    let record = gamepkg::local_record_create(&setup)?;
    if mode == 7 {
        service_owner.service_close(&ram)?;
    }
    let job = startup_spawn(1, record)?;
    let mut ready = [0u8; 64];
    startup_need(gamepkg::local_mailbox_receive(reply, 1000, &mut ready)? == 64)?;
    if mode == 0 || mode == 8 || mode == 9 {
        startup_need(pool.stacks_usage(&ram)? == [1, 16] && pool.stacks_close(&ram) == Err(-2))?;
        if mode == 8 {
            service_owner.service_close(&ram)?;
        }
        if mode == 9 {
            gamepkg::local_call(12, &u64::to_le_bytes(job), &mut [])?;
        } else {
            gamepkg::local_mailbox_send(ack, &[1u8])?;
        }
    }
    startup_join(job)?;
    let release_pool = pool.stacks_close(&ram);
    if mode == 7 || mode == 8 {
        startup_need(release_pool == Err(-4) && pool.stacks_closed())?;
    } else {
        release_pool?;
    }
    let mut owner = ram.heap_take()?;
    let stats = owner.heap_status();
    ram.heap_put(owner);
    let stats = stats?;
    startup_need(stats[2] == if startup_expected(mode) == 0 { 1 } else { 0 })?;
    let mut witness = std::vec::Vec::new();
    witness.extend(ready);
    for value in [
        stats[0],
        stats[1],
        stats[2],
        u32::from(pool.stacks_closed()),
    ] {
        witness.extend(u64::to_le_bytes(u64::from(value)));
    }
    ram.heap_close()?;
    startup_join(heap_job)?;
    if mode != 7 && mode != 8 {
        service_owner.service_close(&ram)?;
    }
    foreign_pool.stacks_close(&foreign)?;
    foreign_services.service_close(&foreign)?;
    gamepkg::local_record_release(foreign.call_handle())?;
    gamepkg::local_record_release(record)?;
    gamepkg::local_mailbox_release(reply)?;
    gamepkg::local_mailbox_release(ack)?;
    gamepkg::local_record_release(ram.call_handle())?;
    Ok(witness)
}
pub fn startup_probe() -> Result<std::vec::Vec<u8>, i32> {
    let mut out = std::vec::Vec::new();
    for mode in 0..10u32 {
        out.extend(startup_sample(mode)?);
    }
    Ok(out)
}
