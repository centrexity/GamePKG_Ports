// Ordinary checked fixture: no platform calls, clock injection or callbacks.
use crate::gamepkg;
use crate::kernel_pool;
use crate::memory;
use crate::wait_service;

fn wait_word(bytes: &[u8], at: usize) -> Result<u64, i32> {
    let b = bytes.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}
fn wait_send(mailbox: u64, first: u64, second: u64) -> Result<(), i32> {
    let bytes = [u64::to_le_bytes(first), u64::to_le_bytes(second)].concat();
    gamepkg::local_mailbox_send(mailbox, &bytes)
}
fn wait_receive(mailbox: u64) -> Result<[u64; 2], i32> {
    for _ in 0..128u32 {
        let mut bytes = [0u8; 16];
        match gamepkg::local_mailbox_receive(mailbox, 1000, &mut bytes) {
            Ok(16) => return Ok([wait_word(&bytes, 0)?, wait_word(&bytes, 8)?]),
            Err(-2) => {}
            _ => return Err(-3),
        }
    }
    Err(-2)
}
fn wait_join(task: u64) -> Result<u64, i32> {
    for _ in 0..4096u32 {
        let mut reply = [0u8; 8];
        match gamepkg::local_call(11, &u64::to_le_bytes(task), &mut reply) {
            Ok(8) => return Ok(u64::from_le_bytes(reply)),
            Err(-2) => memory::Memory::call_retry_pause()?,
            _ => return Err(-3),
        }
    }
    Err(-2)
}
fn wait_start(
    ram: &memory::Memory,
    mailbox: u64,
    mode: u64,
    control: u64,
) -> Result<[u64; 2], i32> {
    let bytes = [
        u64::to_le_bytes(ram.call_handle()),
        u64::to_le_bytes(mailbox),
        u64::to_le_bytes(mode),
        u64::to_le_bytes(control),
    ]
    .concat();
    let record = gamepkg::local_record_create(&bytes)?;
    let request = [
        u32::to_le_bytes(0).as_slice(),
        &u64::to_le_bytes(record),
        &u32::to_le_bytes(0),
    ]
    .concat();
    let mut task = [0u8; 8];
    match gamepkg::local_call(10, &request, &mut task) {
        Ok(8) => Ok([u64::from_le_bytes(task), record]),
        other => {
            let _ = gamepkg::local_record_release(record);
            Err(match other {
                Err(code) => code,
                _ => -3,
            })
        }
    }
}
fn wait_cycle(ram: &memory::Memory, sleeper: &mut wait_service::Sleeper) -> Result<(), i32> {
    if sleeper.sleep_pending(ram)? {
        return Err(-3);
    }
    let observed = sleeper.sleep_observed();
    if wait_service::sleep_wake(ram)? != observed + 1
        || wait_service::sleep_wake(ram)? != observed + 2
        || !sleeper.sleep_pending(ram)?
        || !sleeper.sleep_pending(ram)?
        || sleeper.sleep_observed() != observed
        || sleeper.sleep_ns(ram, 0)? != 1
        || sleeper.sleep_pending(ram)?
        || sleeper.sleep_observed() != observed + 2
        || sleeper.sleep_ns(ram, 0)? != 0
    {
        return Err(-3);
    }
    Ok(())
}
fn wait_actor(record: u64) -> Result<u64, i32> {
    let (_, bytes) = gamepkg::local_record_snapshot(record, 32)?;
    if bytes.len() != 32 {
        return Err(-1);
    }
    let ram = memory::Memory::with_call_handle(wait_word(&bytes, 0)?)?;
    let mailbox = wait_word(&bytes, 8)?;
    let mode = wait_word(&bytes, 16)?;
    let control = wait_word(&bytes, 24)?;
    if !(1..=4).contains(&mode) || mailbox == 0 {
        return Err(-1);
    }
    let mut sleeper = wait_service::Sleeper::sleep_new(&ram)?;
    let observed = sleeper.sleep_observed();
    wait_send(mailbox, 0, observed)?;
    if mode == 1 {
        for cycle in 0..64u64 {
            let before = sleeper.sleep_observed();
            let mut woke = false;
            for _ in 0..8u32 {
                if sleeper.sleep_ns(&ram, 1000000000)? == 1 {
                    woke = true;
                    break;
                }
            }
            if !woke || sleeper.sleep_observed() != before + 1 || sleeper.sleep_pending(&ram)? {
                return Err(-3);
            }
            wait_send(mailbox, cycle + 1, sleeper.sleep_observed())?;
            // The next wake is sent only after this report is read by its producer.
        }
        return Ok(64);
    }
    if mode == 2 {
        for _ in 0..4096u32 {
            let (_, state) = gamepkg::local_record_snapshot(control, 16)?;
            if state.len() != 16 || wait_word(&state, 8)? > 1 {
                return Err(-1);
            }
            if wait_word(&state, 8)? == 1 {
                return wait_word(&state, 0);
            }
            if sleeper.sleep_ns(&ram, 1000000)? != 0 {
                return Err(-3);
            }
            wait_cycle(&ram, &mut sleeper)?;
            let mut updated = false;
            for _ in 0..64u32 {
                let (version, state) = gamepkg::local_record_snapshot(control, 16)?;
                if state.len() != 16 || wait_word(&state, 8)? > 1 {
                    return Err(-1);
                }
                let count = wait_word(&state, 0)?;
                if wait_word(&state, 8)? == 1 {
                    return Ok(count);
                }
                let count = count.checked_add(1).ok_or(-1)?;
                let value = [u64::to_le_bytes(count), u64::to_le_bytes(0)].concat();
                match gamepkg::local_record_compare_exchange(control, version, &value) {
                    Ok(_) => {
                        updated = true;
                        break;
                    }
                    Err(-2) => {}
                    Err(code) => return Err(code),
                }
            }
            if !updated {
                return Err(-2);
            }
        }
        return Err(-2);
    }
    // Cancellation and owner revocation must not acknowledge a pending wake.
    for _ in 0..16u32 {
        match sleeper.sleep_ns(&ram, 1000000000) {
            Err(-4) => {
                if sleeper.sleep_observed() != observed || sleeper.sleep_pending(&ram) != Err(-4) {
                    return Err(-3);
                }
                return Ok(if mode == 3 { 17 } else { 19 });
            }
            Err(code) => return Err(code),
            Ok(_) => {}
        }
    }
    Err(-2)
}
pub fn wait_worker(record: u64) -> u64 {
    match wait_actor(record) {
        Ok(value) => value,
        Err(code) => code as i64 as u64,
    }
}
pub fn wait_probe() -> Result<(), i32> {
    gamepkg::diagnostic(0, 0x544b1810, &[])?;
    let ram = memory::Memory::new()?;
    if wait_service::host_now_ns(&ram) != Err(-5)
        || wait_service::sleep_wake(&ram) != Err(-5)
        || wait_service::Sleeper::sleep_new(&ram).is_ok()
    {
        return Err(-3);
    }
    let services = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2)?;
    let mut sleeper = wait_service::Sleeper::sleep_new(&ram)?;
    let foreign = memory::Memory::new()?;
    if sleeper.sleep_pending(&foreign) != Err(-4) || sleeper.sleep_ns(&foreign, 0) != Err(-4) {
        return Err(-3);
    }
    gamepkg::local_record_release(foreign.call_handle())?;
    for _ in 0..64u32 {
        wait_cycle(&ram, &mut sleeper)?;
    }
    let observed = sleeper.sleep_observed();
    if sleeper.sleep_ns(&ram, 1000000001) != Err(-1) || sleeper.sleep_observed() != observed {
        return Err(-3);
    }
    let before = gamepkg::local_performance_sample()?;
    if sleeper.sleep_ns(&ram, 2000000)? != 0 {
        return Err(-3);
    }
    let after = gamepkg::local_performance_sample()?;
    if before.frequency != after.frequency
        || after.counter - before.counter < before.frequency.div_ceil(500)
    {
        return Err(-3);
    }
    let low = wait_service::host_now_ns(&ram)?;
    let high = wait_service::host_now_ns(&ram)?;
    if high < low || kernel_pool::kernel_tsc_state(&ram)? != [0, 0, 0] {
        return Err(-3);
    }
    services.service_advance(&ram, 7000000000)?;
    let low = wait_service::host_now_ns(&ram)?;
    let guest = wait_service::guest_now_ns(&ram)?;
    let high = wait_service::host_now_ns(&ram)?;
    if guest < low + 7000000000 || guest > high + 7000000000 {
        return Err(-3);
    }
    let mailbox = gamepkg::local_mailbox_create(16, 2)?;
    let actor = wait_start(&ram, mailbox, 1, 0)?;
    let ready = wait_receive(mailbox)?;
    if ready[0] != 0 {
        return Err(-3);
    }
    for cycle in 0..64u64 {
        wait_service::sleep_wake(&ram)?;
        let report = wait_receive(mailbox)?;
        if report != [cycle + 1, ready[1] + cycle + 1] {
            return Err(-3);
        }
    }
    if wait_join(actor[0])? != 64 {
        return Err(-3);
    }
    gamepkg::local_record_release(actor[1])?;
    gamepkg::local_mailbox_release(mailbox)?;
    // A fresh sleeper is an intentional owner handoff after the previous one joined.
    sleeper = wait_service::Sleeper::sleep_new(&ram)?;
    let mailbox = gamepkg::local_mailbox_create(16, 2)?;
    let actor = wait_start(&ram, mailbox, 3, 0)?;
    wait_receive(mailbox)?;
    let observed = sleeper.sleep_observed();
    let mut busy = false;
    gamepkg::diagnostic(0, 0x544b1811, &[])?;
    for _ in 0..4096u32 {
        match sleeper.sleep_ns(&ram, 0) {
            Err(-2) => {
                busy = true;
                break;
            }
            Ok(0) => memory::Memory::call_retry_pause()?,
            _ => return Err(-3),
        }
    }
    if !busy || sleeper.sleep_observed() != observed {
        return Err(-3);
    }
    gamepkg::diagnostic(0, 0x544b1812, &[])?;
    gamepkg::local_call(12, &u64::to_le_bytes(actor[0]), &mut [])?;
    if wait_join(actor[0])? != 17 {
        return Err(-3);
    }
    gamepkg::diagnostic(0, 0x544b1813, &[])?;
    gamepkg::local_record_release(actor[1])?;
    gamepkg::local_mailbox_release(mailbox)?;
    // Malformed/stale metadata must fail before consuming a real pending wake.
    wait_service::sleep_wake(&ram)?;
    let observed = sleeper.sleep_observed();
    let id = services.service_handle();
    let (_, original) = gamepkg::local_record_snapshot(id, 64)?;
    for (offset, value, expected) in [(0usize, 0u8, -1i32), (8, 0, -4), (56, 1, -1)] {
        let (version, _) = gamepkg::local_record_snapshot(id, 64)?;
        let mut bad = original.clone();
        if offset == 8 {
            for byte in &mut bad[8..16] {
                *byte = 0;
            }
        } else {
            bad[offset] = value;
        }
        gamepkg::local_record_compare_exchange(id, version, &bad)?;
        if sleeper.sleep_ns(&ram, 0) != Err(expected) || sleeper.sleep_observed() != observed {
            return Err(-3);
        }
        let (version, _) = gamepkg::local_record_snapshot(id, 64)?;
        gamepkg::local_record_compare_exchange(id, version, &original)?;
    }
    if sleeper.sleep_ns(&ram, 0)? != 1 || sleeper.sleep_pending(&ram)? {
        return Err(-3);
    }
    let mailbox = gamepkg::local_mailbox_create(16, 2)?;
    let actor = wait_start(&ram, mailbox, 4, 0)?;
    wait_receive(mailbox)?;
    services.service_close(&ram)?;
    gamepkg::local_clock_wake()?;
    if wait_join(actor[0])? != 19 || sleeper.sleep_ns(&ram, 0) != Err(-4) {
        return Err(-3);
    }
    gamepkg::local_record_release(actor[1])?;
    gamepkg::local_mailbox_release(mailbox)?;
    let services = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2)?;
    if sleeper.sleep_pending(&ram) != Err(-4) {
        return Err(-3);
    }
    services.service_close(&ram)?;
    gamepkg::local_record_release(ram.call_handle())?;
    gamepkg::diagnostic(0, 0x544b1814, &[])?;
    if sleeper.sleep_ns(&ram, 0) != Err(-4)
        || wait_service::host_now_ns(&ram) != Err(-4)
        || wait_service::sleep_wake(&ram) != Err(-4)
    {
        return Err(-3);
    }
    gamepkg::diagnostic(0, 0x544b1815, &[])?;
    gamepkg::diagnostic(1, 0x544b1802, &[64, 64])?;
    Ok(())
}

fn wait_count(record: u64) -> Result<u64, i32> {
    let (_, bytes) = gamepkg::local_record_snapshot(record, 16)?;
    if bytes.len() != 16 || wait_word(&bytes, 8)? > 1 {
        return Err(-1);
    }
    wait_word(&bytes, 0)
}
pub fn wait_blocked_probe() -> Result<(), i32> {
    let ram = memory::Memory::new()?;
    let services = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2)?;
    let counter = gamepkg::local_record_create(&[0u8; 16])?;
    let mailbox = gamepkg::local_mailbox_create(16, 2)?;
    let actor = wait_start(&ram, mailbox, 2, counter)?;
    wait_receive(mailbox)?;
    let mut running = false;
    for _ in 0..4096u32 {
        if wait_count(counter)? != 0 {
            running = true;
            break;
        }
        memory::Memory::call_retry_pause()?;
    }
    if !running {
        return Err(-2);
    }
    gamepkg::diagnostic(1, 0x544b1801, &[])?;
    let before = wait_count(counter)?;
    if gamepkg::fullscreen(true) != Err(-5) {
        return Err(-3);
    }
    let after = wait_count(counter)?;
    if after <= before {
        return Err(-3);
    }
    let mut stopped = false;
    for _ in 0..64u32 {
        let (version, bytes) = gamepkg::local_record_snapshot(counter, 16)?;
        if bytes.len() != 16 {
            return Err(-1);
        }
        let value = [u64::to_le_bytes(wait_word(&bytes, 0)?), u64::to_le_bytes(1)].concat();
        match gamepkg::local_record_compare_exchange(counter, version, &value) {
            Ok(_) => {
                stopped = true;
                break;
            }
            Err(-2) => {}
            Err(code) => return Err(code),
        }
    }
    if !stopped || wait_join(actor[0])? != wait_count(counter)? {
        return Err(-3);
    }
    gamepkg::local_record_release(actor[1])?;
    gamepkg::local_record_release(counter)?;
    gamepkg::local_mailbox_release(mailbox)?;
    services.service_close(&ram)?;
    gamepkg::local_record_release(ram.call_handle())?;
    gamepkg::diagnostic(1, 0x544b1803, &[before, after])?;
    Ok(())
}
