// Port-owned saveguard I/O. Only the host-supplied GamePKG client touches files.
// Caller serializes each title path and runs blocking work on its worker task.
use crate::gamepkg;

pub enum Publication {
    Durable {},
    // The source was consumed and the new name is visible. Never retry it.
    CommittedNotDurable {},
}

pub fn publish(grant: u64, shadow: &str, real: &str) -> Result<Publication, i32> {
    match gamepkg::file_replace(grant, shadow, real, true) {
        Ok(()) => Ok(Publication::Durable {}),
        Err(-6) => Ok(Publication::CommittedNotDurable {}),
        Err(error) => Err(error),
    }
}

// Keep an opened file alive for the original guard's flush-after-last-close
// behavior. The caller owns this handle until sync_close; grant revocation
// still revokes it, and no filesystem path is acquired by duplication.
pub fn retain_for_flush(handle: u64) -> Result<u64, i32> {
    gamepkg::file_duplicate(handle)
}

pub fn sync_close(handle: u64) -> Result<(), i32> {
    let flushed = gamepkg::file_sync(handle);
    let close_result = gamepkg::file_close(handle);
    match flushed {
        Ok(()) => close_result,
        Err(error) => Err(error),
    }
}

// Fill a block even when an individual positional read is short. A zero read
// terminates at EOF. No implicit shared file cursor crosses the service pipe.
fn read_block(handle: u64, offset: u64, bytes: &mut [u8]) -> Result<usize, i32> {
    let mut filled = 0usize;
    while filled < bytes.len() {
        let position = offset.checked_add(filled as u64).ok_or(-1)?;
        let read = gamepkg::file_read(handle, position, &mut bytes[filled..])?;
        if read == 0 {
            break;
        }
        if read > bytes.len() - filled {
            return Err(-3);
        }
        filled += read;
    }
    Ok(filled)
}

fn copy_open(input: u64, output: u64) -> Result<u64, i32> {
    gamepkg::file_resize(output, 0)?;
    // Keep bounded scratch buffers off the worker stack on 32-bit Windows;
    // large stack arrays require an unapproved compiler stack helper there.
    let mut bytes = std::vec::Vec::new();
    bytes.resize(65536, 0u8);
    let mut offset = 0u64;
    loop {
        let count = read_block(input, offset, &mut bytes)?;
        if count == 0 {
            return Ok(offset);
        }
        gamepkg::file_write(output, offset, &bytes[..count])?;
        offset = offset.checked_add(count as u64).ok_or(-1)?;
    }
}

// Copy the current whole save into the guard's shadow, never over the real
// file. On any copy/close failure, remove the partial shadow and retain real.
// The caller must have settled prior queued writes/recovery for this path.
pub fn copy_shadow(grant: u64, real: &str) -> Result<u64, i32> {
    copy_seed(grant, real, real)
}

// Pending data may be the seed for a subsequent partial write. Destination is
// always real.~new, never the real file or an arbitrary caller-selected path.
pub fn copy_seed(grant: u64, source: &str, real: &str) -> Result<u64, i32> {
    let mut shadow = real.to_owned();
    shadow.push_str(".~new");
    if source == shadow {
        return Err(-1);
    }
    let input = gamepkg::file_open(grant, false, source)?;
    let output = match gamepkg::file_open(grant, true, &shadow) {
        Ok(handle) => handle,
        Err(error) => {
            let _ = gamepkg::file_close(input);
            return Err(error);
        }
    };
    let copied = copy_open(input, output);
    let input_closed = gamepkg::file_close(input);
    let output_closed = gamepkg::file_close(output);
    let result = match copied {
        Err(error) => Err(error),
        Ok(count) => match input_closed {
            Err(error) => Err(error),
            Ok(()) => match output_closed {
                Ok(()) => Ok(count),
                Err(error) => Err(error),
            },
        },
    };
    if result.is_err() {
        let _ = gamepkg::file_remove(grant, &shadow);
    }
    result
}

fn same_open(first: u64, second: u64) -> Result<bool, i32> {
    let mut left = std::vec::Vec::new();
    left.resize(32768, 0u8);
    let mut right = std::vec::Vec::new();
    right.resize(32768, 0u8);
    let mut offset = 0u64;
    loop {
        let a = read_block(first, offset, &mut left)?;
        let b = read_block(second, offset, &mut right)?;
        if a != b || left[..a] != right[..b] {
            return Ok(false);
        }
        if a < left.len() {
            return Ok(true);
        }
        offset = offset.checked_add(a as u64).ok_or(-1)?;
    }
}

pub fn same(grant: u64, first: &str, second: &str) -> Result<bool, i32> {
    let left = gamepkg::file_open(grant, false, first)?;
    let right = match gamepkg::file_open(grant, false, second) {
        Ok(handle) => handle,
        Err(error) => {
            let _ = gamepkg::file_close(left);
            return Err(error);
        }
    };
    let compared = same_open(left, right);
    let closed_left = gamepkg::file_close(left);
    let closed_right = gamepkg::file_close(right);
    closed_left?;
    closed_right?;
    compared
}

// Recovery compares this tuple lexicographically. It never treats a failed
// metadata read as a zero timestamp or proof that a pending save is obsolete.
pub fn modified(grant: u64, path: &str) -> Result<(i64, u32), i32> {
    let handle = gamepkg::file_open(grant, false, path)?;
    let metadata = gamepkg::file_metadata(handle);
    let close_result = gamepkg::file_close(handle);
    let metadata = metadata?;
    close_result?;
    Ok((metadata.modified_seconds, metadata.modified_nanoseconds))
}
