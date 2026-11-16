use crate::gamepkg;
use crate::kernel_pool;
use crate::memory;
use crate::registers;
pub fn clock_memory(commit: bool) -> Result<(memory::Memory, kernel_pool::KernelServices), i32> {
    let mut windows = [0u8; 16];
    if gamepkg::local_call(1, &[], &mut windows)? != 16 {
        return Err(-3);
    }
    let window = u64::from_le_bytes([
        windows[0], windows[1], windows[2], windows[3], windows[4], windows[5], windows[6],
        windows[7],
    ]);
    if commit {
        let mut request = std::vec::Vec::new();
        request.extend(u64::to_le_bytes(window));
        request.extend(u64::to_le_bytes(0));
        request.extend(u64::to_le_bytes(262144));
        request.extend(u32::to_le_bytes(2));
        gamepkg::local_call(2, &request, &mut [])?;
    }
    let mut ram = memory::Memory::new()?;
    ram.attach(0, 262144, window, 0, true)?;
    ram.fill(0, 0xa5, 262144)?;
    let services = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2)?;
    for (slot, ordinal) in [125u32, 126, 127, 128].iter().enumerate() {
        ram.write_u32(0x10000 + slot as u32 * 4, 0x80000000 | ordinal)?;
    }
    kernel_pool::kernel_prepare(&mut ram, 0x10000, 4)?;
    Ok((ram, services))
}
pub fn clock_arguments(ram: &mut memory::Memory, pointer: u32) -> Result<(), i32> {
    ram.write_u32(0x18000, 0x12345678)?;
    ram.write_u32(0x18004, pointer)
}
pub fn clock_cleanup(
    ram: &mut memory::Memory,
    services: &kernel_pool::KernelServices,
) -> Result<(), i32> {
    services.service_close(ram)?;
    ram.kernel_clear()?;
    gamepkg::local_record_release(ram.call_handle())
}
pub fn clock_check_cpu(
    before: &[u8],
    after: &registers::Registers,
    value: u64,
    ordinal: u32,
) -> Result<(), i32> {
    let mut expected = std::vec::Vec::new();
    expected.extend(before);
    expected[..4].copy_from_slice(&u32::to_le_bytes(if ordinal == 128 {
        0
    } else {
        value as u32
    }));
    if ordinal != 128 {
        expected[12..16].copy_from_slice(&u32::to_le_bytes((value >> 32) as u32));
    }
    let stack = u32::from_le_bytes([before[28], before[29], before[30], before[31]]);
    expected[28..32].copy_from_slice(&u32::to_le_bytes(
        stack + if ordinal == 128 { 8 } else { 4 },
    ));
    if expected != after.encode() {
        return Err(-3);
    }
    Ok(())
}
