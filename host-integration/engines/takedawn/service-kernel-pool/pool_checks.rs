// Ordinary admitted integration checks using the actual finite native router.
use crate::calls_runtime;
use crate::gamepkg;
use crate::heap_service;
use crate::kernel_pool;
use crate::memory;
use crate::registers;
fn pool_read_long(bytes: &[u8], at: usize) -> Result<u64, i32> {
    let b = bytes.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}
fn pool_spawn(entry: u32, arg: u64) -> Result<u64, i32> {
    let mut request = std::vec::Vec::new();
    request.extend(u32::to_le_bytes(entry));
    request.extend(u64::to_le_bytes(arg));
    request.extend(u32::to_le_bytes(0));
    let mut reply = [0u8; 8];
    if gamepkg::local_call(10, &request, &mut reply)? != 8 {
        return Err(-3);
    }
    pool_read_long(&reply, 0)
}
fn pool_join(job: u64) -> Result<u64, i32> {
    let pause = gamepkg::local_mailbox_create(1, 1)?;
    let mut reply = [0u8; 8];
    let mut byte = [0u8; 1];
    loop {
        match gamepkg::local_call(11, &u64::to_le_bytes(job), &mut reply) {
            Ok(8) => {
                gamepkg::local_mailbox_release(pause)?;
                return pool_read_long(&reply, 0);
            }
            Err(-2) => {
                if gamepkg::local_mailbox_receive(pause, 1, &mut byte) != Err(-2) {
                    return Err(-3);
                }
            }
            Ok(_) => return Err(-3),
            Err(code) => return Err(code),
        }
    }
}
fn pool_protect(window: u64, page: u64, mode: u32) -> Result<(), i32> {
    let mut request = std::vec::Vec::new();
    request.extend(u64::to_le_bytes(window));
    request.extend(u64::to_le_bytes(page));
    request.extend(u64::to_le_bytes(4096));
    request.extend(u32::to_le_bytes(mode));
    if gamepkg::local_call(3, &request, &mut [])? != 0 {
        return Err(-3);
    }
    Ok(())
}
fn pool_write_table(ram: &mut memory::Memory, base: u32) -> Result<(), i32> {
    for (slot, ordinal) in [(0u32, 14u32), (1, 15), (2, 17), (3, 23)] {
        ram.write_u32(base + slot * 4, 0x80000000 | ordinal)?;
    }
    ram.write_u32(base + 16, 0)?;
    Ok(())
}
fn pool_stats(ram: &mut memory::Memory) -> Result<[u32; 3], i32> {
    let mut client = ram.heap_take()?;
    let result = client.heap_status();
    ram.heap_put(client);
    result
}
fn pool_plans(ram: &mut memory::Memory, window: u64) -> Result<(), i32> {
    for (table, count) in [
        (0u32, 5u32),
        (0x20001, 5),
        (0x20000, 0),
        (0x20000, 379),
        (67108864, 1),
        (u32::MAX, 1),
    ] {
        if kernel_pool::kernel_prepare(ram, table, count) != Err(-1) {
            return Err(-3);
        }
    }
    for (thunk_word, code) in [
        (0x80000010u32, -5i32),
        (0x800000b4, -5),
        (0x8000017a, -5),
        (0x80000000, -5),
        (0xfe000000, -5),
        (14, -1),
        (0x7fffffff, -1),
    ] {
        pool_write_table(ram, 0x20000)?;
        ram.write_u32(0x20004, thunk_word)?;
        let mut before = [0u8; 20];
        ram.read_bytes(0x20000, &mut before)?;
        if kernel_pool::kernel_prepare(ram, 0x20000, 5) != Err(code) || ram.kernel_handle() != 0 {
            return Err(-3);
        }
        let mut after = [0u8; 20];
        ram.read_bytes(0x20000, &mut after)?;
        if before != after {
            return Err(-3);
        }
    }
    pool_write_table(ram, 0x20ffc)?;
    let mut before = [0u8; 20];
    ram.read_bytes(0x20ffc, &mut before)?;
    pool_protect(window, 0x21000, 1)?;
    if kernel_pool::kernel_prepare(ram, 0x20ffc, 5) != Err(-4) || ram.kernel_handle() != 0 {
        return Err(-3);
    }
    let mut after = [0u8; 20];
    ram.read_bytes(0x20ffc, &mut after)?;
    if before != after {
        return Err(-3);
    }
    pool_protect(window, 0x21000, 2)?;
    pool_write_table(ram, 0x20000)?;
    let mut held = [0u64; 64];
    let mut count = 0usize;
    while count < 64 {
        match gamepkg::local_record_create(&[0u8]) {
            Ok(id) => {
                held[count] = id;
                count += 1;
            }
            Err(-2) => break,
            Err(code) => return Err(code),
        }
    }
    if kernel_pool::kernel_prepare(ram, 0x20000, 5) != Err(-2) || ram.kernel_handle() != 0 {
        return Err(-3);
    }
    for id in &held[..count] {
        gamepkg::local_record_release(*id)?;
    }
    if ram.read_u32(0x20000)? != 0x8000000e {
        return Err(-3);
    }
    let mut maximum = ram.call_view();
    maximum.attach(0, 67108864, window, 0, true)?;
    for slot in 0..378u32 {
        maximum.write_u32(0x24000 + slot * 4, 0x8000000e)?;
    }
    kernel_pool::kernel_prepare(&mut maximum, 0x24000, 378)?;
    if kernel_pool::kernel_ordinal(&maximum, 377) != Ok(14)
        || kernel_pool::kernel_ordinal(&maximum, 378) != Err(-5)
    {
        return Err(-3);
    }
    maximum.kernel_clear()?;
    let mut dead = memory::Memory::new()?;
    dead.attach(0, 67108864, window, 0, true)?;
    gamepkg::local_record_release(dead.call_handle())?;
    if kernel_pool::kernel_prepare(&mut dead, 0x20000, 5) != Err(-4) {
        return Err(-3);
    }
    Ok(())
}
fn pool_frame(ram: &mut memory::Memory, sp: u32, first: u32, second: u32) -> Result<(), i32> {
    ram.write_u32(sp, 0x123456)?;
    ram.write_u32(sp + 4, first)?;
    ram.write_u32(sp + 8, second)?;
    Ok(())
}
fn pool_call(
    state: &mut registers::Registers,
    ram: &mut memory::Memory,
    slot: u32,
    offset: u32,
    jump: bool,
) -> Result<(), i32> {
    let before = ram.call_snapshot()?;
    if jump {
        calls_runtime::invoke_jump(state, ram, 0xfe000000 + slot * 4 + offset)?;
    } else {
        calls_runtime::invoke_call(state, ram, 0xfe000000 + slot * 4 + offset, 0xabcdef)?;
    }
    let after = ram.call_snapshot()?;
    if jump {
        if before != after {
            return Err(-3);
        }
    } else if after.1 != before.1.wrapping_add(1) || after.2 != before.2.wrapping_add(1) {
        return Err(-3);
    }
    Ok(())
}
fn pool_values(ram: &mut memory::Memory) -> Result<(), i32> {
    let mut state = registers::Registers::new();
    state.ebx = 0x11223344;
    state.ecx = 0xaabbccdd;
    state.edx = 0x8899aabb;
    state.fs_base = 0x4000;
    state.fp_stack[2] = f64::from_bits(0x7ff8000012345678);
    for offset in 0..4u32 {
        for size in [0u32, 1, 15, 16, 17, 31, 32, 4096] {
            for slot in 0..2u32 {
                let sp = 0x30000 + offset;
                pool_frame(ram, sp, size, 0x41424344)?;
                state.esp = sp;
                state.eax = 0x87654321;
                let before = state.encode();
                pool_call(&mut state, ram, slot, offset, offset == 3)?;
                let address = state.eax;
                if address == 0
                    || state.esp != sp + if slot == 1 { 12 } else { 8 }
                    || ram.kernel_caller() != 0x123456
                {
                    return Err(-3);
                }
                let mut expected = registers::Registers::new();
                expected.decode(&before)?;
                expected.eax = address;
                expected.esp = state.esp;
                if expected.encode() != state.encode() {
                    return Err(-3);
                }
                let mut bytes = std::vec::Vec::new();
                bytes.resize(size.max(16) as usize, 0u8);
                ram.read_bytes(address, &mut bytes)?;
                for value in &bytes {
                    if *value != 0 {
                        return Err(-3);
                    }
                }
                ram.fill(address, 0x7b, size.max(16))?;
                pool_frame(ram, sp, address, 0)?;
                state.esp = sp;
                pool_call(&mut state, ram, 3, offset, false)?;
                let recorded = state.eax;
                if recorded < size.max(16) || state.esp != sp + 8 {
                    return Err(-3);
                }
                pool_frame(ram, sp, address + 1, 0)?;
                state.esp = sp;
                pool_call(&mut state, ram, 3, offset, true)?;
                if state.eax != recorded - 1 {
                    return Err(-3);
                }
                pool_frame(ram, sp, address, 0)?;
                state.esp = sp;
                pool_call(&mut state, ram, 2, offset, false)?;
                if state.eax != 0 || state.esp != sp + 8 {
                    return Err(-3);
                }
            }
        }
    }
    let before = pool_stats(ram)?;
    for slot in [4u32, 5, 377] {
        state.esp = 0x30000;
        let bytes = state.encode();
        if calls_runtime::invoke_call(&mut state, ram, 0xfe000000 + slot * 4, 0) != Err(-5)
            || state.encode() != bytes
        {
            return Err(-3);
        }
    }
    state.esp = 0xfffffffc;
    let bytes = state.encode();
    if kernel_pool::kernel_invoke(&mut state, ram, 0) != Err(-1) || state.encode() != bytes {
        return Err(-3);
    }
    state.esp = 0x30000;
    state.fp_top = 8;
    let bytes = state.encode();
    if kernel_pool::kernel_invoke(&mut state, ram, 0) != Err(-1) || state.encode() != bytes {
        return Err(-3);
    }
    state.fp_top = 0;
    if pool_stats(ram)? != before {
        return Err(-3);
    }
    Ok(())
}
fn pool_bad_reads(ram: &mut memory::Memory, window: u64) -> Result<(), i32> {
    let sp = 0x30000;
    pool_frame(ram, sp, 64, 0x41424344)?;
    let before = pool_stats(ram)?;
    let mut state = registers::Registers::new();
    state.esp = sp;
    state.eax = 0x1234;
    let mut tail = ram.call_view();
    tail.attach(sp, 8, window, u64::from(sp), true)?;
    let bytes = state.encode();
    if kernel_pool::kernel_invoke(&mut state, &mut tail, 1) != Err(-4) || state.encode() != bytes {
        return Err(-3);
    }
    let mut head = ram.call_view();
    head.attach(sp + 4, 8, window, u64::from(sp + 4), true)?;
    if kernel_pool::kernel_invoke(&mut state, &mut head, 1) != Err(-4)
        || state.encode() != bytes
        || pool_stats(ram)? != before
    {
        return Err(-3);
    }
    // ESP zero skips the original dummy-return read. Only arguments are needed.
    ram.write_u32(4, 16)?;
    ram.write_u32(8, 0x41424344)?;
    let mut zero_view = ram.call_view();
    zero_view.attach(4, 8, window, 4, true)?;
    state.esp = 0;
    kernel_pool::kernel_invoke(&mut state, &mut zero_view, 1)?;
    let address = state.eax;
    if address == 0 || state.esp != 12 || zero_view.kernel_caller() != 0 {
        return Err(-3);
    }
    ram.write_u32(4, address)?;
    state.esp = 0;
    kernel_pool::kernel_invoke(&mut state, &mut zero_view, 2)?;
    if state.eax != 0 || state.esp != 8 {
        return Err(-3);
    }
    zero_view.heap_close()?;
    let context = ram.thread_context();
    let mut restored = memory::Memory::with_context(context)?;
    restored.attach(0, 67108864, window, 0, true)?;
    pool_frame(&mut restored, sp, 16, 0)?;
    state.esp = sp;
    kernel_pool::kernel_invoke(&mut state, &mut restored, 0)?;
    let address = state.eax;
    pool_frame(&mut restored, sp, address, 0)?;
    state.esp = sp;
    kernel_pool::kernel_invoke(&mut state, &mut restored, 2)?;
    restored.heap_close()?;
    pool_stats(ram)?;
    Ok(())
}
fn pool_bad_records(ram: &mut memory::Memory) -> Result<(), i32> {
    let id = ram.kernel_handle();
    let mut state = registers::Registers::new();
    state.esp = 0x30000;
    state.eax = 0x1234;
    pool_frame(ram, state.esp, 16, 0)?;
    let cpu = state.encode();
    let stats = pool_stats(ram)?;
    for (at, value, code) in [
        (0usize, 0u32, -1i32),
        (4, 379, -1),
        (8, 0, -1),
        (16, 1, -1),
        (20, 1, -1),
        (24, 180, -5),
    ] {
        let (version, original) = gamepkg::local_record_snapshot(id, 1536)?;
        let mut changed = std::vec::Vec::new();
        changed.extend(&original);
        changed[at..at + 4].copy_from_slice(&u32::to_le_bytes(value));
        let next = gamepkg::local_record_compare_exchange(id, version, &changed)?;
        if kernel_pool::kernel_invoke(&mut state, ram, 0) != Err(code)
            || state.encode() != cpu
            || pool_stats(ram)? != stats
        {
            return Err(-3);
        }
        gamepkg::local_record_compare_exchange(id, next, &original)?;
    }
    Ok(())
}
pub fn pool_worker(setup: u64) -> u64 {
    let mut phase = 0u64;
    match pool_worker_run(setup, &mut phase) {
        Ok(value) => value,
        Err(code) => {
            let _ = gamepkg::diagnostic(3, 0x544b0603, &[setup, phase, code as i64 as u64]);
            code as i64 as u64
        }
    }
}
fn pool_worker_run(setup: u64, phase: &mut u64) -> Result<u64, i32> {
    let mut cfg = [0u8; 48];
    if gamepkg::local_mailbox_receive(setup, 1000, &mut cfg)? != 48 {
        return Err(-3);
    }
    let context = [
        pool_read_long(&cfg, 0)?,
        pool_read_long(&cfg, 8)?,
        pool_read_long(&cfg, 16)?,
        pool_read_long(&cfg, 24)?,
    ];
    let window = pool_read_long(&cfg, 32)?;
    let tag = pool_read_long(&cfg, 40)?;
    if !(1..=4).contains(&tag) {
        return Err(-1);
    }
    *phase = 1;
    let mut ram = memory::Memory::with_context(context)?;
    *phase = 2;
    ram.attach(0, 67108864, window, 0, true)?;
    let sp = 0x50000 + tag as u32 * 4096;
    let mut state = registers::Registers::new();
    for iteration in 0..128 {
        *phase = 10 + iteration * 10;
        pool_frame(&mut ram, sp, 64, 0)?;
        state.esp = sp;
        *phase += 1;
        calls_runtime::invoke_call(&mut state, &mut ram, 0xfe000000, 0)?;
        let address = state.eax;
        if address == 0 {
            return Err(-3);
        }
        *phase += 1;
        ram.fill(address, tag as u8, 64)?;
        pool_frame(&mut ram, sp, address, 0)?;
        state.esp = sp;
        *phase += 1;
        calls_runtime::invoke_call(&mut state, &mut ram, 0xfe00000c, 0)?;
        if state.eax < 64 {
            return Err(-3);
        }
        let mut bytes = [0u8; 64];
        *phase += 1;
        ram.read_bytes(address, &mut bytes)?;
        if bytes != [tag as u8; 64] {
            return Err(-3);
        }
        pool_frame(&mut ram, sp, address, 0)?;
        state.esp = sp;
        *phase += 1;
        calls_runtime::invoke_call(&mut state, &mut ram, 0xfe000008, 0)?;
        if state.eax != 0 {
            return Err(-3);
        }
    }
    *phase = 2000;
    ram.heap_close()?;
    Ok((tag << 32) | 128)
}
fn pool_threads(ram: &memory::Memory, window: u64) -> Result<(), i32> {
    let before = ram.call_snapshot()?;
    let setup = gamepkg::local_mailbox_create(48, 4)?;
    let mut jobs = [0u64; 4];
    for number in 0..4usize {
        let mut cfg = std::vec::Vec::new();
        for value in ram.thread_context() {
            cfg.extend(u64::to_le_bytes(value));
        }
        cfg.extend(u64::to_le_bytes(window));
        cfg.extend(u64::to_le_bytes(number as u64 + 1));
        gamepkg::local_mailbox_send(setup, &cfg)?;
        jobs[number] = pool_spawn(1, setup)?;
    }
    let mut seen = 0u32;
    for job in jobs {
        let value = pool_join(job)?;
        let tag = (value >> 32) as u32;
        if value as u32 != 128 {
            let _ = gamepkg::diagnostic(3, 0x544b0604, &[job, value]);
        }
        if !(1..=4).contains(&tag) || value as u32 != 128 || seen & (1 << tag) != 0 {
            return Err(-3);
        }
        seen |= 1 << tag;
    }
    if seen != 30 {
        return Err(-3);
    }
    gamepkg::local_mailbox_release(setup)?;
    let after = ram.call_snapshot()?;
    if after.1 != before.1.wrapping_add(1536) || after.2 != before.2.wrapping_add(1536) {
        return Err(-3);
    }
    Ok(())
}
pub fn pool_verify(ram: &mut memory::Memory, window: u64) -> Result<(), i32> {
    // Caller supplies its already committed 64 MiB checked RAM window.
    gamepkg::diagnostic(0, 0x544b0600, &[1])?;
    pool_plans(ram, window)?;
    gamepkg::diagnostic(0, 0x544b0600, &[2])?;
    let inbox = heap_service::heap_inbox()?;
    let job = pool_spawn(0, inbox)?;
    let mut client = heap_service::HeapClient::heap_client(inbox, job)?;
    client.heap_initialize(ram.call_handle(), window, 0x1000000, 67108864)?;
    let mut owner = Some(client);
    ram.heap_install(&mut owner)?;
    if owner.is_some() {
        return Err(-3);
    }
    pool_write_table(ram, 0x20000)?;
    kernel_pool::kernel_prepare(ram, 0x20000, 5)?;
    for slot in 0..4u32 {
        if ram.read_u32(0x20000 + slot * 4)? != 0xfe000000 + slot * 4 {
            return Err(-3);
        }
    }
    gamepkg::diagnostic(0, 0x544b0600, &[3])?;
    pool_values(ram)?;
    gamepkg::diagnostic(0, 0x544b0600, &[4])?;
    pool_bad_reads(ram, window)?;
    gamepkg::diagnostic(0, 0x544b0600, &[5])?;
    pool_bad_records(ram)?;
    gamepkg::diagnostic(0, 0x544b0600, &[6])?;
    pool_threads(ram, window)?;
    let mut old = ram.call_view();
    old.attach(0, 67108864, window, 0, true)?;
    gamepkg::local_record_release(ram.kernel_handle())?;
    let mut state = registers::Registers::new();
    state.esp = 0x30000;
    let before = state.encode();
    if calls_runtime::invoke_call(&mut state, &mut old, 0xfe000000, 0) != Err(-4)
        || state.encode() != before
    {
        return Err(-3);
    }
    gamepkg::diagnostic(0, 0x544b0600, &[7])?;
    ram.kernel_clear()?;
    ram.heap_close()?;
    if pool_join(job)? != 0 {
        return Err(-3);
    }
    Ok(())
}
