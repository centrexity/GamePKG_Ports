// Asset-free native conformance entry. Vectors come from the pinned reference,
// not user media. This fixture is not the actual Burnout entry or loader.
use crate::dispatch;
use crate::gamepkg;
use crate::memory;
use crate::registers;
use crate::scalar;
fn fixture_read(handle: u64, offset: &mut u64, bytes: &mut [u8]) -> Result<(), i32> {
    let mut used = 0usize;
    while used < bytes.len() {
        let count = gamepkg::file_read(handle, *offset, &mut bytes[used..])?;
        if count == 0 || count > bytes.len() - used {
            return Err(-3);
        }
        used += count;
        *offset = offset.checked_add(count as u64).ok_or(-1)?;
    }
    Ok(())
}
fn fixture_word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}
fn fixture_wide(bytes: &[u8], at: usize) -> u64 {
    u64::from_le_bytes([
        bytes[at],
        bytes[at + 1],
        bytes[at + 2],
        bytes[at + 3],
        bytes[at + 4],
        bytes[at + 5],
        bytes[at + 6],
        bytes[at + 7],
    ])
}
fn fixture_count(handle: u64, offset: &mut u64, maximum: u32) -> Result<usize, i32> {
    let mut bytes = [0u8; 4];
    fixture_read(handle, offset, &mut bytes)?;
    let count = u32::from_le_bytes(bytes);
    if count > maximum {
        return Err(-1);
    }
    Ok(count as usize)
}
fn fixture_scalar_round(handle: u64, offset: &mut u64) -> Result<(), i32> {
    let mut remaining = fixture_count(handle, offset, 131072)?;
    while remaining != 0 {
        let part = remaining.min(512);
        let mut bytes = std::vec::Vec::new();
        bytes.resize(part * 16, 0u8);
        fixture_read(handle, offset, &mut bytes)?;
        for index in 0..part {
            let at = index * 16;
            let actual = f64::to_bits(scalar::round24(f64::from_bits(fixture_wide(&bytes, at))));
            if actual != fixture_wide(&bytes, at + 8) {
                return Err(-3);
            }
        }
        remaining -= part;
    }
    Ok(())
}
fn fixture_scalar_parity(handle: u64, offset: &mut u64) -> Result<(), i32> {
    let mut remaining = fixture_count(handle, offset, 65536)?;
    while remaining != 0 {
        let part = remaining.min(512);
        let mut bytes = std::vec::Vec::new();
        bytes.resize(part * 8, 0u8);
        fixture_read(handle, offset, &mut bytes)?;
        for index in 0..part {
            let at = index * 8;
            if scalar::parity8(fixture_word(&bytes, at)) != fixture_word(&bytes, at + 4) as i32 {
                return Err(-3);
            }
        }
        remaining -= part;
    }
    Ok(())
}
fn fixture_scalar_rotate(handle: u64, offset: &mut u64) -> Result<(), i32> {
    let mut remaining = fixture_count(handle, offset, 262144)?;
    while remaining != 0 {
        let part = remaining.min(512);
        let mut bytes = std::vec::Vec::new();
        bytes.resize(part * 28, 0u8);
        fixture_read(handle, offset, &mut bytes)?;
        for index in 0..part {
            let at = index * 28;
            let mut carry = fixture_word(&bytes, at + 8) as i32;
            let direction = fixture_word(&bytes, at + 16);
            if direction > 1 {
                return Err(-1);
            }
            let actual = scalar::rotate_carry(
                fixture_word(&bytes, at),
                fixture_word(&bytes, at + 4),
                &mut carry,
                fixture_word(&bytes, at + 12),
                direction != 0,
            )?;
            if actual != fixture_word(&bytes, at + 20)
                || carry != fixture_word(&bytes, at + 24) as i32
            {
                return Err(-3);
            }
        }
        remaining -= part;
    }
    let mut carry = -1;
    if scalar::rotate_carry(1, 1, &mut carry, 64, true) != Err(-1) || carry != -1 {
        return Err(-3);
    }
    Ok(())
}
fn fixture_integer_rounding(handle: u64, offset: &mut u64) -> Result<(), i32> {
    let mut remaining = fixture_count(handle, offset, 1048576)?;
    while remaining != 0 {
        let part = remaining.min(512);
        let mut bytes = std::vec::Vec::new();
        bytes.resize(part * 20, 0u8);
        fixture_read(handle, offset, &mut bytes)?;
        for index in 0..part {
            let at = index * 20;
            let control = fixture_word(&bytes, at + 8);
            if control > 65535 {
                return Err(-1);
            }
            let actual =
                scalar::round_integer(f64::from_bits(fixture_wide(&bytes, at)), control as u16);
            if f64::to_bits(actual) != fixture_wide(&bytes, at + 12) {
                return Err(-3);
            }
        }
        remaining -= part;
    }
    remaining = fixture_count(handle, offset, 2097152)?;
    while remaining != 0 {
        let part = remaining.min(512);
        let mut bytes = std::vec::Vec::new();
        bytes.resize(part * 24, 0u8);
        fixture_read(handle, offset, &mut bytes)?;
        for index in 0..part {
            let at = index * 24;
            let control = fixture_word(&bytes, at + 8);
            if control > 65535 {
                return Err(-1);
            }
            let actual = scalar::integer_store(
                f64::from_bits(fixture_wide(&bytes, at)),
                control as u16,
                fixture_word(&bytes, at + 12),
            )?;
            if actual as u64 != fixture_wide(&bytes, at + 16) {
                return Err(-3);
            }
        }
        remaining -= part;
    }
    for width in [0u32, 1, 8, 15, 17, 31, 33, 63, 65, 4294967295] {
        if scalar::integer_store(f64::from_bits(0x7ff8000000000000), 0x37f, width) != Err(-1) {
            return Err(-3);
        }
    }
    Ok(())
}
fn fixture_execute() -> Result<(), i32> {
    let mut grants = 0u64;
    for item in gamepkg::grants()? {
        if item.kind == 1 {
            grants = item.id;
        }
    }
    let handle = gamepkg::file_open(grants, false, "vectors")?;
    let mut offset = 0u64;
    let mut header = [0u8; 8];
    fixture_read(handle, &mut offset, &mut header)?;
    if header[..4] != [84, 75, 78, 56] {
        return Err(-1);
    }
    let batches = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
    if batches == 0 || batches > 16 {
        return Err(-1);
    }
    let mut windows = [0u8; 16];
    if gamepkg::local_call(1, &[], &mut windows)? != 16 {
        return Err(-3);
    }
    let window = u64::from_le_bytes([
        windows[0], windows[1], windows[2], windows[3], windows[4], windows[5], windows[6],
        windows[7],
    ]);
    let mut commit = std::vec::Vec::new();
    commit.extend(u64::to_le_bytes(window));
    commit.extend(u64::to_le_bytes(0));
    commit.extend(u64::to_le_bytes(67108864));
    commit.extend(u32::to_le_bytes(2));
    if gamepkg::local_call(2, &commit, &mut [])? != 0 {
        return Err(-3);
    }
    let mut ram = memory::Memory::new()?;
    ram.attach(0, 67108864, window, 0, true)?;
    let mut context = registers::Registers::new();
    for _ in 0..batches {
        let mut count = [0u8; 4];
        fixture_read(handle, &mut offset, &mut count)?;
        let cases = u32::from_le_bytes(count);
        if cases > 4096 {
            return Err(-1);
        }
        for _ in 0..cases {
            let mut address_bytes = [0u8; 4];
            fixture_read(handle, &mut offset, &mut address_bytes)?;
            let address = u32::from_le_bytes(address_bytes);
            let mut state_bytes = [0u8; 244];
            fixture_read(handle, &mut offset, &mut state_bytes)?;
            context.decode(&state_bytes)?;
            let mut bounds = [0u8; 8];
            fixture_read(handle, &mut offset, &mut bounds)?;
            ram.call_bounds(fixture_word(&bounds, 0), fixture_word(&bounds, 4))?;
            for (guest, count) in [
                (65536u32, 1024usize),
                (69632, 1024),
                (130560, 1024),
                (3872384, 64),
                (4856896, 64),
                (4602624, 64),
                (3814560, 64),
            ] {
                let mut bytes = std::vec::Vec::new();
                bytes.resize(count, 0u8);
                fixture_read(handle, &mut offset, &mut bytes)?;
                ram.write_bytes(guest, &bytes)?;
            }
            dispatch::invoke(&mut context, &mut ram, address)?;
            fixture_read(handle, &mut offset, &mut state_bytes)?;
            if context.encode() != state_bytes {
                return Err(-3);
            }
            for (guest, count) in [
                (65536u32, 1024usize),
                (69632, 1024),
                (130560, 1024),
                (3872384, 64),
                (4856896, 64),
                (4602624, 64),
                (3814560, 64),
            ] {
                let mut expected = std::vec::Vec::new();
                expected.resize(count, 0u8);
                fixture_read(handle, &mut offset, &mut expected)?;
                let mut actual = std::vec::Vec::new();
                actual.resize(count, 0u8);
                ram.read_bytes(guest, &mut actual)?;
                if actual != expected {
                    return Err(-3);
                }
            }
            let mut calls = [0u8; 76];
            fixture_read(handle, &mut offset, &mut calls)?;
            let (trace, index, total) = ram.call_snapshot()?;
            for item in 0..16 {
                if trace[item] != fixture_word(&calls, item * 4) {
                    return Err(-3);
                }
            }
            if index != fixture_word(&calls, 64) || total != fixture_wide(&calls, 68) {
                return Err(-3);
            }
        }
    }
    fixture_scalar_round(handle, &mut offset)?;
    fixture_scalar_parity(handle, &mut offset)?;
    fixture_scalar_rotate(handle, &mut offset)?;
    fixture_integer_rounding(handle, &mut offset)?;
    if gamepkg::file_size(handle)? != offset {
        return Err(-3);
    }
    gamepkg::file_close(handle)?;
    crate::copy_checks::verify(&mut ram, window)?;
    crate::call_checks::call_verify()?;
    crate::call_checks::call_memory_verify()?;
    // Unknown native call targets and unmapped data are hard failures.
    if dispatch::invoke(&mut context, &mut ram, 0xffffffff) != Err(-5) {
        return Err(-3);
    }
    if ram.read_u32(0xfffffffe) != Err(-1) {
        return Err(-3);
    }
    if ram.write_u32(67108864, 0) != Err(-4) {
        return Err(-3);
    }
    Ok(())
}
pub fn game_main() {
    match fixture_execute() {
        Ok(()) => {
            let _ = gamepkg::audio(&[1234i16], 1);
        }
        Err(error) => {
            let _ = gamepkg::audio(&[error as i16], 1);
        }
    }
}
