// Ordinary checked game source: actual finite router, no raw native handlers.
use crate::calls_runtime;
use crate::gamepkg;
use crate::heap_service;
use crate::kernel_pool;
use crate::layout;
use crate::memory;
use crate::registers;
fn memory_long(bytes: &[u8], at: usize) -> Result<u64, i32> {
    let p = bytes.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        p[0], p[1], p[2], p[3], p[4], p[5], p[6], p[7],
    ]))
}
fn memory_word(bytes: &[u8], at: usize) -> Result<u32, i32> {
    let p = bytes.get(at..at + 4).ok_or(-1)?;
    Ok(u32::from_le_bytes([p[0], p[1], p[2], p[3]]))
}
fn memory_spawn(entry: u32, arg: u64) -> Result<u64, i32> {
    let mut req = std::vec::Vec::new();
    req.extend(u32::to_le_bytes(entry));
    req.extend(u64::to_le_bytes(arg));
    req.extend(u32::to_le_bytes(0));
    let mut out = [0u8; 8];
    if gamepkg::local_call(10, &req, &mut out)? != 8 {
        return Err(-3);
    }
    memory_long(&out, 0)
}
fn memory_join(job: u64) -> Result<u64, i32> {
    let pause = gamepkg::local_mailbox_create(1, 1)?;
    let mut out = [0u8; 8];
    let mut byte = [0u8; 1];
    loop {
        match gamepkg::local_call(11, &u64::to_le_bytes(job), &mut out) {
            Ok(8) => {
                gamepkg::local_mailbox_release(pause)?;
                return memory_long(&out, 0);
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
fn memory_frame(ram: &mut memory::Memory, sp: u32, args: [u32; 5]) -> Result<(), i32> {
    for number in 0..=40u32 {
        ram.write_u32(sp + number * 4, 0x11000000 + number)?;
    }
    ram.write_u32(sp, 0x23456789)?;
    for number in 0..5usize {
        ram.write_u32(sp + 4 + number as u32 * 4, args[number])?;
    }
    Ok(())
}
fn memory_invoke(
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
fn memory_counters(ram: &mut memory::Memory) -> Result<[u32; 6], i32> {
    let mut client = ram.heap_take()?;
    let mut out = [0u32; 6];
    let result = memory_counter_values(&mut client, &mut out);
    ram.heap_put(client);
    result?;
    Ok(out)
}
fn memory_counter_values(
    client: &mut heap_service::HeapClient,
    output: &mut [u32; 6],
) -> Result<(), i32> {
    for index in 0..6u32 {
        output[index as usize] = client.heap_contiguous_counter(index)?;
    }
    Ok(())
}
fn memory_ticket_count(ram: &memory::Memory) -> Result<u32, i32> {
    let (_, bytes) = gamepkg::local_record_snapshot(ram.kernel_handle(), 1540)?;
    let count = memory_word(&bytes, 4)?;
    memory_word(&bytes, 24 + count as usize * 4)
}
fn memory_mode(window: u64, at: u64, mode: u32) -> Result<(), i32> {
    let mut req = std::vec::Vec::new();
    req.extend(u64::to_le_bytes(window));
    req.extend(u64::to_le_bytes(at));
    req.extend(u64::to_le_bytes(4096));
    req.extend(u32::to_le_bytes(mode));
    gamepkg::local_call(3, &req, &mut [])?;
    Ok(())
}
fn memory_table(ram: &mut memory::Memory) -> Result<(), i32> {
    let ordinals = [14u32, 15, 17, 23, 165, 166, 168, 171, 180, 0];
    for index in 0..10usize {
        let ordinal = ordinals[index];
        ram.write_u32(
            0x21000 + index as u32 * 4,
            if ordinal == 0 {
                0
            } else {
                0x80000000 | ordinal
            },
        )?;
    }
    Ok(())
}
fn memory_successes(ram: &mut memory::Memory) -> Result<(), i32> {
    let mut state = registers::Registers::new();
    state.ebx = 0x12345678;
    state.fs_base = 0x4000;
    state.fp_stack[1] = f64::from_bits(0x7ff8000012345678);
    for offset in 0..4u32 {
        let sp = 0x30000 + offset;
        memory_frame(ram, sp, [64, 0, 0, 0, 0])?;
        state.esp = sp;
        memory_invoke(&mut state, ram, 0, offset, offset == 3)?;
        let ordinary = state.eax;
        if ordinary == 0 || state.esp != sp + 8 {
            return Err(-3);
        }
        memory_frame(ram, sp, [4097, 0, 0, 0, 0])?;
        state.esp = sp;
        memory_invoke(&mut state, ram, 4, offset, offset == 3)?;
        let physical = state.eax;
        if physical < 0x80001000 || physical >= 0x83fe0000 || state.esp != sp + 8 {
            return Err(-3);
        }
        memory_frame(ram, sp, [physical + 7, 0, 0, 0, 0])?;
        state.esp = sp;
        memory_invoke(&mut state, ram, 8, offset, false)?;
        if state.eax != 8185 {
            return Err(-3);
        }
        memory_frame(ram, sp, [ordinary + 7, 0, 0, 0, 0])?;
        state.esp = sp;
        memory_invoke(&mut state, ram, 8, offset, true)?;
        if state.eax < 57 {
            return Err(-3);
        }
        for address in [ordinary, physical] {
            memory_frame(ram, sp, [address, 0, 0, 0, 0])?;
            state.esp = sp;
            memory_invoke(&mut state, ram, 7, offset, false)?;
            if state.eax != 0 || state.esp != sp + 8 {
                return Err(-3);
            }
            memory_frame(ram, sp, [address, 0, 0, 0, 0])?;
            state.esp = sp;
            memory_invoke(&mut state, ram, 8, offset, false)?;
            if state.eax != 0 {
                return Err(-3);
            }
        }
        memory_frame(ram, sp, [4096, 0x200003, 0x201002, 1, 0x11223344])?;
        state.esp = sp;
        memory_invoke(&mut state, ram, 5, offset, offset == 3)?;
        if state.eax != 0x80200003 || state.esp != sp + 24 {
            return Err(-3);
        }
        memory_frame(ram, sp, [0x80200000, 0, 0, 0, 0])?;
        state.esp = sp;
        memory_invoke(&mut state, ram, 7, offset, false)?;
        memory_frame(ram, sp, [u32::MAX, 0x35001, 0, 0, 0])?;
        ram.write_u32(0x35001, 0xaabbccdd)?;
        state.esp = sp;
        memory_invoke(&mut state, ram, 6, offset, false)?;
        if state.eax != 0x83fe0000 || state.esp != sp + 12 || ram.read_u32(0x35001)? != 0 {
            return Err(-3);
        }
    }
    Ok(())
}
fn memory_denials(ram: &mut memory::Memory, main: u64, physical: u64) -> Result<(), i32> {
    let mut state = registers::Registers::new();
    state.eax = 0x87654321;
    state.esp = 0x40fec;
    memory_frame(ram, state.esp, [4096, 0, 0xfffffff, 4096, 0])?;
    let original = state.encode();
    let counters = memory_counters(ram)?;
    memory_mode(main, 0x41000, 0)?;
    if calls_runtime::invoke_call(&mut state, ram, 0xfe000014, 0) != Err(-4)
        || state.encode() != original
        || memory_counters(ram)? != counters
    {
        return Err(-3);
    }
    memory_mode(main, 0x41000, 2)?;
    for (slot, sp, code) in [
        (5u32, 0xfffffff0u32, -1i32),
        (5, 0xffffffe8, -4),
        (6, 0xfffffff8, -1),
        (6, 0xfffffff4, -4),
    ] {
        state.esp = sp;
        let cpu = state.encode();
        if calls_runtime::invoke_call(&mut state, ram, 0xfe000000 + slot * 4, 0) != Err(code)
            || state.encode() != cpu
            || memory_counters(ram)? != counters
        {
            return Err(-3);
        }
    }
    // Failed instance-padding writes must also restore the exclusive client.
    memory_frame(ram, 0x42000, [u32::MAX, 0x43fff, 0, 0, 0])?;
    state.esp = 0x42000;
    let original = state.encode();
    ram.fill(0x43ffc, 0x7b, 8)?;
    memory_mode(main, 0x44000, 1)?;
    if calls_runtime::invoke_call(&mut state, ram, 0xfe000018, 0) != Err(-4)
        || state.encode() != original
        || ram.read_u32(0x43ffc)? != 0x7b7b7b7b
    {
        return Err(-3);
    }
    memory_mode(main, 0x44000, 2)?;
    memory_counters(ram)?;
    for (low, size) in [(0x80000000u32, 1u32), (0x3fff000, 8192), (1, u32::MAX)] {
        memory_frame(ram, 0x45000, [size, low, low, 4096, 0])?;
        state.esp = 0x45000;
        let original = state.encode();
        let before = memory_counters(ram)?;
        if calls_runtime::invoke_call(&mut state, ram, 0xfe000014, 0) != Err(-1)
            || state.encode() != original
            || memory_counters(ram)? != before
        {
            return Err(-3);
        }
    }
    gamepkg::local_memory_fill(physical, 4096, 8192, 0x7b)?;
    memory_mode(physical, 8192, 1)?;
    memory_frame(ram, 0x46000, [8192, 4096, 12287, 0, 0])?;
    state.esp = 0x46000;
    let original = state.encode();
    let before = memory_counters(ram)?;
    if calls_runtime::invoke_call(&mut state, ram, 0xfe000014, 0) != Err(-4)
        || state.encode() != original
        || memory_counters(ram)? != before
    {
        return Err(-3);
    }
    let mut bytes = [0u8; 8];
    gamepkg::local_memory_read_segments(&[[physical, 4096, 8]], &mut bytes)?;
    if bytes != [0x7b; 8] {
        return Err(-3);
    }
    memory_mode(physical, 8192, 2)?;
    // The first refusal consumes one ticket before its extra stack read faults.
    memory_frame(ram, 0x4ff70, [u32::MAX, 0, u32::MAX, 4096, 0xaa55])?;
    state.esp = 0x4ff70;
    let original = state.encode();
    memory_mode(main, 0x50000, 0)?;
    if calls_runtime::invoke_call(&mut state, ram, 0xfe000014, 0) != Err(-4)
        || state.encode() != original
        || memory_ticket_count(ram)? != 1
    {
        return Err(-3);
    }
    memory_mode(main, 0x50000, 2)?;
    Ok(())
}
pub fn kernel_memory_worker(setup: u64) -> u64 {
    match memory_worker_run(setup) {
        Ok(value) => value,
        Err(code) => code as i64 as u64,
    }
}
fn memory_worker_run(setup: u64) -> Result<u64, i32> {
    let mut cfg = [0u8; 56];
    loop {
        match gamepkg::local_mailbox_receive(setup, 1000, &mut cfg) {
            Ok(56) => break,
            Err(-2) => {}
            Ok(_) => return Err(-3),
            Err(code) => return Err(code),
        }
    }
    let context = [
        memory_long(&cfg, 0)?,
        memory_long(&cfg, 8)?,
        memory_long(&cfg, 16)?,
        memory_long(&cfg, 24)?,
    ];
    let main = memory_long(&cfg, 32)?;
    let physical = memory_long(&cfg, 40)?;
    let tag = memory_long(&cfg, 48)?;
    if !(1..=4).contains(&tag) {
        return Err(-1);
    }
    let source = memory::Memory::with_context(context)?;
    let plan = layout::Layout::layout_current(&source)?;
    if plan.layout_banks()[0] != main || plan.layout_banks()[1] != physical {
        return Err(-4);
    }
    let mut ram = plan.layout_view(&source)?;
    let sp = 0x70000 + tag as u32 * 4096;
    let mut state = registers::Registers::new();
    for iteration in 0..128u32 {
        memory_frame(&mut ram, sp, [64, 0, 0, 0, 0])?;
        state.esp = sp;
        calls_runtime::invoke_call(&mut state, &mut ram, 0xfe000000, 0)?;
        let ordinary = state.eax;
        memory_frame(&mut ram, sp, [64, 0, 0, 0, 0])?;
        state.esp = sp;
        calls_runtime::invoke_call(&mut state, &mut ram, 0xfe000010, 0)?;
        let first = state.eax;
        memory_frame(&mut ram, sp, [64, 0, u32::MAX, 4096, 0])?;
        state.esp = sp;
        calls_runtime::invoke_call(&mut state, &mut ram, 0xfe000014, 0)?;
        let second = state.eax;
        if ordinary < 16777216
            || ordinary >= 67108864
            || first < 0x80001000
            || first >= 0x83fe0000
            || second < 0x80001000
            || second >= 0x83fe0000
            || first == second
        {
            return Err(-3);
        }
        for address in [ordinary, first, second] {
            ram.fill(address, tag as u8, 64)?;
        }
        let mut held = [0u8; 64];
        for address in [ordinary, first, second] {
            ram.read_bytes(address, &mut held)?;
            if held != [tag as u8; 64] {
                return Err(-3);
            }
        }
        ram.read_bytes(0xf0000000 + (first - 0x80000000), &mut held)?;
        if held != [tag as u8; 64] {
            return Err(-3);
        }
        for address in [ordinary, first, second] {
            memory_frame(&mut ram, sp, [address + 7, 0, 0, 0, 0])?;
            state.esp = sp;
            calls_runtime::invoke_call(&mut state, &mut ram, 0xfe000020, 0)?;
            if state.eax < 57 {
                return Err(-3);
            }
            memory_frame(&mut ram, sp, [address, 0, 0, 0, 0])?;
            state.esp = sp;
            calls_runtime::invoke_call(&mut state, &mut ram, 0xfe00001c, 0)?;
            if state.eax != 0 {
                return Err(-3);
            }
        }
        if iteration < 4 {
            memory_frame(&mut ram, sp, [u32::MAX, 0, u32::MAX, 4096, 0xaa55])?;
            state.esp = sp;
            calls_runtime::invoke_call(&mut state, &mut ram, 0xfe000014, 0)?;
            if state.eax != 0 || state.esp != sp + 24 {
                return Err(-3);
            }
        }
    }
    ram.heap_close()?;
    Ok((tag << 32) | 128)
}
fn memory_threads(ram: &mut memory::Memory, main: u64, physical: u64) -> Result<(), i32> {
    let before = memory_counters(ram)?;
    let setup = gamepkg::local_mailbox_create(56, 4)?;
    let mut jobs = [0u64; 4];
    for number in 0..4usize {
        let mut cfg = std::vec::Vec::new();
        for value in ram.thread_context() {
            cfg.extend(u64::to_le_bytes(value));
        }
        for value in [main, physical, number as u64 + 1] {
            cfg.extend(u64::to_le_bytes(value));
        }
        gamepkg::local_mailbox_send(setup, &cfg)?;
        jobs[number] = memory_spawn(1, setup)?;
    }
    let mut seen = 0u32;
    for job in jobs {
        let result = memory_join(job)?;
        let tag = (result >> 32) as u32;
        if !(1..=4).contains(&tag) || result as u32 != 128 || seen & (1 << tag) != 0 {
            return Err(-3);
        }
        seen |= 1 << tag;
    }
    gamepkg::local_mailbox_release(setup)?;
    let after = memory_counters(ram)?;
    if seen != 30
        || after[0] != 0
        || after[3] != before[3] + 1024
        || after[4] != before[4] + 1024
        || memory_ticket_count(ram)? != 8
    {
        return Err(-3);
    }
    Ok(())
}
fn memory_map_controls(ram: &mut memory::Memory, main: u64) -> Result<(), i32> {
    let mut state = registers::Registers::new();
    state.esp = 0x4ff70;
    memory_frame(ram, state.esp, [u32::MAX, 0, u32::MAX, 4096, 0xaa55])?;
    memory_mode(main, 0x50000, 0)?;
    calls_runtime::invoke_call(&mut state, ram, 0xfe000014, 0)?;
    if state.eax != 0 || state.esp != 0x4ff88 || memory_ticket_count(ram)? != 8 {
        return Err(-3);
    }
    memory_mode(main, 0x50000, 2)?;
    let id = ram.kernel_handle();
    let (version, original) = gamepkg::local_record_snapshot(id, 1540)?;
    let count = memory_word(&original, 4)?;
    let mut corrupted = std::vec::Vec::new();
    corrupted.extend(&original);
    let at = 24 + count as usize * 4;
    let invalid = u32::to_le_bytes(9);
    for byte in 0..4usize {
        corrupted[at + byte] = invalid[byte];
    }
    let next = gamepkg::local_record_compare_exchange(id, version, &corrupted)?;
    let cpu = state.encode();
    if calls_runtime::invoke_call(&mut state, ram, 0xfe000010, 0) != Err(-1)
        || state.encode() != cpu
    {
        return Err(-3);
    }
    gamepkg::local_record_compare_exchange(id, next, &original)?;
    Ok(())
}
pub fn memory_kernel_verify(ram: &mut memory::Memory, main: u64, physical: u64) -> Result<(), i32> {
    let plan = layout::Layout::layout_current(ram)?;
    if plan.layout_banks()[0] != main || plan.layout_banks()[1] != physical {
        return Err(-4);
    }
    *ram = plan.layout_view(ram)?;
    memory_table(ram)?;
    let mut table = [0u8; 40];
    ram.read_bytes(0x21000, &mut table)?;
    if kernel_pool::kernel_prepare(ram, 0x21000, 10) != Err(-5) || ram.kernel_handle() != 0 {
        return Err(-3);
    }
    let mut unchanged = [0u8; 40];
    ram.read_bytes(0x21000, &mut unchanged)?;
    if table != unchanged {
        return Err(-3);
    }
    let inbox = heap_service::heap_inbox()?;
    let job = memory_spawn(0, inbox)?;
    let mut client = heap_service::HeapClient::heap_client(inbox, job)?;
    client.heap_initialize(ram.call_handle(), main, 16777216, 67108864)?;
    client.heap_contiguous_initialize(physical)?;
    let mut owner = Some(client);
    ram.heap_install(&mut owner)?;
    kernel_pool::kernel_prepare(ram, 0x21000, 10)?;
    if memory_ticket_count(ram)? != 0 {
        return Err(-3);
    }
    gamepkg::diagnostic(0, 0x544b0b00, &[1])?;
    memory_successes(ram)?;
    gamepkg::diagnostic(0, 0x544b0b00, &[2])?;
    memory_denials(ram, main, physical)?;
    gamepkg::diagnostic(0, 0x544b0b00, &[3])?;
    memory_threads(ram, main, physical)?;
    gamepkg::diagnostic(0, 0x544b0b00, &[4])?;
    memory_map_controls(ram, main)?;
    gamepkg::diagnostic(0, 0x544b0b00, &[5])?;
    ram.kernel_clear()?;
    ram.heap_close()?;
    if memory_join(job)? != 0 {
        return Err(-3);
    }
    Ok(())
}
