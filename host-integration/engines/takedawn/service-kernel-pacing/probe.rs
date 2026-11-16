// Checked game-source controls. No native device, clock or reference injection.
use crate::gamepkg;
use crate::kernel_pool;
use crate::memory;
use crate::shared;
use crate::wait_service;
fn probe_u64(bytes: &[u8], at: usize) -> Result<u64, i32> {
    let b = bytes.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}
fn probe_report(record: u64) -> Result<[u64; 3], i32> {
    let (_, bytes) = gamepkg::local_record_snapshot(record, 24)?;
    if bytes.len() != 24 || probe_u64(&bytes, 16)? > 1 {
        return Err(-1);
    }
    Ok([
        probe_u64(&bytes, 0)?,
        probe_u64(&bytes, 8)?,
        probe_u64(&bytes, 16)?,
    ])
}
fn probe_publish(record: u64, due: u64) -> Result<bool, i32> {
    for _ in 0..64u32 {
        let (version, bytes) = gamepkg::local_record_snapshot(record, 24)?;
        if bytes.len() != 24 || probe_u64(&bytes, 16)? > 1 || due > 1 {
            return Err(-1);
        }
        if probe_u64(&bytes, 16)? != 0 {
            return Ok(false);
        }
        let value = [
            u64::to_le_bytes(probe_u64(&bytes, 0)?.checked_add(1).ok_or(-1)?),
            u64::to_le_bytes(probe_u64(&bytes, 8)?.checked_add(due).ok_or(-1)?),
            u64::to_le_bytes(0),
        ]
        .concat();
        match gamepkg::local_record_compare_exchange(record, version, &value) {
            Ok(_) => return Ok(true),
            Err(-2) => {}
            Err(code) => return Err(code),
        }
    }
    Err(-2)
}
fn probe_actor(record: u64) -> Result<u64, i32> {
    let (_, bytes) = gamepkg::local_record_snapshot(record, 32)?;
    if bytes.len() != 32 {
        return Err(-1);
    }
    let ram = memory::Memory::with_call_handle(probe_u64(&bytes, 0)?)?;
    let schedule = shared::Schedule::schedule_borrow(&ram, probe_u64(&bytes, 8)?)?;
    let report = probe_u64(&bytes, 16)?;
    let mailbox = probe_u64(&bytes, 24)?;
    let mut sleeper = wait_service::Sleeper::sleep_new(&ram)?;
    // The assigned worker validates its role before reporting readiness.
    let first = schedule.schedule_step(&ram)?;
    if !probe_publish(report, first[1])? {
        return Ok(0);
    }
    gamepkg::local_mailbox_send(mailbox, &u64::to_le_bytes(1))?;
    for _ in 0..4096u32 {
        if probe_report(report)?[2] != 0 {
            return Ok(probe_report(report)?[1]);
        }
        let next = schedule.schedule_step(&ram)?;
        if !probe_publish(report, next[1])? {
            return Ok(probe_report(report)?[1]);
        }
        let delay = next[2].saturating_sub(next[0]).min(10000000);
        sleeper.sleep_ns(&ram, delay)?;
    }
    Err(-2)
}
pub fn pacing_worker(record: u64) -> u64 {
    match probe_actor(record) {
        Ok(value) => value,
        Err(code) => code as i64 as u64,
    }
}
fn probe_start(ram: &memory::Memory, owner: &mut shared::Schedule) -> Result<[u64; 4], i32> {
    let report = gamepkg::local_record_create(&[0u8; 24])?;
    let mailbox = gamepkg::local_mailbox_create(8, 1)?;
    let bytes = [
        u64::to_le_bytes(ram.call_handle()),
        u64::to_le_bytes(owner.schedule_id()),
        u64::to_le_bytes(report),
        u64::to_le_bytes(mailbox),
    ]
    .concat();
    let context = gamepkg::local_record_create(&bytes)?;
    let request = [
        u32::to_le_bytes(0).as_slice(),
        &u64::to_le_bytes(context),
        &u32::to_le_bytes(1),
    ]
    .concat();
    let mut task = [0u8; 8];
    if gamepkg::local_call(10, &request, &mut task)? != 8 {
        return Err(-3);
    }
    let task = u64::from_le_bytes(task);
    owner.schedule_assign(ram, task)?;
    if owner.schedule_assign(ram, task) != Err(-1)
        || owner.schedule_close(ram) != Err(-2)
        || owner.schedule_join(ram) != Err(-2)
    {
        return Err(-3);
    }
    Ok([task, context, report, mailbox])
}
fn probe_resume(task: u64) -> Result<(), i32> {
    let mut reply = [0u8; 4];
    if gamepkg::local_call(13, &u64::to_le_bytes(task), &mut reply)? != 4
        || reply != u32::to_le_bytes(1)
    {
        return Err(-3);
    }
    Ok(())
}
fn probe_ready(mailbox: u64) -> Result<(), i32> {
    let mut reply = [0u8; 8];
    if gamepkg::local_mailbox_receive(mailbox, 1000, &mut reply)? != 8
        || reply != u64::to_le_bytes(1)
    {
        return Err(-3);
    }
    Ok(())
}
fn probe_join(ram: &memory::Memory, owner: &mut shared::Schedule) -> Result<u64, i32> {
    for _ in 0..4096u32 {
        match owner.schedule_join(ram) {
            Err(-2) => memory::Memory::call_retry_pause()?,
            value => return value,
        }
    }
    Err(-2)
}
fn probe_stop(record: u64) -> Result<(), i32> {
    for _ in 0..64u32 {
        let (version, mut bytes) = gamepkg::local_record_snapshot(record, 24)?;
        if bytes.len() != 24 {
            return Err(-1);
        }
        bytes[16..24].copy_from_slice(&u64::to_le_bytes(1));
        match gamepkg::local_record_compare_exchange(record, version, &bytes) {
            Ok(_) => return Ok(()),
            Err(-2) => {}
            Err(code) => return Err(code),
        }
    }
    Err(-2)
}
fn probe_release(resources: [u64; 4]) -> Result<(), i32> {
    gamepkg::local_record_release(resources[1])?;
    gamepkg::local_record_release(resources[2])?;
    gamepkg::local_mailbox_release(resources[3])
}
pub fn pacing_probe() -> Result<(), i32> {
    let ram = memory::Memory::new()?;
    let services = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2)?;
    if shared::Schedule::schedule_new(&ram, 2).is_ok() {
        return Err(-3);
    }
    let mut owner = shared::Schedule::schedule_new(&ram, -1)?;
    let initial = owner.schedule_state(&ram)?;
    let mut records = std::vec::Vec::new();
    let mut exhausted = false;
    for _ in 0..64u32 {
        match gamepkg::local_record_create(&[7u8]) {
            Ok(id) => records.push(id),
            Err(-2) => {
                exhausted = true;
                break;
            }
            Err(code) => return Err(code),
        }
    }
    if !exhausted
        || shared::Schedule::schedule_new(&ram, 0).err() != Some(-2)
        || owner.schedule_state(&ram)? != initial
    {
        return Err(-3);
    }
    for record in records {
        gamepkg::local_record_release(record)?;
    }
    let mut borrowed = shared::Schedule::schedule_borrow(&ram, owner.schedule_id())?;
    let foreign = memory::Memory::new()?;
    if owner.schedule_state(&foreign) != Err(-4)
        || borrowed.schedule_close(&ram) != Err(-4)
        || borrowed.schedule_assign(&ram, 0) != Err(-4)
        || owner.schedule_assign(&ram, 0) != Err(-1)
        || owner.schedule_step(&ram) != Err(-4)
    {
        return Err(-3);
    }
    gamepkg::local_record_release(foreign.call_handle())?;
    owner.schedule_auto(&ram, true)?;
    owner.schedule_relearn(&ram)?;
    owner.schedule_refresh(&ram, wait_service::host_now_ns(&ram)?, 8333333)?;
    let resources = probe_start(&ram, &mut owner)?;
    probe_resume(resources[0])?;
    probe_ready(resources[3])?;
    if owner.schedule_step(&ram) != Err(-4) {
        return Err(-3);
    }
    for index in 0..32u64 {
        owner.schedule_refresh(
            &ram,
            wait_service::host_now_ns(&ram)?,
            if index & 1 == 0 { 8333333 } else { 16666666 },
        )?;
    }
    gamepkg::diagnostic(1, 0x544b1a10, &[])?;
    let before = probe_report(resources[2])?;
    if gamepkg::fullscreen(true) != Err(-5) {
        return Err(-3);
    }
    let after = probe_report(resources[2])?;
    if after[0] <= before[0] || after[1] <= before[1] {
        return Err(-3);
    }
    // Original restart-on-pause path with a real shared guest skew change.
    services.service_advance(&ram, 300000000)?;
    gamepkg::local_clock_wake()?;
    let mut restarted = false;
    for _ in 0..4096u32 {
        let state = owner.schedule_state(&ram)?;
        let epoch = probe_u64(&state, 88)?;
        let guest = wait_service::guest_now_ns(&ram)?;
        if epoch <= guest && guest - epoch < 100000000 {
            restarted = true;
            break;
        }
        memory::Memory::call_retry_pause()?;
    }
    if !restarted {
        return Err(-3);
    }
    probe_stop(resources[2])?;
    gamepkg::local_clock_wake()?;
    let count = probe_join(&ram, &mut owner)?;
    if count != probe_report(resources[2])?[1] || owner.schedule_join(&ram) != Err(-1) {
        return Err(-3);
    }
    owner.schedule_close(&ram)?;
    if !owner.schedule_closed() || borrowed.schedule_state(&ram) != Err(-4) {
        return Err(-3);
    }
    probe_release(resources)?;
    // Malformed records/roles reject at a suspended start, before live work.
    for (offset, value, expected) in [
        (0usize, 0u64, -1i32),
        (8, 0, -4),
        (16, 0, -4),
        (24, 0, -1),
        (24, u64::MAX, -4),
        (32, 2, -1),
        (40, 0, -1),
    ] {
        let mut owner = shared::Schedule::schedule_new(&ram, 0)?;
        let resources = probe_start(&ram, &mut owner)?;
        let (version, mut bytes) = gamepkg::local_record_snapshot(owner.schedule_id(), 144)?;
        let value = u64::to_le_bytes(value);
        for lane in 0..8usize {
            bytes[offset + lane] = value[lane];
        }
        gamepkg::local_record_compare_exchange(owner.schedule_id(), version, &bytes)?;
        probe_resume(resources[0])?;
        if probe_join(&ram, &mut owner)? != expected as i64 as u64 {
            return Err(-3);
        }
        owner.schedule_close(&ram)?;
        probe_release(resources)?;
    }
    let mut owner = shared::Schedule::schedule_new(&ram, 0)?;
    let resources = probe_start(&ram, &mut owner)?;
    probe_resume(resources[0])?;
    probe_ready(resources[3])?;
    gamepkg::local_call(12, &u64::to_le_bytes(resources[0]), &mut [])?;
    if probe_join(&ram, &mut owner)? != (-4i64) as u64 {
        return Err(-3);
    }
    owner.schedule_close(&ram)?;
    probe_release(resources)?;
    let mut owner = shared::Schedule::schedule_new(&ram, 0)?;
    let resources = probe_start(&ram, &mut owner)?;
    probe_resume(resources[0])?;
    probe_ready(resources[3])?;
    services.service_close(&ram)?;
    gamepkg::local_clock_wake()?;
    if probe_join(&ram, &mut owner)? != (-4i64) as u64
        || owner.schedule_close(&ram) != Err(-4)
        || !owner.schedule_closed()
    {
        return Err(-3);
    }
    probe_release(resources)?;
    gamepkg::local_record_release(ram.call_handle())?;
    gamepkg::diagnostic(1, 0x544b1a11, &[before[0], after[0], before[1], after[1]])?;
    Ok(())
}
