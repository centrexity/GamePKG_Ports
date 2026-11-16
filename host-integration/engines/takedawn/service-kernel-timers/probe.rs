use crate::cases;
use crate::gamepkg;
use crate::kernel_pool;
use crate::memory;
use crate::queue;
use crate::timers;
fn timer_need(value: bool) -> Result<(), i32> {
    if value {
        Ok(())
    } else {
        Err(-3)
    }
}
fn timer_word(bytes: &[u8], at: usize) -> Result<u64, i32> {
    let b = bytes.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}
fn timer_send(mailbox: u64, bytes: &[u8]) -> Result<(), i32> {
    let start = gamepkg::local_performance_sample()?;
    loop {
        match gamepkg::local_mailbox_send(mailbox, bytes) {
            Ok(()) => return Ok(()),
            Err(-2) => {
                let now = gamepkg::local_performance_sample()?;
                if now.frequency != start.frequency || now.counter < start.counter {
                    return Err(-3);
                }
                if now.counter - start.counter >= start.frequency {
                    return Err(-2);
                }
                memory::Memory::call_retry_pause()?;
            }
            Err(code) => return Err(code),
        }
    }
}
fn timer_action(
    table: &timers::Timers,
    ram: &mut memory::Memory,
    op: u32,
    entry: [u32; 2],
) -> Result<(), i32> {
    match op % 4 {
        0 => {
            table.timer_set(ram, entry[0], -50000, 5, entry[1])?;
        }
        1 => {
            table.timer_cancel(ram, entry[0].wrapping_add(40))?;
        }
        2 => {
            table.timer_set(ram, 0x1300, 0, 0, entry[1])?;
        }
        _ => {}
    }
    Ok(())
}
fn timer_worker_inner(context: u64) -> Result<u64, i32> {
    let (_, b) = gamepkg::local_record_snapshot(context, 40)?;
    timer_need(b.len() == 40)?;
    let mut ram = memory::Memory::with_call_handle(timer_word(&b, 0)?)?;
    ram.attach(0x740000, 4096, timer_word(&b, 8)?, 0, true)?;
    let mut table = timers::Timers::timers_borrow(&ram, timer_word(&b, 16)?)?;
    let mailbox = timer_word(&b, 24)?;
    timer_need(table.timers_close(&ram, None) == Err(-4))?;
    let mut hash = 14695981039346656037u64;
    for index in 0..512u32 {
        let args = cases::timer_case(index);
        ram.write_u64(0x740000, args[5])?;
        let result = match args[0] {
            0 => table.timer_set(
                &mut ram,
                args[1] as u32,
                args[2] as i64,
                args[3] as u32 as i32,
                args[4] as u32,
            )?,
            1 => [u64::from(table.timer_cancel(&ram, args[1] as u32)?), 0],
            _ => {
                let mut scan = table.timer_fire_begin(&ram, args[5] as i64)?;
                timer_need(scan.timer_scan_result(&ram) == Err(-2))?;
                loop {
                    match table.timer_fire_next(&ram, &mut scan)? {
                        None => break,
                        Some(entry) => {
                            hash = cases::timer_hash(hash, &u32::to_le_bytes(entry[0]));
                            hash = cases::timer_hash(hash, &u32::to_le_bytes(entry[1]));
                            hash = cases::timer_hash(hash, &table.timers_state(&ram)?[64..]);
                            // Controlled literal callback, after slot publication/unlock.
                            if entry[1] != 0 {
                                timer_action(&table, &mut ram, index, entry)?;
                            }
                        }
                    }
                }
                scan.timer_scan_result(&ram)?
            }
        };
        let mut witness = std::vec::Vec::new();
        for value in [result[0], result[1], ram.read_u64(0x740008)?, hash] {
            witness.extend(u64::to_le_bytes(value));
        }
        witness.extend(&table.timers_state(&ram)?[64..]);
        timer_need(witness.len() == 800)?;
        timer_send(mailbox, &witness)?;
    }
    // A scan can be aborted but must never report a completed prefix afterward.
    let mut scan = table.timer_fire_begin(&ram, 4000)?;
    scan.timer_scan_abort(&ram)?;
    timer_need(
        table.timer_fire_next(&ram, &mut scan) == Err(-3)
            && scan.timer_scan_result(&ram) == Err(-3),
    )?;
    Ok(73)
}
pub fn timer_worker(context: u64) -> u64 {
    match timer_worker_inner(context) {
        Ok(value) => value,
        Err(code) => code as i64 as u64,
    }
}
pub fn timer_probe() -> Result<std::vec::Vec<u8>, i32> {
    let mut window = [0u8; 16];
    timer_need(gamepkg::local_call(1, &[], &mut window)? == 16)?;
    let window = timer_word(&window, 0)?;
    gamepkg::local_call(
        2,
        &[
            u64::to_le_bytes(window).as_slice(),
            &u64::to_le_bytes(0),
            &u64::to_le_bytes(4096),
            &u32::to_le_bytes(2),
        ]
        .concat(),
        &mut [],
    )?;
    let mut ram = memory::Memory::new()?;
    ram.attach(0x740000, 4096, window, 0, true)?;
    let services = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2)?;
    let mut pending = queue::Queue::queue_new(&ram, false)?;
    let mut table = timers::Timers::timers_new(&ram, &pending)?;
    timer_need(
        !table.timer_cancel(&ram, 0)?
            && table.timer_set(&mut ram, 0x1000, 0, 0, 0) == Err(-5)
            && pending.queue_join_witness(&ram).err() == Some(-2),
    )?;
    let mailbox = gamepkg::local_mailbox_create(800, 1)?;
    let context = gamepkg::local_record_create(
        &[
            u64::to_le_bytes(ram.call_handle()),
            u64::to_le_bytes(window),
            u64::to_le_bytes(table.timers_id()),
            u64::to_le_bytes(mailbox),
            u64::to_le_bytes(pending.queue_id()),
        ]
        .concat(),
    )?;
    let mut worker = [0u8; 8];
    gamepkg::local_call(
        10,
        &[
            u32::to_le_bytes(0).as_slice(),
            &u64::to_le_bytes(context),
            &u32::to_le_bytes(1),
        ]
        .concat(),
        &mut worker,
    )?;
    let worker = u64::from_le_bytes(worker);
    pending.queue_assign(&ram, worker)?;
    timer_need(pending.queue_join_witness(&ram).err() == Some(-2))?;
    table.timers_start(&ram, &pending)?;
    let mut orphan = timers::Timers::timers_new(&ram, &pending)?;
    orphan.timers_start(&ram, &pending)?;
    timer_need(
        table.timer_fire_begin(&ram, 0).err() == Some(-4)
            && table.timers_close(&ram, None) == Err(-2),
    )?;
    crate::controls::timer_controls(&mut ram, &table)?;
    let mut reply = [0u8; 4];
    gamepkg::local_call(13, &u64::to_le_bytes(worker), &mut reply)?;
    let mut output = std::vec::Vec::new();
    for _ in 0..512u32 {
        let mut witness = [0u8; 800];
        timer_need(gamepkg::local_mailbox_receive(mailbox, 1000, &mut witness)? == 800)?;
        output.extend(witness);
    }
    for _ in 0..1000u32 {
        if gamepkg::local_task_finished(worker)? {
            break;
        }
        memory::Memory::call_retry_pause()?;
    }
    timer_need(pending.queue_join(&ram)? == 73)?;
    let joined = pending.queue_join_witness(&ram)?;

    timer_need(
        joined.joined_for(&ram, pending.queue_id().wrapping_add(1), worker) == Err(-4)
            && joined.joined_for(&ram, pending.queue_id(), worker.wrapping_add(1)) == Err(-4),
    )?;
    timer_need(pending.queue_join(&ram) == Err(-1))?;
    pending.queue_close(&ram)?;
    table.timers_close(&ram, Some(&joined))?;
    timer_need(table.timers_closed())?;
    for index in 0..64u32 {
        let args = cases::timer_clock_case(index);
        let value = crate::production_clock::timer_ms_value(args[0], args[1], args[2])?;
        output.extend(i64::to_le_bytes(value));
    }
    timer_need(
        crate::production_clock::timer_ms_value(0, 0, 0) == Err(-1)
            && crate::production_clock::timer_ms_value(u64::MAX, 1, 0) == Err(-1)
            && crate::production_clock::timer_ms_value(0, u64::MAX, 0) == Err(-1),
    )?;
    gamepkg::local_mailbox_release(mailbox)?;
    gamepkg::local_record_release(context)?;
    services.service_close(&ram)?;
    timer_need(orphan.timers_close(&ram, Some(&joined)) == Err(-4) && orphan.timers_closed())?;
    gamepkg::local_record_release(ram.call_handle())?;
    gamepkg::local_call(
        4,
        &[
            u64::to_le_bytes(window),
            u64::to_le_bytes(0),
            u64::to_le_bytes(4096),
        ]
        .concat(),
        &mut [],
    )?;
    Ok(output)
}
