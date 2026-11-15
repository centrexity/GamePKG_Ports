// Ordinary admitted shared-owner replay: original C remains outside this package.
use crate::denials;
use crate::gamepkg;
use crate::heap_service;
use crate::memory;
use crate::sha256;

fn shared_spawn(inbox: u64) -> Result<u64, i32> {
    let mut req = std::vec::Vec::new();
    req.extend(u32::to_le_bytes(0));
    req.extend(u64::to_le_bytes(inbox));
    req.extend(u32::to_le_bytes(0));
    let mut out = [0u8; 8];
    if gamepkg::local_call(10, &req, &mut out)? != 8 {
        return Err(-3);
    }
    Ok(u64::from_le_bytes(out))
}
fn shared_join(job: u64) -> Result<(), i32> {
    let pause = gamepkg::local_mailbox_create(1, 1)?;
    let mut out = [0u8; 8];
    let mut byte = [0u8; 1];
    loop {
        match gamepkg::local_call(11, &u64::to_le_bytes(job), &mut out) {
            Ok(8) => {
                gamepkg::local_mailbox_release(pause)?;
                if u64::from_le_bytes(out) != 0 {
                    return Err(-3);
                }
                return Ok(());
            }
            Err(-2) => {
                if gamepkg::local_mailbox_receive(pause, 1, &mut byte) != Err(-2) {
                    return Err(-3);
                }
            }
            Ok(_) => return Err(-3),
            Err(code) => return Err(code),
        }
    }
}
fn shared_stats(state: &mut heap_service::HeapClient) -> Result<[u32; 6], i32> {
    let mut stats = [0u32; 6];
    for number in 0..6u32 {
        stats[number as usize] = state.heap_contiguous_counter(number)?;
    }
    Ok(stats)
}
fn shared_record(state: &mut heap_service::HeapClient, page: u32) -> Result<[u32; 2], i32> {
    let copied = state.heap_contiguous_pages(page, 1)?;
    Ok([u32::from(copied[0]), state.heap_contiguous_length(page)?])
}

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
fn contiguous_witness(window: u64, state: &mut heap_service::HeapClient) -> Result<(), i32> {
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
        for value in shared_record(state, number)? {
            hash.sha_update(&u32::to_le_bytes(value))?;
        }
    }
    gamepkg::diagnostic_bytes(1, 0x544b0803, &hash.sha_finish())?;
    let stats = shared_stats(state)?;
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
    if header[..4] == [67, 84, 83, 49] {
        if contiguous_word(&header, 4)? != 0 || gamepkg::file_size(handle)? != 8 {
            return Err(-1);
        }
        gamepkg::file_close(handle)?;
        if crate::shared_checks::combined_verify()? != 1000 {
            return Err(-3);
        }
        return Ok(());
    }
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
    let main = u64::from_le_bytes([
        banks[0], banks[1], banks[2], banks[3], banks[4], banks[5], banks[6], banks[7],
    ]);
    let inbox = heap_service::heap_inbox()?;
    let job = shared_spawn(inbox)?;
    let mut state = heap_service::HeapClient::heap_client(inbox, job)?;
    state.heap_initialize(ram.call_handle(), main, 16777216, 67108864)?;
    state.heap_contiguous_initialize(window)?;
    for vector in vectors.chunks_exact(48) {
        let op = contiguous_word(vector, 0)?;
        let first = contiguous_word(vector, 4)?;
        let second = contiguous_word(vector, 8)?;
        let low = contiguous_word(vector, 12)?;
        let high = contiguous_word(vector, 16)?;
        let result = match op {
            0 => state.heap_contiguous_allocate(first, second, low, high),
            1 => match state.heap_contiguous_claim(first, second) {
                Ok(()) => Ok(0),
                Err(code) => Err(code),
            },
            2 => match state.heap_contiguous_free(first) {
                Ok(value) => Ok(u32::from(value)),
                Err(code) => Err(code),
            },
            3 => state.heap_contiguous_query(first),
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
            5 => state.heap_contiguous_allocate(first, second, 0, u32::MAX),
            6 => match state.heap_contiguous_clear_claim(first, second) {
                Ok(()) => Ok(0),
                Err(code) => Err(code),
            },
            _ => return Err(-1),
        };
        if result != Ok(contiguous_word(vector, 20)?) {
            return Err(-3);
        }
        let stats = shared_stats(&mut state)?;
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
    contiguous_witness(window, &mut state)?;
    state.heap_stop()?;
    state.heap_abandon()?;
    shared_join(job)?;
    Ok(())
}
pub fn game_main() {
    let status = match contiguous_execute() {
        Ok(()) => 0u64,
        Err(code) => code as i64 as u64,
    };
    let _ = gamepkg::diagnostic(1, 0x544b08ff, &[status]);
}
