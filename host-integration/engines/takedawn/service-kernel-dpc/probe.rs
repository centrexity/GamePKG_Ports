// Dependency-free checked fixture: no callbacks are executed by this queue test.
use crate::cases;
use crate::gamepkg;
use crate::kernel_pool;
use crate::memory;
use crate::queue;
fn probe_word(bytes: &[u8], at: usize) -> Result<u64, i32> {
    let b = bytes.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}
fn probe_need(ok: bool) -> Result<(), i32> {
    if ok {
        Ok(())
    } else {
        Err(-3)
    }
}
fn probe_resume(task: u64) -> Result<(), i32> {
    let mut out = [0u8; 4];
    probe_need(
        gamepkg::local_call(13, &u64::to_le_bytes(task), &mut out)? == 4
            && out == u32::to_le_bytes(1),
    )
}
fn probe_worker(context: u64) -> Result<u64, i32> {
    let (_, bytes) = gamepkg::local_record_snapshot(context, 40)?;
    probe_need(bytes.len() == 40)?;
    let mut ram = memory::Memory::with_call_handle(probe_word(&bytes, 0)?)?;
    ram.attach(0x740000, 65536, probe_word(&bytes, 8)?, 0, true)?;
    let owner = queue::Queue::queue_borrow(&ram, probe_word(&bytes, 16)?)?;
    let commands = probe_word(&bytes, 24)?;
    let replies = probe_word(&bytes, 32)?;
    for _ in 0..4096u32 {
        let mut cmd = [0u8; 8];
        probe_need(gamepkg::local_mailbox_receive(commands, 1000, &mut cmd)? == 8)?;
        if u64::from_le_bytes(cmd) == 1 {
            return Ok(0);
        }
        probe_need(u64::from_le_bytes(cmd) == 0 && !owner.queue_pending(&ram)?)?;
        let mut reply = [0u64; 5];
        if let Some(value) = owner.queue_pop(&mut ram)? {
            reply = [1, value[0], value[1], value[2], value[3]];
        }
        let mut bytes = std::vec::Vec::new();
        for word in reply {
            bytes.extend(u64::to_le_bytes(word));
        }
        gamepkg::local_mailbox_send(replies, &bytes)?;
    }
    Err(-2)
}
pub fn dpc_worker(context: u64) -> u64 {
    match probe_worker(context) {
        Ok(v) => v,
        Err(c) => c as i64 as u64,
    }
}
pub fn dpc_probe(stats: bool) -> Result<std::vec::Vec<u8>, i32> {
    let mut windows = [0u8; 512];
    let length = gamepkg::local_call(1, &[], &mut windows)?;
    probe_need(length >= 16 && probe_word(&windows, 8)? >= 65536)?;
    let window = probe_word(&windows, 0)?;
    let request = [
        u64::to_le_bytes(window).as_slice(),
        &u64::to_le_bytes(0),
        &u64::to_le_bytes(65536),
        &u32::to_le_bytes(2),
    ]
    .concat();
    gamepkg::local_call(2, &request, &mut [])?;
    let mut ram = memory::Memory::new()?;
    ram.attach(0x740000, 65536, window, 0, true)?;
    let mut initial = std::vec::Vec::new();
    initial.resize(65536, 0xa5);
    for slot in 0..64usize {
        initial[0x102 + slot * 32] = 0;
    }
    ram.write_bytes(0x740000, &initial)?;
    let services = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2)?;
    let mut owner = queue::Queue::queue_new(&ram, stats)?;
    let mut borrowed = queue::Queue::queue_borrow(&ram, owner.queue_id())?;
    probe_need(
        borrowed.queue_close(&ram) == Err(-4)
            && borrowed.queue_assign(&ram, 0) == Err(-4)
            && borrowed.queue_join(&ram) == Err(-4),
    )?;
    let foreign = memory::Memory::new()?;
    probe_need(owner.queue_state(&foreign) == Err(-4))?;
    gamepkg::local_record_release(foreign.call_handle())?;
    // A forged event cannot be consumed as the queue's lock.
    let mut event = [0u8; 8];
    gamepkg::local_call(
        20,
        &[u32::to_le_bytes(0), u32::to_le_bytes(1)].concat(),
        &mut event,
    )?;
    let event_id = u64::from_le_bytes(event);
    let mut fake = owner.queue_state(&ram)?;
    for (lane, byte) in event.iter().enumerate() {
        fake[24 + lane] = *byte;
    }
    let fake_id = gamepkg::local_record_create(&fake)?;
    probe_need(queue::Queue::queue_borrow(&ram, fake_id).err() == Some(-1))?;
    gamepkg::local_call(
        23,
        &[event.as_slice(), &u32::to_le_bytes(0)].concat(),
        &mut [],
    )?;
    probe_need(
        gamepkg::local_call(
            23,
            &[event.as_slice(), &u32::to_le_bytes(0)].concat(),
            &mut [],
        ) == Err(-2),
    )?;
    gamepkg::local_call(25, &u64::to_le_bytes(event_id), &mut [])?;
    gamepkg::local_record_release(fake_id)?;
    // Guest write denial leaves ring unpublished and releases the mutex.
    if !stats {
        let mut readonly = ram.call_view();
        readonly.attach(0x740000, 65536, window, 0, false)?;
        let before = owner.queue_state(&ram)?;
        probe_need(
            owner.queue_insert(&mut readonly, 0x740100, 1, 2) == Err(-4)
                && owner.queue_state(&ram)? == before,
        )?;
    }
    probe_need(owner.queue_pop(&mut ram) == Err(-4) && owner.queue_assign(&ram, 0) == Err(-1))?;
    let commands = gamepkg::local_mailbox_create(8, 1)?;
    let replies = gamepkg::local_mailbox_create(40, 1)?;
    let context = gamepkg::local_record_create(
        &[
            u64::to_le_bytes(ram.call_handle()),
            u64::to_le_bytes(window),
            u64::to_le_bytes(owner.queue_id()),
            u64::to_le_bytes(commands),
            u64::to_le_bytes(replies),
        ]
        .concat(),
    )?;
    let mut task = [0u8; 8];
    gamepkg::local_call(
        10,
        &[
            u32::to_le_bytes(0).as_slice(),
            &u64::to_le_bytes(context),
            &u32::to_le_bytes(1),
        ]
        .concat(),
        &mut task,
    )?;
    let task = u64::from_le_bytes(task);
    owner.queue_assign(&ram, task)?;
    probe_need(
        owner.queue_close(&ram) == Err(-2)
            && owner.queue_join(&ram) == Err(-2)
            && owner.queue_assign(&ram, task) == Err(-1),
    )?;
    probe_resume(task)?;
    let mut output = std::vec::Vec::new();
    for index in 0..1024u32 {
        let input = cases::queue_case(index);
        let mut value = [0u64; 5];
        match input[0] {
            0 => {
                let result = owner.queue_insert(&mut ram, input[1], input[2], input[3])?;
                value[0] = result[0];
                value[1] = result[1];
            }
            1 => {
                value[0] = u64::from(owner.queue_remove(&mut ram, input[1])?);
            }
            _ => {
                gamepkg::local_mailbox_send(commands, &u64::to_le_bytes(0))?;
                let mut bytes = [0u8; 40];
                probe_need(gamepkg::local_mailbox_receive(replies, 1000, &mut bytes)? == 40)?;
                for lane in 0..5usize {
                    value[lane] = probe_word(&bytes, lane * 8)?;
                }
            }
        }
        let state = owner.queue_state(&ram)?;
        let mut canonical = std::vec::Vec::new();
        canonical.extend(&state[56..72]);
        canonical.extend(&state[80..]);
        let mut bytes = std::vec::Vec::new();
        bytes.resize(65536, 0);
        ram.read_bytes(0x740000, &mut bytes)?;
        output.extend(u64::to_le_bytes(cases::queue_hash(
            14695981039346656037,
            &bytes,
        )));
        output.extend(u64::to_le_bytes(cases::queue_hash(
            14695981039346656037,
            &canonical,
        )));
        for word in value {
            output.extend(u64::to_le_bytes(word));
        }
        output.extend(u64::to_le_bytes(u64::from(owner.queue_pending(&ram)?)));
        output.extend(u64::to_le_bytes(u64::from(
            stats && input[0] == 0 && value[0] == 1,
        )));
    }
    gamepkg::local_mailbox_send(commands, &u64::to_le_bytes(1))?;
    let mut joined = false;
    for _ in 0..4096u32 {
        match owner.queue_join(&ram) {
            Ok(v) => {
                probe_need(v == 0)?;
                joined = true;
                break;
            }
            Err(-2) => memory::Memory::call_retry_pause()?,
            Err(c) => return Err(c),
        }
    }
    probe_need(joined)?;
    let stale = owner.queue_id();
    owner.queue_close(&ram)?;
    probe_need(
        owner.queue_closed()
            && owner.queue_close(&ram) == Ok(())
            && queue::Queue::queue_borrow(&ram, stale).err() == Some(-4),
    )?;
    gamepkg::local_mailbox_release(commands)?;
    gamepkg::local_mailbox_release(replies)?;
    gamepkg::local_record_release(context)?;
    services.service_close(&ram)?;
    gamepkg::local_record_release(ram.call_handle())?;
    Ok(output)
}
// Independent boundary controls, outside the original-C corpus.
pub fn dpc_controls() -> Result<(), i32> {
    let mut windows = [0u8; 512];
    probe_need(gamepkg::local_call(1, &[], &mut windows)? >= 16)?;
    let window = probe_word(&windows, 0)?;
    let request = [
        u64::to_le_bytes(window).as_slice(),
        &u64::to_le_bytes(0),
        &u64::to_le_bytes(65536),
        &u32::to_le_bytes(2),
    ]
    .concat();
    gamepkg::local_call(2, &request, &mut [])?;
    let mut ram = memory::Memory::new()?;
    ram.attach(0x740000, 65536, window, 0, true)?;
    ram.write_u8(0x740102, 0)?;
    let services = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2)?;
    let mut owner = queue::Queue::queue_new(&ram, true)?;
    let initial = owner.queue_state(&ram)?;
    for (offset, value) in [
        (0usize, 0u64),
        (40, 2),
        (48, 2),
        (56, 64),
        (64, 64),
        (72, 1),
        (92, 1),
    ] {
        let mut bad = initial.clone();
        for (lane, byte) in u64::to_le_bytes(value).iter().enumerate() {
            bad[offset + lane] = *byte;
        }
        let id = gamepkg::local_record_create(&bad)?;
        probe_need(queue::Queue::queue_borrow(&ram, id).err() == Some(-1))?;
        gamepkg::local_record_release(id)?;
    }
    for offset in [8usize, 16, 24] {
        let mut bad = initial.clone();
        for lane in 0..8usize {
            bad[offset + lane] = 0;
        }
        let id = gamepkg::local_record_create(&bad)?;
        probe_need(queue::Queue::queue_borrow(&ram, id).err() == Some(-4))?;
        gamepkg::local_record_release(id)?;
    }
    // Record exhaustion after mutex allocation must not leak that mutex.
    let mut records = std::vec::Vec::new();
    let mut exhausted = false;
    for _ in 0..64u32 {
        match gamepkg::local_record_create(&[7u8]) {
            Ok(id) => records.push(id),
            Err(-2) => {
                exhausted = true;
                break;
            }
            Err(c) => return Err(c),
        }
    }
    probe_need(exhausted)?;
    for _ in 0..8u32 {
        probe_need(queue::Queue::queue_new(&ram, false).err() == Some(-2))?;
    }
    for id in records {
        gamepkg::local_record_release(id)?;
    }
    let mut locks = std::vec::Vec::new();
    for _ in 0..128u32 {
        let mut bytes = [0u8; 8];
        match gamepkg::local_call(21, &[], &mut bytes) {
            Ok(8) => locks.push(u64::from_le_bytes(bytes)),
            Err(-2) => break,
            Err(c) => return Err(c),
            _ => return Err(-3),
        }
    }
    probe_need(locks.len() == 127)?;
    for id in locks {
        gamepkg::local_call(25, &u64::to_le_bytes(id), &mut [])?;
    }
    probe_need(owner.queue_state(&ram)? == initial)?;
    let before = crate::wait_service::host_now_ns(&ram)?;
    probe_need(owner.queue_insert(&mut ram, 0x740100, 9, 10)? == [1, 0])?;
    let after = crate::wait_service::host_now_ns(&ram)?;
    let bytes = owner.queue_state(&ram)?;
    let timestamp = probe_word(&bytes, 96)?;
    probe_need(
        before <= timestamp && timestamp <= after && owner.queue_remove(&mut ram, 0x740100)?,
    )?;
    services.service_close(&ram)?;
    probe_need(
        owner.queue_state(&ram) == Err(-4)
            && owner.queue_close(&ram) == Err(-4)
            && owner.queue_closed(),
    )?;
    gamepkg::local_record_release(ram.call_handle())?;
    gamepkg::local_call(
        4,
        &[
            u64::to_le_bytes(window),
            u64::to_le_bytes(0),
            u64::to_le_bytes(65536),
        ]
        .concat(),
        &mut [],
    )?;
    Ok(())
}
