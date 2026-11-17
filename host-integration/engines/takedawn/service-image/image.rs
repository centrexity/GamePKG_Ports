// Checked XBE image plan and ordered data load. Machine bytes never execute.
use crate::disc;
use crate::memory;
use crate::sha256;
struct Section {
    flags: u32,
    address: u32,
    virtual_bytes: u32,
    raw: u32,
    raw_bytes: u32,
}
pub struct Image<'a> {
    file: disc::DiscFile<'a>,
    bytes: std::vec::Vec<u8>,
    headers: u32,
    sections: std::vec::Vec<Section>,
    entry: u32,
    thunk: u32,
    tls: u32,
    digest: [u8; 32],
    image_low: u32,
    image_high: u32,
    code_low: u32,
    code_high: u32,
}
fn xbe_word(bytes: &[u8], offset: u32) -> Result<u32, i32> {
    let end = offset.checked_add(4).ok_or(-1)?;
    if end as usize > bytes.len() {
        return Err(-1);
    }
    let at = offset as usize;
    Ok(u32::from_le_bytes([
        bytes[at],
        bytes[at + 1],
        bytes[at + 2],
        bytes[at + 3],
    ]))
}
fn xbe_extent(offset: u32, bytes: u32, limit: u32) -> Result<(), i32> {
    if offset > limit || bytes > limit - offset {
        return Err(-1);
    }
    Ok(())
}
impl<'a> Image<'a> {
    // The immutable bounded file snapshot is held only in process memory.
    // It prevents a later section read from silently mixing changed file data.
    pub fn image_plan(source: &'a disc::Disc, file: u32, expected: [u8; 32]) -> Result<Self, i32> {
        let mut file = source.disc_open_entry(file)?;
        let length = file.disc_file_size();
        if length < 0x178 || length > 16777216 {
            return Err(-1);
        }
        let mut prefix = [0u8; 0x178];
        file.disc_file_exact(0, &mut prefix)?;
        if prefix[..4] != *b"XBEH" || xbe_word(&prefix, 0x104)? != 0x10000 {
            return Err(-1);
        }
        let headers = xbe_word(&prefix, 0x108)?;
        if !(0x178..=65536).contains(&headers) || headers > length {
            return Err(-1);
        }
        let image_size = xbe_word(&prefix, 0x10c)?;
        if image_size < headers || image_size > 0x4000000 - 0x10000 {
            return Err(-1);
        }
        let image_end = 0x10000 + image_size;
        let count = xbe_word(&prefix, 0x11c)?;
        if count == 0 || count > 64 {
            return Err(-1);
        }
        let table = xbe_word(&prefix, 0x120)?.checked_sub(0x10000).ok_or(-1)?;
        xbe_extent(table, count * 56, headers)?;
        let mut bytes = std::vec::Vec::new();
        bytes.resize(length as usize, 0u8);
        let mut done = 0u32;
        while done < length {
            let part = (length - done).min(65536);
            if file.disc_file_read(&mut bytes[done as usize..(done + part) as usize])?
                != part as usize
            {
                return Err(-3);
            }
            done += part;
        }
        if bytes[..0x178] != prefix {
            return Err(-3);
        }
        let mut digest = sha256::Sha256::sha_new();
        digest.sha_update(&bytes)?;
        let digest = digest.sha_finish();
        if digest != expected {
            return Err(-1);
        }
        let mut sections = std::vec::Vec::new();
        let mut image_low = 0u32;
        let mut image_high = 0u32;
        let mut code_low = 0u32;
        let mut code_high = 0u32;
        for number in 0..count {
            let at = table + number * 56;
            let flags = xbe_word(&bytes, at)?;
            let address = xbe_word(&bytes, at + 4)?;
            let virtual_bytes = xbe_word(&bytes, at + 8)?;
            let raw = xbe_word(&bytes, at + 12)?;
            let raw_bytes = xbe_word(&bytes, at + 16)?;
            if address < 0x10000 || virtual_bytes == 0 {
                return Err(-1);
            }
            xbe_extent(address, virtual_bytes, image_end)?;
            let copied = raw_bytes.min(virtual_bytes);
            if copied != 0 {
                xbe_extent(raw, copied, length)?;
            }
            // Names stay metadata, but bound their lookup before accepting the
            // plan. Original titles can contain non-UTF8 section name bytes.
            let name = xbe_word(&bytes, at + 20)?.checked_sub(0x10000).ok_or(-1)?;
            if name >= headers {
                return Err(-1);
            }
            let mut terminated = false;
            for byte in &bytes[name as usize..headers as usize] {
                if *byte == 0 {
                    terminated = true;
                    break;
                }
            }
            if !terminated {
                return Err(-1);
            }
            if image_low == 0 || address < image_low {
                image_low = address;
            }
            image_high = image_high.max(address + virtual_bytes);
            if flags & 4 != 0 {
                if code_low == 0 || address < code_low {
                    code_low = address;
                }
                code_high = code_high.max(address + virtual_bytes);
            }
            sections.push(Section {
                flags,
                address,
                virtual_bytes,
                raw,
                raw_bytes,
            });
        }
        let encoded = xbe_word(&bytes, 0x128)?;
        let retail = encoded ^ 0xa8fc57ab;
        let debug = encoded ^ 0x94859d4b;
        let entry = if retail >= code_low && retail < code_high {
            retail
        } else {
            debug
        };
        let thunk_raw = xbe_word(&bytes, 0x158)?;
        let retail_thunk = thunk_raw ^ 0x5b6d40b6;
        let thunk = if retail_thunk >= 0x10000 && retail_thunk < 0x4000000 {
            retail_thunk
        } else {
            thunk_raw ^ 0xefb1f152
        };
        if code_low == 0
            || entry < code_low
            || entry >= code_high
            || thunk < 0x10000
            || thunk >= 0x4000000
        {
            return Err(-1);
        }
        // Entrypoints must have actual original bytes in an executable section,
        // not merely land in the min/max range spanning gaps or BSS.
        let mut backed = false;
        for section in &sections {
            if section.flags & 4 != 0
                && entry >= section.address
                && entry - section.address < section.raw_bytes.min(section.virtual_bytes)
            {
                backed = true;
            }
        }
        if !backed {
            return Err(-1);
        }
        let tls = xbe_word(&bytes, 0x12c)?;
        Ok(Self {
            file,
            bytes,
            headers,
            sections,
            entry,
            thunk,
            tls,
            digest,
            image_low,
            image_high,
            code_low,
            code_high,
        })
    }
    pub fn image_digest(&self) -> [u8; 32] {
        self.digest
    }
    pub fn image_entry(&self) -> u32 {
        self.entry
    }
    pub fn image_thunk(&self) -> u32 {
        self.thunk
    }
    pub fn image_tls(&self) -> u32 {
        self.tls
    }
    pub fn image_bounds(&self) -> [u32; 4] {
        [
            self.image_low,
            self.image_high,
            self.code_low,
            self.code_high,
        ]
    }
    pub fn image_section_count(&self) -> u32 {
        self.sections.len() as u32
    }
    pub fn image_load(&self, ram: &mut memory::Memory) -> Result<(), i32> {
        // Revocation remains effective before publication even though the file
        // snapshot is immutable. Cached bytes do not confer a new I/O grant.
        self.file.disc_file_exact(0, &mut [])?;
        ram.write_bytes(0x10000, &self.bytes[..self.headers as usize])?;
        for section in &self.sections {
            ram.fill(section.address, 0, section.virtual_bytes)?;
            let length = section.raw_bytes.min(section.virtual_bytes);
            let mut done = 0u32;
            while done < length {
                let part = (length - done).min(65536);
                let start = section.raw + done;
                ram.write_bytes(
                    section.address + done,
                    &self.bytes[start as usize..(start + part) as usize],
                )?;
                done += part;
            }
        }
        ram.call_bounds(self.code_low, self.code_high)?;
        Ok(())
    }
}
