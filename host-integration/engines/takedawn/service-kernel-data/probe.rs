// Ordinary checked conformance source; all RAM/tasks are GamePKG-owned.
use crate::cases;
use crate::gamepkg;
use crate::heap_service;
use crate::kernel_data;
use crate::layout;
use crate::memory;
fn exports_spawn(inbox: u64) -> Result<u64, i32> {
    let mut req = std::vec::Vec::new();
    req.extend(u32::to_le_bytes(0));
    req.extend(u64::to_le_bytes(inbox));
    req.extend(u32::to_le_bytes(0));
    let mut out = [0u8; 8];
    if gamepkg::local_call(10, &req, &mut out)? != 8 {
        return Err(-3);
    }
    Ok(u64::from_le_bytes(out))
}
fn exports_join(job: u64) -> Result<(), i32> {
    let pause = gamepkg::local_mailbox_create(1, 1)?;
    let mut result = [0u8; 8];
    let mut byte = [0u8; 1];
    loop {
        match gamepkg::local_call(11, &u64::to_le_bytes(job), &mut result) {
            Ok(8) => {
                gamepkg::local_mailbox_release(pause)?;
                return if u64::from_le_bytes(result) == 0 {
                    Ok(())
                } else {
                    Err(-3)
                };
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
fn exports_memory() -> Result<(memory::Memory, u64), i32> {
    let root = memory::Memory::new()?;
    let plan = layout::Layout::layout_current(&root)?;
    let main = plan.layout_banks()[0];
    let mut req = std::vec::Vec::new();
    req.extend(u64::to_le_bytes(main));
    req.extend(u64::to_le_bytes(0));
    req.extend(u64::to_le_bytes(67108864));
    // Synthetic cases own the complete backing and have stopped/joined their
    // previous actor. Reset explicitly; commit must keep rejecting live pages.
    gamepkg::local_call(4, &req, &mut [])?;
    req.extend(u32::to_le_bytes(2));
    gamepkg::local_call(2, &req, &mut [])?;
    let mut ram = plan.layout_view(&root)?;
    ram.fill(0, 0xa5, 67108864)?;
    Ok((ram, main))
}
fn exports_protect(window: u64, offset: u64, mode: u32) -> Result<(), i32> {
    let mut req = std::vec::Vec::new();
    req.extend(u64::to_le_bytes(window));
    req.extend(u64::to_le_bytes(offset));
    req.extend(u64::to_le_bytes(4096));
    req.extend(u32::to_le_bytes(mode));
    gamepkg::local_call(3, &req, &mut [])?;
    Ok(())
}
pub fn data_sample(mode: u32) -> Result<std::vec::Vec<u8>, i32> {
    let case = cases::Case::data_case(mode)?;
    let (mut ram, main) = exports_memory()?;
    case.data_seed(&mut ram)?;
    let inbox = heap_service::heap_inbox()?;
    let job = exports_spawn(inbox)?;
    let mut client = heap_service::HeapClient::heap_client(inbox, job)?;
    client.heap_initialize(ram.call_handle(), main, 0x1000000, case.top)?;
    let mut primary = Some(client);
    ram.heap_install(&mut primary)?;
    let initialized = kernel_data::Data::data_initialize(
        &mut ram,
        case.region,
        case.tick,
        case.video,
        &case.command,
        case.hd,
    )?;
    let base = initialized.data_base(&ram)?;
    let launch = initialized.data_launch(&ram)?;
    let mut client = ram.heap_take()?;
    let counters = client.heap_status();
    ram.heap_put(client);
    let counters = counters?;
    let mut witness = std::vec::Vec::new();
    for value in [mode, base, launch, counters[0], counters[1], counters[2]] {
        witness.extend(u32::to_le_bytes(value));
    }
    witness.extend(initialized.data_keys(&ram)?);
    let mut page = std::vec::Vec::new();
    page.resize(4096, 0u8);
    ram.read_bytes(base, &mut page)?;
    witness.extend_from_slice(&page);
    if launch != 0 {
        let mut bytes = std::vec::Vec::new();
        bytes.resize(7168, 0u8);
        ram.read_bytes(launch, &mut bytes)?;
        witness.extend(bytes);
    }
    for ordinal in 0..400u32 {
        witness.extend(u32::to_le_bytes(initialized.data_address(&ram, ordinal)?));
    }
    if initialized.data_address(&ram, u32::MAX)? != 0 {
        return Err(-3);
    }
    for alias in 0..29u32 {
        let mut copied = std::vec::Vec::new();
        copied.resize(4096, 0u8);
        ram.read_bytes(alias * 67108864 + base, &mut copied)?;
        if copied != page {
            return Err(-3);
        }
    }
    let next = case.tick.wrapping_add(123);
    initialized.data_tick(&mut ram, next)?;
    for alias in 0..29u32 {
        if ram.read_u32(alias * 67108864 + base + 0x20)? != next {
            return Err(-3);
        }
    }
    let mut other = memory::Memory::new()?;
    if initialized.data_base(&other) != Err(-4)
        || initialized.data_address(&other, 16) != Err(-4)
        || initialized.data_tick(&mut other, 0) != Err(-4)
        || initialized.data_keys(&other) != Err(-4)
        || initialized.data_launch(&other) != Err(-4)
    {
        return Err(-3);
    }
    gamepkg::local_record_release(other.call_handle())?;
    ram.heap_close()?;
    exports_join(job)?;
    gamepkg::local_record_release(ram.call_handle())?;
    if initialized.data_base(&ram) != Err(-4)
        || initialized.data_tick(&mut ram, 0) != Err(-4)
        || ram.read_u32(base + 0x20)? != next
    {
        return Err(-3);
    }
    Ok(witness)
}
fn exports_unchanged(page: &[u8]) -> bool {
    for byte in page {
        if *byte != 0xa5 {
            return false;
        }
    }
    true
}
fn exports_rejected(result: Result<kernel_data::Data, i32>, expected: i32) -> Result<(), i32> {
    match result {
        Err(code) => {
            if code == expected {
                Ok(())
            } else {
                Err(-3)
            }
        }
        Ok(_) => Err(-3),
    }
}
pub fn data_denials() -> Result<(), i32> {
    let (mut ram, main) = exports_memory()?;
    for region in [0u32, 0x6f0000, 0x700001, 0x3fc0000, u32::MAX] {
        exports_rejected(
            kernel_data::Data::data_initialize(&mut ram, region, 0, 0, &[], None),
            -1,
        )?;
    }
    let mut command = std::vec::Vec::new();
    command.resize(65537, 65u8);
    exports_rejected(
        kernel_data::Data::data_initialize(&mut ram, 0x700000, 0, 0, &command, None),
        -1,
    )?;
    exports_rejected(
        kernel_data::Data::data_initialize(&mut ram, 0x700000, 0, 0, b"command", None),
        -5,
    )?;
    exports_protect(main, 0x740000, 1)?;
    exports_rejected(
        kernel_data::Data::data_initialize(&mut ram, 0x700000, 0, 0, &[], None),
        -4,
    )?;
    let mut page = std::vec::Vec::new();
    page.resize(4096, 0u8);
    ram.read_bytes(0x740000, &mut page)?;
    if !exports_unchanged(&page) {
        return Err(-3);
    }
    exports_protect(main, 0x740000, 2)?;
    ram.write_u32(0x10000, 0x48454258)?;
    ram.write_u32(0x10118, 0x12000)?;
    exports_protect(main, 0x12000, 0)?;
    exports_rejected(
        kernel_data::Data::data_initialize(&mut ram, 0x700000, 0, 0, &[], Some([0u8; 16])),
        -4,
    )?;
    ram.read_bytes(0x740000, &mut page)?;
    if !exports_unchanged(&page) {
        return Err(-3);
    }
    exports_protect(main, 0x12000, 2)?;
    let inbox = heap_service::heap_inbox()?;
    let job = exports_spawn(inbox)?;
    let mut client = heap_service::HeapClient::heap_client(inbox, job)?;
    client.heap_initialize(ram.call_handle(), main, 0x1000000, 67108864)?;
    let before = client.heap_status()?;
    let mut primary = Some(client);
    ram.heap_install(&mut primary)?;
    // A denied allocator clear commits nothing and restores the client.
    exports_protect(main, 0x1000000, 1)?;
    exports_rejected(
        kernel_data::Data::data_initialize(&mut ram, 0x700000, 0, 0, b"command", None),
        -4,
    )?;
    let mut client = ram.heap_take()?;
    let observed = client.heap_status();
    ram.heap_put(client);
    if observed? != before {
        return Err(-3);
    }
    exports_protect(main, 0x1000000, 2)?;
    // Allocation can succeed before final export-page publication fails.
    // Observe its single acceptance for proof, then abort/cleanup, never replay.
    exports_protect(main, 0x740000, 1)?;
    exports_rejected(
        kernel_data::Data::data_initialize(&mut ram, 0x700000, 0, 0, b"command", None),
        -4,
    )?;
    let mut client = ram.heap_take()?;
    let observed = client.heap_status();
    ram.heap_put(client);
    if observed? != [0x1001c00, 1, 1] || ram.read_u32(0x1000000)? != 3 {
        return Err(-3);
    }
    ram.read_bytes(0x740000, &mut page)?;
    if !exports_unchanged(&page) {
        return Err(-3);
    }
    exports_protect(main, 0x740000, 2)?;
    ram.heap_close()?;
    exports_join(job)?;
    gamepkg::local_record_release(ram.call_handle())?;
    exports_rejected(
        kernel_data::Data::data_initialize(&mut ram, 0x700000, 0, 0, &[], None),
        -4,
    )?;
    exports_key_denial()
}
fn exports_key_denial() -> Result<(), i32> {
    let (mut ram, main) = exports_memory()?;
    ram.write_u32(0x10000, 0x48454258)?;
    ram.write_u32(0x10118, 0x12000)?;
    let inbox = heap_service::heap_inbox()?;
    let job = exports_spawn(inbox)?;
    let mut client = heap_service::HeapClient::heap_client(inbox, job)?;
    client.heap_initialize(ram.call_handle(), main, 0x1000000, 67108864)?;
    let mut primary = Some(client);
    ram.heap_install(&mut primary)?;
    exports_protect(main, 0x12000, 0)?;
    exports_rejected(
        kernel_data::Data::data_initialize(&mut ram, 0x700000, 0, 0, b"command", Some([0u8; 16])),
        -4,
    )?;
    let mut client = ram.heap_take()?;
    let observed = client.heap_status();
    ram.heap_put(client);
    if observed? != [0x1001c00, 1, 1] || ram.read_u32(0x1000000)? != 3 {
        return Err(-3);
    }
    let mut page = std::vec::Vec::new();
    page.resize(4096, 0u8);
    ram.read_bytes(0x740000, &mut page)?;
    if !exports_unchanged(&page) {
        return Err(-3);
    }
    exports_protect(main, 0x12000, 2)?;
    ram.heap_close()?;
    exports_join(job)?;
    gamepkg::local_record_release(ram.call_handle())?;
    Ok(())
}
