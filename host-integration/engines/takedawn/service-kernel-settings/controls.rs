use crate::gamepkg;
use crate::kernel_pool;
use crate::memory;
use crate::probe;
pub fn settings_worker(root: u64) -> u64 {
    let ram = match memory::Memory::with_call_handle(root) {
        Ok(v) => v,
        Err(code) => return code as i64 as u64,
    };
    for _ in 0..64usize {
        if let Err(code) = kernel_pool::kernel_advance_services(&ram, 1) {
            return code as i64 as u64;
        }
    }
    64
}
pub fn settings_trace_worker(root: u64) -> u64 {
    let ram = match memory::Memory::with_call_handle(root) {
        Ok(v) => v,
        Err(code) => return code as i64 as u64,
    };
    for _ in 0..512usize {
        if let Err(code) = ram.call_begin(0xfe000000) {
            return code as i64 as u64;
        }
    }
    512
}
fn settings_denied(ram: &mut memory::Memory, step: [u32; 6], code: i32) -> Result<(), i32> {
    probe::settings_arguments(ram, step)?;
    let mut state = probe::settings_state(1, 0);
    let before = state.encode();
    if kernel_pool::kernel_invoke(&mut state, ram, 0) != Err(code) || state.encode() != before {
        return Err(-3);
    }
    Ok(())
}
fn settings_protect(ram: &memory::Memory, address: u32, mode: u32) -> Result<(), i32> {
    let backing = ram.memory_page_backing(address)?;
    let mut request = std::vec::Vec::new();
    request.extend(u64::to_le_bytes(backing[0]));
    request.extend(u64::to_le_bytes(backing[1]));
    request.extend(u64::to_le_bytes(4096));
    request.extend(u32::to_le_bytes(mode));
    gamepkg::local_call(3, &request, &mut [])?;
    Ok(())
}
fn settings_join(job: u64) -> Result<u64, i32> {
    for _ in 0..1000usize {
        let mut out = [0u8; 8];
        match gamepkg::local_call(11, &u64::to_le_bytes(job), &mut out) {
            Ok(8) => return Ok(u64::from_le_bytes(out)),
            Err(-2) => memory::Memory::call_retry_pause()?,
            Err(code) => return Err(code),
            _ => return Err(-3),
        }
    }
    Err(-2)
}
fn settings_init_error(ram: &memory::Memory, speakers: u32, code: i32) -> bool {
    match kernel_pool::kernel_services_initialize(ram, 0, speakers) {
        Err(found) => found == code,
        Ok(_) => false,
    }
}
pub fn settings_denials() -> Result<(), i32> {
    let (mut ram, services) = probe::settings_memory(1)?;
    let id = services.service_handle();
    let original_values = kernel_pool::kernel_service_values(&ram)?;
    let view = memory::Memory::with_context(ram.thread_context())?;
    if kernel_pool::kernel_service_values(&view)? != original_values
        || view.services_handle()? != id
    {
        return Err(-3);
    }
    gamepkg::diagnostic(1, 0x544b12fd, &[1u64])?;
    // Root trace/bounds updates retain the trailing shared ID.
    ram.call_bounds(0x10000, 0x20000)?;
    ram.call_begin(0xfe000000)?;
    if ram.services_handle()? != id {
        return Err(-3);
    }
    gamepkg::diagnostic(1, 0x544b12fd, &[2u64])?;
    // Neither old/truncated nor oversized root schemas can be admitted.
    for length in [280usize, 287, 289] {
        let (version, original) = gamepkg::local_record_snapshot(ram.call_handle(), 288)?;
        let mut changed = std::vec::Vec::new();
        changed.extend(&original);
        changed.resize(length, 0u8);
        if gamepkg::local_record_compare_exchange(ram.call_handle(), version, &changed) != Err(-1)
            || gamepkg::local_record_snapshot(ram.call_handle(), 288)? != (version, original)
        {
            return Err(-3);
        }
        let malformed = gamepkg::local_record_create(&changed)?;
        if memory::Memory::with_call_handle(malformed).is_ok() {
            return Err(-3);
        }
        gamepkg::local_record_release(malformed)?;
    }
    // A stale owner cannot advance a different configuration, even with the
    // same root and same record version. Pin the selected ID through CAS.
    let (_, original) = gamepkg::local_record_snapshot(id, 64)?;
    let mut replacement = std::vec::Vec::new();
    replacement.extend(&original);
    replacement[16..20].copy_from_slice(&u32::to_le_bytes(0));
    let alternate = gamepkg::local_record_create(&replacement)?;
    ram.services_link(id, alternate)?;
    let before = gamepkg::local_record_snapshot(alternate, 64)?;
    if services.service_advance(&ram, 1) != Err(-4)
        || gamepkg::local_record_snapshot(alternate, 64)? != before
    {
        return Err(-3);
    }
    ram.services_link(alternate, id)?;
    gamepkg::local_record_release(alternate)?;
    let foreign = memory::Memory::new()?;
    let foreign_services = kernel_pool::kernel_services_initialize(&foreign, 0, 2)?;
    ram.services_link(id, foreign_services.service_handle())?;
    if kernel_pool::kernel_service_values(&ram) != Err(-4)
        || services.service_advance(&ram, 1) != Err(-4)
        || foreign_services.service_advance(&ram, 1) != Err(-4)
    {
        return Err(-3);
    }
    ram.services_link(foreign_services.service_handle(), id)?;
    foreign_services.service_close(&foreign)?;
    gamepkg::local_record_release(foreign.call_handle())?;
    gamepkg::diagnostic(1, 0x544b12fd, &[3u64])?;
    // Malformed typed service records fail before any output/CPU mutation.
    for (at, value, code) in [
        (0usize, 0u64, -1i32),
        (0, 0x3156534b, -1),
        (4, 1, -1),
        (8, 0, -4),
        (16, 1, -1),
        (20, 1, -1),
        (32, 1, -1),
        (40, 1, -1),
        (48, 1, -1),
        (56, 1, -1),
    ] {
        let (version, original) = gamepkg::local_record_snapshot(id, 64)?;
        let mut changed = std::vec::Vec::new();
        changed.extend(&original);
        let bytes = u64::to_le_bytes(value);
        let count = if at == 0 || at == 4 || at == 16 || at == 20 {
            4
        } else {
            8
        };
        for index in 0..count {
            changed[at + index] = bytes[index];
        }
        let next = gamepkg::local_record_compare_exchange(id, version, &changed)?;
        settings_denied(&mut ram, [24, 7, 0x11000, 0x12000, 4, 0x14000], code)?;
        gamepkg::local_record_compare_exchange(id, next, &original)?;
    }
    for length in [63usize, 65] {
        let (version, original) = gamepkg::local_record_snapshot(id, 64)?;
        let mut changed = std::vec::Vec::new();
        changed.extend(&original);
        changed.resize(length, 0u8);
        if gamepkg::local_record_compare_exchange(id, version, &changed) != Err(-1)
            || gamepkg::local_record_snapshot(id, 64)? != (version, original)
        {
            return Err(-3);
        }
        let malformed = gamepkg::local_record_create(&changed)?;
        ram.services_link(id, malformed)?;
        if kernel_pool::kernel_service_values(&ram) != Err(-1) {
            return Err(-3);
        }
        ram.services_link(malformed, id)?;
        gamepkg::local_record_release(malformed)?;
    }
    gamepkg::diagnostic(1, 0x544b12fd, &[4u64])?;
    // A full record quota cannot leak a half-linked configuration.
    let mut empty = memory::Memory::new()?;
    let backing = ram.memory_page_backing(0)?;
    empty.attach(0, 262144, backing[0], 0, true)?;
    let mut held = [0u64; 64];
    let mut count = 0usize;
    while count < 64 {
        match gamepkg::local_record_create(&[0u8]) {
            Ok(id) => {
                held[count] = id;
                count += 1;
            }
            Err(-2) => break,
            Err(code) => return Err(code),
        }
    }
    if !settings_init_error(&empty, 2, -2) || empty.services_handle()? != 0 {
        return Err(-3);
    }
    for id in &held[..count] {
        gamepkg::local_record_release(*id)?;
    }
    if !settings_init_error(&empty, 4, -1) {
        return Err(-3);
    }
    empty.write_u32(0x10000, 0x80000018)?;
    if kernel_pool::kernel_prepare(&mut empty, 0x10000, 1) != Err(-5)
        || empty.read_u32(0x10000)? != 0x80000018
    {
        return Err(-3);
    }
    empty.write_u32(0x10000, 0x8000000e)?;
    kernel_pool::kernel_prepare(&mut empty, 0x10000, 1)?;
    if !settings_init_error(&empty, 2, -1) {
        return Err(-3);
    }
    empty.kernel_clear()?;
    gamepkg::local_record_release(empty.call_handle())?;
    ram.write_u32(0x10000, 0xfe000000)?;
    gamepkg::diagnostic(1, 0x544b12fd, &[5u64])?;
    // Guest-write failures preserve CPU; prior stores are not rolled back.
    settings_protect(&ram, 0x12000, 1)?;
    settings_denied(&mut ram, [24, 7, 0x11000, 0x12000, 4, 0x14000], -4)?;
    settings_protect(&ram, 0x12000, 2)?;
    settings_protect(&ram, 0x11000, 1)?;
    settings_denied(&mut ram, [24, 7, 0x11000, 0x12000, 4, 0x14000], -4)?;
    if ram.read_u32(0x12000)? != 1 {
        return Err(-3);
    }
    settings_protect(&ram, 0x11000, 2)?;
    settings_protect(&ram, 0x14000, 1)?;
    settings_denied(&mut ram, [24, 8, 0x11000, 0x12000, 4, 0x14000], -4)?;
    if ram.read_u32(0x12000)? != original_values[0] as u32 || ram.read_u32(0x11000)? != 4 {
        return Err(-3);
    }
    settings_protect(&ram, 0x14000, 2)?;
    settings_denied(&mut ram, [24, 7, 0, 0xfffffffe, 4, 0], -1)?;
    settings_denied(&mut ram, [24, 7, 0, 0x40000, 4, 0], -4)?;
    settings_denied(&mut ram, [24, 0xffffffff, 0, 0x12000, 0xffffffff, 0], -1)?;
    gamepkg::diagnostic(1, 0x544b12fd, &[6u64])?;
    // A logical output may cross adjacent guest views with different physical
    // offsets. The readonly second view must still deny one complete chunk.
    let backing = ram.memory_page_backing(0)?;
    let mut split = ram.call_view();
    split.attach(0, 0x21000, backing[0], 0, true)?;
    split.attach(0x21000, 4096, backing[0], 0x30000, true)?;
    ram.fill(0x20ff0, 0xa5, 32)?;
    ram.fill(0x30000, 0xa5, 16)?;
    probe::settings_arguments(&mut ram, [24, 0xffffffff, 0, 0x20ff0, 32, 0])?;
    let mut state = probe::settings_state(1, 0);
    kernel_pool::kernel_invoke(&mut state, &mut split, 0)?;
    let mut first = [0u8; 16];
    let mut second = [0u8; 16];
    let mut gap = [0u8; 16];
    ram.read_bytes(0x20ff0, &mut first)?;
    ram.read_bytes(0x30000, &mut second)?;
    ram.read_bytes(0x21000, &mut gap)?;
    if first != [0u8; 16] || second != [0u8; 16] || gap != [0xa5u8; 16] {
        return Err(-3);
    }
    ram.fill(0x20ff0, 0xa5, 16)?;
    let mut readonly = ram.call_view();
    readonly.attach(0, 0x21000, backing[0], 0, true)?;
    readonly.attach(0x21000, 4096, backing[0], 0x30000, false)?;
    let mut state = probe::settings_state(1, 0);
    let before = state.encode();
    if kernel_pool::kernel_invoke(&mut state, &mut readonly, 0) != Err(-4)
        || state.encode() != before
    {
        return Err(-3);
    }
    ram.read_bytes(0x20ff0, &mut first)?;
    if first != [0xa5u8; 16] {
        return Err(-3);
    }
    // All task views read one shared offset and preserve the same root metadata.
    let mut jobs = [0u64; 4];
    for job in &mut jobs {
        let mut req = std::vec::Vec::new();
        req.extend(u32::to_le_bytes(0));
        req.extend(u64::to_le_bytes(ram.call_handle()));
        req.extend(u32::to_le_bytes(0));
        let mut reply = [0u8; 8];
        if gamepkg::local_call(10, &req, &mut reply)? != 8 {
            return Err(-3);
        }
        *job = u64::from_le_bytes(reply);
    }
    for _ in 0..256usize {
        ram.call_begin(0xfe000000)?;
        if view.services_handle()? != id {
            return Err(-3);
        }
    }
    for job in jobs {
        if settings_join(job)? != 64 {
            return Err(-3);
        }
    }
    gamepkg::diagnostic(1, 0x544b12fd, &[7u64])?;
    if kernel_pool::kernel_service_values(&view)?[2] != 256 {
        return Err(-3);
    }
    // Root metadata publication itself races trace writes via the same CAS.
    // No settings users run while the pointer is deliberately absent here.
    let trace_before = ram.call_snapshot()?.2;
    let mut tracing = [0u64; 2];
    for job in &mut tracing {
        let mut req = std::vec::Vec::new();
        req.extend(u32::to_le_bytes(1));
        req.extend(u64::to_le_bytes(ram.call_handle()));
        req.extend(u32::to_le_bytes(0));
        let mut reply = [0u8; 8];
        if gamepkg::local_call(10, &req, &mut reply)? != 8 {
            return Err(-3);
        }
        *job = u64::from_le_bytes(reply);
    }
    for _ in 0..128usize {
        ram.services_link(id, 0)?;
        ram.services_link(0, id)?;
    }
    for job in tracing {
        if settings_join(job)? != 512 {
            return Err(-3);
        }
    }
    if ram.call_snapshot()?.2 != trace_before + 1024
        || view.services_handle()? != id
        || kernel_pool::kernel_service_values(&view)?[2] != 256
    {
        return Err(-3);
    }
    if services.service_advance(&ram, u64::MAX - 256)? != u64::MAX {
        return Err(-3);
    }
    let before = gamepkg::local_record_snapshot(id, 64)?;
    if services.service_advance(&ram, 1) != Err(-1)
        || gamepkg::local_record_snapshot(id, 64)? != before
    {
        return Err(-3);
    }
    gamepkg::diagnostic(1, 0x544b12fd, &[8u64])?;
    services.service_close(&ram)?;
    services.service_close(&ram)?;
    if kernel_pool::kernel_service_values(&view) != Err(-5)
        || gamepkg::local_record_snapshot(id, 64) != Err(-4)
    {
        return Err(-3);
    }
    ram.kernel_clear()?;
    let revoked = kernel_pool::kernel_services_initialize(&ram, 0, 2)?;
    let revoked_id = revoked.service_handle();
    gamepkg::local_record_release(ram.call_handle())?;
    if kernel_pool::kernel_service_values(&view) != Err(-4)
        || revoked.service_close(&ram) != Err(-4)
        || gamepkg::local_record_snapshot(revoked_id, 64) != Err(-4)
    {
        return Err(-3);
    }
    Ok(())
}
