// Asset-free native conformance entry. Vectors come from the pinned reference,
// not user media. This fixture is not the actual Burnout entry or loader.
use crate::dispatch;
use crate::gamepkg;
use crate::memory;
use crate::registers;
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
    if header[..4] != [84, 75, 78, 49] {
        return Err(-1);
    }
    let cases = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
    if cases > 4096 {
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
    let mut ram = memory::Memory::new();
    ram.attach(0, 67108864, window, 0, true)?;
    let mut context = registers::Registers::new();
    for _ in 0..cases {
        let mut address_bytes = [0u8; 4];
        fixture_read(handle, &mut offset, &mut address_bytes)?;
        let address = u32::from_le_bytes(address_bytes);
        let mut state_bytes = [0u8; 112];
        fixture_read(handle, &mut offset, &mut state_bytes)?;
        context.decode(&state_bytes)?;
        for (guest, count) in [
            (65536u32, 1024usize),
            (69632, 1024),
            (130560, 1024),
            (3872384, 64),
            (4856896, 64),
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
    }
    if gamepkg::file_size(handle)? != offset {
        return Err(-3);
    }
    gamepkg::file_close(handle)?;
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
