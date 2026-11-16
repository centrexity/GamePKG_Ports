// Checked negative/lifecycle fixture; actual source uses the real SDK, not mocks.
use crate::gamepkg;
use crate::kernel_data;
use crate::kernel_pool;
use crate::memory;
use crate::probe;
use crate::registers;
use crate::tick_service;
fn dispatcher_protect(window: u64, offset: u64, mode: u32) -> Result<(), i32> {
    let request = [
        u64::to_le_bytes(window).as_slice(),
        &u64::to_le_bytes(offset),
        &u64::to_le_bytes(4096),
        &u32::to_le_bytes(mode),
    ]
    .concat();
    gamepkg::local_call(3, &request, &mut [])?;
    Ok(())
}
fn dispatcher_denied(
    ram: &mut memory::Memory,
    window: u64,
    state: &mut registers::Registers,
    slot: u32,
    error: i32,
) -> Result<(), i32> {
    let cpu = state.encode();
    let bytes = probe::dispatcher_snapshot(window)?;
    if kernel_pool::kernel_invoke(state, ram, slot) != Err(error)
        || state.encode() != cpu
        || probe::dispatcher_snapshot(window)? != bytes
    {
        return Err(-3);
    }
    Ok(())
}
fn dispatcher_join(task: u64) -> Result<(), i32> {
    for _ in 0..4096u32 {
        let mut reply = [0u8; 8];
        match gamepkg::local_call(11, &u64::to_le_bytes(task), &mut reply) {
            Ok(8) => {
                return if u64::from_le_bytes(reply) == 16 {
                    Ok(())
                } else {
                    Err(-3)
                }
            }
            Err(-2) => memory::Memory::call_retry_pause()?,
            Ok(_) => return Err(-3),
            Err(code) => return Err(code),
        }
    }
    Err(-2)
}
pub fn dispatcher_worker(record: u64) -> u64 {
    match dispatcher_worker_run(record) {
        Ok(()) => 16,
        Err(code) => code as i64 as u64,
    }
}
fn dispatcher_worker_run(record: u64) -> Result<(), i32> {
    let (_, bytes) = gamepkg::local_record_snapshot(record, 40)?;
    if bytes.len() != 40 {
        return Err(-1);
    }
    let mut values = [0u64; 5];
    for index in 0..5usize {
        let at = index * 8;
        values[index] = u64::from_le_bytes([
            bytes[at],
            bytes[at + 1],
            bytes[at + 2],
            bytes[at + 3],
            bytes[at + 4],
            bytes[at + 5],
            bytes[at + 6],
            bytes[at + 7],
        ]);
    }
    if values[0] == 0 || values[1] == 0 || values[2] == 0 || values[3] > 1 || values[4] != 0 {
        return Err(-1);
    }
    let mut ram = memory::Memory::with_context([values[0], values[1], 0, 0])?;
    probe::dispatcher_attach(&mut ram, values[2])?;
    let stack = 0x1a000 + values[3] as u32 * 4096;
    let ordinal = [107u32, 109][values[3] as usize];
    for _ in 0..8u32 {
        ram.write_u32(stack, 0x12345678)?;
        for index in 1..8u32 {
            ram.write_u32(stack + index * 4, 0)?;
        }
        let mut state = registers::Registers::new();
        state.esp = stack;
        kernel_pool::kernel_invoke(&mut state, &mut ram, probe::dispatcher_slot(ordinal)?)?;
        if state.eax != 0 || state.esp != stack + if ordinal == 107 { 16 } else { 32 } {
            return Err(-3);
        }
    }
    Ok(())
}
fn dispatcher_concurrent(
    ram: &mut memory::Memory,
    window: u64,
    services: &kernel_pool::KernelServices,
) -> Result<(), i32> {
    gamepkg::diagnostic(1, 0x544b1710, &[0])?;
    let _ = probe::dispatcher_reset(ram, window, 0)?;
    let data = kernel_data::Data::data_initialize_live(ram, 0x700000, 0x10000, &[], None)?;
    let mut ticker = tick_service::Tick::tick_new();
    ticker.tick_start(ram, &data, 1)?;
    let _ = kernel_pool::kernel_tsc_read(ram)?;
    let tsc = kernel_pool::kernel_tsc_state(ram)?;
    let mut tasks = [0u64; 2];
    let mut records = [0u64; 2];
    for index in 0..2usize {
        let mut bytes = std::vec::Vec::new();
        for value in [
            ram.call_handle(),
            ram.kernel_handle(),
            window,
            index as u64,
            0,
        ] {
            bytes.extend(u64::to_le_bytes(value));
        }
        let record = gamepkg::local_record_create(&bytes)?;
        records[index] = record;
        let request = [
            u32::to_le_bytes(0).as_slice(),
            &u64::to_le_bytes(record),
            &u32::to_le_bytes(0),
        ]
        .concat();
        let mut reply = [0u8; 8];
        if gamepkg::local_call(10, &request, &mut reply)? != 8 {
            return Err(-3);
        }
        tasks[index] = u64::from_le_bytes(reply);
    }
    services.service_advance(ram, 1000000000)?;
    for index in 0..2usize {
        dispatcher_join(tasks[index])?;
        gamepkg::local_record_release(records[index])?;
    }
    let (_, bytes) = gamepkg::local_record_snapshot(services.service_handle(), 64)?;
    if bytes.len() != 64
        || bytes[4..8] != u32::to_le_bytes(3)
        || kernel_pool::kernel_tsc_state(ram)? != tsc
        || kernel_pool::kernel_service_values(ram)? != [0x10000, 0, 1000000000]
    {
        return Err(-3);
    }
    let sample = gamepkg::local_performance_sample()?;
    let lower = (sample.uptime_ms as u32).wrapping_add(1000);
    let mut observed = false;
    for _ in 0..4096u32 {
        let word = ram.read_u32(data.data_address(ram, 156)?)?;
        let after = gamepkg::local_performance_sample()?;
        if u64::from(word.wrapping_sub(lower))
            <= after.uptime_ms.checked_sub(sample.uptime_ms).ok_or(-3)?
        {
            observed = true;
            break;
        }
        memory::Memory::call_retry_pause()?;
    }
    if !observed || ticker.tick_finished(ram)? {
        return Err(-3);
    }
    for _ in 0..4096u32 {
        match ticker.tick_close(ram) {
            Ok(()) => break,
            Err(-2) => memory::Memory::call_retry_pause()?,
            Err(code) => return Err(code),
        }
    }
    if !ticker.tick_closed() {
        return Err(-2);
    }
    gamepkg::diagnostic(1, 0x544b1710, &[1])?;
    Ok(())
}
pub fn dispatcher_controls() -> Result<(), i32> {
    let (mut ram, services, window) = probe::dispatcher_memory(false)?;
    dispatcher_concurrent(&mut ram, window, &services)?;
    let mut state = probe::dispatcher_reset(&mut ram, window, 13)?; // ordinal 107, ordinary unaligned target
    dispatcher_protect(window, 0x11000, 1)?;
    dispatcher_denied(&mut ram, window, &mut state, 1, -4)?;
    dispatcher_protect(window, 0x11000, 2)?;
    // A complete scatter-clear is checked before touching either page.
    ram.write_u32(0x18004, 0x11ff0)?;
    dispatcher_protect(window, 0x12000, 1)?;
    dispatcher_denied(&mut ram, window, &mut state, 1, -4)?;
    dispatcher_protect(window, 0x12000, 2)?;
    // A read-only guest view cannot gain access through writable host backing.
    let mut readonly = ram.call_view();
    readonly.attach(0, 131072, window, 0, false)?;
    let mut selected = registers::Registers::new();
    selected.decode(&state.encode())?;
    dispatcher_denied(&mut readonly, window, &mut selected, 1, -4)?;
    // Missing bytes in a split view cannot produce a prefix clear.
    let mut gap = ram.call_view();
    gap.attach(0, 0x12000, window, 0, true)?;
    gap.attach(0x12001, 0xdfff, window, 0x12001, true)?;
    let mut selected = registers::Registers::new();
    selected.decode(&state.encode())?;
    dispatcher_denied(&mut gap, window, &mut selected, 1, -4)?;
    // All seven declared interrupt arguments must be readable before mutation.
    let mut state = probe::dispatcher_reset(&mut ram, window, 14)?;
    ram.write_u32(0x18004, 0x11000)?;
    let mut truncated = ram.call_view();
    truncated.attach(0, 0x18010, window, 0, true)?;
    dispatcher_denied(&mut truncated, window, &mut state, 2, -4)?;
    // The unchecked original IRQL byte output still passes host page access.
    let mut state = probe::dispatcher_reset(&mut ram, window, 12)?;
    ram.write_u32(0x18008, 0x11000)?;
    dispatcher_protect(window, 0x11000, 1)?;
    dispatcher_denied(&mut ram, window, &mut state, 0, -4)?;
    dispatcher_protect(window, 0x11000, 2)?;
    // Old versions and unsupported bits never become valid service state.
    let mut state = probe::dispatcher_reset(&mut ram, window, 1)?;
    for (at, value, expected) in [
        (0usize, 0x3156534bu64, -1i32),
        (0, 0x3256534b, -1),
        (4, 4, -1),
        (8, 0, -4),
        (16, 1, -1),
        (56, 1, -1),
    ] {
        let (version, original) = gamepkg::local_record_snapshot(services.service_handle(), 64)?;
        let mut changed = original.clone();
        let encoded = u64::to_le_bytes(value);
        for byte in 0..8usize {
            changed[at + byte] = encoded[byte];
        }
        gamepkg::local_record_compare_exchange(services.service_handle(), version, &changed)?;
        dispatcher_denied(&mut ram, window, &mut state, 1, expected)?;
        let (version, _) = gamepkg::local_record_snapshot(services.service_handle(), 64)?;
        gamepkg::local_record_compare_exchange(services.service_handle(), version, &original)?;
    }
    let foreign = memory::Memory::new()?;
    let other = kernel_pool::kernel_services_initialize(&foreign, 0x10000, 2)?;
    ram.services_link(services.service_handle(), other.service_handle())?;
    dispatcher_denied(&mut ram, window, &mut state, 1, -4)?;
    ram.services_link(other.service_handle(), services.service_handle())?;
    other.service_close(&foreign)?;
    gamepkg::local_record_release(foreign.call_handle())?;
    // Releasing the configuration also denies a valid target, before a clear.
    ram.write_u32(0x18004, 0x11000)?;
    gamepkg::local_record_release(services.service_handle())?;
    dispatcher_denied(&mut ram, window, &mut state, 1, -4)?;
    services.service_close(&ram)?;
    ram.kernel_clear()?;
    ram.write_u32(0x10000, 0x8000002c)?;
    kernel_pool::kernel_prepare(&mut ram, 0x10000, 1)?;
    ram.write_u32(0x18004, 0x12345678)?;
    ram.write_u32(0x18008, 0x11000)?;
    let mut state = registers::Registers::new();
    state.esp = 0x18000;
    kernel_pool::kernel_invoke(&mut state, &mut ram, 0)?;
    if state.eax != 0x12345678 || state.esp != 0x1800c || ram.read_u8(0x11000)? != 0x78 {
        return Err(-3);
    }
    ram.kernel_clear()?;
    ram.write_u32(0x10000, 0x8000006b)?;
    ram.write_u32(0x10004, 0x8000006d)?;
    if kernel_pool::kernel_prepare(&mut ram, 0x10000, 2) != Err(-5)
        || ram.kernel_handle() != 0
        || ram.read_u32(0x10000)? != 0x8000006b
        || ram.read_u32(0x10004)? != 0x8000006d
    {
        return Err(-3);
    }
    probe::dispatcher_cleanup(&mut ram, &services)?;
    gamepkg::diagnostic(1, 0x544b1710, &[2])?;
    Ok(())
}
