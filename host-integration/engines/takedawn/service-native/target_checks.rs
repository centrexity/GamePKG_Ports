// Admitted game-side lookup checks using read-only synthetic reference data.
use crate::gamepkg;
use crate::targets;
fn target_read(handle: u64, offset: &mut u64, bytes: &mut [u8]) -> Result<(), i32> {
    let mut used = 0usize;
    while used < bytes.len() {
        let amount = gamepkg::file_read(handle, *offset, &mut bytes[used..])?;
        if amount == 0 || amount > bytes.len() - used {
            return Err(-3);
        }
        used += amount;
        *offset = offset.checked_add(amount as u64).ok_or(-1)?;
    }
    Ok(())
}
pub fn target_verify() -> Result<(), i32> {
    let mut grant = 0u64;
    for item in gamepkg::grants()? {
        if item.kind == 1 {
            grant = item.id;
        }
    }
    let file = gamepkg::file_open(grant, false, "target-vectors")?;
    let mut offset = 0u64;
    let mut header = [0u8; 4];
    target_read(file, &mut offset, &mut header)?;
    let mut remaining = u32::from_le_bytes(header) as usize;
    if remaining > 131072 {
        return Err(-1);
    }
    while remaining != 0 {
        let count = remaining.min(512);
        let mut bytes = std::vec::Vec::new();
        bytes.resize(count * 8, 0u8);
        target_read(file, &mut offset, &mut bytes)?;
        for index in 0..count {
            let at = index * 8;
            let address =
                u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
            let expected =
                u32::from_le_bytes([bytes[at + 4], bytes[at + 5], bytes[at + 6], bytes[at + 7]]);
            if targets::original_target(address) != expected {
                return Err(-3);
            }
        }
        remaining -= count;
    }
    if gamepkg::file_size(file)? != offset {
        return Err(-3);
    }
    gamepkg::file_close(file)?;
    Ok(())
}
