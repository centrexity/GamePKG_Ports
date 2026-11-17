// Port-owned XDVDFS metadata over a GamePKG read-only file/ROM handle.
// Offsets and disc paths are data; no host path, descriptor or device escapes.
use crate::gamepkg;
pub struct Entry {
    path: [u8; 512],
    length: u32,
    pub offset: u64,
    pub size: u32,
    pub directory: bool,
}
struct Pending {
    sector: u32,
    bytes: u32,
    prefix: [u8; 512],
    length: u32,
    depth: u32,
}
pub struct Disc {
    handle: u64,
    length: u64,
    base: u64,
    entries: std::vec::Vec<Entry>,
}
// An opened regular extent tied to the indexed image lifetime. Opening a
// second stream on the same entry creates independent sequential state.
pub struct DiscFile<'a> {
    disc: &'a Disc,
    entry: &'a Entry,
    position: u64,
}
fn disc_word(bytes: &[u8], at: u32) -> u32 {
    let index = at as usize;
    u32::from_le_bytes([
        bytes[index],
        bytes[index + 1],
        bytes[index + 2],
        bytes[index + 3],
    ])
}
fn disc_half(bytes: &[u8], at: u32) -> u32 {
    let index = at as usize;
    u32::from(u16::from_le_bytes([bytes[index], bytes[index + 1]]))
}
fn disc_lower(byte: u8) -> u8 {
    if byte >= b'A' && byte <= b'Z' {
        byte + 32
    } else {
        byte
    }
}
fn disc_extent(offset: u64, bytes: u64, limit: u64) -> Result<(), i32> {
    if offset > limit || bytes > limit - offset {
        return Err(-1);
    }
    Ok(())
}
fn disc_exact(handle: u64, limit: u64, offset: u64, bytes: &mut [u8]) -> Result<(), i32> {
    if bytes.len() > 1048576 {
        return Err(-1);
    }
    disc_extent(offset, bytes.len() as u64, limit)?;
    if bytes.is_empty() {
        if gamepkg::file_read(handle, offset, bytes)? != 0 {
            return Err(-3);
        }
        return Ok(());
    }
    let mut done = 0u32;
    while (done as usize) < bytes.len() {
        let part = (bytes.len() - done as usize).min(65536);
        let count = gamepkg::file_read(
            handle,
            offset + u64::from(done),
            &mut bytes[done as usize..done as usize + part],
        )?;
        if count == 0 || count > part {
            return Err(-3);
        }
        done += count as u32;
    }
    Ok(())
}
fn disc_emit(path: &mut [u8; 512], length: &mut u32, byte: u8) -> Result<(), i32> {
    // Match the existing reader's <512-byte UTF-8 path bound.
    if *length >= 511 {
        return Err(-1);
    }
    path[*length as usize] = byte;
    *length += 1;
    Ok(())
}
fn disc_same(first: &[u8], second: &[u8]) -> bool {
    if first.len() != second.len() {
        return false;
    }
    let mut index = 0u32;
    while (index as usize) < first.len() {
        if disc_lower(first[index as usize]) != disc_lower(second[index as usize]) {
            return false;
        }
        index += 1;
    }
    true
}
impl Entry {
    pub fn disc_path_bytes(&self) -> &[u8] {
        &self.path[..self.length as usize]
    }
}
impl Disc {
    // The caller owns/closes the capability. Creating/dropping an index never
    // duplicates or reopens it, and revocation is enforced by GamePKG reads.
    pub fn disc_open(handle: u64) -> Result<Self, i32> {
        let length = gamepkg::file_size(handle)?;
        let mut descriptor = [0u8; 2048];
        let mut base = None;
        for candidate in [
            0u64, 0xfd90, 0x2080, 0x30600, 0xfd90000, 0x18300000, 0x1fb20000,
        ] {
            let offset = candidate + 65536;
            if disc_extent(offset, 2048, length).is_err() {
                continue;
            }
            disc_exact(handle, length, offset, &mut descriptor)?;
            if descriptor[..20] == *b"MICROSOFT*XBOX*MEDIA"
                && descriptor[0x7ec..] == *b"MICROSOFT*XBOX*MEDIA"
            {
                base = Some(candidate);
                break;
            }
        }
        let base = base.ok_or(-1)?;
        let mut image = Self {
            handle,
            length,
            base,
            entries: std::vec::Vec::new(),
        };
        image.disc_index(disc_word(&descriptor, 0x14), disc_word(&descriptor, 0x18))?;
        Ok(image)
    }
    fn disc_index(&mut self, sector: u32, bytes: u32) -> Result<(), i32> {
        let mut pending = std::vec::Vec::new();
        pending.push(Pending {
            sector,
            bytes,
            prefix: [0; 512],
            length: 0,
            depth: 0,
        });
        let mut tables = std::vec::Vec::new();
        tables.resize(8193, 0u32);
        let mut table_count = 0u32;
        while let Some(directory) = pending.pop() {
            if directory.depth > 32 || directory.bytes > 1048576 {
                return Err(-1);
            }
            if directory.bytes == 0 {
                continue;
            }
            for slot in 0..table_count {
                if tables[slot as usize] == directory.sector {
                    return Err(-1);
                }
            }
            if table_count >= 8193 {
                return Err(-1);
            }
            tables[table_count as usize] = directory.sector;
            table_count += 1;
            let offset = self.base + u64::from(directory.sector) * 2048;
            disc_extent(offset, u64::from(directory.bytes), self.length)?;
            let mut table = std::vec::Vec::new();
            table.resize(directory.bytes as usize, 0u8);
            disc_exact(self.handle, self.length, offset, &mut table)?;
            let slots = directory.bytes.div_ceil(4);
            let mut occupied = std::vec::Vec::new();
            occupied.resize(slots as usize, 0u8);
            let mut seen = std::vec::Vec::new();
            seen.resize(slots as usize, 0u8);
            let mut todo = std::vec::Vec::new();
            todo.push(0u32);
            while let Some(at) = todo.pop() {
                if at.checked_add(14).ok_or(-1)? > directory.bytes || seen[(at / 4) as usize] != 0 {
                    return Err(-1);
                }
                seen[(at / 4) as usize] = 1;
                let left = disc_half(&table, at);
                if left == 65535 {
                    continue;
                }
                let right = disc_half(&table, at + 2);
                let start = disc_word(&table, at + 4);
                let size = disc_word(&table, at + 8);
                let attributes = table[(at + 12) as usize];
                let count = u32::from(table[(at + 13) as usize]);
                let end = at + 14 + count;
                if count == 0 || end > directory.bytes {
                    return Err(-1);
                }
                for slot in at / 4..end.div_ceil(4) {
                    if occupied[slot as usize] != 0 {
                        return Err(-1);
                    }
                    occupied[slot as usize] = 1;
                }
                let name = &table[(at + 14) as usize..end as usize];
                if name == b"." || name == b".." {
                    return Err(-1);
                }
                let mut path = directory.prefix;
                let mut length = directory.length;
                if length != 0 {
                    disc_emit(&mut path, &mut length, b'/')?;
                }
                for byte in name {
                    if *byte < 32
                        || *byte == 127
                        || *byte == b'/'
                        || *byte == b'\\'
                        || *byte == b':'
                    {
                        return Err(-1);
                    }
                    if *byte < 128 {
                        disc_emit(&mut path, &mut length, *byte)?;
                    } else {
                        disc_emit(&mut path, &mut length, 192 | (*byte >> 6))?;
                        disc_emit(&mut path, &mut length, 128 | (*byte & 63))?;
                    }
                }
                if self.entries.len() >= 8192 {
                    return Err(-1);
                }
                for previous in &self.entries {
                    if disc_same(previous.disc_path_bytes(), &path[..length as usize]) {
                        return Err(-1);
                    }
                }
                let position = self.base + u64::from(start) * 2048;
                disc_extent(position, u64::from(size), self.length)?;
                let is_directory = attributes & 16 != 0;
                self.entries.push(Entry {
                    path,
                    length,
                    offset: position,
                    size,
                    directory: is_directory,
                });
                if is_directory {
                    pending.push(Pending {
                        sector: start,
                        bytes: size,
                        prefix: path,
                        length,
                        depth: directory.depth + 1,
                    });
                }
                if left != 0 {
                    todo.push(left * 4);
                }
                if right != 0 {
                    todo.push(right * 4);
                }
            }
        }
        Ok(())
    }
    pub fn disc_partition_offset(&self) -> u64 {
        self.base
    }
    pub fn disc_count(&self) -> u32 {
        self.entries.len() as u32
    }
    pub fn disc_entry(&self, index: u32) -> Result<&Entry, i32> {
        self.entries.get(index as usize).ok_or(-1)
    }
    pub fn disc_find(&self, path: &str) -> Result<u32, i32> {
        let bytes = path.as_bytes();
        if bytes.is_empty() || bytes.len() >= 512 {
            return Err(-1);
        }
        let mut normalized = [0u8; 512];
        let mut component = 0u32;
        for (index, byte) in bytes.iter().enumerate() {
            if *byte < 32 || *byte == 127 || *byte == b':' {
                return Err(-1);
            }
            let next = if *byte == b'\\' { b'/' } else { *byte };
            if next == b'/' {
                if component == 0 {
                    return Err(-1);
                }
                let segment = &normalized[index - component as usize..index];
                if segment == b"." || segment == b".." {
                    return Err(-1);
                }
                component = 0;
            } else {
                component += 1;
            }
            normalized[index] = next;
        }
        if component == 0 {
            return Err(-1);
        }
        let segment = &normalized[bytes.len() - component as usize..bytes.len()];
        if segment == b"." || segment == b".." {
            return Err(-1);
        }
        for (index, item) in self.entries.iter().enumerate() {
            if disc_same(item.disc_path_bytes(), &normalized[..bytes.len()]) {
                return Ok(index as u32);
            }
        }
        Err(-5)
    }
    fn disc_stream(&self, index: u32) -> Result<DiscFile<'_>, i32> {
        let entry = self.disc_entry(index)?;
        if entry.directory {
            return Err(-1);
        }
        Ok(DiscFile {
            disc: self,
            entry,
            position: 0,
        })
    }
    pub fn disc_open_entry(&self, index: u32) -> Result<DiscFile<'_>, i32> {
        let file = self.disc_stream(index)?;
        file.disc_file_exact(0, &mut [])?;
        Ok(file)
    }
    pub fn disc_open_file(&self, path: &str) -> Result<DiscFile<'_>, i32> {
        self.disc_open_entry(self.disc_find(path)?)
    }
    pub fn disc_read(&self, index: u32, offset: u64, bytes: &mut [u8]) -> Result<(), i32> {
        self.disc_stream(index)?.disc_file_exact(offset, bytes)
    }
}
impl DiscFile<'_> {
    pub fn disc_file_size(&self) -> u32 {
        self.entry.size
    }

    pub fn disc_file_position(&self) -> u64 {
        self.position
    }

    pub fn disc_file_exact(&self, offset: u64, bytes: &mut [u8]) -> Result<(), i32> {
        disc_extent(offset, bytes.len() as u64, u64::from(self.entry.size))?;
        let absolute = self.entry.offset.checked_add(offset).ok_or(-1)?;
        disc_exact(self.disc.handle, self.disc.length, absolute, bytes)
    }

    // Report short requests at logical EOF without touching the next extent.
    // Even an empty or EOF read must validate that the grant remains live.
    pub fn disc_file_read_at(&self, offset: u64, bytes: &mut [u8]) -> Result<usize, i32> {
        if bytes.len() > 1048576 {
            return Err(-1);
        }
        let end = u64::from(self.entry.size);
        let start = offset.min(end);
        let count = (end - start).min(bytes.len() as u64) as usize;
        self.disc_file_exact(start, &mut bytes[..count])?;
        Ok(count)
    }

    // A failed service read can leave a partial buffer but never advances the
    // cursor. Retrying remains the caller's decision.
    pub fn disc_file_read(&mut self, bytes: &mut [u8]) -> Result<usize, i32> {
        let count = self.disc_file_read_at(self.position, bytes)?;
        self.position += count as u64;
        Ok(count)
    }
}
