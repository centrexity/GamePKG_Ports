use crate::events;
use crate::gamepkg;
use crate::handles;
use crate::kernel_pool;
use crate::memory;
fn events_need(stage: i32, ok: bool) -> Result<(), i32> {
    if ok {
        Ok(())
    } else {
        Err(-stage)
    }
}
fn events_load(b: &[u8], at: usize) -> Result<u64, i32> {
    let p = b.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        p[0], p[1], p[2], p[3], p[4], p[5], p[6], p[7],
    ]))
}
fn events_commit_window(window: u64, bytes: u64) -> Result<(), i32> {
    gamepkg::local_call(
        2,
        &[
            u64::to_le_bytes(window).as_slice(),
            &u64::to_le_bytes(0),
            &u64::to_le_bytes(bytes),
            &u32::to_le_bytes(2),
        ]
        .concat(),
        &mut [],
    )?;
    Ok(())
}
pub fn events_probe() -> Result<std::vec::Vec<u8>, i32> {
    let mut ram = memory::Memory::new()?;
    let services = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2)?;
    let mut inventory = [0u8; 64];
    events_need(101, gamepkg::local_call(1, &[], &mut inventory)? == 64)?;
    let window = events_load(&inventory, 48)?;
    let main = events_load(&inventory, 0)?;
    events_commit_window(window, 1048576)?;
    events_commit_window(main, 4096)?;
    ram.attach(0x10000, 4096, main, 0, true)?;
    let mut map = handles::Handles::handles_new(&ram)?;
    map.handles_bind(&ram, window, 0)?;
    let mut domain = events::Events::events_new(&ram, &map)?;
    let mut borrowed = events::Events::events_borrow(&ram, &map, domain.events_id())?;
    events_need(102, borrowed.events_close(&ram, &map) == Err(-4))?;
    events_need(
        103,
        events::Events::events_borrow(&ram, &map, domain.events_id() | 0x100000000).is_err(),
    )?;
    let other = memory::Memory::new()?;
    events_need(
        104,
        events::Events::events_borrow(&other, &map, domain.events_id()).is_err(),
    )?;
    gamepkg::local_record_release(other.call_handle())?;
    let (_, pristine) = gamepkg::local_record_snapshot(domain.events_id(), 2112)?;
    for at in [0usize, 8, 16, 24, 32, 40, 48, 56, 64, 72] {
        let (version, mut bad) = gamepkg::local_record_snapshot(domain.events_id(), 2112)?;
        bad[at] ^= 0xff;
        gamepkg::local_record_compare_exchange(domain.events_id(), version, &bad)?;
        events_need(
            105,
            if at == 32 {
                domain.events_begin(&ram, &map).is_err()
            } else {
                events::Events::events_borrow(&ram, &map, domain.events_id()).is_err()
            },
        )?;
        let (version, _) = gamepkg::local_record_snapshot(domain.events_id(), 2112)?;
        gamepkg::local_record_compare_exchange(domain.events_id(), version, &pristine)?;
    }
    let mut output = std::vec::Vec::new();
    output.resize(48, 0);
    let mut cursor = 0usize;
    for event_type in [0u32, 1, 2, 0xffffffff] {
        for initial in [0u32, 1, 0x100, 0x101, 0xffffff00, 0xffffffff] {
            let mut incoming = borrowed.events_begin(&ram, &map)?;
            events_need(106, domain.events_close(&ram, &map) == Err(-2))?;
            incoming.event_create(&ram, event_type, initial)?;
            events_need(
                107,
                incoming.event_create(&ram, event_type, initial) == Err(-1),
            )?;
            let id = incoming.event_creation_id();
            let token = incoming.event_publish(&mut ram, 0x10000)?;
            events_need(
                108,
                token == 0x48000001
                    && ram.read_u32(0x10000)? == token
                    && incoming.event_creation_phase() == 4,
            )?;
            events_need(109, incoming.event_publish(&mut ram, 0x10000) == Err(-1))?;
            events_need(110, domain.events_close(&ram, &map) == Err(-2))?;
            incoming.event_creation_close(&ram)?;
            incoming.event_creation_close(&ram)?;
            events_need(
                111,
                incoming.event_creation_closed() && incoming.event_create(&ram, 0, 0) == Err(-4),
            )?;
            let state = borrowed.events_state(&ram, &map, token)?;
            events_need(112, state == [event_type == 0, initial & 255 != 0])?;
            output[cursor] = u8::from(state[0]);
            output[cursor + 1] = u8::from(state[1]);
            cursor += 2;
            let alias = domain.events_alias(&ram, &map, token, true)?;
            events_need(
                113,
                alias == 0x48000002 && domain.events_alias(&ram, &map, token, false)? == token,
            )?;
            domain.events_set(&ram, &map, alias, true)?;
            events_need(114, gamepkg::local_event_state(id)?.signaled)?;
            gamepkg::local_event_wait(id, 0)?;
            events_need(
                115,
                gamepkg::local_event_state(id)?.signaled == (event_type == 0),
            )?;
            domain.events_set(&ram, &map, token, false)?;
            events_need(116, !borrowed.events_state(&ram, &map, alias)?[1])?;
            borrowed.events_remove(&ram, &map, token)?;
            events_need(
                117,
                gamepkg::local_event_state(id).is_ok()
                    && domain.events_state(&ram, &map, token) == Err(-4),
            )?;
            domain.events_remove(&ram, &map, alias)?;
            events_need(
                118,
                gamepkg::local_event_state(id).err() == Some(-4)
                    && domain.events_remove(&ram, &map, alias) == Err(-4),
            )?;
        }
    }
    // Null output retains a native orphan without consuming the next guest tag.
    let mut orphan = domain.events_begin(&ram, &map)?;
    orphan.event_create(&ram, 0, 1)?;
    let orphan_id = orphan.event_creation_id();
    events_need(119, orphan.event_publish(&mut ram, 0)? == 0)?;
    orphan.event_creation_close(&ram)?;
    let mut bad_write = domain.events_begin(&ram, &map)?;
    bad_write.event_create(&ram, 1, 0)?;
    let bad_id = bad_write.event_creation_id();
    events_need(
        120,
        bad_write.event_publish(&mut ram, 0x90000000).is_err()
            && bad_write.event_creation_phase() == 3
            && bad_write.event_creation_token() == 0x48000001,
    )?;
    bad_write.event_creation_close(&ram)?;
    events_need(
        121,
        domain.events_state(&ram, &map, 0x48000001)? == [false, false],
    )?;
    domain.events_remove(&ram, &map, 0x48000001)?;
    events_need(122, gamepkg::local_event_state(bad_id).err() == Some(-4))?;
    // A metadata event ID outside the domain, or a different kind, is not owned.
    let mut foreign_bytes = [0u8; 8];
    gamepkg::local_call(20, &[0u8; 8], &mut foreign_bytes)?;
    let foreign = u64::from_le_bytes(foreign_bytes);
    let mut guard = map.handles_lock(&ram)?;
    let foreign_token = guard.handle_guard_insert(&ram, foreign, 1, false)?;
    let wrong_kind = guard.handle_guard_insert(&ram, foreign, 2, true)?;
    guard.handles_guard_close(&ram)?;
    events_need(
        123,
        domain.events_remove(&ram, &map, foreign_token) == Err(-4)
            && domain.events_set(&ram, &map, wrong_kind, true) == Err(-1),
    )?;
    events_need(124, gamepkg::local_event_state(foreign).is_ok())?;
    events_need(
        136,
        domain.events_state(&ram, &map, foreign_token) == Err(-4)
            && domain.events_set(&ram, &map, foreign_token, true) == Err(-4)
            && domain.events_alias(&ram, &map, foreign_token, true) == Err(-4),
    )?;
    let mut guard = map.handles_lock(&ram)?;
    guard.handle_guard_remove(&ram, foreign_token)?;
    guard.handle_guard_remove(&ram, wrong_kind)?;
    guard.handles_guard_close(&ram)?;
    gamepkg::local_call(25, &u64::to_le_bytes(foreign), &mut [])?;
    // Accepted events remain owned after KSV revocation; deletion lane closes them.
    events_parallel(&ram, &map, &domain)?;
    let mut untransferred = domain.events_begin(&ram, &map)?;
    untransferred.event_create(&ram, 0, 1)?;
    let pending = untransferred.event_creation_id();
    // One table mutex + 127 actual events exhaust the unchanged 128-sync budget.
    // There is already one orphan and this one private incoming event.
    let mut retained = std::vec::Vec::new();
    for _ in 0..125u32 {
        let mut item = domain.events_begin(&ram, &map)?;
        item.event_create(&ram, 0, 0)?;
        retained.push(item.event_creation_id());
        events_need(137, item.event_publish(&mut ram, 0)? == 0)?;
        item.event_creation_close(&ram)?;
    }
    let mut exhausted = domain.events_begin(&ram, &map)?;
    events_need(
        138,
        exhausted.event_create(&ram, 0, 0) == Err(-2) && exhausted.event_creation_id() == 0,
    )?;
    exhausted.event_creation_close(&ram)?;

    services.service_close(&ram)?;
    events_need(125, untransferred.event_publish(&mut ram, 0) == Err(-4))?;
    events_need(126, domain.events_close(&ram, &map) == Err(-2))?;
    untransferred.event_creation_close(&ram)?;
    events_need(
        127,
        gamepkg::local_event_state(pending).err() == Some(-4)
            && borrowed.events_begin(&ram, &map).is_err(),
    )?;
    events_need(
        128,
        domain.events_close(&ram, &map) == Err(-4) && domain.events_closed(),
    )?;
    events_need(
        129,
        domain.events_close(&ram, &map) == Err(-4)
            && gamepkg::local_event_state(orphan_id).err() == Some(-4),
    )?;
    events_need(
        130,
        map.handles_close(&ram) == Err(-4) && map.handles_closed(),
    )?;
    events_need(
        140,
        domain.events_close(&ram, &map) == Err(-4) && domain.events_closed(),
    )?;
    for id in retained {
        events_need(139, gamepkg::local_event_state(id).err() == Some(-4))?;
    }
    gamepkg::local_record_release(ram.call_handle())?;
    Ok(output)
}
fn events_parallel(
    ram: &memory::Memory,
    map: &handles::Handles,
    domain: &events::Events,
) -> Result<(), i32> {
    let bytes = [
        u64::to_le_bytes(ram.call_handle()).as_slice(),
        &u64::to_le_bytes(map.handles_id()),
        &u64::to_le_bytes(domain.events_id()),
    ]
    .concat();
    let argument = gamepkg::local_record_create(&bytes)?;
    let mut jobs = std::vec::Vec::new();
    for _ in 0..4u32 {
        let request = [
            u32::to_le_bytes(0).as_slice(),
            &u64::to_le_bytes(argument),
            &u32::to_le_bytes(0),
        ]
        .concat();
        let mut reply = [0u8; 8];
        gamepkg::local_call(10, &request, &mut reply)?;
        jobs.push(u64::from_le_bytes(reply));
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
        let result = proof.ok_or(-2)?.result();
        let token = result as u32;
        events_need(
            131,
            result == u64::from(token) && token >= 0x48000001 && token < 0x48000005,
        )?;
        for before in &tokens {
            events_need(132, *before != token)?;
        }
        tokens.push(token);
        events_need(133, domain.events_state(ram, map, token)? == [true, true])?;
    }
    for token in tokens {
        domain.events_remove(ram, map, token)?;
    }
    gamepkg::local_record_release(argument)?;
    Ok(())
}
fn events_actor(argument: u64) -> Result<u32, i32> {
    let (_, b) = gamepkg::local_record_snapshot(argument, 24)?;
    events_need(134, b.len() == 24)?;
    let mut ram = memory::Memory::with_call_handle(events_load(&b, 0)?)?;
    let map = handles::Handles::handles_borrow(&ram, events_load(&b, 8)?)?;
    let mut domain = events::Events::events_borrow(&ram, &map, events_load(&b, 16)?)?;
    events_need(135, domain.events_close(&ram, &map) == Err(-4))?;
    let mut incoming = domain.events_begin(&ram, &map)?;
    incoming.event_create(&ram, 0, 1)?;
    // Worker views do not attach private guest output spans; use a retained orphan,
    // then publish an alias by metadata only after its registered ownership exists.
    incoming.event_publish(&mut ram, 0)?;
    let id = incoming.event_creation_id();
    incoming.event_creation_close(&ram)?;
    let mut guard = map.handles_lock(&ram)?;
    let token = guard.handle_guard_insert(&ram, id, 1, false)?;
    guard.handles_guard_close(&ram)?;
    Ok(token)
}
pub fn event_worker(argument: u64) -> u64 {
    match events_actor(argument) {
        Ok(token) => u64::from(token),
        Err(code) => code as i64 as u64,
    }
}
