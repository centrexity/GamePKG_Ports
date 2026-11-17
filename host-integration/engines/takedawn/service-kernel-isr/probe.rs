// Checked ISR conformance on an actual worker, never a production launcher.
use crate::cases;
use crate::connections;
use crate::gamepkg;
use crate::heap_service;
use crate::irql;
use crate::isr_call;
use crate::layout;
use crate::memory;
use crate::registers;
use crate::stacks;
use crate::worker_setup;
fn isr_need(ok: bool) -> Result<(), i32> {
    if ok {
        Ok(())
    } else {
        Err(-3)
    }
}
fn isr_word(b: &[u8], at: usize) -> Result<u64, i32> {
    let s = b.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
    ]))
}
fn isr_one(
    index: u32,
    registry: &connections::Connections,
    ram: &mut memory::Memory,
    task: &mut irql::Task,
    cpu: &mut registers::Registers,
    lease: &stacks::Lease,
) -> Result<std::vec::Vec<u8>, i32> {
    let input = cases::isr_case(index);
    let mode = input[0];
    let vector = input[1];
    let context = input[4];
    ram.fill(0x740100, 0xa5, 44)?;
    ram.fill(0x740200, 0xa5, 44)?;
    let routine = match mode {
        1 => 0,
        2 => 0x3300,
        5 => 0x2200,
        _ => 0x1100,
    };
    isr_need(connections::interrupt_initialize(
        ram, 0x740100, routine, context, vector,
    )?)?;
    let linked = registry.connection_connect(ram, if mode == 0 { 0 } else { 0x740100 })?;
    let log_values = if mode == 0 {
        [1, u32::MAX, 0, 0]
    } else if mode == 6 {
        [1, vector, 0, 0]
    } else {
        [1, vector, routine, context]
    };
    isr_need(linked == log_values)?;
    if mode == 7 {
        connections::interrupt_initialize(ram, 0x740200, 0x2200, context ^ 0x80000001, vector)?;
        isr_need(
            registry.connection_connect(ram, 0x740200)?
                == [1, vector, 0x2200, context ^ 0x80000001],
        )?;
    }
    cpu.eax = input[5];
    cpu.esp = 0x7bfff0;
    ram.fill(cpu.esp - 12, 0xa5, 12)?;
    task.irql_raise(ram, cpu, input[3] as u8)?;
    ram.write_u32(0x741000, input[2])?;
    ram.fill(0x741004, 0, 16)?;
    // The original vblank/IRQ caller already holds the single-CPU gate.
    task.irql_gate_enter(ram, 1000)?;
    let outcome = isr_call::isr_call(registry, ram, task, cpu, lease, vector)?;
    let callable = mode != 0 && mode != 1 && mode != 2 && mode != 6;
    let eax = if callable {
        if mode == 7 {
            context ^ 0x80000001
        } else {
            context
        }
    } else {
        input[5]
    };
    let mut expected = registers::Registers::new();
    expected.esp = 0x7bfff0;
    expected.fs_base = 0x1000000;
    expected.eax = eax;
    isr_need(cpu.encode() == expected.encode())?;
    isr_need(outcome == if callable { (eax & 1) as i32 } else { -1 })?;
    let mut out = std::vec::Vec::new();
    for word in [
        linked[0],
        outcome as u32,
        cpu.esp,
        cpu.eax,
        u32::from(task.irql_current(ram)?),
        u32::from(ram.read_u8(cpu.fs_base + 0x24)?),
        ram.read_u32(0x741004)?,
        ram.read_u32(0x741008)?,
        ram.read_u32(0x74100c)?,
        ram.read_u32(0x741010)?,
    ] {
        out.extend(u32::to_le_bytes(word));
    }
    let mut arguments = [0u8; 12];
    ram.read_bytes(0x7bfff0 - 12, &mut arguments)?;
    out.extend(arguments);
    for entry in 0..32u32 {
        out.extend(u32::to_le_bytes(registry.connection_get(ram, entry)?));
    }
    let mut object = [0u8; 44];
    ram.read_bytes(0x740100, &mut object)?;
    out.extend(object);
    ram.read_bytes(0x740200, &mut object)?;
    out.extend(object);
    out.extend(u32::to_le_bytes(connections::interrupt_vector(
        ram, input[6], 0x740380,
    )?));
    out.extend(u32::to_le_bytes(u32::from(ram.read_u8(0x740380)?)));
    isr_need(out.len() == 276)?;
    task.irql_gate_leave(ram, cpu)?;
    task.irql_lower(ram, cpu, 0)?;
    Ok(out)
}
fn isr_actor(record: u64) -> Result<(), i32> {
    let (_, b) = gamepkg::local_record_snapshot(record, 64)?;
    isr_need(b.len() == 64)?;
    let mut context = [0u64; 4];
    for at in 0..4usize {
        context[at] = isr_word(&b, at * 8)?;
    }
    let mut running =
        worker_setup::Worker::worker_start(context, isr_word(&b, 40)?, 0x770000, 148)?;
    let (ram, cpu, lease) = running.worker_parts()?;
    let control = irql::Controller::controller_borrow(ram, isr_word(&b, 32)?)?;
    let mut task = control.controller_task(ram)?;
    let registry = connections::Connections::connections_borrow(ram, isr_word(&b, 48)?)?;
    let mut vectors = std::vec::Vec::new();
    for index in 0..128u32 {
        vectors.extend(isr_one(index, &registry, ram, &mut task, cpu, lease)?);
    }
    task.irql_abort(ram)?;
    running.worker_finish()?;
    gamepkg::local_mailbox_send(isr_word(&b, 56)?, &vectors)
}
pub fn isr_worker(record: u64) -> u64 {
    match isr_actor(record) {
        Ok(()) => 0,
        Err(code) => code as i64 as u64,
    }
}
pub fn isr_spawn(entry: u32, arg: u64) -> Result<u64, i32> {
    let mut b = std::vec::Vec::new();
    b.extend(u32::to_le_bytes(entry));
    b.extend(u64::to_le_bytes(arg));
    b.extend(u32::to_le_bytes(0));
    let mut out = [0u8; 8];
    isr_need(gamepkg::local_call(10, &b, &mut out)? == 8)?;
    Ok(u64::from_le_bytes(out))
}
pub fn isr_join(job: u64) -> Result<(), i32> {
    let pause = gamepkg::local_mailbox_create(1, 1)?;
    let mut byte = [0u8; 1];
    loop {
        match gamepkg::local_task_join(job) {
            Ok(proof) => {
                gamepkg::local_mailbox_release(pause)?;
                return isr_need(proof.result() == 0);
            }
            Err(-2) => isr_need(gamepkg::local_mailbox_receive(pause, 1, &mut byte) == Err(-2))?,
            Err(code) => return Err(code),
        }
    }
}
fn isr_registry_controls(
    ram: &mut memory::Memory,
    registry: &connections::Connections,
    main: u64,
) -> Result<(), i32> {
    let mut borrowed =
        connections::Connections::connections_borrow(ram, registry.connections_id())?;
    isr_need(borrowed.connections_close(ram) == Err(-4))?;
    let foreign = memory::Memory::new()?;
    isr_need(
        connections::Connections::connections_borrow(&foreign, registry.connections_id()).err()
            == Some(-4),
    )?;
    gamepkg::local_record_release(foreign.call_handle())?;
    let (_, original) = gamepkg::local_record_snapshot(registry.connections_id(), 192)?;
    for (offset, code) in [(0usize, -1), (24, -1), (8, -4), (16, -4)] {
        let (version, mut bytes) = gamepkg::local_record_snapshot(registry.connections_id(), 192)?;
        bytes[offset] ^= 1;
        gamepkg::local_record_compare_exchange(registry.connections_id(), version, &bytes)?;
        isr_need(
            connections::Connections::connections_borrow(ram, registry.connections_id()).err()
                == Some(code),
        )?;
        let (version, _) = gamepkg::local_record_snapshot(registry.connections_id(), 192)?;
        gamepkg::local_record_compare_exchange(registry.connections_id(), version, &original)?;
    }
    isr_need(!connections::interrupt_initialize(ram, 0, 0, 0, 0)?)?;
    isr_need(connections::interrupt_initialize(ram, u32::MAX - 4, 0, 0, 0) == Err(-1))?;
    isr_need(registry.connection_connect(ram, u32::MAX - 4) == Err(-1))?;
    isr_need(connections::interrupt_vector(ram, u32::MAX, 0) == Ok(u32::MAX))?;
    connections::interrupt_initialize(ram, 0x740100, 0x1100, 0x89abcdef, 3)?;
    let mut partial = ram.call_view();
    partial.attach(0x740108, 4, main, 0x740108, true)?;
    isr_need(registry.connection_connect(&partial, 0x740100) == Err(-4))?;
    isr_need(registry.connection_get(ram, 3)? == 0x740100)?;
    let (version, _) = gamepkg::local_record_snapshot(registry.connections_id(), 192)?;
    gamepkg::local_record_compare_exchange(registry.connections_id(), version, &original)?;
    let mut protected = ram.call_view();
    protected.attach(0x740100, 44, main, 0x740100, false)?;
    let mut before = [0u8; 44];
    ram.read_bytes(0x740100, &mut before)?;
    isr_need(connections::interrupt_initialize(&mut protected, 0x740100, 0, 0, 0) == Err(-4))?;
    let mut after = [0u8; 44];
    ram.read_bytes(0x740100, &mut after)?;
    isr_need(after == before)?;
    Ok(())
}
pub fn isr_probe() -> Result<std::vec::Vec<u8>, i32> {
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
    let services = crate::kernel_pool::kernel_services_initialize(&ram, 0x10000, 2)?;
    let mut control = irql::Controller::controller_new(&ram)?;
    let mut pool = stacks::Pool::stacks_new(&ram, 0x780000, 8 * 1048576)?;
    let mut registry = connections::Connections::connections_new(&ram)?;
    isr_registry_controls(&mut ram, &registry, main)?;
    let inbox = heap_service::heap_inbox()?;
    let heap_job = isr_spawn(0, inbox)?;
    let mut client = heap_service::HeapClient::heap_client(inbox, heap_job)?;
    client.heap_initialize(ram.call_handle(), main, 0x1000000, 0x2000000)?;
    let mut primary = Some(client);
    ram.heap_install(&mut primary)?;
    let reply = gamepkg::local_mailbox_create(35328, 1)?;
    let mut setup = std::vec::Vec::new();
    for id in ram.thread_context() {
        setup.extend(u64::to_le_bytes(id));
    }
    for id in [
        control.controller_id(),
        pool.stacks_id(),
        registry.connections_id(),
        reply,
    ] {
        setup.extend(u64::to_le_bytes(id));
    }
    let record = gamepkg::local_record_create(&setup)?;
    let job = isr_spawn(1, record)?;
    let mut vectors = std::vec::Vec::new();
    vectors.resize(35328, 0u8);
    let mut received = false;
    for _ in 0..30u32 {
        match gamepkg::local_mailbox_receive(reply, 1000, &mut vectors) {
            Ok(35328) => {
                received = true;
                break;
            }
            Err(-2) => {
                if gamepkg::local_task_finished(job)? {
                    return Err(-3);
                }
            }
            Err(code) => return Err(code),
            _ => return Err(-3),
        }
    }
    isr_need(received)?;
    isr_join(job)?;
    isr_need(pool.stacks_usage(&ram)? == [0, 16])?;
    registry.connections_close(&ram)?;
    registry.connections_close(&ram)?;
    control.controller_close(&ram)?;
    pool.stacks_close(&ram)?;
    ram.heap_close()?;
    isr_join(heap_job)?;
    gamepkg::local_record_release(record)?;
    gamepkg::local_mailbox_release(reply)?;
    services.service_close(&ram)?;
    gamepkg::local_record_release(ram.call_handle())?;
    Ok(vectors)
}
