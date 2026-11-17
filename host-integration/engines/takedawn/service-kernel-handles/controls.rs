use crate::gamepkg;
use crate::handles;
use crate::memory;
fn handle_control(stage: i32, ok: bool) -> Result<(), i32> {
    if ok {
        Ok(())
    } else {
        Err(-stage)
    }
}
pub fn handle_controls(ram: &memory::Memory, map: &mut handles::Handles) -> Result<(), i32> {
    let mut borrowed = handles::Handles::handles_borrow(ram, map.handles_id())?;
    handle_control(101, borrowed.handles_close(ram) == Err(-4))?;
    let other = memory::Memory::new()?;
    handle_control(
        102,
        handles::Handles::handles_borrow(&other, map.handles_id()).err() == Some(-4),
    )?;
    gamepkg::local_record_release(other.call_handle())?;
    handle_control(
        103,
        handles::Handles::handles_borrow(ram, map.handles_id() | 0x100000000).err() == Some(-4),
    )?;
    let mut guard = map.handles_lock(ram)?;
    for token in [
        0u32, 0xffffffff, 0xfffffffe, 0xdead0001, 0xbeef0010, 0x48000000, 0x48004000, 0x48010001,
        0x49000001,
    ] {
        handle_control(
            104,
            guard.handle_guard_lookup(ram, token) == Err(-1)
                && guard.handle_guard_remove(ram, token) == Err(-1),
        )?;
    }
    handle_control(
        105,
        guard.handle_guard_insert(ram, 0, 1, false) == Err(-1)
            && guard.handle_guard_insert(ram, 1, 0, false) == Err(-1),
    )?;
    let first = guard.handle_guard_insert(ram, 0x100000001, 1, false)?;
    let second = guard.handle_guard_insert(ram, 0x200000001, 1, false)?;
    let other_kind = guard.handle_guard_insert(ram, 0x100000001, 2, false)?;
    let duplicate = guard.handle_guard_insert(ram, 0x100000001, 1, true)?;
    handle_control(
        106,
        [first, second, other_kind, duplicate] == [0x48000001, 0x48000002, 0x48000003, 0x48000004],
    )?;
    handle_control(
        107,
        guard.handle_guard_remove(ram, first)? == Some([0x100000001, 1])
            && guard.handle_guard_lookup(ram, duplicate)? == Some([0x100000001, 1]),
    )?;
    handle_control(
        108,
        guard.handle_guard_insert(ram, 0x100000001, 1, false)? == duplicate,
    )?;
    handle_control(
        109,
        guard.handle_guard_insert(ram, 0x300000001, 1, false)? == first,
    )?;
    handle_control(
        138,
        guard.handle_guard_references(ram, 0x100000001, 1)? == 1
            && guard.handle_guard_references(ram, 0x100000001, 2)? == 1,
    )?;
    for token in [first, second, other_kind, duplicate] {
        handle_control(110, guard.handle_guard_remove(ram, token)?.is_some())?;
    }
    guard.handles_guard_close(ram)?;
    let (_, original) = gamepkg::local_record_snapshot(map.handles_id(), 80)?;
    let window = handle_word(&original, 48)?;
    let offset = handle_word(&original, 56)?;
    for at in [0usize, 8, 16, 24, 40, 48, 56, 64, 72] {
        let (version, mut changed) = gamepkg::local_record_snapshot(map.handles_id(), 80)?;
        changed[at] ^= 1;
        gamepkg::local_record_compare_exchange(map.handles_id(), version, &changed)?;
        handle_control(
            111,
            handles::Handles::handles_borrow(ram, map.handles_id()).is_err(),
        )?;
        let (version, _) = gamepkg::local_record_snapshot(map.handles_id(), 80)?;
        gamepkg::local_record_compare_exchange(map.handles_id(), version, &original)?;
    }
    let mut checked_guard = map.handles_lock(ram)?;
    let page = handle_page_bytes(window, offset)?;
    for at in [0usize, 4, 8, 24, 32, 16000] {
        let mut changed = page.clone();
        changed[at] ^= 1;
        handle_control(
            112,
            gamepkg::local_memory_compare_exchange(window, offset, &page, &changed)?,
        )?;
        handle_control(
            113,
            checked_guard.handle_guard_lookup(ram, 0x48000001).is_err(),
        )?;
        handle_control(
            114,
            gamepkg::local_memory_compare_exchange(window, offset, &changed, &page)?,
        )?;
    }
    let mut maximum = page.clone();
    for at in 16..24usize {
        maximum[at] = 255;
    }
    handle_control(
        115,
        gamepkg::local_memory_compare_exchange(window, offset, &page, &maximum)?,
    )?;
    handle_control(
        116,
        checked_guard.handle_guard_insert(ram, 17, 1, false) == Err(-3)
            && checked_guard.handle_guard_lookup(ram, 0x48000001)? == None,
    )?;
    handle_control(
        117,
        gamepkg::local_memory_compare_exchange(window, offset, &maximum, &page)?,
    )?;
    checked_guard.handles_guard_close(ram)?;
    let mut overlap = handles::Handles::handles_new(ram)?;
    handle_control(
        118,
        overlap.handles_bind(ram, window, offset) == Err(-2)
            && overlap.handles_progress() == [1, 0],
    )?;
    overlap.handles_close(ram)?;
    handle_control(119, overlap.handles_closed())?;
    // Fail after five actual accepted pages; never clear the unclaimed sixth.
    let partial_offset = 262144u64;
    let dirty_offset = partial_offset + 5 * 16384;
    handle_write_byte(window, dirty_offset, 0xa5)?;
    let mut partial = handles::Handles::handles_new(ram)?;
    handle_control(
        120,
        partial.handles_bind(ram, window, partial_offset) == Err(-2)
            && partial.handles_progress() == [1, 5],
    )?;
    handle_control(
        121,
        partial.handles_lock(ram).err() == Some(-4)
            && partial.handles_bind(ram, window, partial_offset) == Err(-1),
    )?;
    partial.handles_close(ram)?;
    handle_control(
        122,
        partial.handles_closed() && handle_page_bytes(window, dirty_offset)?[0] == 0xa5,
    )?;
    handle_write_byte(window, dirty_offset, 0)?;
    let mut interrupted = handles::Handles::handles_new(ram)?;
    interrupted.handles_bind(ram, window, partial_offset)?;
    let borrowed_ready = handles::Handles::handles_borrow(ram, interrupted.handles_id())?;
    let denied_offset = partial_offset + 14 * 16384;
    let protect = [
        u64::to_le_bytes(window).as_slice(),
        &u64::to_le_bytes(denied_offset),
        &u64::to_le_bytes(16384),
        &u32::to_le_bytes(1),
    ]
    .concat();
    gamepkg::local_call(3, &protect, &mut [])?;
    handle_control(
        123,
        interrupted.handles_close(ram) == Err(-4)
            && !interrupted.handles_closed()
            && interrupted.handles_progress() == [3, 15],
    )?;
    handle_control(124, borrowed_ready.handles_lock(ram).err() == Some(-4))?;
    let protect = [
        u64::to_le_bytes(window).as_slice(),
        &u64::to_le_bytes(denied_offset),
        &u64::to_le_bytes(16384),
        &u32::to_le_bytes(2),
    ]
    .concat();
    gamepkg::local_call(3, &protect, &mut [])?;
    let mut blocked = handles::Handles::handles_new(ram)?;
    handle_control(
        125,
        blocked.handles_bind(ram, window, partial_offset) == Err(-2)
            && blocked.handles_progress() == [1, 0],
    )?;
    blocked.handles_close(ram)?;
    interrupted.handles_close(ram)?;
    handle_control(126, interrupted.handles_closed())?;
    let mut records = std::vec::Vec::new();
    for _ in 0..64usize {
        match gamepkg::local_record_create(&[0u8; 8]) {
            Ok(id) => records.push(id),
            Err(-2) => break,
            Err(code) => return Err(code),
        }
    }
    handle_control(127, handles::Handles::handles_new(ram).err() == Some(-2))?;
    let mut events = std::vec::Vec::new();
    for _ in 0..127usize {
        let mut bytes = [0u8; 8];
        handle_control(128, gamepkg::local_call(20, &[0u8; 8], &mut bytes)? == 8)?;
        events.push(u64::from_le_bytes(bytes));
    }
    handle_control(
        129,
        gamepkg::local_call(20, &[0u8; 8], &mut [0u8; 8]) == Err(-2),
    )?;
    for id in events {
        gamepkg::local_call(25, &u64::to_le_bytes(id), &mut [])?;
    }
    for id in records {
        gamepkg::local_record_release(id)?;
    }
    handle_parallel(ram, map)?;
    Ok(())
}

fn handle_parallel(ram: &memory::Memory, map: &handles::Handles) -> Result<(), i32> {
    let mut bytes = std::vec::Vec::new();
    bytes.extend(u64::to_le_bytes(ram.call_handle()));
    bytes.extend(u64::to_le_bytes(map.handles_id()));
    let argument = gamepkg::local_record_create(&bytes)?;
    let mut jobs = std::vec::Vec::new();
    for _ in 0..4u32 {
        let request = [
            u32::to_le_bytes(0).as_slice(),
            &u64::to_le_bytes(argument),
            &u32::to_le_bytes(0),
        ]
        .concat();
        let mut out = [0u8; 8];
        handle_control(130, gamepkg::local_call(10, &request, &mut out)? == 8)?;
        jobs.push(u64::from_le_bytes(out));
    }
    let mut tokens = std::vec::Vec::new();
    for job in jobs {
        let mut proof = None;
        for _ in 0..4096u32 {
            match gamepkg::local_task_join(job) {
                Ok(value) => {
                    proof = Some(value);
                    break;
                }
                Err(-2) => memory::Memory::call_retry_pause()?,
                Err(code) => return Err(code),
            }
        }
        let joined = proof.ok_or(-2)?;
        let token = joined.result() as u32;
        handle_control(
            131,
            joined.result() == u64::from(token) && token >= 0x48000001 && token < 0x48000005,
        )?;
        for previous in &tokens {
            handle_control(132, *previous != token)?;
        }
        tokens.push(token);
        let mut guard = map.handles_lock(ram)?;
        handle_control(
            133,
            guard.handle_guard_lookup(ram, token)? == Some([0x4000000000000000 + job, 1]),
        )?;
        guard.handles_guard_close(ram)?;
    }
    let mut guard = map.handles_lock(ram)?;
    for token in tokens {
        handle_control(134, guard.handle_guard_remove(ram, token)?.is_some())?;
    }
    guard.handles_guard_close(ram)?;
    gamepkg::local_record_release(argument)?;
    Ok(())
}
fn handle_actor(argument: u64) -> Result<u32, i32> {
    let (_, bytes) = gamepkg::local_record_snapshot(argument, 16)?;
    handle_control(135, bytes.len() == 16)?;
    let root = u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ]);
    let id = u64::from_le_bytes([
        bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
    ]);
    let ram = memory::Memory::with_call_handle(root)?;
    let map = handles::Handles::handles_borrow(&ram, id)?;
    let mut guard = map.handles_lock(&ram)?;
    let token = guard.handle_guard_insert(
        &ram,
        0x4000000000000000 + gamepkg::local_task_status()?.0,
        1,
        false,
    )?;
    guard.handles_guard_close(&ram)?;
    Ok(token)
}
pub fn handle_worker(argument: u64) -> u64 {
    match handle_actor(argument) {
        Ok(token) => u64::from(token),
        Err(code) => code as i64 as u64,
    }
}

fn handle_word(b: &[u8], at: usize) -> Result<u64, i32> {
    let p = b.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        p[0], p[1], p[2], p[3], p[4], p[5], p[6], p[7],
    ]))
}
fn handle_page_bytes(window: u64, offset: u64) -> Result<std::vec::Vec<u8>, i32> {
    let mut output = std::vec::Vec::new();
    output.resize(16384, 0u8);
    let request = [
        u64::to_le_bytes(window).as_slice(),
        &u64::to_le_bytes(offset),
        &u32::to_le_bytes(16384),
    ]
    .concat();
    handle_control(
        136,
        gamepkg::local_call(5, &request, &mut output)? == output.len(),
    )?;
    Ok(output)
}
fn handle_write_byte(window: u64, offset: u64, value: u8) -> Result<(), i32> {
    let request = [
        u64::to_le_bytes(window).as_slice(),
        &u64::to_le_bytes(offset),
        &[value],
    ]
    .concat();
    handle_control(137, gamepkg::local_call(6, &request, &mut [])? == 0)
}
