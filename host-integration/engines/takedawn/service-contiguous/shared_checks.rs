// Ordinary checked game-source controls for mixed v1/v2 allocator clients.
use crate::gamepkg;
use crate::heap_service;
use crate::memory;
fn combined_long(bytes: &[u8], at: usize) -> Result<u64, i32> {
    let p = bytes.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        p[0], p[1], p[2], p[3], p[4], p[5], p[6], p[7],
    ]))
}
fn combined_word(bytes: &[u8], at: usize) -> Result<u32, i32> {
    let p = bytes.get(at..at + 4).ok_or(-1)?;
    Ok(u32::from_le_bytes([p[0], p[1], p[2], p[3]]))
}
fn combined_spawn(entry: u32, arg: u64) -> Result<u64, i32> {
    let mut req = std::vec::Vec::new();
    req.extend(u32::to_le_bytes(entry));
    req.extend(u64::to_le_bytes(arg));
    req.extend(u32::to_le_bytes(0));
    let mut out = [0u8; 8];
    if gamepkg::local_call(10, &req, &mut out)? != 8 {
        return Err(-3);
    }
    combined_long(&out, 0)
}
fn combined_join(job: u64) -> Result<u64, i32> {
    let pause = gamepkg::local_mailbox_create(1, 1)?;
    let mut out = [0u8; 8];
    let mut byte = [0u8; 1];
    loop {
        match gamepkg::local_call(11, &u64::to_le_bytes(job), &mut out) {
            Ok(8) => {
                gamepkg::local_mailbox_release(pause)?;
                return combined_long(&out, 0);
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
fn combined_receive(inbox: u64, output: &mut [u8]) -> Result<usize, i32> {
    loop {
        if gamepkg::local_task_status()?.1 {
            return Err(-4);
        }
        match gamepkg::local_mailbox_receive(inbox, 1000, output) {
            Err(-2) => {}
            value => return value,
        }
    }
}
fn combined_raw(
    inbox: u64,
    reply: u64,
    version: u32,
    op: u32,
    sequence: u64,
    first: u32,
    second: u32,
    extra: &[u8],
    reply_version: u32,
) -> Result<[u32; 5], i32> {
    let mut req = std::vec::Vec::new();
    req.extend(u32::to_le_bytes(version));
    req.extend(u32::to_le_bytes(op));
    req.extend(u64::to_le_bytes(reply));
    req.extend(u64::to_le_bytes(sequence));
    req.extend(u32::to_le_bytes(first));
    req.extend(u32::to_le_bytes(second));
    req.extend(extra);
    gamepkg::local_mailbox_send(inbox, &req)?;
    let mut out = [0u8; 32];
    if combined_receive(reply, &mut out)? != 32
        || combined_word(&out, 0)? != reply_version
        || combined_long(&out, 8)? != sequence
    {
        return Err(-3);
    }
    Ok([
        combined_word(&out, 4)?,
        combined_word(&out, 16)?,
        combined_word(&out, 20)?,
        combined_word(&out, 24)?,
        combined_word(&out, 28)?,
    ])
}
fn combined_protocol(
    owner: &mut heap_service::HeapClient,
    inbox: u64,
    window: u64,
) -> Result<(), i32> {
    let reply = gamepkg::local_mailbox_create(32, 1)?;
    if combined_raw(inbox, reply, 1, 7, 1, 0, 0, &[], 1)?[0] != 0 {
        return Err(-3);
    }
    let denied = combined_raw(
        inbox,
        reply,
        2,
        8,
        2,
        window as u32,
        (window >> 32) as u32,
        &[],
        2,
    )?;
    if denied[0] as i32 != -4 {
        return Err(-3);
    }
    // v1 unknown operations still consume a correctly framed sequence.
    if combined_raw(inbox, reply, 1, 9, 3, 4096, 4096, &[], 1)?[0] as i32 != -1 {
        return Err(-3);
    }
    // Bad v2 framing/version and sequence gaps never advance membership state.
    if combined_raw(inbox, reply, 2, 9, 4, 4096, 4096, &[], 1)?[0] as i32 != -1 {
        return Err(-3);
    }
    let mut range = std::vec::Vec::new();
    range.extend(u32::to_le_bytes(0));
    range.extend(u32::to_le_bytes(u32::MAX));
    let mut long = std::vec::Vec::new();
    long.extend(&range);
    long.extend(u32::to_le_bytes(0));
    if combined_raw(inbox, reply, 2, 9, 4, 4096, 4096, &long, 1)?[0] as i32 != -1
        || combined_raw(inbox, reply, 3, 9, 4, 4096, 4096, &range, 1)?[0] as i32 != -1
        || combined_raw(inbox, reply, 2, 9, 5, 4096, 4096, &range, 2)?[0] as i32 != -1
    {
        return Err(-3);
    }
    let before = owner.heap_contiguous_counter(3)?;
    let allocated = combined_raw(inbox, reply, 2, 9, 4, 4096, 4096, &range, 2)?;
    if allocated[0] != 0 || allocated[1] == 0 {
        return Err(-3);
    }
    if combined_raw(inbox, reply, 2, 9, 4, 4096, 4096, &range, 2)?[0] as i32 != -1
        || owner.heap_contiguous_counter(3)? != before + 1
    {
        return Err(-3);
    }
    if combined_raw(inbox, reply, 2, 11, 5, allocated[1], 0, &[], 2)?[..2] != [0, 1]
        || combined_raw(inbox, reply, 2, 11, 6, allocated[1], 1, &[], 2)?[0] as i32 != -1
    {
        return Err(-3);
    }
    let ordinary_response = combined_raw(inbox, reply, 1, 1, 7, 64, 16, &[], 1)?;
    if ordinary_response[0] != 0 || ordinary_response[1] == 0 {
        return Err(-3);
    }
    if combined_raw(inbox, reply, 2, 14, 8, ordinary_response[1], 0, &[], 2)?[..2] != [0, 0]
        || combined_raw(
            inbox,
            reply,
            2,
            19,
            9,
            window as u32,
            (window >> 32) as u32,
            &[],
            2,
        )?[0] as i32
            != -4
        || combined_raw(
            inbox,
            reply,
            1,
            19,
            10,
            window as u32,
            (window >> 32) as u32,
            &[],
            1,
        )?[0] as i32
            != -1
        || combined_raw(inbox, reply, 1, 6, 11, 0, 0, &[], 1)?[..2] != [0, 0]
    {
        return Err(-3);
    }
    if owner.heap_query(ordinary_response[1])? != 0
        || owner.heap_contiguous_query(allocated[1])? != 0
    {
        return Err(-3);
    }
    gamepkg::local_mailbox_release(reply)?;
    Ok(())
}
fn combined_mode(window: u64, mode: u32) -> Result<(), i32> {
    let mut req = std::vec::Vec::new();
    req.extend(u64::to_le_bytes(window));
    req.extend(u64::to_le_bytes(8192));
    req.extend(u64::to_le_bytes(4096));
    req.extend(u32::to_le_bytes(mode));
    gamepkg::local_call(3, &req, &mut [])?;
    Ok(())
}
fn combined_ranges(owner: &mut heap_service::HeapClient, window: u64) -> Result<(), i32> {
    gamepkg::local_memory_fill(window, 0x3000000, 12288, 0xa5)?;
    owner.heap_contiguous_clear_claim(0x83000003, 4096)?;
    if owner.heap_contiguous_query(0x83000003)? != 8189
        || owner.heap_contiguous_pages(0x3000, 2)? != [1, 1, 0, 0]
        || owner.heap_contiguous_length(0x3000)? != 2
    {
        return Err(-3);
    }
    let mut bytes = [0u8; 8];
    gamepkg::local_memory_read_segments(&[[window, 0x3000000, 8]], &mut bytes)?;
    if bytes != [0xa5, 0xa5, 0xa5, 0, 0, 0, 0, 0] {
        return Err(-3);
    }
    gamepkg::local_memory_read_segments(&[[window, 0x3000fff, 8]], &mut bytes)?;
    if bytes != [0, 0, 0, 0, 0xa5, 0xa5, 0xa5, 0xa5] {
        return Err(-3);
    }
    owner.heap_allocation_release(0x83000000)?;
    let before = owner.heap_contiguous_counter(3)?;
    gamepkg::local_memory_fill(window, 4096, 8192, 0x7b)?;
    combined_mode(window, 1)?;
    if owner.heap_contiguous_clear_claim(0x80001000, 8192) != Err(-4)
        || owner.heap_contiguous_allocate(8192, 4096, 0, u32::MAX) != Err(-4)
        || owner.heap_contiguous_counter(3)? != before
    {
        return Err(-3);
    }
    gamepkg::local_memory_read_segments(&[[window, 4096, 8]], &mut bytes)?;
    if bytes != [0x7b; 8] {
        return Err(-3);
    }
    combined_mode(window, 2)?;
    if owner.heap_contiguous_clear_claim(0x7fffffff, 1) != Err(-1)
        || owner.heap_contiguous_clear_claim(0x80001000, u32::MAX) != Err(-1)
        || owner.heap_contiguous_counter(6) != Err(-1)
        || owner.heap_contiguous_pages(0, 0) != Err(-1)
        || owner.heap_contiguous_pages(16383, 2) != Err(-1)
        || owner.heap_contiguous_length(16384) != Err(-1)
    {
        return Err(-3);
    }
    Ok(())
}
pub fn combined_client(setup: u64) -> u64 {
    match combined_client_run(setup) {
        Ok(value) => value,
        Err(code) => code as i64 as u64,
    }
}
fn combined_client_run(setup: u64) -> Result<u64, i32> {
    let mut bytes = [0u8; 48];
    if combined_receive(setup, &mut bytes)? != 48 {
        return Err(-3);
    }
    let inbox = combined_long(&bytes, 8)?;
    let job = combined_long(&bytes, 16)?;
    let main = combined_long(&bytes, 24)?;
    let window = combined_long(&bytes, 32)?;
    let tag = combined_long(&bytes, 40)?;
    if !(1..=4).contains(&tag) {
        return Err(-1);
    }
    let mut client = heap_service::HeapClient::heap_client(inbox, job)?;
    client.heap_register()?;
    if client.heap_contiguous_initialize(window) != Err(-4) {
        return Err(-3);
    }
    let fixed = 0x83fe0000 + tag as u32 * 8192;
    client.heap_contiguous_clear_claim(fixed, 8192)?;
    for _ in 0..250 {
        let ordinary = client.heap_alloc(64, 16)?;
        let address = client.heap_contiguous_allocate(4097, 4096, 0, u32::MAX)?;
        if ordinary == 0 || address == 0 || address == fixed {
            return Err(-3);
        }
        gamepkg::local_memory_fill(main, u64::from(ordinary), 64, tag as u8)?;
        gamepkg::local_memory_fill(window, u64::from(address - 0x80000000), 64, tag as u8)?;
        gamepkg::local_memory_fill(window, u64::from(fixed - 0x80000000), 64, tag as u8)?;
        if client.heap_allocation_query(ordinary + 12)? < 52
            || client.heap_allocation_query(address + 12)? != 8180
            || client.heap_allocation_query(fixed + 12)? != 8180
        {
            return Err(-3);
        }
        let mut held = [0u8; 64];
        for span in [
            [main, u64::from(ordinary), 64],
            [window, u64::from(address - 0x80000000), 64],
            [window, u64::from(fixed - 0x80000000), 64],
        ] {
            gamepkg::local_memory_read_segments(&[span], &mut held)?;
            if held != [tag as u8; 64] {
                return Err(-3);
            }
        }
        client.heap_allocation_release(ordinary)?;
        client.heap_allocation_release(address)?;
        // Ownership ended: another caller may immediately reuse either
        // address. The supervisor checks exact counts after all callers join.
    }
    client.heap_allocation_release(fixed)?;
    client.heap_unregister()?;
    client.heap_abandon()?;
    Ok((tag << 32) | 250)
}
fn combined_threads(
    owner: &mut heap_service::HeapClient,
    record: u64,
    inbox: u64,
    job: u64,
    main: u64,
    window: u64,
) -> Result<(), i32> {
    let allocations = owner.heap_contiguous_counter(3)?;
    let frees = owner.heap_contiguous_counter(4)?;
    let setup = gamepkg::local_mailbox_create(48, 4)?;
    let mut jobs = [0u64; 4];
    for number in 0..4usize {
        let mut bytes = std::vec::Vec::new();
        for value in [record, inbox, job, main, window, number as u64 + 1] {
            bytes.extend(u64::to_le_bytes(value));
        }
        gamepkg::local_mailbox_send(setup, &bytes)?;
        jobs[number] = combined_spawn(1, setup)?;
    }
    let mut seen = 0u32;
    for job in jobs {
        let result = combined_join(job)?;
        let tag = (result >> 32) as u32;
        if !(1..=4).contains(&tag) || result as u32 != 250 || seen & (1 << tag) != 0 {
            return Err(-3);
        }
        seen |= 1 << tag;
    }
    gamepkg::local_mailbox_release(setup)?;
    if seen != 30
        || owner.heap_contiguous_counter(0)? != 0
        || owner.heap_contiguous_counter(3)? != allocations + 1004
        || owner.heap_contiguous_counter(4)? != frees + 1004
    {
        return Err(-3);
    }
    Ok(())
}
pub fn combined_fake(inbox: u64) -> u64 {
    match combined_fake_run(inbox) {
        Ok(()) => 0,
        Err(code) => code as i64 as u64,
    }
}
fn combined_fake_run(inbox: u64) -> Result<(), i32> {
    let mut request = [0u8; 48];
    if combined_receive(inbox, &mut request)? != 48 {
        return Err(-3);
    }
    let reply = combined_long(&request, 8)?;
    let mode = combined_word(&request, 24)? & 255;
    let mut output = [0u8; 32];
    output[..4].copy_from_slice(&u32::to_le_bytes(1));
    output[8..16].copy_from_slice(&u64::to_le_bytes(1));
    gamepkg::local_mailbox_send(reply, &output)?;
    if combined_receive(inbox, &mut request)? != 32
        || combined_word(&request, 0)? != 2
        || combined_word(&request, 4)? != 8
    {
        return Err(-3);
    }
    output[..4].copy_from_slice(&u32::to_le_bytes(2));
    output[8..16].copy_from_slice(&u64::to_le_bytes(2));
    gamepkg::local_mailbox_send(reply, &output)?;
    let expected = if mode == 4 { 32 } else { 40 };
    if combined_receive(inbox, &mut request)? != expected || combined_word(&request, 0)? != 2 {
        return Err(-3);
    }
    if mode == 6 {
        let accepted = u64::from(combined_word(&request, 32)?)
            | (u64::from(combined_word(&request, 36)?) << 32);
        gamepkg::local_mailbox_send(accepted, &[1])?;
        if gamepkg::local_mailbox_receive(inbox, 1000, &mut request) != Err(-2) {
            return Err(-3);
        }
        gamepkg::local_mailbox_release(inbox)?;
        return Ok(());
    }
    output[8..16].copy_from_slice(&u64::to_le_bytes(3));
    output[16..20].copy_from_slice(&u32::to_le_bytes(0x80001000));
    if mode == 1 {
        output[..4].copy_from_slice(&u32::to_le_bytes(1));
    }
    if mode == 2 {
        output[8..16].copy_from_slice(&u64::to_le_bytes(4));
    }
    if mode == 4 {
        output[16..20].copy_from_slice(&u32::to_le_bytes(2));
    }
    if mode == 5 {
        output[4..8].copy_from_slice(&i32::to_le_bytes(-6));
    }
    gamepkg::local_mailbox_send(reply, &output[..if mode == 3 { 31 } else { 32 }])?;
    if gamepkg::local_mailbox_receive(inbox, 20, &mut request) != Err(-2) {
        return Err(-3);
    }
    gamepkg::local_mailbox_release(inbox)?;
    Ok(())
}
fn combined_peers(record: u64, main: u64, window: u64) -> Result<(), i32> {
    for mode in 1..=5u32 {
        let inbox = heap_service::heap_inbox()?;
        let job = combined_spawn(2, inbox)?;
        let mut client = heap_service::HeapClient::heap_client(inbox, job)?;
        client.heap_initialize(record, main, 16777216 + mode, 67108864)?;
        client.heap_contiguous_initialize(window)?;
        let result = if mode == 4 {
            match client.heap_contiguous_free(0x80001000) {
                Ok(value) => Ok(u32::from(value)),
                Err(code) => Err(code),
            }
        } else {
            client.heap_contiguous_allocate(4096, 4096, 0, u32::MAX)
        };
        if result != Err(-3) || client.heap_query(0) != Err(-4) {
            return Err(-3);
        }
        client.heap_abandon()?;
        if combined_join(job)? != 0 {
            return Err(-3);
        }
    }
    Ok(())
}
pub fn combined_pending(setup: u64) -> u64 {
    match combined_pending_run(setup) {
        Ok(()) => 0,
        Err(code) => code as i64 as u64,
    }
}
fn combined_pending_run(setup: u64) -> Result<(), i32> {
    let mut bytes = [0u8; 48];
    if combined_receive(setup, &mut bytes)? != 48 {
        return Err(-3);
    }
    let mut client = heap_service::HeapClient::heap_client(
        combined_long(&bytes, 0)?,
        combined_long(&bytes, 8)?,
    )?;
    client.heap_initialize(
        combined_long(&bytes, 16)?,
        combined_long(&bytes, 24)?,
        16777222,
        67108864,
    )?;
    client.heap_contiguous_initialize(combined_long(&bytes, 32)?)?;
    let accepted = combined_long(&bytes, 40)?;
    if client.heap_contiguous_allocate(4096, 4096, accepted as u32, (accepted >> 32) as u32)
        != Err(-4)
        || client.heap_contiguous_query(0) != Err(-4)
    {
        return Err(-3);
    }
    client.heap_abandon()?;
    Ok(())
}
fn combined_cancel_pending(record: u64, main: u64, window: u64) -> Result<(), i32> {
    let inbox = heap_service::heap_inbox()?;
    let peer = combined_spawn(2, inbox)?;
    let accepted = gamepkg::local_mailbox_create(1, 1)?;
    let setup = gamepkg::local_mailbox_create(48, 1)?;
    let mut bytes = std::vec::Vec::new();
    for value in [inbox, peer, record, main, window, accepted] {
        bytes.extend(u64::to_le_bytes(value));
    }
    gamepkg::local_mailbox_send(setup, &bytes)?;
    let caller = combined_spawn(3, setup)?;
    let mut marker = [0u8; 1];
    if combined_receive(accepted, &mut marker)? != 1 || marker != [1] {
        return Err(-3);
    }
    if gamepkg::local_call(12, &u64::to_le_bytes(caller), &mut [])? != 0 {
        return Err(-3);
    }
    if combined_join(caller)? != 0 || combined_join(peer)? != 0 {
        return Err(-3);
    }
    gamepkg::local_mailbox_release(accepted)?;
    gamepkg::local_mailbox_release(setup)?;
    Ok(())
}
pub fn combined_verify() -> Result<u64, i32> {
    let mut banks = [0u8; 48];
    if gamepkg::local_call(1, &[], &mut banks)? != 48 {
        return Err(-3);
    }
    let main = combined_long(&banks, 0)?;
    let window = combined_long(&banks, 16)?;
    for id in [main, window] {
        let mut req = std::vec::Vec::new();
        req.extend(u64::to_le_bytes(id));
        req.extend(u64::to_le_bytes(0));
        req.extend(u64::to_le_bytes(67108864));
        req.extend(u32::to_le_bytes(2));
        gamepkg::local_call(2, &req, &mut [])?;
    }
    let ram = memory::Memory::new()?;
    let inbox = heap_service::heap_inbox()?;
    let job = combined_spawn(0, inbox)?;
    let mut owner = heap_service::HeapClient::heap_client(inbox, job)?;
    owner.heap_initialize(ram.call_handle(), main, 16777216, 67108864)?;
    if owner.heap_contiguous_allocate(1, 4096, 0, u32::MAX) != Err(-5)
        || owner.heap_contiguous_initialize(main) != Err(-4)
        || owner.heap_allocation_query(0) != Err(-5)
    {
        return Err(-3);
    }
    owner.heap_contiguous_initialize(window)?;
    if owner.heap_contiguous_initialize(window) != Err(-1) {
        return Err(-3);
    }
    gamepkg::diagnostic(0, 0x544b0900, &[1])?;
    combined_protocol(&mut owner, inbox, window)?;
    gamepkg::diagnostic(0, 0x544b0900, &[2])?;
    combined_ranges(&mut owner, window)?;
    gamepkg::diagnostic(0, 0x544b0900, &[3])?;
    combined_threads(&mut owner, ram.call_handle(), inbox, job, main, window)?;
    gamepkg::diagnostic(0, 0x544b0900, &[4])?;
    gamepkg::local_record_release(ram.call_handle())?;
    if owner.heap_contiguous_allocate(1, 4096, 0, u32::MAX) != Err(-4)
        || owner.heap_allocation_query(0) != Err(-4)
        || owner.heap_contiguous_counter(0) != Err(-4)
        || owner.heap_contiguous_pages(0, 1) != Err(-4)
    {
        return Err(-3);
    }
    owner.heap_stop()?;
    owner.heap_abandon()?;
    if combined_join(job)? != 0 {
        return Err(-3);
    }
    gamepkg::diagnostic(0, 0x544b0900, &[5])?;
    let fresh = memory::Memory::new()?;
    combined_peers(fresh.call_handle(), main, window)?;
    combined_cancel_pending(fresh.call_handle(), main, window)?;
    gamepkg::local_record_release(fresh.call_handle())?;
    gamepkg::diagnostic(0, 0x544b0900, &[6])?;
    Ok(1000)
}
