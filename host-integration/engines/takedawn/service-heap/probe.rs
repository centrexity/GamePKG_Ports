// Ordinary dependency-free test entry. No original C or raw reference imports.
use crate::denials;
use crate::gamepkg;
use crate::heap;
use crate::memory;
use crate::sha256;
fn heap_word(bytes: &[u8], at: usize) -> Result<u32, i32> {
    let part = bytes.get(at..at + 4).ok_or(-1)?;
    Ok(u32::from_le_bytes([part[0], part[1], part[2], part[3]]))
}
fn heap_read(handle: u64, offset: &mut u64, output: &mut [u8]) -> Result<(), i32> {
    let mut done = 0usize;
    while done < output.len() {
        let count = gamepkg::file_read(handle, *offset, &mut output[done..])?;
        if count == 0 || count > output.len() - done {
            return Err(-3);
        }
        *offset = offset.checked_add(count as u64).ok_or(-1)?;
        done += count;
    }
    Ok(())
}
fn heap_witness(ram: &memory::Memory, state: &heap::Heap) -> Result<(), i32> {
    let mut bytes = std::vec::Vec::new();
    bytes.resize(1048576, 0u8);
    for number in 0..64u32 {
        ram.read_bytes(number * 1048576, &mut bytes)?;
        let mut hash = sha256::Sha256::sha_new();
        hash.sha_update(&bytes)?;
        let mut output = [0u8; 36];
        output[..4].copy_from_slice(&u32::to_le_bytes(number));
        output[4..].copy_from_slice(&hash.sha_finish());
        gamepkg::diagnostic_bytes(1, 0x544b0402, &output)?;
    }
    let stats = state.heap_stats();
    let mut hash = sha256::Sha256::sha_new();
    for number in 0..stats[1] {
        for word in state.heap_record(number)? {
            hash.sha_update(&u32::to_le_bytes(word))?;
        }
    }
    if state.heap_record(stats[1]) != Err(-1) {
        return Err(-3);
    }
    gamepkg::diagnostic_bytes(1, 0x544b0403, &hash.sha_finish())?;
    gamepkg::diagnostic(
        1,
        0x544b0400,
        &[
            u64::from(stats[0]),
            u64::from(stats[1]),
            u64::from(stats[2]),
        ],
    )?;
    Ok(())
}
fn heap_execute() -> Result<(), i32> {
    let mut assets = 0u64;
    for grant in gamepkg::grants()? {
        if grant.kind == 1 {
            assets = grant.id;
        }
    }
    if assets == 0 {
        return Err(-4);
    }
    let handle = gamepkg::file_open(assets, false, "vectors")?;
    let mut offset = 0u64;
    let mut header = [0u8; 16];
    heap_read(handle, &mut offset, &mut header)?;
    if header[..4] != [71, 80, 72, 49] {
        return Err(-1);
    }
    let base = heap_word(&header, 4)?;
    let top = heap_word(&header, 8)?;
    let count = heap_word(&header, 12)?;
    if count > 200000 {
        return Err(-1);
    }
    let expected_size = 16u64 + u64::from(count) * 32;
    if gamepkg::file_size(handle)? != expected_size {
        return Err(-1);
    }
    let mut vectors = std::vec::Vec::new();
    vectors.resize(count as usize * 32, 0u8);
    for chunk in vectors.chunks_mut(65536) {
        heap_read(handle, &mut offset, chunk)?;
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
    gamepkg::local_call(2, &commit, &mut [])?;
    let mut ram = memory::Memory::new()?;
    ram.attach(0, 67108864, window, 0, true)?;
    ram.fill(0, 0xa5, 67108864)?;
    denials::heap_denials(&mut ram, window)?;
    let mut state = heap::Heap::heap_new(&ram, base, top)?;
    for vector in vectors.chunks_exact(32) {
        let op = heap_word(vector, 0)?;
        let first = heap_word(vector, 4)?;
        let second = heap_word(vector, 8)?;
        let expected = heap_word(vector, 12)?;
        let expected_error = heap_word(vector, 16)? as i32;
        let result = match op {
            0 => state.heap_allocate(&mut ram, first, second),
            1 => match state.heap_free(&ram, first) {
                Ok(()) => Ok(0),
                Err(code) => Err(code),
            },
            2 => state.heap_size(&ram, first),
            3 => {
                if second > 65536 {
                    return Err(-1);
                }
                match ram.fill(first, 0x7b, second) {
                    Ok(()) => Ok(0),
                    Err(code) => Err(code),
                }
            }
            _ => return Err(-1),
        };
        if expected_error == 0 {
            if result != Ok(expected) {
                return Err(-3);
            }
        } else if result != Err(expected_error) {
            return Err(-3);
        }
        if state.heap_stats()
            != [
                heap_word(vector, 20)?,
                heap_word(vector, 24)?,
                heap_word(vector, 28)?,
            ]
        {
            return Err(-3);
        }
    }
    let mut tail = [0u8; 1];
    if gamepkg::file_read(handle, offset, &mut tail)? != 0 {
        return Err(-1);
    }
    gamepkg::file_close(handle)?;
    heap_witness(&ram, &state)?;
    Ok(())
}
pub fn game_main() {
    let status = match heap_execute() {
        Ok(()) => 0u64,
        Err(code) => code as i64 as u64,
    };
    let _ = gamepkg::diagnostic(1, 0x544b04ff, &[status]);
}
