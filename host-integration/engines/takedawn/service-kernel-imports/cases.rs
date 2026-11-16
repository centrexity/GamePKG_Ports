// Synthetic complete import runs. No image/assets/keys from a real title.
use crate::gamepkg;
use crate::memory;
pub struct Case {
    pub table: u32,
    pub count: u32,
    pub remap: std::vec::Vec<u16>,
    pub mixed: bool,
    pub mode: u32,
}
impl Case {
    pub fn import_case(mode: u32) -> Result<Self, i32> {
        if mode >= 16 {
            return Err(-1);
        }
        let table = match mode {
            8 => 0x20fff,
            9 => 0x21004,
            10 => 0x20ff0,
            11 => 0x10000,
            _ => 0x20000,
        };
        let count = match mode {
            1 | 12 => 1,
            2 => 35,
            3 => 3,
            4 | 5 => 378,
            6 => 361,
            7 => 376,
            11 | 13 => 2,
            _ => 33,
        };
        let mut remap = std::vec::Vec::new();
        if mode == 1 {
            remap.resize(501, 0u16);
            remap[500] = 16;
            remap[14] = 23;
        }
        if mode == 2 {
            remap.resize(166, 0u16);
            remap[165] = 14;
        }
        if mode == 3 {
            remap.resize(65536, 0u16);
            remap[65535] = 15;
            remap[14] = 22;
            remap[0] = 14;
        }
        Ok(Self {
            table,
            count,
            remap,
            mixed: mode == 10,
            mode,
        })
    }
    pub fn import_seed(&self, ram: &mut memory::Memory) -> Result<(), i32> {
        ram.fill(self.table - 64, 0, 64 + 379 * 4)?;
        for index in 0..self.count {
            let ordinal = if self.mode == 5 {
                [14u32, 16, 15, 22][index as usize % 4]
            } else if self.mode == 6 || self.mode == 7 {
                14
            } else {
                [
                    14u32, 15, 17, 23, 165, 166, 168, 171, 180, 16, 22, 30, 31, 40, 41, 42, 64, 70,
                    71, 156, 157, 164, 259, 322, 323, 324, 325, 326, 353, 354, 355, 356, 357,
                ][index as usize % 33]
            };
            ram.write_u32(self.table + index * 4, 0x80000000 | ordinal)?;
        }
        match self.mode {
            1 => ram.write_u32(self.table, 0x800001f4)?,
            2 => {
                ram.write_u32(self.table, 0x12345678)?;
                ram.write_u32(self.table + 34 * 4, 0x76543210)?;
            }
            3 => {
                ram.write_u32(self.table, 0x8000ffff)?;
                ram.write_u32(self.table + 4, 0x8000000e)?;
                ram.write_u32(self.table + 8, 0x80000000)?;
            }
            4 => {
                ram.fill(self.table, 0, 378 * 4)?;
                ram.write_u32(self.table + 5 * 4, 0x8000000e)?;
                ram.write_u32(self.table + 19 * 4, 0x80000010)?;
                ram.write_u32(self.table + 377 * 4, 0x80000017)?;
            }
            11 => {
                ram.write_u32(self.table, 0x80000010)?;
                ram.write_u32(self.table + 4, 0x8000000e)?;
            }
            12 => ram.write_u32(self.table, 0x12345678)?,
            13 => {
                ram.write_u32(self.table, 0x80000161)?;
                ram.write_u32(self.table + 4, 0x80000165)?;
            }
            _ => {}
        }
        for (distance, ordinal) in match self.mode {
            1 => [(1u32, 14u32), (0, 0)],
            2 => [(2, 165), (0, 0)],
            3 => [(16, 17), (0, 0)],
            4 | 5 | 12 => [(0, 0), (0, 0)],
            6 => [(16, 23), (0, 0)],
            7 => [(1, 14), (0, 0)],
            13 => [(1, 171), (0, 0)],
            14 => [(1, 166), (16, 14)],
            _ => [(1, 14), (16, 15)],
        } {
            if distance != 0 {
                ram.write_u32(self.table - distance * 4, 0x80000000 | ordinal)?;
            }
        }
        ram.write_u32(self.table - 12, 0x1234)?;
        Ok(())
    }
    pub fn import_states(&self, ram: &memory::Memory) -> Result<std::vec::Vec<[u64; 4]>, i32> {
        let count = self.count;
        let end = self.table + count * 4 + if self.mode == 4 { 0 } else { 4 };
        let mut pages = std::vec::Vec::new();
        pages.resize(2, [0u64; 4]);
        let mut used = 0usize;
        let mut page = (self.table - 64) & !4095u32;
        while page < end {
            let backing = ram.memory_page_backing(page)?;
            let mode = if self.mixed && used == 1 {
                2u32
            } else if self.mode % 3 == 0 {
                2
            } else {
                1
            };
            let mut request = std::vec::Vec::new();
            request.extend(u64::to_le_bytes(backing[0]));
            request.extend(u64::to_le_bytes(backing[1]));
            request.extend(u64::to_le_bytes(4096));
            request.extend(u32::to_le_bytes(mode));
            gamepkg::local_call(3, &request, &mut [])?;
            pages[used] = [u64::from(page), backing[0], backing[1], u64::from(mode + 1)];
            used += 1;
            page += 4096;
        }
        pages.truncate(used);
        Ok(pages)
    }
}
