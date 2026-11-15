// Port-owned guest heap bookkeeping; all guest byte access uses checked RAM.
use crate::memory;
pub struct Heap {
    record: u64,
    next: u32,
    top: u32,
    count: u32,
    allocations: u32,
    blocks: std::vec::Vec<u32>,
}
impl Heap {
    pub fn heap_new(ram: &memory::Memory, base: u32, top: u32) -> Result<Self, i32> {
        let _ = ram.call_snapshot()?;
        if base < 65536 || base > top || top > 67108864 {
            return Err(-1);
        }
        let mut blocks = std::vec::Vec::new();
        blocks.resize(196608, 0u32);
        Ok(Self {
            record: ram.call_handle(),
            next: base,
            top,
            count: 0,
            allocations: 0,
            blocks,
        })
    }
    fn heap_live(&self, ram: &memory::Memory) -> Result<(), i32> {
        if ram.call_handle() != self.record {
            return Err(-4);
        }
        let _ = ram.call_snapshot()?;
        Ok(())
    }
    pub fn heap_allocate(
        &mut self,
        ram: &mut memory::Memory,
        size: u32,
        alignment: u32,
    ) -> Result<u32, i32> {
        self.heap_live(ram)?;
        let size = size.max(16);
        let alignment = alignment.max(4);
        for number in 0..self.count as usize {
            let at = number * 3;
            if self.blocks[at + 2] == 0 || self.blocks[at + 1] < size {
                continue;
            }
            let address = self.blocks[at];
            if address & (alignment - 1) != 0 {
                continue;
            }
            // Complete RAM validation/fill precedes bookkeeping publication.
            // Reuse clears only the requested bytes, without shrinking/split.
            ram.fill(address, 0, size)?;
            self.blocks[at + 2] = 0;
            return Ok(address);
        }
        let address = self.next.checked_add(alignment - 1).ok_or(-1)? & !(alignment - 1);
        let end = address.checked_add(size).ok_or(-1)?;
        if end > self.top {
            return Ok(0);
        }
        let allocations = self.allocations.checked_add(1).ok_or(-3)?;
        ram.fill(address, 0, size)?;
        self.next = end;
        if self.count < 65536 {
            let at = self.count as usize * 3;
            self.blocks[at] = address;
            self.blocks[at + 1] = size;
            self.blocks[at + 2] = 0;
            self.count += 1;
        }
        // Preserve the original successful but untracked allocation when its
        // fixed block table is full. Free/query cannot subsequently find it.
        self.allocations = allocations;
        Ok(address)
    }
    pub fn heap_size(&self, ram: &memory::Memory, address: u32) -> Result<u32, i32> {
        self.heap_live(ram)?;
        if address == 0 {
            return Ok(0);
        }
        for number in 0..self.count as usize {
            let at = number * 3;
            if self.blocks[at + 2] != 0 {
                continue;
            }
            let start = self.blocks[at];
            let bytes = self.blocks[at + 1];
            if address >= start && address - start < bytes {
                return Ok(bytes - (address - start));
            }
        }
        Ok(0)
    }
    pub fn heap_free(&mut self, ram: &memory::Memory, address: u32) -> Result<(), i32> {
        self.heap_live(ram)?;
        if address == 0 {
            return Ok(());
        }
        for number in 0..self.count as usize {
            let at = number * 3;
            if self.blocks[at] != address || self.blocks[at + 2] != 0 {
                continue;
            }
            self.blocks[at + 2] = 1;
            if number + 1 < self.count as usize
                && self.blocks[at + 5] != 0
                && self.blocks[at] + self.blocks[at + 1] == self.blocks[at + 3]
            {
                self.blocks[at + 1] += self.blocks[at + 4];
                self.blocks[at + 4] = 0;
                self.blocks[at + 3] = 0;
            }
            if number > 0
                && self.blocks[at - 1] != 0
                && self.blocks[at - 3] + self.blocks[at - 2] == self.blocks[at]
            {
                self.blocks[at - 2] += self.blocks[at + 1];
                self.blocks[at + 1] = 0;
                self.blocks[at] = 0;
            }
            return Ok(());
        }
        Ok(())
    }
    // Copied value diagnostics, not a way to mutate or share the owned heap.
    pub fn heap_stats(&self) -> [u32; 3] {
        [self.next, self.count, self.allocations]
    }
    pub fn heap_record(&self, number: u32) -> Result<[u32; 3], i32> {
        if number >= self.count {
            return Err(-1);
        }
        let at = number as usize * 3;
        Ok([self.blocks[at], self.blocks[at + 1], self.blocks[at + 2]])
    }
}
