use crate::events;
use crate::gamepkg;
use crate::handles;
use crate::memory;
fn wait_need(stage: i32, ok: bool) -> Result<(), i32> {
    if ok {
        Ok(())
    } else {
        Err(-stage)
    }
}
fn wait_word(b: &[u8], at: usize) -> Result<u64, i32> {
    let p = b.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        p[0], p[1], p[2], p[3], p[4], p[5], p[6], p[7],
    ]))
}
fn wait_new_event(
    ram: &mut memory::Memory,
    map: &handles::Handles,
    domain: &events::Events,
) -> Result<[u64; 2], i32> {
    let mut creation = domain.events_begin(ram, map)?;
    creation.event_create(ram, 0, 0)?;
    let id = creation.event_creation_id();
    let token = creation.event_publish(ram, 0x10000)?;
    creation.event_creation_close(ram)?;
    Ok([id, u64::from(token)])
}
pub fn wait_probe(
    ram: &mut memory::Memory,
    map: &handles::Handles,
    domain: &mut events::Events,
) -> Result<(), i32> {
    let item = wait_new_event(ram, map, domain)?;
    let token = item[1] as u32;
    let mut lease = domain.events_wait_begin(ram, map, token)?;
    let record = lease.event_wait_id();
    wait_need(201, domain.events_close(ram, map) == Err(-2))?;
    wait_need(
        202,
        lease.event_wait(ram, 1001) == Err(-1) && !lease.event_wait_published(),
    )?;
    lease.event_wait_close(ram)?;
    lease.event_wait_close(ram)?;
    wait_need(
        203,
        lease.event_wait_closed() && gamepkg::local_record_snapshot(record, 80).err() == Some(-4),
    )?;
    let mut lease = domain.events_wait_begin(ram, map, token)?;
    let record = lease.event_wait_id();
    wait_need(
        204,
        lease.event_wait(ram, 0) == Err(-2) && lease.event_wait_published(),
    )?;
    wait_need(205, lease.event_wait(ram, 0) == Err(-1))?;
    lease.event_wait_close(ram)?;
    let (_, bytes) = gamepkg::local_record_snapshot(record, 80)?;
    wait_need(
        206,
        wait_word(&bytes, 56)? == 1 && wait_word(&bytes, 64)? == (-2i64) as u64,
    )?;
    domain.events_remove(ram, map, token)?;
    wait_need(
        207,
        gamepkg::local_record_snapshot(record, 80).err() == Some(-4)
            && gamepkg::local_event_state(item[0]).err() == Some(-4),
    )?;
    wait_bounds(ram, map, domain)?;
    for mode in 0..3u32 {
        let item = wait_new_event(ram, map, domain)?;
        let token = item[1] as u32;
        let args = [
            u64::to_le_bytes(ram.call_handle()).as_slice(),
            &u64::to_le_bytes(map.handles_id()),
            &u64::to_le_bytes(domain.events_id()),
            &u64::to_le_bytes(u64::from(token)),
        ]
        .concat();
        let argument = gamepkg::local_record_create(&args)?;
        let request = [
            u32::to_le_bytes(1).as_slice(),
            &u64::to_le_bytes(argument),
            &u32::to_le_bytes(0),
        ]
        .concat();
        let mut reply = [0u8; 8];
        gamepkg::local_call(10, &request, &mut reply)?;
        let task = u64::from_le_bytes(reply);
        let mut active = 0u64;
        for _ in 0..4096u32 {
            let (_, b) = gamepkg::local_record_snapshot(domain.events_id(), 2624)?;
            for slot in 0..32usize {
                if wait_word(&b, 2120 + slot * 16)? == 1 {
                    let id = wait_word(&b, 2112 + slot * 16)?;
                    let (_, w) = gamepkg::local_record_snapshot(id, 80)?;
                    if wait_word(&w, 40)? == item[0]
                        && wait_word(&w, 48)? == task
                        && wait_word(&w, 56)? == 0
                    {
                        active = id;
                    }
                }
            }
            if active != 0 {
                break;
            }
            memory::Memory::call_retry_pause()?;
        }
        wait_need(208, active != 0)?;
        wait_need(
            209,
            domain.events_remove(ram, map, token) == Err(-2)
                && gamepkg::local_event_state(item[0]).is_ok(),
        )?;
        wait_need(
            210,
            domain.events_close(ram, map) == Err(-2) && !domain.events_closed(),
        )?;
        let services = ram.services_handle()?;
        if mode == 0 {
            domain.events_set(ram, map, token, true)?;
        } else if mode == 1 {
            gamepkg::local_call(12, &u64::to_le_bytes(task), &mut [])?;
        } else {
            ram.services_link(services, 0)?;
            gamepkg::local_call(
                22,
                &[u64::to_le_bytes(item[0]).as_slice(), &u32::to_le_bytes(1)].concat(),
                &mut [],
            )?;
        }
        let mut joined = None;
        for _ in 0..4096u32 {
            match gamepkg::local_task_join(task) {
                Ok(proof) => {
                    joined = Some(proof);
                    break;
                }
                Err(-2) => memory::Memory::call_retry_pause()?,
                Err(code) => return Err(code),
            }
        }
        wait_need(
            211,
            joined.ok_or(-2)?.result() == if mode == 0 { 0 } else { (-4i64) as u64 },
        )?;
        let (_, w) = gamepkg::local_record_snapshot(active, 80)?;
        wait_need(
            212,
            wait_word(&w, 56)? == 1
                && wait_word(&w, 64)? == if mode == 0 { 0 } else { (-4i64) as u64 },
        )?;
        if mode == 2 {
            ram.services_link(0, services)?;
        }
        domain.events_remove(ram, map, token)?;
        wait_need(
            213,
            gamepkg::local_record_snapshot(active, 80).err() == Some(-4)
                && gamepkg::local_event_state(item[0]).err() == Some(-4),
        )?;
        gamepkg::local_record_release(argument)?;
    }
    Ok(())
}
fn wait_actor(argument: u64) -> Result<i32, i32> {
    let (_, b) = gamepkg::local_record_snapshot(argument, 32)?;
    wait_need(214, b.len() == 32)?;
    let ram = memory::Memory::with_call_handle(wait_word(&b, 0)?)?;
    let map = handles::Handles::handles_borrow(&ram, wait_word(&b, 8)?)?;
    let domain = events::Events::events_borrow(&ram, &map, wait_word(&b, 16)?)?;
    let mut lease = domain.events_wait_begin(&ram, &map, wait_word(&b, 24)? as u32)?;
    let result = lease.event_wait(&ram, 1000);
    lease.event_wait_close(&ram)?;
    wait_need(215, lease.event_wait_closed())?;
    Ok(match result {
        Ok(()) => 0,
        Err(code) => code,
    })
}
pub fn wait_worker(argument: u64) -> u64 {
    match wait_actor(argument) {
        Ok(code) | Err(code) => code as i64 as u64,
    }
}

fn wait_bounds(
    ram: &mut memory::Memory,
    map: &handles::Handles,
    domain: &mut events::Events,
) -> Result<(), i32> {
    let item = wait_new_event(ram, map, domain)?;
    let token = item[1] as u32;
    // Synthetic schema/capacity controls: 32 registered ACTIVE leases, not 32
    // physical waiting tasks. Record capacity remains independently enforced.
    let mut guard = map.handles_lock(ram)?;
    let (version, mut b) = gamepkg::local_record_snapshot(domain.events_id(), 2624)?;
    let mut leases = std::vec::Vec::new();
    for slot in 0..32usize {
        let mut w = [0u8; 80];
        for (at, value) in [
            (0usize, 0x31544c57u64),
            (8, ram.call_handle()),
            (16, ram.services_handle()?),
            (24, map.handles_id()),
            (32, domain.events_id()),
            (40, item[0]),
            (48, gamepkg::local_task_status()?.0),
        ] {
            w[at..at + 8].copy_from_slice(&u64::to_le_bytes(value));
        }
        let record = gamepkg::local_record_create(&w)?;
        leases.push(record);
        b[2112 + slot * 16..2120 + slot * 16].copy_from_slice(&u64::to_le_bytes(record));
        b[2120 + slot * 16..2128 + slot * 16].copy_from_slice(&u64::to_le_bytes(1));
    }
    gamepkg::local_record_compare_exchange(domain.events_id(), version, &b)?;
    guard.handles_guard_close(ram)?;
    wait_need(
        216,
        domain.events_wait_begin(ram, map, token).err() == Some(-2),
    )?;
    wait_need(
        217,
        domain.events_remove(ram, map, token) == Err(-2)
            && domain.events_close(ram, map) == Err(-2),
    )?;
    // A duplicate lease ID cannot create a second apparent lifetime.
    let (version, mut forged) = gamepkg::local_record_snapshot(domain.events_id(), 2624)?;
    forged[2128..2136].copy_from_slice(&u64::to_le_bytes(leases[0]));
    gamepkg::local_record_compare_exchange(domain.events_id(), version, &forged)?;
    wait_need(
        218,
        events::Events::events_borrow(ram, map, domain.events_id()).is_err(),
    )?;
    let (version, _) = gamepkg::local_record_snapshot(domain.events_id(), 2624)?;
    gamepkg::local_record_compare_exchange(domain.events_id(), version, &b)?;
    for record in &leases {
        let (version, mut w) = gamepkg::local_record_snapshot(*record, 80)?;
        w[56..64].copy_from_slice(&u64::to_le_bytes(1));
        gamepkg::local_record_compare_exchange(*record, version, &w)?;
    }
    domain.events_remove(ram, map, token)?;
    for record in leases {
        wait_need(
            219,
            gamepkg::local_record_snapshot(record, 80).err() == Some(-4),
        )?;
    }
    let item = wait_new_event(ram, map, domain)?;
    let token = item[1] as u32;
    let mut records = std::vec::Vec::new();
    for _ in 0..64u32 {
        match gamepkg::local_record_create(&[0u8; 8]) {
            Ok(record) => records.push(record),
            Err(-2) => break,
            Err(code) => return Err(code),
        }
    }
    wait_need(
        220,
        domain.events_wait_begin(ram, map, token).err() == Some(-2),
    )?;
    domain.events_set(ram, map, token, true)?;
    for record in records {
        gamepkg::local_record_release(record)?;
    }
    domain.events_remove(ram, map, token)?;
    Ok(())
}
