// Checked game-source fixture. No clock injection, native pointers or OS calls.
use crate::gamepkg;
use crate::kernel_data;
use crate::kernel_pool;
use crate::memory;
use crate::tick_service;
fn tick_pause() -> Result<(), i32> {
    memory::Memory::call_retry_pause()
}
fn tick_close(owner: &mut tick_service::Tick, ram: &memory::Memory) -> Result<(), i32> {
    for _ in 0..4096u32 {
        match owner.tick_close(ram) {
            Err(-2) => tick_pause()?,
            result => {
                if !owner.tick_closed() {
                    return Err(-3);
                }
                return result;
            }
        }
    }
    Err(-2)
}
fn tick_finish(owner: &tick_service::Tick, ram: &memory::Memory) -> Result<(), i32> {
    for _ in 0..4096u32 {
        if owner.tick_finished(ram)? {
            return Ok(());
        }
        tick_pause()?;
    }
    Err(-2)
}
fn tick_protect(window: u64, mode: u32) -> Result<(), i32> {
    let request = [
        u64::to_le_bytes(window),
        u64::to_le_bytes(0),
        u64::to_le_bytes(4096),
    ]
    .concat();
    let request = [request.as_slice(), &u32::to_le_bytes(mode)].concat();
    gamepkg::local_call(3, &request, &mut [])?;
    Ok(())
}
fn tick_word(ram: &memory::Memory) -> Result<u32, i32> {
    ram.read_u32(0x740020)
}
fn tick_observe(ram: &memory::Memory, skew: u64) -> Result<u32, i32> {
    let before = gamepkg::local_performance_sample()?;
    let low = tick_service::tick_value(before.uptime_ms, skew);
    for _ in 0..4096u32 {
        let word = tick_word(ram)?;
        let after = gamepkg::local_performance_sample()?;
        let span = after.uptime_ms.checked_sub(before.uptime_ms).ok_or(-3)?;
        if span > u64::from(u32::MAX) {
            return Err(-3);
        }
        if u64::from(word.wrapping_sub(low)) <= span {
            return Ok(word);
        }
        tick_pause()?;
    }
    Err(-2)
}
fn tick_startup() -> Result<
    (
        memory::Memory,
        kernel_data::Data,
        kernel_pool::KernelServices,
        u64,
    ),
    i32,
> {
    let mut reply = [0u8; 16];
    if gamepkg::local_call(1, &[], &mut reply)? != 16 {
        return Err(-3);
    }
    let window = u64::from_le_bytes([
        reply[0], reply[1], reply[2], reply[3], reply[4], reply[5], reply[6], reply[7],
    ]);
    let request = [
        u64::to_le_bytes(window),
        u64::to_le_bytes(0),
        u64::to_le_bytes(65536),
    ]
    .concat();
    let request = [request.as_slice(), &u32::to_le_bytes(2)].concat();
    gamepkg::local_call(2, &request, &mut [])?;
    let mut ram = memory::Memory::new()?;
    ram.attach(0x740000, 65536, window, 0, true)?;
    ram.fill(0x740000, 0xa5, 65536)?;
    let services = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2)?;
    services.service_advance(&ram, 7000000000)?;
    let before = gamepkg::local_performance_sample()?;
    let data = kernel_data::Data::data_initialize_live(&mut ram, 0x700000, 0x10000, &[], None)?;
    let after = gamepkg::local_performance_sample()?;
    let word = tick_word(&ram)?;
    if u64::from(word.wrapping_sub(before.uptime_ms as u32))
        > after.uptime_ms.checked_sub(before.uptime_ms).ok_or(-3)?
    {
        return Err(-3);
    }
    if data.data_tick_binding(&ram)? != [ram.call_handle(), window, 32] {
        return Err(-3);
    }
    if kernel_pool::kernel_tsc_state(&ram)? != [0, 0, 0] {
        return Err(-3);
    }
    Ok((ram, data, services, window))
}
fn tick_no_scribbles(ram: &memory::Memory, snapshot: &[u8]) -> Result<(), i32> {
    let mut after = std::vec::Vec::new();
    after.resize(65536, 0);
    ram.read_bytes(0x740000, &mut after)?;
    if snapshot.len() != 65536 || after[..32] != snapshot[..32] || after[36..] != snapshot[36..] {
        return Err(-3);
    }
    Ok(())
}
fn tick_bad_record(bytes: &[u8], expected: u64) -> Result<(), i32> {
    let record = gamepkg::local_record_create(bytes)?;
    let request = [
        u32::to_le_bytes(0).as_slice(),
        &u64::to_le_bytes(record),
        &u32::to_le_bytes(0),
    ]
    .concat();
    let mut result = [0u8; 8];
    if gamepkg::local_call(10, &request, &mut result)? != 8 {
        return Err(-3);
    }
    let task = u64::from_le_bytes(result);
    let mut joined = false;
    for _ in 0..4096u32 {
        match gamepkg::local_call(11, &u64::to_le_bytes(task), &mut result) {
            Ok(8) => {
                joined = true;
                break;
            }
            Err(-2) => tick_pause()?,
            Ok(_) => return Err(-3),
            Err(code) => return Err(code),
        }
    }
    gamepkg::local_record_release(record)?;
    if !joined || u64::from_le_bytes(result) != expected {
        return Err(-3);
    }
    Ok(())
}
fn tick_quota_controls(ram: &memory::Memory, data: &kernel_data::Data) -> Result<(), i32> {
    let mut owner = tick_service::Tick::tick_new();
    let mut records = std::vec::Vec::new();
    for _ in 0..65u32 {
        match gamepkg::local_record_create(&[0]) {
            Ok(id) => records.push(id),
            Err(-2) => break,
            Err(code) => return Err(code),
        }
    }
    if records.is_empty() || records.len() > 64 || owner.tick_start(ram, data, 0) != Err(-2) {
        return Err(-3);
    }
    tick_close(&mut owner, ram)?;
    for record in records {
        gamepkg::local_record_release(record)?;
    }
    let mut events = std::vec::Vec::new();
    for _ in 0..129u32 {
        let mut reply = [0u8; 8];
        match gamepkg::local_call(20, &[0u8; 8], &mut reply) {
            Ok(8) => events.push(u64::from_le_bytes(reply)),
            Err(-2) => break,
            Ok(_) => return Err(-3),
            Err(code) => return Err(code),
        }
    }
    if events.is_empty() || events.len() > 128 || owner.tick_start(ram, data, 0) != Err(-2) {
        return Err(-3);
    }
    tick_close(&mut owner, ram)?;
    for event in events {
        gamepkg::local_call(25, &u64::to_le_bytes(event), &mut [])?;
    }
    let mut tasks = std::vec::Vec::new();
    for _ in 0..33u32 {
        let request = [
            u32::to_le_bytes(0).as_slice(),
            &u64::to_le_bytes(0),
            &u32::to_le_bytes(1),
        ]
        .concat();
        let mut reply = [0u8; 8];
        match gamepkg::local_call(10, &request, &mut reply) {
            Ok(8) => tasks.push(u64::from_le_bytes(reply)),
            Err(-2) => break,
            Ok(_) => return Err(-3),
            Err(code) => return Err(code),
        }
    }
    if tasks.is_empty() || tasks.len() > 32 || owner.tick_start(ram, data, 0) != Err(-2) {
        return Err(-3);
    }
    tick_close(&mut owner, ram)?;
    for task in tasks {
        gamepkg::local_call(12, &u64::to_le_bytes(task), &mut [])?;
        let mut joined = false;
        for _ in 0..4096u32 {
            let mut reply = [0u8; 8];
            match gamepkg::local_call(11, &u64::to_le_bytes(task), &mut reply) {
                Ok(8) => {
                    if reply != [0u8; 8] {
                        return Err(-3);
                    }
                    joined = true;
                    break;
                }
                Err(-2) => tick_pause()?,
                Ok(_) => return Err(-3),
                Err(code) => return Err(code),
            }
        }
        if !joined {
            return Err(-2);
        }
    }
    owner.tick_start(ram, data, 0)?;
    tick_close(&mut owner, ram)
}
fn tick_metadata_controls(
    ram: &memory::Memory,
    data: &kernel_data::Data,
    services: &kernel_pool::KernelServices,
) -> Result<(), i32> {
    let binding = data.data_tick_binding(ram)?;
    let mut reply = [0u8; 8];
    if gamepkg::local_call(20, &[0u8; 8], &mut reply)? != 8 {
        return Err(-3);
    }
    let event = u64::from_le_bytes(reply);
    let mut tick_bytes = std::vec::Vec::new();
    for value in [
        0x314b4354u64,
        binding[0],
        binding[1],
        binding[2],
        event,
        1,
        services.service_handle(),
        0,
    ] {
        tick_bytes.extend(u64::to_le_bytes(value));
    }
    let record = gamepkg::local_record_create(&tick_bytes)?;
    if tick_service::tick_worker(record) != 4 {
        return Err(-3);
    }
    gamepkg::local_record_release(record)?;
    for (at, value) in [
        (0usize, 0u64),
        (0, 0x1000000000000000),
        (56, 1),
        (8, 0),
        (16, 0),
        (24, 1),
        (24, u64::MAX - 3),
        (32, 0),
        (40, 0),
        (48, 0),
    ] {
        let mut bytes = tick_bytes.clone();
        bytes[at..at + 8].copy_from_slice(&u64::to_le_bytes(value));
        tick_bad_record(&bytes, 1)?;
    }
    gamepkg::local_call(25, &u64::to_le_bytes(event), &mut [])?;
    Ok(())
}
pub fn tick_probe() -> Result<(), i32> {
    gamepkg::diagnostic(1, 0x544b1610, &[0])?;
    let (ram, data, services, window) = tick_startup()?;
    let mut snapshot = std::vec::Vec::new();
    snapshot.resize(65536, 0);
    ram.read_bytes(0x740000, &mut snapshot)?;
    let mut owner = tick_service::Tick::tick_new();
    let foreign = memory::Memory::new()?;
    if data.data_tick_binding(&foreign) != Err(-4) {
        return Err(-3);
    }
    let mut readonly = ram.call_view();
    readonly.attach(0x740000, 65536, window, 0, false)?;
    if owner.tick_start(&readonly, &data, 0) != Err(-4) {
        return Err(-3);
    }
    if ram.memory_writable_word_backing(0x740021) != Err(-1) {
        return Err(-3);
    }
    if ram.memory_writable_word_backing(0x750000) != Err(-4) {
        return Err(-3);
    }
    let mut unaligned = ram.call_view();
    unaligned.attach(0x740000, 4096, window, 1, true)?;
    if unaligned.memory_writable_word_backing(0x740020) != Err(-1) {
        return Err(-3);
    }
    tick_protect(window, 1)?;
    if owner.tick_start(&ram, &data, 0) != Err(-4) {
        return Err(-3);
    }
    tick_protect(window, 2)?;
    gamepkg::diagnostic(1, 0x544b1610, &[1])?;
    owner.tick_start(&ram, &data, 0)?;
    if owner.tick_start(&ram, &data, 0) != Err(-1)
        || owner.tick_stop(&foreign) != Err(-4)
        || owner.tick_finished(&foreign) != Err(-4)
        || owner.tick_close(&foreign) != Err(-4)
    {
        return Err(-3);
    }
    gamepkg::diagnostic(1, 0x544b1610, &[2])?;
    tick_observe(&ram, 7000000000)?;
    gamepkg::diagnostic(1, 0x544b1610, &[3])?;
    services.service_advance(&ram, 999999999)?;
    tick_observe(&ram, 7999999999)?;
    if kernel_pool::kernel_tsc_state(&ram)? != [0, 0, 0] {
        return Err(-3);
    }
    // The host deliberately withholds owner dispatch after this diagnostic.
    // The game blocks in the window call while this independent writer runs.
    gamepkg::diagnostic(1, 0x544b1601, &[])?;
    let before = tick_word(&ram)?;
    if gamepkg::fullscreen(true) != Err(-5) {
        return Err(-3);
    }
    let after = tick_word(&ram)?;
    if after.wrapping_sub(before) < 20 {
        return Err(-3);
    }
    gamepkg::diagnostic(1, 0x544b1610, &[4])?;
    tick_close(&mut owner, &ram)?;
    let stopped = tick_word(&ram)?;
    tick_pause()?;
    tick_pause()?;
    if tick_word(&ram)? != stopped || !owner.tick_finished(&ram)? {
        return Err(-3);
    }
    tick_no_scribbles(&ram, &snapshot)?;
    owner.tick_close(&ram)?;
    if !owner.tick_closed() {
        return Err(-3);
    }
    // More cycles than both the record and sync quotas catch leaked ownership.
    gamepkg::diagnostic(1, 0x544b1610, &[5])?;
    for _ in 0..132u32 {
        owner.tick_start(&ram, &data, 0)?;
        tick_close(&mut owner, &ram)?;
        if owner.tick_start(&ram, &data, 999) != Err(-4) || owner.tick_closed() {
            return Err(-3);
        }
        tick_close(&mut owner, &ram)?;
    }
    gamepkg::diagnostic(1, 0x544b1610, &[6])?;
    tick_quota_controls(&ram, &data)?;
    // Revoking host page write access terminates the worker without a fallback.
    owner.tick_start(&ram, &data, 0)?;
    tick_observe(&ram, 7999999999)?;
    tick_protect(window, 1)?;
    let frozen = tick_word(&ram)?;
    tick_finish(&owner, &ram)?;
    if owner.tick_closed() {
        return Err(-3);
    }
    if tick_close(&mut owner, &ram) != Err(-4) || tick_word(&ram)? != frozen {
        return Err(-3);
    }
    tick_protect(window, 2)?;
    owner.tick_close(&ram)?;
    gamepkg::diagnostic(1, 0x544b1610, &[7])?;
    tick_metadata_controls(&ram, &data, &services)?;
    // Old/malformed records and bootstrap-thread calls cannot become workers.
    if tick_service::tick_worker(0) != 4 {
        return Err(-3);
    }
    for bytes in [
        std::vec::Vec::from([0u8; 63]),
        std::vec::Vec::from([0u8; 64]),
        std::vec::Vec::from([0u8; 65]),
    ] {
        tick_bad_record(&bytes, 1)?;
    }
    owner.tick_start(&ram, &data, 0)?;
    tick_observe(&ram, 7999999999)?;
    services.service_close(&ram)?;
    tick_finish(&owner, &ram)?;
    if owner.tick_closed() {
        return Err(-3);
    }
    if tick_close(&mut owner, &ram) != Err(-4) {
        return Err(-3);
    }
    owner.tick_close(&ram)?;
    let replacement = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2)?;
    owner.tick_start(&ram, &data, 0)?;
    tick_observe(&ram, 0)?;
    gamepkg::local_record_release(ram.call_handle())?;
    tick_finish(&owner, &ram)?;
    if owner.tick_closed() {
        return Err(-3);
    }
    if tick_close(&mut owner, &ram) != Err(-4) || replacement.service_close(&ram) != Err(-4) {
        return Err(-3);
    }
    if gamepkg::local_record_snapshot(replacement.service_handle(), 64) != Err(-4) {
        return Err(-3);
    }
    owner.tick_close(&ram)?;
    gamepkg::local_record_release(foreign.call_handle())?;
    gamepkg::diagnostic(1, 0x544b1602, &[stopped as u64, 132])?;
    Ok(())
}
