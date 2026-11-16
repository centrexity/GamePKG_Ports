// Checked fixture: a real SDK worker creates its TIB using a separate heap actor.
use crate::gamepkg;
use crate::heap_service;
use crate::layout;
use crate::memory;
use crate::sha256;
use crate::thread_tib;
fn tib_need(ok: bool) -> Result<(), i32> {
    if ok {
        Ok(())
    } else {
        Err(-3)
    }
}
pub fn tib_case(mode: u32) -> Result<[u32; 3], i32> {
    let total = match mode {
        0 => 0,
        1 => 4,
        2 => 20,
        3 => 148,
        4 => 516,
        5 => 65524,
        6 => 20,
        _ => return Err(-1),
    };
    Ok([
        0x770000,
        total,
        if mode == 6 { 0x1000010 } else { 0x2000000 },
    ])
}
fn tib_spawn(entry: u32, value: u64) -> Result<u64, i32> {
    let mut request = std::vec::Vec::new();
    request.extend(u32::to_le_bytes(entry));
    request.extend(u64::to_le_bytes(value));
    request.extend(u32::to_le_bytes(0));
    let mut out = [0u8; 8];
    tib_need(gamepkg::local_call(10, &request, &mut out)? == 8)?;
    Ok(u64::from_le_bytes(out))
}
fn tib_join(job: u64) -> Result<(), i32> {
    let pause = gamepkg::local_mailbox_create(1, 1)?;
    let mut byte = [0u8; 1];
    loop {
        match gamepkg::local_task_join(job) {
            Ok(proof) => {
                gamepkg::local_mailbox_release(pause)?;
                return tib_need(proof.result() == 0);
            }
            Err(-2) => {
                tib_need(gamepkg::local_mailbox_receive(pause, 1, &mut byte) == Err(-2))?;
            }
            Err(code) => return Err(code),
        }
    }
}
fn tib_worker_run(inbox: u64) -> Result<(), i32> {
    let mut message = [0u8; 48];
    tib_need(gamepkg::local_mailbox_receive(inbox, 1000, &mut message)? == 48)?;
    let mut ids = [0u64; 4];
    for at in 0..4usize {
        let b = message.get(at * 8..at * 8 + 8).ok_or(-1)?;
        ids[at] = u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]);
    }
    let source = memory::Memory::with_context(ids)?;
    let plan = layout::Layout::layout_current(&source)?;
    let mut ram = plan.layout_view(&source)?;
    let template = u32::from_le_bytes([message[32], message[33], message[34], message[35]]);
    let total = u32::from_le_bytes([message[36], message[37], message[38], message[39]]);
    let reply = u64::from_le_bytes([
        message[40],
        message[41],
        message[42],
        message[43],
        message[44],
        message[45],
        message[46],
        message[47],
    ]);
    let first = thread_tib::thread_tib_new(&mut ram, template, total)?;
    // Inherit current source bytes, rather than a constructor-time snapshot.
    ram.write_u32(0x4010, 0x12345678)?;
    if total != 0 {
        ram.write_u8(template + total - 1, 0x6d)?;
    }
    let second = thread_tib::thread_tib_new(&mut ram, template, total)?;
    ram.heap_close()?;
    let mut values = [0u8; 8];
    let addresses = [first, second];
    for at in 0..2usize {
        let bytes = u32::to_le_bytes(addresses[at]);
        for lane in 0..4usize {
            values[at * 4 + lane] = bytes[lane];
        }
    }
    gamepkg::local_mailbox_send(reply, &values)
}
pub fn tib_worker(inbox: u64) -> u64 {
    match tib_worker_run(inbox) {
        Ok(()) => 0,
        Err(code) => code as i64 as u64,
    }
}
pub fn tib_sample(mode: u32) -> Result<std::vec::Vec<u8>, i32> {
    let case = tib_case(mode)?;
    let root = memory::Memory::new()?;
    let plan = layout::Layout::layout_current(&root)?;
    let main = plan.layout_banks()[0];
    let mut request = std::vec::Vec::new();
    request.extend(u64::to_le_bytes(main));
    request.extend(u64::to_le_bytes(0));
    request.extend(u64::to_le_bytes(67108864));
    gamepkg::local_call(4, &request, &mut [])?;
    request.extend(u32::to_le_bytes(2));
    gamepkg::local_call(2, &request, &mut [])?;
    let mut ram = plan.layout_view(&root)?;
    ram.fill(0, 0xa5, 67108864)?;
    for at in 0..64u32 {
        ram.write_u8(0x4000 + at, (at * 37 + mode * 11) as u8)?;
    }
    for at in 0..case[1] {
        ram.write_u8(case[0] + at, (at * 71 + mode * 13) as u8)?;
    }
    tib_need(thread_tib::thread_tib_new(&mut ram, u32::MAX, 0) == Ok(0))?;
    tib_need(thread_tib::thread_tib_new(&mut ram, case[0], 4) == Err(-5))?;
    let heap_channel = heap_service::heap_inbox()?;
    let heap_job = tib_spawn(0, heap_channel)?;
    let mut client = heap_service::HeapClient::heap_client(heap_channel, heap_job)?;
    client.heap_initialize(ram.call_handle(), main, 0x1000000, case[2])?;
    let mut primary = Some(client);
    ram.heap_install(&mut primary)?;
    // Invalid metadata rejects before allocation. No TLS ignores its source.
    for values in [
        [case[0], 1],
        [case[0], 16],
        [case[0], 65540],
        [0, 4],
        [0xfffffff0, 20],
    ] {
        tib_need(thread_tib::thread_tib_new(&mut ram, values[0], values[1]) == Err(-1))?;
    }
    let inbox = gamepkg::local_mailbox_create(48, 1)?;
    let reply = gamepkg::local_mailbox_create(8, 1)?;
    let mut message = std::vec::Vec::new();
    for id in ram.thread_context() {
        message.extend(u64::to_le_bytes(id));
    }
    message.extend(u32::to_le_bytes(case[0]));
    message.extend(u32::to_le_bytes(case[1]));
    message.extend(u64::to_le_bytes(reply));
    gamepkg::local_mailbox_send(inbox, &message)?;
    let worker = tib_spawn(1, inbox)?;
    let mut values = [0u8; 8];
    tib_need(gamepkg::local_mailbox_receive(reply, 1000, &mut values)? == 8)?;
    tib_join(worker)?;
    gamepkg::local_mailbox_release(inbox)?;
    gamepkg::local_mailbox_release(reply)?;
    let mut owner = ram.heap_take()?;
    let stats = owner.heap_status();
    ram.heap_put(owner);
    let stats = stats?;
    let mut witness = std::vec::Vec::new();
    witness.extend(values);
    for value in stats {
        witness.extend(u32::to_le_bytes(value));
    }
    let mut hash = sha256::Sha256::sha_new();
    let mut bytes = std::vec::Vec::new();
    bytes.resize(1048576, 0u8);
    for page in 0..64u32 {
        ram.read_bytes(page * 1048576, &mut bytes)?;
        hash.sha_update(&bytes)?;
    }
    witness.extend(hash.sha_finish());
    ram.heap_close()?;
    tib_join(heap_job)?;
    gamepkg::local_record_release(ram.call_handle())?;
    tib_need(thread_tib::thread_tib_new(&mut ram, 0, 0) == Err(-4))?;
    Ok(witness)
}
pub fn tib_probe() -> Result<std::vec::Vec<u8>, i32> {
    let mut bytes = std::vec::Vec::new();
    for mode in 0..7u32 {
        bytes.extend(tib_sample(mode)?);
    }
    Ok(bytes)
}
