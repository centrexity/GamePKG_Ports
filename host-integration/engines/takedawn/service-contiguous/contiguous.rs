// Port-owned contiguous allocator data. One reviewed task must own this state;
// guest callers must not construct independent allocators over the same bank.
use crate::gamepkg;
use crate::memory;
pub struct Contiguous {
    record: u64,
    window: u64,
    used: std::vec::Vec<u8>,
    lengths: std::vec::Vec<u32>,
    in_use: u32,
    high: u32,
    first_free: u32,
    allocations: u32,
    frees: u32,
    peak: u32,
}
fn contiguous_long(bytes: &[u8], at: usize) -> Result<u64, i32> {
    let part = bytes.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        part[0], part[1], part[2], part[3], part[4], part[5], part[6], part[7],
    ]))
}
impl Contiguous {
    pub fn contiguous_new(ram: &memory::Memory, window: u64) -> Result<Self, i32> {
        ram.call_snapshot()?;
        let mut banks = [0u8; 48];
        if gamepkg::local_call(1, &[], &mut banks)? != 48
            || contiguous_long(&banks, 8)? != 67108864
            || contiguous_long(&banks, 24)? != 67108864
            || contiguous_long(&banks, 40)? != 1048576
        {
            return Err(-1);
        }
        if window == 0 || window != contiguous_long(&banks, 16)? {
            return Err(-4);
        }
        let mut used = std::vec::Vec::new();
        used.resize(16384, 0u8);
        used[0] = 2; // Physical page zero is reserved, not a title allocation.
        let mut lengths = std::vec::Vec::new();
        lengths.resize(16384, 0u32);
        Ok(Self {
            record: ram.call_handle(),
            window,
            used,
            lengths,
            in_use: 0,
            high: 4096,
            first_free: 1,
            allocations: 0,
            frees: 0,
            peak: 0,
        })
    }
    fn contiguous_live(&self, ram: &memory::Memory) -> Result<(), i32> {
        if ram.call_handle() != self.record {
            return Err(-4);
        }
        ram.call_snapshot()?;
        Ok(())
    }
    fn contiguous_take(&mut self, first: u32, pages: u32) {
        self.used[first as usize..(first + pages) as usize].fill(1);
        self.lengths[first as usize] = pages;
        self.in_use += pages;
        self.peak = self.peak.max(self.in_use);
        self.high = self.high.max((first + pages) * 4096);
        while self.first_free < 16384 && self.used[self.first_free as usize] != 0 {
            self.first_free += 1;
        }
    }
    pub fn contiguous_allocate(
        &mut self,
        ram: &memory::Memory,
        size: u32,
        alignment: u32,
        phys_low: u32,
        phys_high: u32,
    ) -> Result<u32, i32> {
        self.contiguous_live(ram)?;
        let mut alignment = alignment.max(4096);
        if alignment & (alignment - 1) != 0 {
            let mut rounded = 4096u32;
            while rounded != 0 && rounded < alignment {
                rounded = rounded.wrapping_shl(1);
            }
            alignment = if rounded == 0 { 0x80000000 } else { rounded };
        }
        let pages = ((u64::from(size.max(1)) + 4095) / 4096) as u32;
        let align_pages = alignment / 4096;
        let low_page = ((u64::from(phys_low) + 4095) / 4096) as u32;
        let limit = (((u64::from(phys_high) + 1) / 4096) as u32).min(16352);
        let mut at = (low_page.max(self.first_free) + align_pages - 1) & !(align_pages - 1);
        while u64::from(at) + u64::from(pages) <= u64::from(limit) {
            // Reserve 15..16 MiB for callers constrained below 16 MiB.
            if limit > 4096 && at < 4096 && at + pages > 3840 {
                at = (4096 + align_pages - 1) & !(align_pages - 1);
                continue;
            }
            let mut end = at;
            while end < at + pages && self.used[end as usize] == 0 {
                end += 1;
            }
            if end == at + pages {
                // Validate/zero the complete bank range before publishing the
                // allocation. Failed page access leaves all metadata unchanged.
                gamepkg::local_memory_fill(
                    self.window,
                    u64::from(at) * 4096,
                    u64::from(pages) * 4096,
                    0,
                )?;
                self.contiguous_take(at, pages);
                self.allocations = self.allocations.wrapping_add(1);
                return Ok(0x80000000 + at * 4096);
            }
            while end < limit && self.used[end as usize] != 0 {
                end += 1;
            }
            at = (end + align_pages - 1) & !(align_pages - 1);
        }
        Ok(0)
    }
    pub fn contiguous_claim(
        &mut self,
        ram: &memory::Memory,
        address: u32,
        size: u32,
    ) -> Result<(), i32> {
        self.contiguous_live(ram)?;
        if address < 0x80000000 || size == 0 {
            return Ok(());
        }
        let first = (address - 0x80000000) / 4096;
        let pages = ((u64::from(address & 4095) + u64::from(size) + 4095) / 4096) as u32;
        if u64::from(first) + u64::from(pages) > 16384 {
            return Ok(());
        }
        for number in first..first + pages {
            if self.used[number as usize] != 0 {
                return Ok(());
            }
        }
        self.contiguous_take(first, pages);
        self.allocations = self.allocations.wrapping_add(1);
        Ok(())
    }
    pub fn contiguous_free(&mut self, ram: &memory::Memory, address: u32) -> Result<bool, i32> {
        self.contiguous_live(ram)?;
        if !(0x80000000..0x84000000).contains(&address) || address & 4095 != 0 {
            return Ok(false);
        }
        let first = (address - 0x80000000) / 4096;
        let pages = self.lengths[first as usize];
        if pages == 0 {
            return Ok(false);
        }
        self.used[first as usize..(first + pages) as usize].fill(0);
        self.lengths[first as usize] = 0;
        self.in_use -= pages;
        self.first_free = self.first_free.min(first);
        self.frees = self.frees.wrapping_add(1);
        Ok(true)
    }
    pub fn contiguous_size(&self, ram: &memory::Memory, address: u32) -> Result<u32, i32> {
        self.contiguous_live(ram)?;
        if !(0x80000000..0x84000000).contains(&address) {
            return Ok(0);
        }
        let page = (address - 0x80000000) / 4096;
        if self.used[page as usize] != 1 {
            return Ok(0);
        }
        let mut first = page;
        while first > 0 && self.lengths[first as usize] == 0 {
            first -= 1;
        }
        Ok(0x80000000 + (first + self.lengths[first as usize]) * 4096 - address)
    }
    // Copied metadata, never a mutable table reference/native page-map pointer.
    pub fn contiguous_stats(&self) -> [u32; 6] {
        [
            self.in_use,
            self.high,
            self.first_free,
            self.allocations,
            self.frees,
            self.peak,
        ]
    }
    pub fn contiguous_record(&self, page: u32) -> Result<[u32; 2], i32> {
        if page >= 16384 {
            return Err(-1);
        }
        Ok([
            u32::from(self.used[page as usize]),
            self.lengths[page as usize],
        ])
    }
    pub fn contiguous_pages(&self, first: u32, output: &mut [u8]) -> Result<(), i32> {
        if output.len() > 4096 || u64::from(first) + output.len() as u64 > 16384 {
            return Err(-1);
        }
        output.copy_from_slice(&self.used[first as usize..first as usize + output.len()]);
        Ok(())
    }
}
