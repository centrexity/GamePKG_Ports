// Ordinary admitted replay entry: original C remains outside this package.
use crate::contiguous;
use crate::denials;
use crate::gamepkg;
use crate::memory;
use crate::sha256;
fn contiguous_word(bytes: &[u8], at: usize) -> Result<u32, i32> {
    let part = bytes.get(at..at + 4).ok_or(-1)?;
    Ok(u32::from_le_bytes([part[0], part[1], part[2], part[3]]))
}
fn contiguous_read(handle: u64, offset: &mut u64, output: &mut [u8]) -> Result<(), i32> {
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
fn contiguous_witness(window: u64, state: &contiguous::Contiguous) -> Result<(), i32> {
    let mut bytes = std::vec::Vec::new();
    bytes.resize(1048576, 0u8);
    for number in 0..64u32 {
        gamepkg::local_memory_read_segments(
            &[[window, u64::from(number) * 1048576, 1048576]],
            &mut bytes,
        )?;
        let mut hash = sha256::Sha256::sha_new();
        hash.sha_update(&bytes)?;
        let mut output = [0u8; 36];
        output[..4].copy_from_slice(&u32::to_le_bytes(number));
        output[4..].copy_from_slice(&hash.sha_finish());
        gamepkg::diagnostic_bytes(1, 0x544b0802, &output)?;
    }
    let mut hash = sha256::Sha256::sha_new();
    for number in 0..16384u32 {
        for value in state.contiguous_record(number)? {
            hash.sha_update(&u32::to_le_bytes(value))?;
        }
    }
    gamepkg::diagnostic_bytes(1, 0x544b0803, &hash.sha_finish())?;
    let stats = state.contiguous_stats();
    gamepkg::diagnostic(
        1,
        0x544b0800,
        &[
            u64::from(stats[0]),
            u64::from(stats[1]),
            u64::from(stats[2]),
            u64::from(stats[3]),
            u64::from(stats[4]),
            u64::from(stats[5]),
        ],
    )?;
    Ok(())
}
fn contiguous_execute() -> Result<(), i32> {
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
    let mut header = [0u8; 8];
    contiguous_read(handle, &mut offset, &mut header)?;
    if header[..4] != [67, 84, 71, 49] {
        return Err(-1);
    }
    let count = contiguous_word(&header, 4)?;
    if count > 200000 || gamepkg::file_size(handle)? != 8 + u64::from(count) * 48 {
        return Err(-1);
    }
    let mut vectors = std::vec::Vec::new();
    vectors.resize(count as usize * 48, 0u8);
    for chunk in vectors.chunks_mut(65536) {
        contiguous_read(handle, &mut offset, chunk)?;
    }
    let mut banks = [0u8; 48];
    if gamepkg::local_call(1, &[], &mut banks)? != 48 {
        return Err(-3);
    }
    let window = u64::from_le_bytes([
        banks[16], banks[17], banks[18], banks[19], banks[20], banks[21], banks[22], banks[23],
    ]);
    let mut commit = std::vec::Vec::new();
    commit.extend(u64::to_le_bytes(window));
    commit.extend(u64::to_le_bytes(0));
    commit.extend(u64::to_le_bytes(67108864));
    commit.extend(u32::to_le_bytes(2));
    gamepkg::local_call(2, &commit, &mut [])?;
    let ram = memory::Memory::new()?;
    denials::contiguous_denials(&ram, window)?;
    gamepkg::local_memory_fill(window, 0, 67108864, 0xa5)?;
    let mut state = contiguous::Contiguous::contiguous_new(&ram, window)?;
    for vector in vectors.chunks_exact(48) {
        let op = contiguous_word(vector, 0)?;
        let first = contiguous_word(vector, 4)?;
        let second = contiguous_word(vector, 8)?;
        let low = contiguous_word(vector, 12)?;
        let high = contiguous_word(vector, 16)?;
        let result = match op {
            0 => state.contiguous_allocate(&ram, first, second, low, high),
            1 => match state.contiguous_claim(&ram, first, second) {
                Ok(()) => Ok(0),
                Err(code) => Err(code),
            },
            2 => match state.contiguous_free(&ram, first) {
                Ok(value) => Ok(u32::from(value)),
                Err(code) => Err(code),
            },
            3 => state.contiguous_size(&ram, first),
            4 => {
                if second > 65536 {
                    return Err(-1);
                }
                match gamepkg::local_memory_fill(window, u64::from(first), u64::from(second), 0x7b)
                {
                    Ok(()) => Ok(0),
                    Err(code) => Err(code),
                }
            }
            5 => state.contiguous_allocate(&ram, first, second, 0, u32::MAX),
            6 => match state.contiguous_clear_claim(&ram, first, second) {
                Ok(()) => Ok(0),
                Err(code) => Err(code),
            },
            _ => return Err(-1),
        };
        if result != Ok(contiguous_word(vector, 20)?) {
            return Err(-3);
        }
        let stats = state.contiguous_stats();
        for number in 0..6usize {
            if stats[number] != contiguous_word(vector, 24 + number * 4)? {
                return Err(-3);
            }
        }
    }
    let mut tail = [0u8; 1];
    if gamepkg::file_read(handle, offset, &mut tail)? != 0 {
        return Err(-1);
    }
    gamepkg::file_close(handle)?;
    contiguous_witness(window, &state)?;
    Ok(())
}
pub fn game_main() {
    let status = match contiguous_execute() {
        Ok(()) => 0u64,
        Err(code) => code as i64 as u64,
    };
    let _ = gamepkg::diagnostic(1, 0x544b08ff, &[status]);
}
