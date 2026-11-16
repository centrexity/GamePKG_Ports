use crate::gamepkg;
use crate::kernel_pool;
use crate::memory;
use crate::probe;
fn sha_denied(ram: &mut memory::Memory, step: [u32; 4], error: i32) -> Result<(), i32> {
    probe::sha_arguments(ram, step)?;
    let mut state = probe::sha_state(1, 0);
    let before = state.encode();
    if kernel_pool::kernel_invoke(&mut state, ram, step[0] - 335) != Err(error)
        || state.encode() != before
    {
        return Err(-3);
    }
    Ok(())
}
fn sha_protect(ram: &memory::Memory, mode: u32) -> Result<(), i32> {
    let backing = ram.memory_page_backing(0x11000)?;
    let mut request = std::vec::Vec::new();
    request.extend(u64::to_le_bytes(backing[0]));
    request.extend(u64::to_le_bytes(backing[1]));
    request.extend(u64::to_le_bytes(4096));
    request.extend(u32::to_le_bytes(mode));
    gamepkg::local_call(3, &request, &mut [])?;
    Ok(())
}
fn sha_success(ram: &mut memory::Memory, step: [u32; 4]) -> Result<(), i32> {
    probe::sha_arguments(ram, step)?;
    let mut state = probe::sha_state(1, 0);
    kernel_pool::kernel_invoke(&mut state, ram, step[0] - 335)?;
    if state.eax != 0 || state.esp != 0x18004 + kernel_pool::kernel_supported(step[0])? {
        return Err(-3);
    }
    Ok(())
}
pub fn sha_denials() -> Result<(), i32> {
    let mut ram = probe::sha_memory(1)?;
    for ordinal in [335u32, 336, 337] {
        if kernel_pool::kernel_physical(ordinal) {
            return Err(-3);
        }
    }
    sha_success(&mut ram, [335, 0x11000, 0, 0])?;
    ram.write_bytes(0x20000, &[97u8, 98, 99])?;
    sha_success(&mut ram, [336, 0x11000, 0x20000, 3])?;
    sha_success(&mut ram, [337, 0x11000, 0x12000, 0])?;
    let mut digest = [0u8; 20];
    ram.read_bytes(0x12000, &mut digest)?;
    if digest
        != [
            0xa9, 0x99, 0x3e, 0x36, 0x47, 0x06, 0x81, 0x6a, 0xba, 0x3e, 0x25, 0x71, 0x78, 0x50,
            0xc2, 0x6c, 0x9c, 0xd0, 0xd8, 0x9d,
        ]
    {
        return Err(-3);
    }
    sha_success(&mut ram, [335, 0x11000, 0, 0])?;
    sha_success(&mut ram, [337, 0x11000, 0x12000, 0])?;
    ram.read_bytes(0x12000, &mut digest)?;
    if digest
        != [
            0xda, 0x39, 0xa3, 0xee, 0x5e, 0x6b, 0x4b, 0x0d, 0x32, 0x55, 0xbf, 0xef, 0x95, 0x60,
            0x18, 0x90, 0xaf, 0xd8, 0x07, 0x09,
        ]
    {
        return Err(-3);
    }
    // Bounds reject before SHA writes; CPU changes only after a successful call.
    sha_success(&mut ram, [335, 0x11000, 0, 0])?;
    let mut original = [0u8; 92];
    ram.read_bytes(0x11000, &mut original)?;
    for step in [
        [335, 0xfffffff0, 0, 0],
        [336, 0xfffffff0, 0x20000, 64],
        [336, 0x11000, 0xfffffff0, 64],
        [337, 0x11000, 0xfffffff8, 0],
    ] {
        sha_denied(&mut ram, step, -1)?;
    }
    let mut after = [0u8; 92];
    ram.read_bytes(0x11000, &mut after)?;
    if after != original {
        return Err(-3);
    }
    for step in [
        [335, 0x40000, 0, 0],
        [336, 0x40000, 0x20000, 8],
        [337, 0x40000, 0x12000, 0],
    ] {
        sha_denied(&mut ram, step, -4)?;
    }
    sha_protect(&ram, 1)?;
    for step in [
        [335, 0x11000, 0, 0],
        [336, 0x11000, 0x20000, 8],
        [337, 0x11000, 0x12000, 0],
    ] {
        sha_denied(&mut ram, step, -4)?;
    }
    ram.read_bytes(0x11000, &mut after)?;
    if after != original {
        return Err(-3);
    }
    sha_protect(&ram, 2)?;
    // A denied late input read retains the original earlier count update.
    sha_denied(&mut ram, [336, 0x11000, 0x3ffff, 2], -4)?;
    if ram.read_u32(0x11014)? != 16 {
        return Err(-3);
    }
    sha_success(&mut ram, [335, 0x11000, 0, 0])?;
    // Late digest denial leaves the finalized context; never replay on error.
    sha_denied(&mut ram, [337, 0x11000, 0x40000, 0], -4)?;
    if ram.read_u32(0x11014)? != 512 {
        return Err(-3);
    }
    // Translation-level RO remains authoritative even with host RW pages.
    let mut readonly = ram.call_view();
    let backing = ram.memory_page_backing(0)?;
    readonly.attach(0, 262144, backing[0], 0, false)?;
    probe::sha_arguments(&mut ram, [335, 0x11000, 0, 0])?;
    let mut readonly_state = probe::sha_state(1, 0);
    let readonly_before = readonly_state.encode();
    if kernel_pool::kernel_invoke(&mut readonly_state, &mut readonly, 0) != Err(-4)
        || readonly_state.encode() != readonly_before
    {
        return Err(-3);
    }
    let mut state = probe::sha_state(1, 0);
    let before = state.encode();
    if kernel_pool::kernel_invoke(&mut state, &mut ram, 3) != Err(-5) || state.encode() != before {
        return Err(-3);
    }
    state.esp = 0xfffffffc;
    let before = state.encode();
    if kernel_pool::kernel_invoke(&mut state, &mut ram, 0) != Err(-1) || state.encode() != before {
        return Err(-3);
    }
    state = probe::sha_state(1, 0);
    state.fp_top = 8;
    let before = state.encode();
    if kernel_pool::kernel_invoke(&mut state, &mut ram, 0) != Err(-1) || state.encode() != before {
        return Err(-3);
    }
    // Revoked map roots cannot use an otherwise live import record.
    ram.kernel_clear()?;
    gamepkg::local_record_release(ram.call_handle())?;
    state = probe::sha_state(1, 0);
    let before = state.encode();
    if kernel_pool::kernel_invoke(&mut state, &mut ram, 0) != Err(-4) || state.encode() != before {
        return Err(-3);
    }
    Ok(())
}
