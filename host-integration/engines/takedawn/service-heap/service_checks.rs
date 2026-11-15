// Ordinary game Rust used in both confined and direct-runtime qualification.
use crate::gamepkg;
use crate::memory;
use crate::service;
fn check_word(bytes: &[u8], at: usize) -> Result<u32, i32> {
    let b = bytes.get(at..at + 4).ok_or(-1)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}
fn check_long(bytes: &[u8], at: usize) -> Result<u64, i32> {
    let b = bytes.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}
fn check_spawn(entry: u32, arg: u64, suspended: bool) -> Result<u64, i32> {
    let mut bytes = std::vec::Vec::new();
    bytes.extend(u32::to_le_bytes(entry));
    bytes.extend(u64::to_le_bytes(arg));
    bytes.extend(u32::to_le_bytes(u32::from(suspended)));
    let mut out = [0u8; 8];
    if gamepkg::local_call(10, &bytes, &mut out)? != 8 {
        return Err(-3);
    }
    check_long(&out, 0)
}
fn check_join(job: u64) -> Result<u64, i32> {
    let pause = gamepkg::local_mailbox_create(1, 1)?;
    let mut byte = [0u8; 1];
    let mut out = [0u8; 8];
    loop {
        match gamepkg::local_call(11, &u64::to_le_bytes(job), &mut out) {
            Ok(8) => {
                gamepkg::local_mailbox_release(pause)?;
                return check_long(&out, 0);
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
fn check_cancel(job: u64) -> Result<(), i32> {
    if gamepkg::local_call(12, &u64::to_le_bytes(job), &mut [])? != 0 {
        return Err(-3);
    }
    Ok(())
}
fn check_frame(
    reply: u64,
    sequence: u64,
    operation: u32,
    first: u32,
    second: u32,
) -> std::vec::Vec<u8> {
    let mut b = std::vec::Vec::new();
    b.extend(u32::to_le_bytes(1));
    b.extend(u32::to_le_bytes(operation));
    b.extend(u64::to_le_bytes(reply));
    b.extend(u64::to_le_bytes(sequence));
    b.extend(u32::to_le_bytes(first));
    b.extend(u32::to_le_bytes(second));
    b
}
fn check_reply(channel: u64, sequence: u64, status: i32) -> Result<[u8; 32], i32> {
    let mut bytes = [0u8; 32];
    if gamepkg::local_mailbox_receive(channel, 1000, &mut bytes)? != 32
        || check_word(&bytes, 0)? != 1
        || check_long(&bytes, 8)? != sequence
        || check_word(&bytes, 4)? as i32 != status
    {
        return Err(-3);
    }
    Ok(bytes)
}
fn check_raw(inbox: u64) -> Result<(), i32> {
    let reply = gamepkg::local_mailbox_create(32, 1)?;
    gamepkg::local_mailbox_send(inbox, &check_frame(reply, 1, 7, u32::MAX, u32::MAX))?;
    check_reply(reply, 1, -4)?;
    gamepkg::local_mailbox_send(inbox, &check_frame(reply, 1, 7, 0, 0))?;
    check_reply(reply, 1, 0)?;
    gamepkg::local_mailbox_send(inbox, &check_frame(reply, 2, 1, 32, 4))?;
    let first = check_reply(reply, 2, 0)?;
    let address = check_word(&first, 16)?;
    // Duplicated and out-of-order commands cannot allocate twice.
    gamepkg::local_mailbox_send(inbox, &check_frame(reply, 2, 1, 64, 4))?;
    let duplicate = check_reply(reply, 2, -1)?;
    if first[20..] != duplicate[20..] {
        return Err(-3);
    }
    gamepkg::local_mailbox_send(inbox, &check_frame(reply, 4, 1, 64, 4))?;
    check_reply(reply, 4, -1)?;
    let mut malformed = check_frame(reply, 3, 3, address, 0);
    malformed[0] = 2;
    gamepkg::local_mailbox_send(inbox, &malformed)?;
    check_reply(reply, 3, -1)?;
    malformed = check_frame(reply, 3, 3, address, 0);
    malformed.extend([0u8]);
    gamepkg::local_mailbox_send(inbox, &malformed)?;
    check_reply(reply, 3, -1)?;
    gamepkg::local_mailbox_send(inbox, &check_frame(reply, 3, 3, address, 0))?;
    if check_word(&check_reply(reply, 3, 0)?, 16)? < 32 {
        return Err(-3);
    }
    gamepkg::local_mailbox_send(inbox, &check_frame(reply, 4, 99, 0, 0))?;
    check_reply(reply, 4, -1)?;
    gamepkg::local_mailbox_send(inbox, &check_frame(reply, 5, 2, address, 1))?;
    check_reply(reply, 5, -1)?;
    gamepkg::local_mailbox_send(inbox, &check_frame(reply, 6, 2, address, 0))?;
    check_reply(reply, 6, 0)?;
    gamepkg::local_mailbox_send(inbox, &check_frame(reply, 7, 6, 0, 0))?;
    let before = check_reply(reply, 7, 0)?;
    gamepkg::local_mailbox_send(inbox, &check_frame(reply, 8, 1, 64, 4))?;
    let after = check_reply(reply, 8, -4)?;
    if before[20..] != after[20..] {
        return Err(-3);
    }
    gamepkg::local_mailbox_release(reply)?;
    Ok(())
}
fn check_members(inbox: u64, job: u64) -> Result<(), i32> {
    let mut clients: [Option<service::HeapClient>; 11] = [
        None, None, None, None, None, None, None, None, None, None, None,
    ];
    for number in 0..11usize {
        let mut client = service::HeapClient::heap_client(inbox, job)?;
        client.heap_register()?;
        clients[number] = Some(client);
    }
    let mut excess = service::HeapClient::heap_client(inbox, job)?;
    if excess.heap_register() != Err(-2) {
        return Err(-3);
    }
    excess.heap_abandon()?;
    for slot in &mut clients {
        let client = slot.as_mut().ok_or(-3)?;
        client.heap_unregister()?;
        client.heap_abandon()?;
    }
    // Slot capacity is reclaimed; monotonic host queue IDs are not reused.
    let mut client = service::HeapClient::heap_client(inbox, job)?;
    client.heap_register()?;
    if client.heap_stop() != Err(-4) {
        return Err(-3);
    }
    client.heap_status()?;
    client.heap_unregister()?;
    client.heap_abandon()?;
    Ok(())
}
pub fn heap_test_client(setup: u64) -> u64 {
    match check_client_run(setup) {
        Ok(tag) => tag,
        Err(code) => code as i64 as u64,
    }
}
fn check_client_run(setup: u64) -> Result<u64, i32> {
    let mut cfg = [0u8; 40];
    if gamepkg::local_mailbox_receive(setup, 1000, &mut cfg)? != 40 {
        return Err(-3);
    }
    let inbox = check_long(&cfg, 0)?;
    let job = check_long(&cfg, 8)?;
    let record = check_long(&cfg, 16)?;
    let window = check_long(&cfg, 24)?;
    let tag = check_word(&cfg, 32)?;
    if !(1..=4).contains(&tag) || check_word(&cfg, 36)? != 500 {
        return Err(-1);
    }
    let mut client = service::HeapClient::heap_client(inbox, job)?;
    client.heap_register()?;
    let mut ram = memory::Memory::with_call_handle(record)?;
    ram.attach(0, 67108864, window, 0, true)?;
    let pause = gamepkg::local_mailbox_create(1, 1)?;
    let mut one = [0u8; 1];
    for turn in 0..500u32 {
        let address = client.heap_alloc(64, 64)?;
        if address == 0 {
            return Err(-3);
        }
        let size = client.heap_query(address)?;
        if size < 64 || client.heap_query(address + 16)? != size - 16 {
            return Err(-3);
        }
        let mut bytes = [0u8; 64];
        ram.read_bytes(address, &mut bytes)?;
        if bytes != [0; 64] {
            return Err(-3);
        }
        let value = ((tag as u8).wrapping_add(turn as u8)) | 1;
        ram.fill(address, value, 64)?;
        if gamepkg::local_mailbox_receive(pause, 1, &mut one) != Err(-2) {
            return Err(-3);
        }
        ram.read_bytes(address, &mut bytes)?;
        if bytes != [value; 64] {
            return Err(-3);
        }
        client.heap_release_block(address)?;
    }
    gamepkg::local_mailbox_release(pause)?;
    client.heap_unregister()?;
    client.heap_abandon()?;
    Ok((u64::from(tag) << 32) | 500)
}
fn check_concurrent(inbox: u64, job: u64, record: u64, window: u64) -> Result<(), i32> {
    let setup = gamepkg::local_mailbox_create(40, 4)?;
    let mut jobs = [0u64; 4];
    for number in 0..4usize {
        let mut cfg = std::vec::Vec::new();
        for value in [inbox, job, record, window] {
            cfg.extend(u64::to_le_bytes(value));
        }
        cfg.extend(u32::to_le_bytes(number as u32 + 1));
        cfg.extend(u32::to_le_bytes(500));
        gamepkg::local_mailbox_send(setup, &cfg)?;
        jobs[number] = check_spawn(1, setup, false)?;
    }
    let mut tags = 0u32;
    for job in jobs {
        let result = check_join(job)?;
        let tag = (result >> 32) as u32;
        if result as u32 != 500 || !(1..=4).contains(&tag) || tags & (1 << tag) != 0 {
            return Err(-3);
        }
        tags |= 1 << tag;
    }
    if tags != 30 {
        return Err(-3);
    }
    gamepkg::local_mailbox_release(setup)?;
    Ok(())
}
pub fn heap_test_fake(inbox: u64) -> u64 {
    match check_fake_run(inbox) {
        Ok(()) => 0,
        Err(code) => code as i64 as u64,
    }
}
fn check_fake_run(inbox: u64) -> Result<(), i32> {
    let mut bytes = [0u8; 48];
    if gamepkg::local_mailbox_receive(inbox, 1000, &mut bytes)? != 48 {
        return Err(-3);
    }
    let reply = check_long(&bytes, 8)?;
    let mode = check_word(&bytes, 24)?;
    let mut out = [0u8; 32];
    out[..4].copy_from_slice(&u32::to_le_bytes(1));
    out[8..16].copy_from_slice(&u64::to_le_bytes(1));
    if mode == 1 {
        out[8] = 2;
    }
    if mode == 2 {
        out[0] = 2;
    }
    if mode == 4 {
        out[4..8].copy_from_slice(&i32::to_le_bytes(-100));
    }
    if mode != 5 {
        gamepkg::local_mailbox_send(reply, if mode == 3 { &out[..31] } else { &out })?;
    }
    gamepkg::local_mailbox_release(inbox)?;
    Ok(())
}
fn check_failed_responses(record: u64, window: u64) -> Result<(), i32> {
    for mode in 1..=5u32 {
        let inbox = service::heap_inbox()?;
        let job = check_spawn(2, inbox, false)?;
        let mut client = service::HeapClient::heap_client(inbox, job)?;
        if client.heap_initialize(record, window, mode, 0) != Err(-3)
            || client.heap_alloc(16, 4) != Err(-4)
        {
            return Err(-3);
        }
        client.heap_abandon()?;
        if check_join(job)? != 0 {
            return Err(-3);
        }
    }
    Ok(())
}
fn check_bad_initialization(record: u64, window: u64) -> Result<(), i32> {
    for (selected_record, selected_window, base, error) in [
        (record, window, 0u32, -1i32),
        (record, window + 1000000, 65536, -4),
    ] {
        let inbox = service::heap_inbox()?;
        let job = check_spawn(0, inbox, false)?;
        let mut client = service::HeapClient::heap_client(inbox, job)?;
        if client.heap_initialize(selected_record, selected_window, base, 67108864) != Err(error) {
            return Err(-3);
        }
        if check_join(job)? != error as i64 as u64 {
            return Err(-3);
        }
        client.heap_abandon()?;
    }
    Ok(())
}
fn check_revocation(window: u64) -> Result<(), i32> {
    let closed = memory::Memory::new()?;
    let record = closed.call_handle();
    let inbox = service::heap_inbox()?;
    let job = check_spawn(0, inbox, false)?;
    let mut client = service::HeapClient::heap_client(inbox, job)?;
    client.heap_initialize(record, window, 0x1000000, 67108864)?;
    let before = client.heap_status()?;
    gamepkg::local_record_release(record)?;
    if client.heap_alloc(64, 4) != Err(-4)
        || client.heap_release_block(0x1000000) != Err(-4)
        || client.heap_query(0x1000000) != Err(-4)
        || client.heap_status() != Err(-4)
    {
        return Err(-3);
    }
    // Graceful cleanup must not require revoked RAM authority.
    client.heap_stop()?;
    client.heap_abandon()?;
    if check_join(job)? != 0 || before != [0x1000000, 0, 0] {
        return Err(-3);
    }
    let inbox = service::heap_inbox()?;
    let job = check_spawn(0, inbox, false)?;
    let mut client = service::HeapClient::heap_client(inbox, job)?;
    if client.heap_initialize(record, window, 0x1000000, 67108864) != Err(-4) {
        return Err(-3);
    }
    client.heap_abandon()?;
    if check_join(job)? != (-4i64) as u64 {
        return Err(-3);
    }
    Ok(())
}
fn check_actor_cancel(record: u64, window: u64) -> Result<(), i32> {
    let inbox = service::heap_inbox()?;
    let job = check_spawn(0, inbox, false)?;
    let mut client = service::HeapClient::heap_client(inbox, job)?;
    client.heap_initialize(record, window, 0x1000000, 67108864)?;
    check_cancel(job)?;
    if check_join(job)? != (-4i64) as u64 || client.heap_alloc(64, 4) != Err(-4) {
        return Err(-3);
    }
    client.heap_abandon()?;
    Ok(())
}

pub fn heap_test_waiting_peer(setup: u64) -> u64 {
    match check_waiting_peer(setup) {
        Ok(()) => 0,
        Err(code) => code as i64 as u64,
    }
}
fn check_waiting_peer(setup: u64) -> Result<(), i32> {
    let mut config = [0u8; 16];
    if gamepkg::local_mailbox_receive(setup, 1000, &mut config)? != 16 {
        return Err(-3);
    }
    let inbox = check_long(&config, 0)?;
    let ready = check_long(&config, 8)?;
    let mut request = [0u8; 48];
    if gamepkg::local_mailbox_receive(inbox, 1000, &mut request)? != 48 {
        return Err(-3);
    }
    gamepkg::local_mailbox_send(ready, &[])?;
    loop {
        if gamepkg::local_task_status()?.1 {
            gamepkg::local_mailbox_release(inbox)?;
            return Err(-4);
        }
        match gamepkg::local_mailbox_receive(inbox, 1000, &mut request) {
            Err(-2) => {}
            Err(-4) => {
                gamepkg::local_mailbox_release(inbox)?;
                return Err(-4);
            }
            _ => return Err(-3),
        }
    }
}
pub fn heap_test_cancelled_caller(setup: u64) -> u64 {
    match check_cancelled_caller(setup) {
        Ok(()) => 0,
        Err(code) => code as i64 as u64,
    }
}
fn check_cancelled_caller(setup: u64) -> Result<(), i32> {
    let mut config = [0u8; 32];
    if gamepkg::local_mailbox_receive(setup, 1000, &mut config)? != 32 {
        return Err(-3);
    }
    let mut client =
        service::HeapClient::heap_client(check_long(&config, 0)?, check_long(&config, 8)?)?;
    let result = client.heap_initialize(
        check_long(&config, 16)?,
        check_long(&config, 24)?,
        0x1000000,
        67108864,
    );
    if result != Err(-4) || client.heap_alloc(16, 4) != Err(-4) {
        return Err(-3);
    }
    client.heap_abandon()?;
    Err(-4)
}
fn check_pending_cancellation(record: u64, window: u64) -> Result<(), i32> {
    let inbox = service::heap_inbox()?;
    let ready = gamepkg::local_mailbox_create(1, 1)?;
    let peer_setup = gamepkg::local_mailbox_create(16, 1)?;
    let caller_setup = gamepkg::local_mailbox_create(32, 1)?;
    let mut config = std::vec::Vec::new();
    config.extend(u64::to_le_bytes(inbox));
    config.extend(u64::to_le_bytes(ready));
    gamepkg::local_mailbox_send(peer_setup, &config)?;
    let peer = check_spawn(3, peer_setup, false)?;
    config.clear();
    for value in [inbox, peer, record, window] {
        config.extend(u64::to_le_bytes(value));
    }
    gamepkg::local_mailbox_send(caller_setup, &config)?;
    let caller = check_spawn(4, caller_setup, false)?;
    let mut byte = [0u8; 1];
    if gamepkg::local_mailbox_receive(ready, 1000, &mut byte)? != 0 {
        return Err(-3);
    }
    // The peer has consumed the request and deliberately sends no response.
    // Cancellation must wake the caller without replaying that accepted call.
    check_cancel(caller)?;
    if check_join(caller)? != (-4i64) as u64 {
        return Err(-3);
    }
    check_cancel(peer)?;
    if check_join(peer)? != (-4i64) as u64 {
        return Err(-3);
    }
    for queue in [ready, peer_setup, caller_setup] {
        gamepkg::local_mailbox_release(queue)?;
    }
    Ok(())
}

pub fn heap_test_abandoned_client(setup: u64) -> u64 {
    match check_abandoned_client(setup) {
        Ok(()) => 0,
        Err(code) => code as i64 as u64,
    }
}
fn check_abandoned_client(setup: u64) -> Result<(), i32> {
    let mut config = [0u8; 16];
    if gamepkg::local_mailbox_receive(setup, 1000, &mut config)? != 16 {
        return Err(-3);
    }
    let mut client =
        service::HeapClient::heap_client(check_long(&config, 0)?, check_long(&config, 8)?)?;
    client.heap_register()?;
    let address = client.heap_alloc(16, 4)?;
    if address == 0 {
        return Err(-3);
    }
    client.heap_release_block(address)?;
    // Deliberately exit without unregister/release: the next registration must
    // reclaim this finished task's membership and response queue.
    Ok(())
}
fn check_reclaimed_clients(inbox: u64, job: u64) -> Result<(), i32> {
    let setup = gamepkg::local_mailbox_create(16, 1)?;
    for _ in 0..24 {
        let mut config = std::vec::Vec::new();
        config.extend(u64::to_le_bytes(inbox));
        config.extend(u64::to_le_bytes(job));
        gamepkg::local_mailbox_send(setup, &config)?;
        let client = check_spawn(5, setup, false)?;
        if check_join(client)? != 0 {
            return Err(-3);
        }
    }
    let mut cleanup = service::HeapClient::heap_client(inbox, job)?;
    cleanup.heap_register()?;
    cleanup.heap_unregister()?;
    cleanup.heap_abandon()?;
    gamepkg::local_mailbox_release(setup)?;
    Ok(())
}
fn check_fatal_frames(record: u64, window: u64) -> Result<(), i32> {
    for mode in 0..2u32 {
        let inbox = service::heap_inbox()?;
        let job = check_spawn(0, inbox, false)?;
        let mut client = service::HeapClient::heap_client(inbox, job)?;
        client.heap_initialize(record, window, 0x1000000, 67108864)?;
        if mode == 0 {
            gamepkg::local_mailbox_send(inbox, &[])?;
        } else {
            let closed = gamepkg::local_mailbox_create(32, 1)?;
            gamepkg::local_mailbox_release(closed)?;
            gamepkg::local_mailbox_send(inbox, &check_frame(closed, 1, 7, 0, 0))?;
        }
        let expected = if mode == 0 { -1i64 } else { -4i64 };
        if check_join(job)? != expected as u64 || client.heap_status() != Err(-4) {
            return Err(-3);
        }
        client.heap_abandon()?;
        client.heap_abandon()?;
    }
    Ok(())
}
pub fn heap_service_verify() -> Result<u64, i32> {
    let mut windows = [0u8; 16];
    if gamepkg::local_call(1, &[], &mut windows)? != 16 {
        return Err(-3);
    }
    let window = check_long(&windows, 0)?;
    let mut commit = std::vec::Vec::new();
    commit.extend(u64::to_le_bytes(window));
    commit.extend(u64::to_le_bytes(0));
    commit.extend(u64::to_le_bytes(67108864));
    commit.extend(u32::to_le_bytes(2));
    gamepkg::local_call(2, &commit, &mut [])?;
    let mut ram = memory::Memory::new()?;
    ram.attach(0, 67108864, window, 0, true)?;
    let record = ram.call_handle();
    let inbox = service::heap_inbox()?;
    let job = check_spawn(0, inbox, true)?;
    // A full input queue returns Busy before acceptance and retains sequence 1.
    for _ in 0..32 {
        gamepkg::local_mailbox_send(inbox, &[])?;
    }
    let mut client = service::HeapClient::heap_client(inbox, job)?;
    if client.heap_initialize(record, window, 0x1000000, 67108864) != Err(-2) {
        return Err(-3);
    }
    let mut frame = [0u8; 48];
    for _ in 0..32 {
        if gamepkg::local_mailbox_receive(inbox, 0, &mut frame)? != 0 {
            return Err(-3);
        }
    }
    let mut resumed = [0u8; 4];
    if gamepkg::local_call(13, &u64::to_le_bytes(job), &mut resumed)? != 4
        || check_word(&resumed, 0)? != 1
    {
        return Err(-3);
    }
    client.heap_initialize(record, window, 0x1000000, 67108864)?;
    if client.heap_initialize(record, window, 0x1000000, 67108864) != Err(-1)
        || client.heap_unregister() != Err(-4)
    {
        return Err(-3);
    }
    let initial = client.heap_status()?;
    if initial != [0x1000000, 0, 0] {
        return Err(-3);
    }
    if client.heap_alloc(u32::MAX, 4) != Err(-1) || client.heap_status()? != initial {
        return Err(-3);
    }
    let a = client.heap_alloc(64, 4)?;
    let b = client.heap_alloc(64, 4)?;
    client.heap_release_block(a)?;
    client.heap_release_block(b)?;
    if client.heap_alloc(16, 4)? != a || client.heap_query(a)? != 128 {
        return Err(-3);
    }
    client.heap_release_block(a)?;
    check_raw(inbox)?;
    check_members(inbox, job)?;
    check_concurrent(inbox, job, record, window)?;
    check_reclaimed_clients(inbox, job)?;
    client.heap_stop()?;
    client.heap_abandon()?;
    if check_join(job)? != 0 {
        return Err(-3);
    }
    if gamepkg::local_mailbox_send(inbox, &[]) != Err(-4) {
        return Err(-3);
    }
    check_bad_initialization(record, window)?;
    check_failed_responses(record, window)?;
    check_revocation(window)?;
    check_actor_cancel(record, window)?;
    check_pending_cancellation(record, window)?;
    check_fatal_frames(record, window)?;
    Ok(2000)
}
