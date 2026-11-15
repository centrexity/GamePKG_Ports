// Source-checked fill conformance, also executed in the confined game child.
use crate::gamepkg;
use crate::memory;
fn protect(window: u64, offset: u64, bytes: u64, mode: u32) -> Result<(), i32> {
    let mut request = std::vec::Vec::new();
    request.extend(u64::to_le_bytes(window));
    request.extend(u64::to_le_bytes(offset));
    request.extend(u64::to_le_bytes(bytes));
    request.extend(u32::to_le_bytes(mode));
    if gamepkg::local_call(3, &request, &mut [])? != 0 {
        return Err(-3);
    }
    Ok(())
}
pub fn verify(ram: &mut memory::Memory, window: u64) -> Result<(), i32> {
    for value in [0u32, 1, 255, 0xabcdef12] {
        for length in [0u32, 1, 3, 1025, 1048593] {
            ram.fill(0x600003, value as u8, length)?;
            let mut offset = 0u32;
            while offset < length {
                let count = (length - offset).min(65536);
                let mut bytes = std::vec::Vec::new();
                bytes.resize(count as usize, 0u8);
                ram.read_bytes(0x600003 + offset, &mut bytes)?;
                for byte in bytes {
                    if byte != value as u8 {
                        return Err(-3);
                    }
                }
                offset += count;
            }
        }
    }
    let page = 0x7f0000u64;
    ram.fill(page as u32, 0x55, 8192)?;
    protect(window, page + 4096, 4096, 1)?;
    if ram.fill(page as u32 + 4090, 1, 12) != Err(-4) {
        return Err(-3);
    }
    let mut bytes = [0u8; 12];
    ram.read_bytes(page as u32 + 4090, &mut bytes)?;
    if bytes != [0x55; 12] {
        return Err(-3);
    }
    protect(window, page + 4096, 4096, 0)?;
    if ram.fill(page as u32 + 4090, 2, 12) != Err(-4) {
        return Err(-3);
    }
    protect(window, page + 4096, 4096, 2)?;
    ram.read_bytes(page as u32 + 4090, &mut bytes)?;
    if bytes != [0x55; 12] {
        return Err(-3);
    }
    let mut readonly = ram.call_view();
    readonly.attach(0x90000000, 8192, window, page, false)?;
    if readonly.fill(0x90000000, 3, 1) != Err(-4)
        || ram.fill(0xfffffffe, 1, 4) != Err(-1)
        || ram.fill(0, 1, 67108865) != Err(-1)
        || ram.fill(0x70000000, 1, 1) != Err(-4)
        || ram.fill(0xffffffff, 1, 0) != Ok(())
    {
        return Err(-3);
    }
    ram.read_bytes(page as u32 + 4090, &mut bytes)?;
    if bytes != [0x55; 12] {
        return Err(-3);
    }
    Ok(())
}
