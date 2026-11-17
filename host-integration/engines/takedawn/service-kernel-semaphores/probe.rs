use crate::gamepkg;
use crate::handles;
use crate::kernel_pool;
use crate::memory;
use crate::semaphores;
fn semaphore_probe_need(stage: i32, value: bool) -> Result<(), i32> {
    if value {
        Ok(())
    } else {
        Err(-stage)
    }
}
fn semaphore_probe_word(b: &[u8], at: usize) -> Result<u64, i32> {
    let p = b.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        p[0], p[1], p[2], p[3], p[4], p[5], p[6], p[7],
    ]))
}
fn semaphore_probe_put(output: &mut [u8], cursor: &mut usize, value: u32) -> Result<(), i32> {
    let bytes = u32::to_le_bytes(value);
    let target = output.get_mut(*cursor..*cursor + 4).ok_or(-1)?;
    for lane in 0..4usize {
        target[lane] = bytes[lane];
    }
    *cursor += 4;
    Ok(())
}
fn semaphore_probe_poke(bytes: &mut [u8], at: usize, value: u64) -> Result<(), i32> {
    let target = bytes.get_mut(at..at + 8).ok_or(-1)?;
    let encoded = u64::to_le_bytes(value);
    for lane in 0..8usize {
        target[lane] = encoded[lane];
    }
    Ok(())
}
fn semaphore_probe_commit(window: u64, bytes: u64) -> Result<(), i32> {
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
fn semaphore_probe_created(
    ram: &mut memory::Memory,
    map: &handles::Handles,
    domain: &semaphores::Semaphores,
    initial: u32,
    maximum: u32,
    output: u32,
) -> Result<[u64; 2], i32> {
    let mut incoming = domain.semaphores_begin(ram, map)?;
    let result = incoming.semaphore_create(ram, initial, maximum);
    if let Err(code) = result {
        incoming.semaphore_creation_close(ram)?;
        return Err(code);
    }
    let id = incoming.semaphore_creation_id();
    let result = incoming.semaphore_publish(ram, output);
    let closed = incoming.semaphore_creation_close(ram);
    let token = result?;
    closed?;
    Ok([id, u64::from(token)])
}
pub fn semaphore_probe() -> Result<std::vec::Vec<u8>, i32> {
    let mut ram = memory::Memory::new()?;
    let services = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2)?;
    let mut inventory = [0u8; 64];
    semaphore_probe_need(501, gamepkg::local_call(1, &[], &mut inventory)? == 64)?;
    let main = semaphore_probe_word(&inventory, 0)?;
    let scratch = semaphore_probe_word(&inventory, 48)?;
    semaphore_probe_commit(main, 4096)?;
    semaphore_probe_commit(scratch, 1048576)?;
    ram.attach(0x10000, 4096, main, 0, true)?;
    let mut map = handles::Handles::handles_new(&ram)?;
    map.handles_bind(&ram, scratch, 0)?;
    let mut domain = semaphores::Semaphores::semaphores_new(&ram, &map)?;
    let mut borrowed =
        semaphores::Semaphores::semaphores_borrow(&ram, &map, domain.semaphores_id())?;
    semaphore_probe_need(502, borrowed.semaphores_close(&ram, &map) == Err(-4))?;
    let other = memory::Memory::new()?;
    semaphore_probe_need(
        503,
        semaphores::Semaphores::semaphores_borrow(&other, &map, domain.semaphores_id()).is_err(),
    )?;
    gamepkg::local_record_release(other.call_handle())?;
    semaphore_probe_need(
        504,
        semaphores::Semaphores::semaphores_borrow(&ram, &map, domain.semaphores_id() | 0x100000000)
            .is_err(),
    )?;
    let (_, pristine) = gamepkg::local_record_snapshot(domain.semaphores_id(), 2624)?;
    for at in [0usize, 8, 16, 24, 32, 40, 48, 56, 64, 72, 2112, 2120, 2616] {
        let (version, mut bad) = gamepkg::local_record_snapshot(domain.semaphores_id(), 2624)?;
        bad[at] ^= 255;
        gamepkg::local_record_compare_exchange(domain.semaphores_id(), version, &bad)?;
        semaphore_probe_need(
            505,
            if at == 32 {
                domain.semaphores_begin(&ram, &map).is_err()
            } else {
                semaphores::Semaphores::semaphores_borrow(&ram, &map, domain.semaphores_id())
                    .is_err()
            },
        )?;
        let (version, _) = gamepkg::local_record_snapshot(domain.semaphores_id(), 2624)?;
        gamepkg::local_record_compare_exchange(domain.semaphores_id(), version, &pristine)?;
    }
    let mut event_domain = kernel_pool::Events::events_new(&ram, &map)?;
    semaphore_probe_need(
        506,
        semaphores::Semaphores::semaphores_borrow(&ram, &map, event_domain.events_id()).is_err()
            && kernel_pool::Events::events_borrow(&ram, &map, domain.semaphores_id()).is_err(),
    )?;
    event_domain.events_close(&ram, &map)?;
    let mut output = std::vec::Vec::new();
    output.resize(1344, 0);
    let mut cursor = 0usize;
    for maximum in [1u32, 2, 4, 0x7fffffff] {
        for initial in [0u32, 1, maximum / 2, maximum] {
            ram.write_u32(0x10004, 0xabcdef01)?;
            let mut incoming = borrowed.semaphores_begin(&ram, &map)?;
            semaphore_probe_need(507, domain.semaphores_close(&ram, &map) == Err(-2))?;
            incoming.semaphore_create(&ram, initial, maximum)?;
            semaphore_probe_need(
                508,
                incoming.semaphore_create(&ram, initial, maximum) == Err(-1),
            )?;
            let id = incoming.semaphore_creation_id();
            let token = incoming.semaphore_publish(&mut ram, 0x10000)?;
            semaphore_probe_need(
                509,
                token == 0x48000001
                    && ram.read_u32(0x10000)? == token
                    && ram.read_u32(0x10004)? == 0xabcdef01
                    && incoming.semaphore_creation_phase() == 4,
            )?;
            semaphore_probe_need(
                510,
                incoming.semaphore_publish(&mut ram, 0x10000) == Err(-1)
                    && domain.semaphores_close(&ram, &map) == Err(-2),
            )?;
            incoming.semaphore_creation_close(&ram)?;
            incoming.semaphore_creation_close(&ram)?;
            semaphore_probe_need(
                511,
                incoming.semaphore_creation_closed()
                    && incoming.semaphore_create(&ram, 0, 1) == Err(-4),
            )?;
            semaphore_probe_need(
                512,
                domain.semaphores_state(&ram, &map, token)? == [initial, maximum],
            )?;
            semaphore_probe_put(&mut output, &mut cursor, token)?;
            semaphore_probe_put(&mut output, &mut cursor, initial)?;
            semaphore_probe_put(&mut output, &mut cursor, maximum)?;
            for permits in [0u32, 1, 2, 0x7fffffff, 0x80000000, 0xffffffff] {
                let result = domain.semaphores_release(&ram, &map, token, permits);
                let (success, previous) = match result {
                    Ok(previous) => (1u32, previous),
                    Err(-1) => (0u32, 0x55555555),
                    Err(code) => return Err(code),
                };
                semaphore_probe_put(&mut output, &mut cursor, success)?;
                semaphore_probe_put(&mut output, &mut cursor, previous)?;
                semaphore_probe_put(
                    &mut output,
                    &mut cursor,
                    domain.semaphores_state(&ram, &map, token)?[0],
                )?;
            }
            let mut guard = map.handles_lock(&ram)?;
            semaphore_probe_need(
                513,
                guard.handle_guard_lookup(&ram, token)? == Some([id, 3]),
            )?;
            guard.handles_guard_close(&ram)?;
            domain.semaphores_remove(&ram, &map, token)?;
            semaphore_probe_need(514, gamepkg::local_semaphore_state(id).err() == Some(-4))?;
        }
    }
    semaphore_probe_need(515, cursor == 1344)?;
    for pair in [
        [0u32, 0],
        [1, 0],
        [2, 1],
        [0xffffffff, 0x7fffffff],
        [0, 0x80000000],
    ] {
        let mut incoming = domain.semaphores_begin(&ram, &map)?;
        semaphore_probe_need(
            516,
            incoming.semaphore_create(&ram, pair[0], pair[1]) == Err(-1)
                && incoming.semaphore_creation_phase() == 0
                && incoming.semaphore_creation_id() == 0,
        )?;
        incoming.semaphore_creation_close(&ram)?;
    }
    let orphan = semaphore_probe_created(&mut ram, &map, &domain, 0, 3, 0)?;
    semaphore_probe_need(517, orphan[1] == 0)?;
    let mut bad = domain.semaphores_begin(&ram, &map)?;
    bad.semaphore_create(&ram, 0, 2)?;
    let bad_id = bad.semaphore_creation_id();
    semaphore_probe_need(
        518,
        bad.semaphore_publish(&mut ram, 0x90000000) == Err(-4)
            && bad.semaphore_creation_phase() == 3
            && bad.semaphore_creation_token() == 0x48000001,
    )?;
    bad.semaphore_creation_close(&ram)?;
    domain.semaphores_remove(&ram, &map, 0x48000001)?;
    semaphore_probe_need(
        519,
        gamepkg::local_semaphore_state(bad_id).err() == Some(-4),
    )?;
    ram.write_u32(0x10000, 0x33333333)?;
    gamepkg::local_call(
        3,
        &[
            u64::to_le_bytes(main).as_slice(),
            &u64::to_le_bytes(0),
            &u64::to_le_bytes(4096),
            &u32::to_le_bytes(1),
        ]
        .concat(),
        &mut [],
    )?;
    let mut readonly = domain.semaphores_begin(&ram, &map)?;
    readonly.semaphore_create(&ram, 0, 1)?;
    semaphore_probe_need(
        543,
        readonly.semaphore_publish(&mut ram, 0x10000) == Err(-4)
            && readonly.semaphore_creation_phase() == 3
            && ram.read_u32(0x10000)? == 0x33333333,
    )?;
    readonly.semaphore_creation_close(&ram)?;
    gamepkg::local_call(
        3,
        &[
            u64::to_le_bytes(main).as_slice(),
            &u64::to_le_bytes(0),
            &u64::to_le_bytes(4096),
            &u32::to_le_bytes(2),
        ]
        .concat(),
        &mut [],
    )?;
    domain.semaphores_remove(&ram, &map, 0x48000001)?;
    let item = semaphore_probe_created(&mut ram, &map, &domain, 1, 4, 0x10000)?;
    let token = item[1] as u32;
    semaphore_probe_metadata(&ram, &map, &domain, scratch, item[0], token)?;
    let alias = domain.semaphores_alias(&ram, &map, token, true)?;
    semaphore_probe_need(
        520,
        alias == 0x48000002 && domain.semaphores_alias(&ram, &map, token, false)? == token,
    )?;
    semaphore_probe_need(
        521,
        domain.semaphores_release(&ram, &map, alias, 2)? == 1
            && borrowed.semaphores_state(&ram, &map, token)? == [3, 4],
    )?;
    domain.semaphores_remove(&ram, &map, token)?;
    semaphore_probe_need(
        522,
        gamepkg::local_semaphore_state(item[0]).is_ok()
            && domain.semaphores_state(&ram, &map, token) == Err(-4),
    )?;
    domain.semaphores_remove(&ram, &map, alias)?;
    semaphore_probe_need(
        523,
        gamepkg::local_semaphore_state(item[0]).err() == Some(-4)
            && domain.semaphores_remove(&ram, &map, alias) == Err(-4),
    )?;
    let foreign = gamepkg::local_semaphore_create(1, 2)?;
    let mut guard = map.handles_lock(&ram)?;
    let fake = guard.handle_guard_insert(&ram, foreign, 3, false)?;
    let wrong = guard.handle_guard_insert(&ram, foreign, 2, true)?;
    let high = guard.handle_guard_insert(&ram, 0xfedcba9876543210, 3, true)?;
    guard.handles_guard_close(&ram)?;
    semaphore_probe_need(
        524,
        domain.semaphores_state(&ram, &map, fake) == Err(-4)
            && domain.semaphores_release(&ram, &map, fake, 1) == Err(-4)
            && domain.semaphores_alias(&ram, &map, fake, true) == Err(-4)
            && domain.semaphores_remove(&ram, &map, fake) == Err(-4),
    )?;
    semaphore_probe_need(
        525,
        domain.semaphores_release(&ram, &map, wrong, 1) == Err(-1)
            && domain.semaphores_state(&ram, &map, high) == Err(-4)
            && gamepkg::local_semaphore_state(foreign)?.count == 1,
    )?;
    let mut guard = map.handles_lock(&ram)?;
    for token in [fake, wrong, high] {
        guard.handle_guard_remove(&ram, token)?;
    }
    guard.handles_guard_close(&ram)?;
    gamepkg::local_call(25, &u64::to_le_bytes(foreign), &mut [])?;
    // A forged kind-3 tag/ledger cannot turn an actual event into a semaphore.
    let mut native = [0u8; 8];
    gamepkg::local_call(
        20,
        &[u32::to_le_bytes(0), u32::to_le_bytes(1)].concat(),
        &mut native,
    )?;
    let event_id = semaphore_probe_word(&native, 0)?;
    let (version, original) = gamepkg::local_record_snapshot(domain.semaphores_id(), 2624)?;
    let mut forged = original.clone();
    semaphore_probe_poke(&mut forged, 80, event_id)?;
    semaphore_probe_poke(&mut forged, 88, 2)?;
    gamepkg::local_record_compare_exchange(domain.semaphores_id(), version, &forged)?;
    let mut guard = map.handles_lock(&ram)?;
    let token = guard.handle_guard_insert(&ram, event_id, 3, false)?;
    guard.handles_guard_close(&ram)?;
    semaphore_probe_need(
        544,
        domain.semaphores_state(&ram, &map, token) == Err(-1)
            && domain.semaphores_release(&ram, &map, token, 1) == Err(-1)
            && domain.semaphores_wait_begin(&ram, &map, token).is_err()
            && domain.semaphores_remove(&ram, &map, token) == Err(-1)
            && gamepkg::local_event_state(event_id)?.signaled,
    )?;
    let (version, _) = gamepkg::local_record_snapshot(domain.semaphores_id(), 2624)?;
    gamepkg::local_record_compare_exchange(domain.semaphores_id(), version, &original)?;
    let mut guard = map.handles_lock(&ram)?;
    guard.handle_guard_remove(&ram, token)?;
    guard.handles_guard_close(&ram)?;
    gamepkg::local_call(25, &native, &mut [])?;
    semaphore_probe_native_kind(&ram, &map, &domain)?;
    semaphore_probe_external_revoke(&mut ram, &map)?;
    crate::wait_probe::wait_probe(&mut ram, &map, &mut domain)?;
    semaphore_probe_parallel(&ram, &map, &domain)?;
    semaphore_probe_cancelled_creations(&mut ram, &map)?;
    // A native release is accepted before a later guest copy can fail.
    let item = semaphore_probe_created(&mut ram, &map, &domain, 0, 4, 0x10000)?;
    let previous = domain.semaphores_release(&ram, &map, item[1] as u32, 3)?;
    semaphore_probe_need(
        526,
        previous == 0
            && ram.write_u32(0x90000000, previous) == Err(-4)
            && gamepkg::local_semaphore_state(item[0])?.count == 3,
    )?;
    domain.semaphores_remove(&ram, &map, item[1] as u32)?;
    let mut pending = domain.semaphores_begin(&ram, &map)?;
    pending.semaphore_create(&ram, 0, 1)?;
    let pending_id = pending.semaphore_creation_id();
    // One table mutex + one orphan + one incoming + 125 retained semaphores.
    let mut retained = std::vec::Vec::new();
    for _ in 0..125u32 {
        retained.push(gamepkg::local_semaphore_create(0, 1)?);
    }
    let mut exhausted = domain.semaphores_begin(&ram, &map)?;
    semaphore_probe_need(
        527,
        exhausted.semaphore_create(&ram, 0, 1) == Err(-2)
            && exhausted.semaphore_creation_phase() == 0,
    )?;
    exhausted.semaphore_creation_close(&ram)?;
    let services_id = ram.services_handle()?;
    ram.services_link(services_id, 0)?;
    semaphore_probe_need(528, pending.semaphore_publish(&mut ram, 0x10000) == Err(-4))?;
    pending.semaphore_creation_close(&ram)?;
    semaphore_probe_need(
        529,
        gamepkg::local_semaphore_state(pending_id).err() == Some(-4),
    )?;
    semaphore_probe_need(
        530,
        domain.semaphores_close(&ram, &map) == Err(-4)
            && domain.semaphores_closed()
            && gamepkg::local_semaphore_state(orphan[0]).err() == Some(-4),
    )?;
    semaphore_probe_need(531, domain.semaphores_close(&ram, &map) == Err(-4))?;
    for id in retained {
        gamepkg::local_call(25, &u64::to_le_bytes(id), &mut [])?;
    }
    semaphore_probe_need(
        532,
        map.handles_close(&ram) == Err(-4) && map.handles_closed(),
    )?;
    services.service_close(&ram)?;
    gamepkg::local_record_release(ram.call_handle())?;
    Ok(output)
}
fn semaphore_probe_joined(task: u64) -> Result<u64, i32> {
    for _ in 0..4096u32 {
        match gamepkg::local_task_join(task) {
            Ok(proof) => return Ok(proof.result()),
            Err(-2) => memory::Memory::call_retry_pause()?,
            Err(code) => return Err(code),
        }
    }
    Err(-2)
}
fn semaphore_probe_spawn(entry: u32, argument: u64) -> Result<u64, i32> {
    let mut out = [0u8; 8];
    semaphore_probe_need(
        533,
        gamepkg::local_call(
            10,
            &[
                u32::to_le_bytes(entry).as_slice(),
                &u64::to_le_bytes(argument),
                &u32::to_le_bytes(0),
            ]
            .concat(),
            &mut out,
        )? == 8,
    )?;
    semaphore_probe_word(&out, 0)
}
fn semaphore_probe_parallel(
    ram: &memory::Memory,
    map: &handles::Handles,
    domain: &semaphores::Semaphores,
) -> Result<(), i32> {
    let args = gamepkg::local_record_create(
        &[
            u64::to_le_bytes(ram.call_handle()).as_slice(),
            &u64::to_le_bytes(map.handles_id()),
            &u64::to_le_bytes(domain.semaphores_id()),
        ]
        .concat(),
    )?;
    let mut jobs = std::vec::Vec::new();
    for _ in 0..4u32 {
        jobs.push(semaphore_probe_spawn(0, args)?);
    }
    let mut tokens = std::vec::Vec::new();
    for job in jobs {
        let value = semaphore_probe_joined(job)?;
        let token = value as u32;
        semaphore_probe_need(
            534,
            value == u64::from(token) && token >= 0x48000001 && token < 0x48000005,
        )?;
        for before in &tokens {
            semaphore_probe_need(535, *before != token)?;
        }
        tokens.push(token);
        semaphore_probe_need(536, domain.semaphores_state(ram, map, token)? == [1, 4])?;
    }
    for token in tokens {
        domain.semaphores_remove(ram, map, token)?;
    }
    gamepkg::local_record_release(args)?;
    Ok(())
}
fn semaphore_probe_actor(argument: u64) -> Result<u64, i32> {
    let (_, b) = gamepkg::local_record_snapshot(argument, 24)?;
    let mut ram = memory::Memory::with_call_handle(semaphore_probe_word(&b, 0)?)?;
    let map = handles::Handles::handles_borrow(&ram, semaphore_probe_word(&b, 8)?)?;
    let domain =
        semaphores::Semaphores::semaphores_borrow(&ram, &map, semaphore_probe_word(&b, 16)?)?;
    // Borrowed worker views carry no guest output span. Transfer a retained
    // orphan, then publish its typed tag under the shared metadata guard.
    let item = semaphore_probe_created(&mut ram, &map, &domain, 1, 4, 0)?;
    let mut guard = map.handles_lock(&ram)?;
    let token = guard.handle_guard_insert(&ram, item[0], 3, false)?;
    guard.handles_guard_close(&ram)?;
    Ok(u64::from(token))
}
pub fn semaphore_worker(argument: u64) -> u64 {
    match semaphore_probe_actor(argument) {
        Ok(value) => value,
        Err(code) => code as i64 as u64,
    }
}
fn semaphore_probe_cancelled_creations(
    ram: &mut memory::Memory,
    map: &handles::Handles,
) -> Result<(), i32> {
    for mode in 0..3u64 {
        let mut domain = semaphores::Semaphores::semaphores_new(ram, map)?;
        let ready = gamepkg::local_mailbox_create(16, 1)?;
        let argument = gamepkg::local_record_create(
            &[
                u64::to_le_bytes(ram.call_handle()).as_slice(),
                &u64::to_le_bytes(map.handles_id()),
                &u64::to_le_bytes(domain.semaphores_id()),
                &u64::to_le_bytes(ready),
                &u64::to_le_bytes(mode),
            ]
            .concat(),
        )?;
        let task = semaphore_probe_spawn(2, argument)?;
        let mut out = [0u8; 16];
        semaphore_probe_need(
            537,
            gamepkg::local_mailbox_receive(ready, 1000, &mut out)? == 16,
        )?;
        let id = semaphore_probe_word(&out, 0)?;
        let token = semaphore_probe_word(&out, 8)?;
        semaphore_probe_need(
            538,
            gamepkg::local_semaphore_state(id)?.count == 0
                && token == if mode == 2 { 0x48000001 } else { 0 },
        )?;
        gamepkg::local_call(12, &u64::to_le_bytes(task), &mut [])?;
        semaphore_probe_need(539, semaphore_probe_joined(task)? == id)?;
        semaphore_probe_need(
            540,
            gamepkg::local_semaphore_state(id).is_ok() == (mode != 0),
        )?;
        domain.semaphores_close(ram, map)?;
        semaphore_probe_need(541, gamepkg::local_semaphore_state(id).err() == Some(-4))?;
        gamepkg::local_record_release(argument)?;
        gamepkg::local_mailbox_release(ready)?;
    }
    Ok(())
}
fn semaphore_probe_creation_hold(
    ram: &mut memory::Memory,
    incoming: &mut semaphores::SemaphoreCreation,
    mode: u64,
    ready: u64,
) -> Result<u64, i32> {
    incoming.semaphore_create(ram, 0, 1)?;
    let id = incoming.semaphore_creation_id();
    let token = if mode == 0 {
        0
    } else if mode == 1 {
        incoming.semaphore_publish(ram, 0)?
    } else {
        // A worker has no private output mapping here: the accepted domain/tag
        // survives this deliberate guest-write failure and subsequent cancel.
        semaphore_probe_need(
            542,
            incoming.semaphore_publish(ram, 0x10000) == Err(-4)
                && incoming.semaphore_creation_phase() == 3,
        )?;
        incoming.semaphore_creation_token()
    };
    gamepkg::local_mailbox_send(
        ready,
        &[u64::to_le_bytes(id), u64::to_le_bytes(u64::from(token))].concat(),
    )?;
    for _ in 0..4096u32 {
        if gamepkg::local_task_status()?.1 {
            return Ok(id);
        }
        let _ = memory::Memory::call_retry_pause();
    }
    Err(-2)
}
fn semaphore_probe_creation_actor(argument: u64) -> Result<u64, i32> {
    let (_, b) = gamepkg::local_record_snapshot(argument, 40)?;
    let mut ram = memory::Memory::with_call_handle(semaphore_probe_word(&b, 0)?)?;
    let map = handles::Handles::handles_borrow(&ram, semaphore_probe_word(&b, 8)?)?;
    let domain =
        semaphores::Semaphores::semaphores_borrow(&ram, &map, semaphore_probe_word(&b, 16)?)?;
    let mut incoming = domain.semaphores_begin(&ram, &map)?;
    let result = semaphore_probe_creation_hold(
        &mut ram,
        &mut incoming,
        semaphore_probe_word(&b, 32)?,
        semaphore_probe_word(&b, 24)?,
    );
    let closed = incoming.semaphore_creation_close(&ram);
    let id = result?;
    closed?;
    Ok(id)
}
pub fn creation_worker(argument: u64) -> u64 {
    match semaphore_probe_creation_actor(argument) {
        Ok(id) => id,
        Err(code) => code as i64 as u64,
    }
}

fn semaphore_probe_metadata(
    ram: &memory::Memory,
    map: &handles::Handles,
    domain: &semaphores::Semaphores,
    scratch: u64,
    id: u64,
    token: u32,
) -> Result<(), i32> {
    let request = [
        u64::to_le_bytes(scratch).as_slice(),
        &u64::to_le_bytes(0),
        &u64::to_le_bytes(16384),
        &u32::to_le_bytes(0),
    ]
    .concat();
    gamepkg::local_call(3, &request, &mut [])?;
    semaphore_probe_need(
        544,
        domain.semaphores_release(ram, map, token, 1) == Err(-4)
            && gamepkg::local_semaphore_state(id)?.count == 1,
    )?;
    gamepkg::local_call(
        3,
        &[
            u64::to_le_bytes(scratch).as_slice(),
            &u64::to_le_bytes(0),
            &u64::to_le_bytes(16384),
            &u32::to_le_bytes(2),
        ]
        .concat(),
        &mut [],
    )?;
    let mut magic = [0u8; 1];
    gamepkg::local_call(
        5,
        &[
            u64::to_le_bytes(scratch).as_slice(),
            &u64::to_le_bytes(0),
            &u32::to_le_bytes(1),
        ]
        .concat(),
        &mut magic,
    )?;
    gamepkg::local_call(
        6,
        &[
            u64::to_le_bytes(scratch).as_slice(),
            &u64::to_le_bytes(0),
            &[0u8],
        ]
        .concat(),
        &mut [],
    )?;
    semaphore_probe_need(
        545,
        domain.semaphores_release(ram, map, token, 1) == Err(-1)
            && gamepkg::local_semaphore_state(id)?.count == 1,
    )?;
    gamepkg::local_call(
        6,
        &[
            u64::to_le_bytes(scratch).as_slice(),
            &u64::to_le_bytes(0),
            &magic,
        ]
        .concat(),
        &mut [],
    )?;
    gamepkg::local_call(
        3,
        &[
            u64::to_le_bytes(scratch).as_slice(),
            &u64::to_le_bytes(0),
            &u64::to_le_bytes(16384),
            &u32::to_le_bytes(1),
        ]
        .concat(),
        &mut [],
    )?;
    semaphore_probe_need(
        546,
        domain.semaphores_alias(ram, map, token, true) == Err(-4),
    )?;
    gamepkg::local_call(
        3,
        &[
            u64::to_le_bytes(scratch).as_slice(),
            &u64::to_le_bytes(0),
            &u64::to_le_bytes(16384),
            &u32::to_le_bytes(2),
        ]
        .concat(),
        &mut [],
    )?;
    let mut guard = map.handles_lock(ram)?;
    semaphore_probe_need(547, guard.handle_guard_references(ram, id, 3)? == 1)?;
    guard.handles_guard_close(ram)?;
    Ok(())
}
fn semaphore_probe_native_kind(
    ram: &memory::Memory,
    map: &handles::Handles,
    domain: &semaphores::Semaphores,
) -> Result<(), i32> {
    let mut bytes = [0u8; 8];
    gamepkg::local_call(
        20,
        &[u32::to_le_bytes(0), u32::to_le_bytes(1)].concat(),
        &mut bytes,
    )?;
    let event = u64::from_le_bytes(bytes);
    let (version, pristine) = gamepkg::local_record_snapshot(domain.semaphores_id(), 2624)?;
    semaphore_probe_need(
        548,
        semaphore_probe_word(&pristine, 80)? == 0 && semaphore_probe_word(&pristine, 88)? == 0,
    )?;
    let mut forged = pristine.clone();
    semaphore_probe_poke(&mut forged, 80, event)?;
    semaphore_probe_poke(&mut forged, 88, 2)?;
    gamepkg::local_record_compare_exchange(domain.semaphores_id(), version, &forged)?;
    let mut guard = map.handles_lock(ram)?;
    let token = guard.handle_guard_insert(ram, event, 3, false)?;
    guard.handles_guard_close(ram)?;
    semaphore_probe_need(
        549,
        domain.semaphores_state(ram, map, token) == Err(-1)
            && domain.semaphores_release(ram, map, token, 1) == Err(-1)
            && domain.semaphores_alias(ram, map, token, true) == Err(-1)
            && domain.semaphores_wait_begin(ram, map, token).is_err()
            && domain.semaphores_remove(ram, map, token) == Err(-1),
    )?;
    semaphore_probe_need(550, gamepkg::local_event_state(event)?.signaled)?;
    let mut guard = map.handles_lock(ram)?;
    semaphore_probe_need(
        551,
        guard.handle_guard_remove(ram, token)? == Some([event, 3]),
    )?;
    guard.handles_guard_close(ram)?;
    let (version, current) = gamepkg::local_record_snapshot(domain.semaphores_id(), 2624)?;
    semaphore_probe_need(552, current == forged)?;
    gamepkg::local_record_compare_exchange(domain.semaphores_id(), version, &pristine)?;
    gamepkg::local_call(25, &u64::to_le_bytes(event), &mut [])?;
    Ok(())
}
fn semaphore_probe_external_revoke(
    ram: &mut memory::Memory,
    map: &handles::Handles,
) -> Result<(), i32> {
    let mut domain = semaphores::Semaphores::semaphores_new(ram, map)?;
    let item = semaphore_probe_created(ram, map, &domain, 0, 1, 0x10000)?;
    gamepkg::local_call(25, &u64::to_le_bytes(item[0]), &mut [])?;
    semaphore_probe_need(
        553,
        domain.semaphores_state(ram, map, item[1] as u32) == Err(-4)
            && domain.semaphores_remove(ram, map, item[1] as u32) == Err(-4),
    )?;
    semaphore_probe_need(
        554,
        domain.semaphores_close(ram, map) == Err(-4)
            && domain.semaphores_closed()
            && domain.semaphores_close(ram, map) == Err(-4),
    )?;
    let mut guard = map.handles_lock(ram)?;
    semaphore_probe_need(
        555,
        guard.handle_guard_lookup(ram, item[1] as u32)?.is_none(),
    )?;
    guard.handles_guard_close(ram)?;
    Ok(())
}
