// Ordinary checked game probe. Metadata only; no file bytes leave the child.
use crate::disc;
use crate::gamepkg;
fn disc_probe(handle: u64) -> Result<(), i32> {
    let image = disc::Disc::disc_open(handle)?;
    gamepkg::diagnostic(
        1,
        0x544b0100,
        &[image.disc_partition_offset(), u64::from(image.disc_count())],
    )?;
    let mut record = std::vec::Vec::new();
    record.resize(532, 0u8);
    let mut retained_view = None;
    for number in 0..image.disc_count() {
        let item = image.disc_entry(number)?;
        let name = item.disc_path_bytes();
        record[..4].copy_from_slice(&u32::to_le_bytes(name.len() as u32));
        record[4..12].copy_from_slice(&u64::to_le_bytes(item.offset));
        record[12..16].copy_from_slice(&u32::to_le_bytes(item.size));
        record[16..20].copy_from_slice(&u32::to_le_bytes(u32::from(item.directory)));
        record[20..20 + name.len()].copy_from_slice(name);
        gamepkg::diagnostic_bytes(1, 0x544b0101, &record[..20 + name.len()])?;
        if !item.directory {
            let lookup = image.disc_find(gamepkg::text_from_utf8(name)?)?;
            if lookup != number {
                return Err(-3);
            }
            let mut alternate = [0u8; 512];
            for (position, byte) in name.iter().enumerate() {
                alternate[position] = if *byte >= b'a' && *byte <= b'z' {
                    *byte - 32
                } else if *byte == b'/' {
                    b'\\'
                } else {
                    *byte
                };
            }
            if image.disc_find(gamepkg::text_from_utf8(&alternate[..name.len()])?)? != number {
                return Err(-3);
            }
            let mut first = [0u8; 64];
            let length = item.size.min(64) as usize;
            let mut stream = image.disc_open_file(gamepkg::text_from_utf8(name)?)?;
            if stream.disc_file_size() != item.size
                || stream.disc_file_read(&mut first[..length])? != length
                || stream.disc_file_position() != length as u64
            {
                return Err(-3);
            }
            let mut tail = [0u8; 2];
            let tail_length = usize::from(item.size != 0);
            if stream.disc_file_read_at(u64::from(item.size.saturating_sub(1)), &mut tail)?
                != tail_length
                || stream.disc_file_position() != length as u64
                || stream.disc_file_read_at(u64::MAX, &mut tail)? != 0
            {
                return Err(-3);
            }
            let view = stream.disc_file_open_handle()?;
            let valid = disc_probe_view(view, item.size, &first[..length]);
            if retained_view.is_none() {
                retained_view = Some(view);
                valid?;
            } else {
                let closed = gamepkg::file_close(view);
                valid?;
                closed?;
            }
            // Data witnesses only: lengths and a simple non-security checksum.
            // Integrity admission must use the independently pinned hashes.
            let mut checksum = 0u64;
            for byte in &first[..length] {
                checksum = checksum.wrapping_mul(257).wrapping_add(u64::from(*byte));
            }
            gamepkg::diagnostic(0, 0x544b0102, &[u64::from(number), length as u64, checksum])?;
            let mut empty = [];
            image.disc_read(number, u64::from(item.size), &mut empty)?;
            let mut byte = [0u8; 1];
            if image.disc_read(number, u64::from(item.size), &mut byte) != Err(-1) {
                return Err(-3);
            }
        }
    }
    for path in [
        "../secret",
        "/absolute",
        "C:/secret",
        "a//b",
        "a/../b",
        "a/./b",
        ".",
        "..",
        "a/",
        "a\\..\\b",
    ] {
        if image.disc_find(path) != Err(-1) {
            return Err(-3);
        }
    }
    // Deterministic host revocation handshake, using only the ordinary clock
    // service. Sample before announcing readiness so an early wake is retained.
    let (now, generation) = gamepkg::monotonic()?;
    gamepkg::diagnostic(1, 0x544b0103, &[handle, generation])?;
    let (_, _, reason) = gamepkg::wait_until(now.checked_add(1000000000).ok_or(-1)?, generation)?;
    if reason != 1 || gamepkg::file_size(handle) != Err(-4) {
        return Err(-3);
    }
    if let Some(view) = retained_view {
        if gamepkg::file_size(view) != Err(-4)
            || gamepkg::file_read(view, 0, &mut []) != Err(-4)
            || gamepkg::file_close(view) != Err(-4)
        {
            return Err(-3);
        }
    }
    for number in 0..image.disc_count() {
        let item = image.disc_entry(number)?;
        if !item.directory {
            if image.disc_open_entry(number).err() != Some(-4) {
                return Err(-3);
            }
            let mut bytes = [0u8; 1];
            if item.size != 0 && image.disc_read(number, 0, &mut bytes) != Err(-4) {
                return Err(-3);
            }
            if image.disc_read(number, 0, &mut []) != Err(-4) {
                return Err(-3);
            }
        }
    }
    match disc::Disc::disc_open(handle) {
        Err(-4) => {}
        _ => return Err(-3),
    }
    Ok(())
}
fn disc_probe_view(view: u64, size: u32, prefix: &[u8]) -> Result<(), i32> {
    let mut bytes = [0u8; 64];
    let mut filled = 0usize;
    while filled < prefix.len() {
        let count = gamepkg::file_read(view, filled as u64, &mut bytes[filled..prefix.len()])?;
        if count == 0 || count > prefix.len() - filled {
            return Err(-3);
        }
        filled += count;
    }
    if &bytes[..filled] != prefix
        || gamepkg::file_size(view)? != u64::from(size)
        || gamepkg::file_read(view, u64::from(size), &mut bytes)? != 0
    {
        return Err(-3);
    }
    Ok(())
}
pub fn game_main() {
    let result = disc_run();
    let code = match result {
        Ok(()) => 0u64,
        Err(error) => error as i64 as u64,
    };
    let _ = gamepkg::diagnostic(1, 0x544b01ff, &[code]);
}
fn disc_run() -> Result<(), i32> {
    let mut found = false;
    for grant in gamepkg::grants()? {
        if grant.kind == 3 {
            found = true;
            disc_probe(grant.id)?;
        }
    }
    if !found {
        return Err(-4);
    }
    Ok(())
}
