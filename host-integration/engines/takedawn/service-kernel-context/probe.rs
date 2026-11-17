// Checked complete-import + context + actual-worker ABI composition.
use crate::gamepkg;
use crate::heap_service;
use crate::imports;
use crate::kernel_data;
use crate::kernel_pool;
use crate::layout;
use crate::memory;
use crate::registers;
fn context_need(ok: bool) -> Result<(), i32> {
    if ok {
        Ok(())
    } else {
        Err(-3)
    }
}
fn context_word(b: &[u8], at: usize) -> Result<u64, i32> {
    let s = b.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
    ]))
}
fn context_spawn(entry: u32, arg: u64) -> Result<u64, i32> {
    let mut b = std::vec::Vec::new();
    b.extend(u32::to_le_bytes(entry));
    b.extend(u64::to_le_bytes(arg));
    b.extend(u32::to_le_bytes(0));
    let mut out = [0u8; 8];
    context_need(gamepkg::local_call(10, &b, &mut out)? == 8)?;
    Ok(u64::from_le_bytes(out))
}
fn context_join(job: u64) -> Result<(), i32> {
    let pause = gamepkg::local_mailbox_create(1, 1)?;
    let mut byte = [0u8; 1];
    loop {
        match gamepkg::local_task_join(job) {
            Ok(proof) => {
                gamepkg::local_mailbox_release(pause)?;
                return context_need(proof.result() == 0);
            }
            Err(-2) => {
                context_need(gamepkg::local_mailbox_receive(pause, 1, &mut byte) == Err(-2))?
            }
            Err(code) => return Err(code),
        }
    }
}
fn context_call(
    ram: &mut memory::Memory,
    cpu: &mut registers::Registers,
    slot: u32,
    args: &[u32],
    eax: u32,
) -> Result<u32, i32> {
    let sp = 0x760000u32;
    cpu.esp = sp;
    cpu.eax = 0xdeadbeef;
    ram.write_u32(sp, 0x11223344)?;
    for at in 0..args.len() {
        ram.write_u32(sp + 4 + at as u32 * 4, args[at])?;
    }
    let before = cpu.encode();
    kernel_pool::kernel_invoke(cpu, ram, slot)?;
    let mut expected = registers::Registers::new();
    expected.decode(&before)?;
    expected.eax = eax;
    expected.esp = sp + 4 + args.len() as u32 * 4;
    context_need(cpu.encode() == expected.encode() && ram.kernel_caller() == 0x11223344)?;
    Ok(cpu.esp - sp)
}
fn context_actor(record: u64) -> Result<(), i32> {
    let (_, b) = gamepkg::local_record_snapshot(record, 48)?;
    context_need(b.len() == 48)?;
    let mut ids = [0u64; 4];
    for at in 0..4usize {
        ids[at] = context_word(&b, at * 8)?;
    }
    let source = memory::Memory::with_context(ids)?;
    let mapping = layout::Layout::layout_current(&source)?;
    let mut ram = mapping.layout_view(&source)?;
    let mode = context_word(&b, 32)? as u32;
    let mut cpu = registers::Registers::new();
    cpu.ebx = 0xabcdef01;
    cpu.eflags = 0x202;
    cpu.mm3 = [0x11223344, 0xfedcba98];
    let first = context_call(
        &mut ram,
        &mut cpu,
        0,
        &[0x750000, 0x1100, 0x89abcdef, 3, 16, 1, 0],
        0,
    )?;
    let second = context_call(&mut ram, &mut cpu, 1, &[0x750000], 1)?;
    let third = context_call(&mut ram, &mut cpu, 2, &[0x12345678, 0x760040], 0x12345678)?;
    let last = context_call(
        &mut ram,
        &mut cpu,
        if mode == 1 {
            4
        } else if mode == 2 {
            377
        } else {
            1
        },
        &[0],
        1,
    )?;
    let (_, bytes) = gamepkg::local_record_snapshot(ram.kernel_handle(), 1548)?;
    let bound = kernel_pool::KernelContext::kernel_context_borrow(&ram, context_word(&bytes, 24)?)?;
    let registry = bound.kernel_context_registry(&ram)?;
    context_need(registry.connection_get(&ram, 3)? == 0x750000)?;
    let mut result = std::vec::Vec::new();
    for word in [
        mode,
        first,
        second,
        third,
        last,
        cpu.eax,
        u32::from(ram.read_u8(0x760040)?),
        ram.read_u32(0x750000)?,
        ram.read_u32(0x750004)?,
        ram.read_u32(0x750008)?,
        ram.kernel_caller(),
        1,
        0x344c504b,
        bytes.len() as u32,
        registry.connection_get(&ram, 3)?,
        1,
    ] {
        result.extend(u32::to_le_bytes(word));
    }
    gamepkg::local_mailbox_send(context_word(&b, 40)?, &result)
}
pub fn context_worker(record: u64) -> u64 {
    match context_actor(record) {
        Ok(()) => 0,
        Err(code) => code as i64 as u64,
    }
}
fn context_controls(
    ram: &mut memory::Memory,
    data: &kernel_data::Data,
    table: u32,
    registry: &kernel_pool::Connections,
    bound: &kernel_pool::KernelContext,
) -> Result<(), i32> {
    let mut borrowed =
        kernel_pool::KernelContext::kernel_context_borrow(ram, bound.kernel_context_id())?;
    context_need(borrowed.kernel_context_close(ram) == Err(-4))?;
    let foreign = memory::Memory::new()?;
    context_need(
        kernel_pool::KernelContext::kernel_context_borrow(&foreign, bound.kernel_context_id())
            .err()
            == Some(-4),
    )?;
    gamepkg::local_record_release(foreign.call_handle())?;
    let (_, original) = gamepkg::local_record_snapshot(bound.kernel_context_id(), 64)?;
    context_need(
        kernel_pool::KernelContext::kernel_context_borrow(
            ram,
            bound.kernel_context_id() | 0x100000000,
        )
        .err()
            == Some(-4),
    )?;
    let (version, mut high) = gamepkg::local_record_snapshot(bound.kernel_context_id(), 64)?;
    let encoded = u64::to_le_bytes(registry.connections_id() | 0x100000000);
    for lane in 0..8usize {
        high[24 + lane] = encoded[lane];
    }
    gamepkg::local_record_compare_exchange(bound.kernel_context_id(), version, &high)?;
    context_need(
        kernel_pool::KernelContext::kernel_context_borrow(ram, bound.kernel_context_id()).err()
            == Some(-4),
    )?;
    let (version, _) = gamepkg::local_record_snapshot(bound.kernel_context_id(), 64)?;
    gamepkg::local_record_compare_exchange(bound.kernel_context_id(), version, &original)?;

    for (offset, code) in [(0usize, -1), (8, -4), (16, -4), (32, -1)] {
        let (version, mut b) = gamepkg::local_record_snapshot(bound.kernel_context_id(), 64)?;
        b[offset] ^= 1;
        gamepkg::local_record_compare_exchange(bound.kernel_context_id(), version, &b)?;
        context_need(
            kernel_pool::KernelContext::kernel_context_borrow(ram, bound.kernel_context_id()).err()
                == Some(code),
        )?;
        let (version, _) = gamepkg::local_record_snapshot(bound.kernel_context_id(), 64)?;
        gamepkg::local_record_compare_exchange(bound.kernel_context_id(), version, &original)?;
    }
    let (version, mut b) = gamepkg::local_record_snapshot(bound.kernel_context_id(), 64)?;
    for byte in 24..32usize {
        b[byte] = 0;
    }
    gamepkg::local_record_compare_exchange(bound.kernel_context_id(), version, &b)?;
    context_need(
        kernel_pool::KernelContext::kernel_context_borrow(ram, bound.kernel_context_id()).err()
            == Some(-1),
    )?;
    let (version, _) = gamepkg::local_record_snapshot(bound.kernel_context_id(), 64)?;
    gamepkg::local_record_compare_exchange(bound.kernel_context_id(), version, &original)?;
    let before = ram.read_u32(table)?;
    ram.write_u32(table, 0x80000064)?;
    context_need(
        imports::Imports::imports_scan_with_context(ram, table, data, &[], bound).err() == Some(-5),
    )?;
    context_need(ram.read_u32(table)? == 0x80000064 && ram.kernel_handle() == 0)?;
    ram.write_u32(table, before)?;
    let mut temporary = kernel_pool::KernelContext::kernel_context_new(ram, registry)?;
    let prepared = imports::Imports::imports_scan_with_context(ram, table, data, &[], &temporary)?;
    temporary.kernel_context_close(ram)?;
    context_need(
        prepared.imports_publish(ram) == Err(-4)
            && ram.kernel_handle() == 0
            && ram.read_u32(table)? == before,
    )?;
    let mut temporary_registry = kernel_pool::Connections::connections_new(ram)?;
    let mut temporary = kernel_pool::KernelContext::kernel_context_new(ram, &temporary_registry)?;
    let prepared = imports::Imports::imports_scan_with_context(ram, table, data, &[], &temporary)?;
    temporary_registry.connections_close(ram)?;
    context_need(
        prepared.imports_publish(ram) == Err(-4)
            && ram.kernel_handle() == 0
            && ram.read_u32(table)? == before,
    )?;
    temporary.kernel_context_close(ram)?;
    Ok(())
}
fn context_sample(mode: u32) -> Result<std::vec::Vec<u8>, i32> {
    let root = memory::Memory::new()?;
    let mapping = layout::Layout::layout_current(&root)?;
    let main = mapping.layout_banks()[0];
    let mut request = std::vec::Vec::new();
    request.extend(u64::to_le_bytes(main));
    request.extend(u64::to_le_bytes(0));
    request.extend(u64::to_le_bytes(67108864));
    gamepkg::local_call(4, &request, &mut [])?;
    request.extend(u32::to_le_bytes(2));
    gamepkg::local_call(2, &request, &mut [])?;
    let mut ram = mapping.layout_view(&root)?;
    ram.fill(0, 0, 67108864)?;
    let inbox = heap_service::heap_inbox()?;
    let heap_job = context_spawn(0, inbox)?;
    let mut client = heap_service::HeapClient::heap_client(inbox, heap_job)?;
    client.heap_initialize(ram.call_handle(), main, 0x1000000, 67108864)?;
    let mut primary = Some(client);
    ram.heap_install(&mut primary)?;
    let data = kernel_data::Data::data_initialize(&mut ram, 0x700000, 123, 0, &[], None)?;
    let services = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2)?;
    let mut registry = kernel_pool::Connections::connections_new(&ram)?;
    let mut bound = kernel_pool::KernelContext::kernel_context_new(&ram, &registry)?;
    let table = if mode == 1 { 0x120003u32 } else { 0x120000 };
    let count = if mode == 2 { 378u32 } else { 3 };
    ram.fill(table - 64, 0, 64 + count * 4 + 4)?;
    for slot in 0..count {
        ram.write_u32(
            table + slot * 4,
            0x80000000
                | if slot == 0 {
                    109
                } else if slot == 2 {
                    44
                } else {
                    98
                },
        )?;
    }
    if mode == 1 {
        ram.write_u32(table - 4, 0x80000062)?;
    }
    context_need(
        imports::Imports::imports_scan(&ram, table, &data, &[]).err() == Some(-5)
            && ram.kernel_handle() == 0
            && ram.read_u32(table)? == 0x8000006d,
    )?;
    if mode != 1 {
        context_need(
            kernel_pool::kernel_prepare(&mut ram, table, count) == Err(-5)
                && ram.kernel_handle() == 0
                && ram.read_u32(table)? == 0x8000006d,
        )?;
    }
    context_controls(&mut ram, &data, table, &registry, &bound)?;
    let prepared = imports::Imports::imports_scan_with_context(&ram, table, &data, &[], &bound)?;
    prepared.imports_publish(&mut ram)?;
    let (_, saved) = gamepkg::local_record_snapshot(ram.kernel_handle(), 1548)?;
    context_need(
        saved.len()
            == if mode == 2 {
                1548
            } else if mode == 1 {
                56
            } else {
                48
            },
    )?;
    let reply = gamepkg::local_mailbox_create(64, 1)?;
    let mut setup = std::vec::Vec::new();
    for id in ram.thread_context() {
        setup.extend(u64::to_le_bytes(id));
    }
    setup.extend(u64::to_le_bytes(u64::from(mode)));
    setup.extend(u64::to_le_bytes(reply));
    let record = gamepkg::local_record_create(&setup)?;
    let job = context_spawn(1, record)?;
    let mut result = [0u8; 64];
    context_need(gamepkg::local_mailbox_receive(reply, 1000, &mut result)? == 64)?;
    context_join(job)?;
    gamepkg::local_record_release(record)?;
    gamepkg::local_mailbox_release(reply)?;
    let mut rejected = registers::Registers::new();
    rejected.eax = 0xabcdef01;
    rejected.esp = 0x760000;
    let untouched = rejected.encode();
    let (version, mut high) = gamepkg::local_record_snapshot(ram.kernel_handle(), 1548)?;
    let encoded = u64::to_le_bytes(bound.kernel_context_id() | 0x100000000);
    for lane in 0..8usize {
        high[24 + lane] = encoded[lane];
    }
    gamepkg::local_record_compare_exchange(ram.kernel_handle(), version, &high)?;
    context_need(
        kernel_pool::kernel_invoke(&mut rejected, &mut ram, 1) == Err(-4)
            && rejected.encode() == untouched,
    )?;
    let (version, _) = gamepkg::local_record_snapshot(ram.kernel_handle(), 1548)?;
    gamepkg::local_record_compare_exchange(ram.kernel_handle(), version, &saved)?;
    for counter in [8u32, 9] {
        let (version, mut changed) = gamepkg::local_record_snapshot(ram.kernel_handle(), 1548)?;
        let at = changed.len() - 4;
        let bytes = u32::to_le_bytes(counter);
        for lane in 0..4usize {
            changed[at + lane] = bytes[lane];
        }
        gamepkg::local_record_compare_exchange(ram.kernel_handle(), version, &changed)?;
        context_need(
            kernel_pool::kernel_ordinal(&ram, 1) == if counter == 8 { Ok(98) } else { Err(-1) },
        )?;
    }
    let (version, _) = gamepkg::local_record_snapshot(ram.kernel_handle(), 1548)?;
    gamepkg::local_record_compare_exchange(ram.kernel_handle(), version, &saved)?;
    ram.kernel_clear()?;
    let mut legacy = std::vec::Vec::new();
    legacy.extend_from_slice(&saved[..24]);
    legacy[..4].copy_from_slice(&u32::to_le_bytes(0x334c504b));
    legacy.extend_from_slice(&saved[32..]);
    let id = gamepkg::local_record_create(&legacy)?;
    ram.kernel_install(id)?;
    let mut cpu = registers::Registers::new();
    cpu.eax = 0xabcdef01;
    cpu.esp = 0x760000;
    let before = cpu.encode();
    let denied = kernel_pool::kernel_invoke(&mut cpu, &mut ram, 1);
    context_need(denied == if mode == 1 { Err(-1) } else { Err(-5) } && cpu.encode() == before)?;
    ram.kernel_clear()?;
    let id = gamepkg::local_record_create(&saved)?;
    ram.kernel_install(id)?;
    bound.kernel_context_close(&ram)?;
    context_need(
        kernel_pool::kernel_invoke(&mut cpu, &mut ram, 1) == Err(-4) && cpu.encode() == before,
    )?;
    ram.kernel_clear()?;
    registry.connections_close(&ram)?;
    services.service_close(&ram)?;
    ram.heap_close()?;
    context_join(heap_job)?;
    gamepkg::local_record_release(ram.call_handle())?;
    Ok(result.to_vec())
}
pub fn context_probe() -> Result<std::vec::Vec<u8>, i32> {
    let mut out = std::vec::Vec::new();
    for mode in 0..3u32 {
        out.extend(context_sample(mode)?);
    }
    Ok(out)
}
