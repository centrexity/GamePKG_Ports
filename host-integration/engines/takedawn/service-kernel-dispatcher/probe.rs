// Dependency-free checked source. All RAM and diagnostics remain host services.
use crate::cases;
use crate::gamepkg;
use crate::kernel_pool;
use crate::memory;
use crate::registers;
use crate::sha256;
pub fn dispatcher_attach(ram: &mut memory::Memory, window: u64) -> Result<(), i32> {
    for entry in [
        [0u32, 131072, 0],
        [0x3fff000, 4096, 0x20000],
        [0x80000000, 4096, 0x21000],
        [0x83fff000, 4096, 0x22000],
        [0x4000000, 4096, 0x23000],
        [0xf0000000, 4096, 0x21000],
        [0xff000000, 4096, 0x24000],
    ] {
        ram.attach(
            entry[0],
            u64::from(entry[1]),
            window,
            u64::from(entry[2]),
            true,
        )?;
    }
    ram.attach(0x740000, 65536, window, 0x30000, true)?;
    Ok(())
}
pub fn dispatcher_memory(
    commit: bool,
) -> Result<(memory::Memory, kernel_pool::KernelServices, u64), i32> {
    let mut reply = [0u8; 16];
    if gamepkg::local_call(1, &[], &mut reply)? != 16 {
        return Err(-3);
    }
    let window = u64::from_le_bytes([
        reply[0], reply[1], reply[2], reply[3], reply[4], reply[5], reply[6], reply[7],
    ]);
    if commit {
        let request = [
            u64::to_le_bytes(window).as_slice(),
            &u64::to_le_bytes(0),
            &u64::to_le_bytes(262144),
            &u32::to_le_bytes(2),
        ]
        .concat();
        gamepkg::local_call(2, &request, &mut [])?;
    }
    let mut ram = memory::Memory::new()?;
    dispatcher_attach(&mut ram, window)?;
    let services = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2)?;
    Ok((ram, services, window))
}
pub fn dispatcher_reset(
    ram: &mut memory::Memory,
    window: u64,
    mode: u32,
) -> Result<registers::Registers, i32> {
    ram.kernel_clear()?;
    gamepkg::local_memory_fill(window, 0, 262144, 0xa5)?;
    for (slot, ordinal) in [44u32, 107, 109].iter().enumerate() {
        ram.write_u32(0x10000 + slot as u32 * 4, 0x80000000 | ordinal)?;
    }
    kernel_pool::kernel_prepare(ram, 0x10000, 3)?;
    let values = cases::dispatcher_case(mode);
    ram.write_u32(0x18000, 0x12345678)?;
    for index in 1..8usize {
        ram.write_u32(0x18000 + index as u32 * 4, values[index])?;
    }
    Ok(cases::dispatcher_state(mode))
}
pub fn dispatcher_slot(ordinal: u32) -> Result<u32, i32> {
    match ordinal {
        44 => Ok(0),
        107 => Ok(1),
        109 => Ok(2),
        _ => Err(-5),
    }
}
pub fn dispatcher_snapshot(window: u64) -> Result<std::vec::Vec<u8>, i32> {
    let request = [
        u64::to_le_bytes(window).as_slice(),
        &u64::to_le_bytes(0),
        &u32::to_le_bytes(262144),
    ]
    .concat();
    let mut bytes = std::vec::Vec::new();
    bytes.resize(262144, 0u8);
    if gamepkg::local_call(5, &request, &mut bytes)? != 262144 {
        return Err(-3);
    }
    Ok(bytes)
}
pub fn dispatcher_flags(services: &kernel_pool::KernelServices) -> Result<u32, i32> {
    let (_, bytes) = gamepkg::local_record_snapshot(services.service_handle(), 64)?;
    if bytes.len() != 64 {
        return Err(-3);
    }
    Ok(u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]))
}
pub fn dispatcher_wire(
    state: &registers::Registers,
    bytes: &[u8],
    flags: u32,
) -> Result<std::vec::Vec<u8>, i32> {
    let mut hash = sha256::Sha256::sha_new();
    hash.sha_update(bytes)?;
    let mut wire = state.encode();
    wire.extend(hash.sha_finish());
    wire.extend(u32::to_le_bytes(flags));
    Ok(wire)
}
pub fn dispatcher_cleanup(
    ram: &mut memory::Memory,
    services: &kernel_pool::KernelServices,
) -> Result<(), i32> {
    ram.kernel_clear()?;
    services.service_close(ram)?;
    gamepkg::local_record_release(ram.call_handle())
}
pub fn dispatcher_probe() -> Result<(), i32> {
    let (mut ram, services, window) = dispatcher_memory(true)?;
    let mut batch = std::vec::Vec::new();
    for mode in 0..768u32 {
        let mut state = dispatcher_reset(&mut ram, window, mode)?;
        kernel_pool::kernel_invoke(
            &mut state,
            &mut ram,
            dispatcher_slot(cases::dispatcher_case(mode)[0])?,
        )?;
        let bytes = dispatcher_snapshot(window)?;
        batch.extend(dispatcher_wire(
            &state,
            &bytes,
            dispatcher_flags(&services)?,
        )?);
        if mode % 64 == 63 {
            gamepkg::diagnostic_bytes(1, 0x544b1701, &batch)?;
            batch.clear();
        }
    }
    if dispatcher_flags(&services)? != 3 {
        return Err(-3);
    }
    let before = kernel_pool::kernel_tsc_read(&ram)?;
    let pair = kernel_pool::kernel_tsc_state(&ram)?;
    services.service_advance(&ram, 1234)?;
    let after = kernel_pool::kernel_tsc_read(&ram)?;
    if after < before
        || kernel_pool::kernel_tsc_state(&ram)? != pair
        || dispatcher_flags(&services)? != 3
        || kernel_pool::kernel_service_values(&ram)? != [0x10000, 0, 1234]
    {
        return Err(-3);
    }
    dispatcher_cleanup(&mut ram, &services)?;
    crate::controls::dispatcher_controls()
}
