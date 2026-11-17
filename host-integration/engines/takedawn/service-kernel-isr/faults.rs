// Checked fatal-prefix/lifetime controls; never production ISR bodies.
use crate::connections;
use crate::gamepkg;
use crate::irql;
use crate::isr_call;
use crate::kernel_pool;
use crate::layout;
use crate::memory;
use crate::probe;
use crate::stacks;
use crate::worker_setup;
fn fault_need(ok: bool) -> Result<(), i32> {
    if ok {
        Ok(())
    } else {
        Err(-3)
    }
}
fn fault_word(b: &[u8], at: usize) -> Result<u64, i32> {
    let s = b.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
    ]))
}
fn fault_actor(record: u64) -> Result<(), i32> {
    let (_, b) = gamepkg::local_record_snapshot(record, 80)?;
    fault_need(b.len() == 80)?;
    let mut context = [0u64; 4];
    for at in 0..4usize {
        context[at] = fault_word(&b, at * 8)?;
    }
    let mode = fault_word(&b, 64)? as u32;
    let mut running =
        worker_setup::Worker::worker_start(context, fault_word(&b, 40)?, 0x770000, 148)?;
    let (ram, cpu, lease) = running.worker_parts()?;
    let control = irql::Controller::controller_borrow(ram, fault_word(&b, 32)?)?;
    let mut task = control.controller_task(ram)?;
    let registry = connections::Connections::connections_borrow(ram, fault_word(&b, 48)?)?;
    if mode == 7 {
        gamepkg::local_mailbox_send(fault_word(&b, 56)?, &[1u8])?;
        let mut byte = [0u8; 1];
        fault_need(gamepkg::local_mailbox_receive(fault_word(&b, 72)?, 1000, &mut byte)? == 1)?;
    }
    cpu.eax = 0x55667788;
    if mode == 5 {
        cpu.esp = 0x780008;
    }
    if mode == 10 {
        cpu.fs_base = 0;
    }
    task.irql_gate_enter(ram, 1000)?;
    let result = isr_call::isr_call(&registry, ram, &mut task, cpu, lease, 3);
    let expected = match mode {
        1 => -5,
        2 => -3,
        3 => -1,
        4 => -2,
        5 | 10 => -1,
        _ => -4,
    };
    fault_need(result == Err(expected))?;
    let healthy = mode == 5 || mode == 10;
    let task_result = task.irql_current(ram);
    fault_need(task_result == if healthy { Ok(0) } else { Err(-4) })?;
    let mut words = [0u32; 16];
    words[0] = expected as u32;
    words[1] = cpu.esp;
    words[2] = cpu.eax;
    words[3] = u32::from(ram.read_u8(0x1000024)?);
    for at in 0..4u32 {
        words[4 + at as usize] = ram.read_u32(0x741004 + at * 4)?;
    }
    for at in 0..3u32 {
        words[8 + at as usize] = ram.read_u32(0x7bfff0 - 12 + at * 4)?;
    }
    words[11] = if healthy { 0 } else { (-4i32) as u32 };
    fault_need(
        words[1]
            == match mode {
                2 => 0x7bffec,
                4 | 10 => 0x7bfff0,
                5 => 0x780008,
                6 => 0x7bffec,
                _ => 0x7bffe4,
            },
    )?;
    fault_need(
        words[2]
            == if mode == 1 || mode == 2 || mode == 4 || mode == 8 || mode == 9 {
                0x89abcdef
            } else {
                0x55667788
            },
    )?;
    fault_need(words[3] == if mode == 8 || mode == 9 { 16 } else { 0 })?;
    let trace = match mode {
        1 => [3, 1, 2, 4],
        2 => [1, 2, 0, 0],
        3 => [1, 1, 0, 0],
        4 => [3, 1, 2, 3],
        8 | 9 => [2, 1, 2, 0],
        _ => [0, 0, 0, 0],
    };
    fault_need(words[4..8] == trace)?;
    fault_need(
        words[8..11]
            == if mode == 5 || mode == 6 || mode == 10 {
                [0xa5a5a5a5; 3]
            } else {
                [0, 0x740100, 0x89abcdef]
            },
    )?;
    if healthy {
        cpu.esp = 0x7bfff0;
        cpu.fs_base = 0x1000000;
        fault_need(isr_call::isr_call(&registry, ram, &mut task, cpu, lease, 3) == Ok(1))?;
        task.irql_abort(ram)?;
    } else {
        fault_need(isr_call::isr_call(&registry, ram, &mut task, cpu, lease, 3) == Err(-4))?;
    }
    let finished = running.worker_finish();
    let finish_code = if mode == 8 || mode == 9 { -4 } else { 0 };
    fault_need(
        finished
            == if finish_code == 0 {
                Ok(())
            } else {
                Err(finish_code)
            },
    )?;
    fault_need(running.worker_finished())?;
    fault_need(running.worker_finish() == finished)?;
    words[12] = 1;
    words[15] = finish_code as u32;
    let (ram, _) = running.worker_cleanup()?;
    let mut bytes = [0u8; 64];
    for at in 0..16usize {
        let value = u32::to_le_bytes(words[at]);
        for lane in 0..4usize {
            bytes[at * 4 + lane] = value[lane];
        }
    }
    ram.write_bytes(0x742000, &bytes)
}
pub fn fault_worker(record: u64) -> u64 {
    match fault_actor(record) {
        Ok(()) => 0,
        Err(code) => code as i64 as u64,
    }
}
fn fault_protect(window: u64, address: u64, protection: u32) -> Result<(), i32> {
    let mut b = std::vec::Vec::new();
    b.extend(u64::to_le_bytes(window));
    b.extend(u64::to_le_bytes(address));
    b.extend(u64::to_le_bytes(4096));
    b.extend(u32::to_le_bytes(protection));
    gamepkg::local_call(3, &b, &mut [])?;
    Ok(())
}
fn fault_sample(mode: u32) -> Result<std::vec::Vec<u8>, i32> {
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
    let services = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2)?;
    let mut control = irql::Controller::controller_new(&ram)?;
    let mut pool = stacks::Pool::stacks_new(&ram, 0x780000, 8 * 1048576)?;
    let mut registry = connections::Connections::connections_new(&ram)?;
    let inbox = crate::heap_service::heap_inbox()?;
    let heap_job = probe::isr_spawn(0, inbox)?;
    let mut client = crate::heap_service::HeapClient::heap_client(inbox, heap_job)?;
    client.heap_initialize(ram.call_handle(), main, 0x1000000, 0x2000000)?;
    let mut primary = Some(client);
    ram.heap_install(&mut primary)?;
    connections::interrupt_initialize(&mut ram, 0x740100, 0x1100, 0x89abcdef, 3)?;
    registry.connection_connect(&ram, 0x740100)?;
    let reply = gamepkg::local_mailbox_create(1, 1)?;
    let ack = gamepkg::local_mailbox_create(1, 1)?;
    ram.write_u32(0x741000, if mode == 2 { 0 } else { 1 })?;
    ram.write_u32(0x741020, mode)?;
    ram.write_u64(0x741028, reply)?;
    ram.write_u64(0x741030, ack)?;
    ram.fill(0x7bfff0 - 12, 0xa5, 12)?;
    let mut setup = std::vec::Vec::new();
    for id in ram.thread_context() {
        setup.extend(u64::to_le_bytes(id));
    }
    for id in [
        control.controller_id(),
        pool.stacks_id(),
        registry.connections_id(),
        reply,
        u64::from(mode),
        ack,
    ] {
        setup.extend(u64::to_le_bytes(id));
    }
    let record = gamepkg::local_record_create(&setup)?;
    let (_, iql) = gamepkg::local_record_snapshot(control.controller_id(), 2176)?;
    let raw_gate = fault_word(&iql, 24)?;
    let (_, gate_bytes) = gamepkg::local_record_snapshot(raw_gate, 64)?;
    let mutex = fault_word(&gate_bytes, 24)?;
    if mode == 6 {
        fault_protect(main, 0x7bf000, 1)?;
    }
    let job = probe::isr_spawn(2, record)?;
    if mode == 7 || mode == 8 || mode == 9 {
        let mut byte = [0u8; 1];
        fault_need(gamepkg::local_mailbox_receive(reply, 1000, &mut byte)? == 1)?;
        fault_need(pool.stacks_usage(&ram)? == [1, 16] && pool.stacks_close(&ram) == Err(-2))?;
        if mode == 7 {
            fault_protect(main, 0x1000000, 1)?;
            gamepkg::local_mailbox_send(ack, &[1u8])?;
        } else {
            fault_need(gamepkg::local_mutex_lock(mutex, 0) == Err(-2))?;
            if mode == 8 {
                gamepkg::local_call(12, &u64::to_le_bytes(job), &mut [])?;
            } else {
                services.service_close(&ram)?;
                gamepkg::local_mailbox_send(ack, &[1u8])?;
            }
        }
    }
    probe::isr_join(job)?;
    if mode == 6 {
        fault_protect(main, 0x7bf000, 2)?;
    }
    if mode == 7 {
        fault_protect(main, 0x1000000, 2)?;
    }
    gamepkg::local_mutex_lock(mutex, 0)?;
    gamepkg::local_call(24, &u64::to_le_bytes(mutex), &mut [])?;
    let mut witness = std::vec::Vec::new();
    witness.resize(64, 0);
    ram.read_bytes(0x742000, &mut witness)?;
    let (_, iql) = gamepkg::local_record_snapshot(control.controller_id(), 2176)?;
    fault_need(fault_word(&iql, 32)? == 0 && fault_word(&iql, 40)? == 0)?;
    let closed = registry.connections_close(&ram);
    fault_need(
        closed == if mode == 9 { Err(-4) } else { Ok(()) } && registry.connections_closed(),
    )?;
    let closed = control.controller_close(&ram);
    fault_need(closed == if mode == 9 { Err(-4) } else { Ok(()) } && control.controller_closed())?;
    let closed = pool.stacks_close(&ram);
    fault_need(closed == if mode == 9 { Err(-4) } else { Ok(()) } && pool.stacks_closed())?;
    ram.heap_close()?;
    probe::isr_join(heap_job)?;
    if mode != 9 {
        services.service_close(&ram)?;
    }
    gamepkg::local_record_release(record)?;
    gamepkg::local_mailbox_release(reply)?;
    gamepkg::local_mailbox_release(ack)?;
    gamepkg::local_record_release(ram.call_handle())?;
    Ok(witness)
}
pub fn fault_probe() -> Result<std::vec::Vec<u8>, i32> {
    let mut out = std::vec::Vec::new();
    for mode in 1..=10u32 {
        out.extend(fault_sample(mode)?);
    }
    Ok(out)
}
