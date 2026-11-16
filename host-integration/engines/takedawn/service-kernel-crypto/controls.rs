use crate::gamepkg;
use crate::kernel_pool;
use crate::memory;
use crate::probe;
fn crypto_success(ram: &mut memory::Memory, step: [u32; 8]) -> Result<(), i32> {
    probe::crypto_arguments(ram, step)?;
    let mut state = probe::crypto_state(1, 0);
    kernel_pool::kernel_invoke(&mut state, ram, step[0] - 338)?;
    if state.eax != 0 || state.esp != 0x18004 + kernel_pool::kernel_supported(step[0])? {
        return Err(-3);
    }
    Ok(())
}
fn crypto_denied(ram: &mut memory::Memory, step: [u32; 8], code: i32) -> Result<(), i32> {
    probe::crypto_arguments(ram, step)?;
    let mut state = probe::crypto_state(1, 0);
    let before = state.encode();
    if kernel_pool::kernel_invoke(&mut state, ram, step[0] - 338) != Err(code)
        || state.encode() != before
    {
        return Err(-3);
    }
    Ok(())
}
fn crypto_protect(ram: &memory::Memory, address: u32, mode: u32) -> Result<(), i32> {
    let backing = ram.memory_page_backing(address)?;
    let mut req = std::vec::Vec::new();
    req.extend(u64::to_le_bytes(backing[0]));
    req.extend(u64::to_le_bytes(backing[1]));
    req.extend(u64::to_le_bytes(4096));
    req.extend(u32::to_le_bytes(mode));
    gamepkg::local_call(3, &req, &mut [])?;
    Ok(())
}
pub fn crypto_denials() -> Result<(), i32> {
    let mut ram = probe::crypto_memory(1)?;
    ram.write_bytes(0x20000, &[75u8, 101, 121])?;
    ram.write_bytes(0x22000, &[80u8, 108, 97, 105, 110, 116, 101, 120, 116])?;
    crypto_success(&mut ram, [338, 0x11003, 3, 0x20000, 0, 0, 0, 0])?;
    crypto_success(&mut ram, [339, 0x11003, 9, 0x22000, 0, 0, 0, 0])?;
    let mut cipher = [0u8; 9];
    ram.read_bytes(0x22000, &mut cipher)?;
    if cipher != [0xbb, 0xf3, 0x16, 0xe8, 0xd9, 0x40, 0xaf, 0x0a, 0xd3] {
        return Err(-3);
    }
    ram.fill(0x20000, 0x0b, 20)?;
    ram.write_bytes(0x21000, &[72u8, 105, 32])?;
    ram.write_bytes(0x24000, &[84u8, 104, 101, 114, 101])?;
    crypto_success(
        &mut ram,
        [340, 0x20000, 20, 0x21000, 3, 0x24000, 5, 0x12000],
    )?;
    let mut digest = [0u8; 20];
    ram.read_bytes(0x12000, &mut digest)?;
    if digest
        != [
            0xb6, 0x17, 0x31, 0x86, 0x55, 0x05, 0x72, 0x64, 0xe2, 0x8b, 0xc0, 0xb6, 0xfb, 0x37,
            0x8c, 0x8e, 0xf1, 0x46, 0xbe, 0x00,
        ]
    {
        return Err(-3);
    }
    let mut retained = [0u8; 20];
    ram.read_bytes(0x20000, &mut retained)?;
    if retained != [0x0bu8; 20] {
        return Err(-3);
    }
    for ordinal in [338u32, 339, 340] {
        if kernel_pool::kernel_physical(ordinal) {
            return Err(-3);
        }
    }
    for ordinal in 341..351u32 {
        if kernel_pool::kernel_supported(ordinal) != Err(-5) {
            return Err(-3);
        }
    }
    // Active extents cannot wrap; no-op pointer arguments are covered by corpus.
    for step in [
        [338, 0xfffffff0, 1, 0x20000, 0, 0, 0, 0],
        [338, 0x11000, 256, 0xfffffff0, 0, 0, 0, 0],
        [339, 0x11000, 32, 0xfffffff0, 0, 0, 0, 0],
        [340, 0xfffffff0, 65, 0, 0, 0, 0, 0x12000],
        [340, 0x20000, 20, 0xfffffff0, 32, 0, 0, 0x12000],
        [340, 0x20000, 20, 0, 0, 0, 0, 0xfffffff0],
    ] {
        crypto_denied(&mut ram, step, -1)?;
    }
    for step in [
        [338, 0x40000, 1, 0x20000, 0, 0, 0, 0],
        [339, 0x40000, 1, 0x20000, 0, 0, 0, 0],
        [340, 0x40000, 20, 0, 0, 0, 0, 0x12000],
        [340, 0x20000, 20, 0x40000, 1, 0, 0, 0x12000],
        [340, 0x20000, 20, 0, 0, 0, 0, 0x40000],
    ] {
        crypto_denied(&mut ram, step, -4)?;
    }
    // KSA can have published the identity permutation before a bad key read.
    crypto_denied(&mut ram, [338, 0x11000, 1, 0x40000, 0, 0, 0, 0], -4)?;
    if ram.read_u8(0x11000)? != 0 || ram.read_u8(0x110ff)? != 255 {
        return Err(-3);
    }
    crypto_protect(&ram, 0x11000, 1)?;
    crypto_denied(&mut ram, [338, 0x11000, 1, 0x20000, 0, 0, 0, 0], -4)?;
    crypto_denied(&mut ram, [339, 0x11000, 1, 0x22000, 0, 0, 0, 0], -4)?;
    crypto_protect(&ram, 0x11000, 2)?;
    crypto_success(&mut ram, [338, 0x11000, 3, 0x20000, 0, 0, 0, 0])?;
    crypto_protect(&ram, 0x22000, 1)?;
    crypto_denied(&mut ram, [339, 0x11000, 1, 0x22000, 0, 0, 0, 0], -4)?;
    if ram.read_u8(0x11100)? != 1 {
        return Err(-3);
    }
    crypto_protect(&ram, 0x22000, 2)?;
    crypto_protect(&ram, 0x12000, 1)?;
    crypto_denied(
        &mut ram,
        [340, 0x20000, 20, 0x21000, 3, 0x24000, 5, 0x12000],
        -4,
    )?;
    let mut after = [0u8; 20];
    ram.read_bytes(0x12000, &mut after)?;
    if after != digest {
        return Err(-3);
    }
    crypto_protect(&ram, 0x12000, 2)?;
    // Complete seven-word HMAC arguments are required before computation.
    probe::crypto_arguments(
        &mut ram,
        [340, 0x20000, 20, 0x21000, 3, 0x24000, 5, 0x12000],
    )?;
    let backing = ram.memory_page_backing(0)?;
    let mut short = ram.call_view();
    short.attach(0, 0x1801c, backing[0], 0, true)?;
    let mut state = probe::crypto_state(1, 0);
    let before = state.encode();
    if kernel_pool::kernel_invoke(&mut state, &mut short, 2) != Err(-4) || state.encode() != before
    {
        return Err(-3);
    }
    let mut readonly = ram.call_view();
    readonly.attach(0, 262144, backing[0], 0, false)?;
    if kernel_pool::kernel_invoke(&mut state, &mut readonly, 2) != Err(-4)
        || state.encode() != before
    {
        return Err(-3);
    }
    ram.kernel_clear()?;
    gamepkg::local_record_release(ram.call_handle())?;
    if kernel_pool::kernel_invoke(&mut state, &mut ram, 0) != Err(-4) || state.encode() != before {
        return Err(-3);
    }
    Ok(())
}
