// Owned values for the pinned ICALL trace and non-code diagnostic throttle.
// Targets are guest numbers. This module never resolves/calls a native pointer.
use crate::gamepkg;
pub struct CallState {
    low: u32,
    high: u32,
    trace: [u32; 16],
    index: u32,
    total: u64,
    seen: [u32; 16],
    hits: [u64; 16],
    used: u32,
    services: u64,
}
impl CallState {
    pub fn call_default() -> Self {
        Self {
            low: 0,
            high: 0,
            trace: [0; 16],
            index: 0,
            total: 0,
            seen: [0; 16],
            hits: [0; 16],
            used: 0,
            services: 0,
        }
    }
    pub fn call_new(low: u32, high: u32) -> Result<Self, i32> {
        let mut state = Self::call_default();
        state.call_set_bounds(low, high)?;
        Ok(state)
    }
    pub fn call_set_bounds(&mut self, low: u32, high: u32) -> Result<(), i32> {
        if high != 0 && low > high {
            return Err(-1);
        }
        self.low = low;
        self.high = high;
        Ok(())
    }
    // Match the original ordering: write target, advance the ring and total,
    // then test code bounds. Zero high disables bounds; >= FE000000 is kernel.
    pub fn call_record(&mut self, target: u32) -> bool {
        self.trace[(self.index & 15) as usize] = target;
        self.index = self.index.wrapping_add(1);
        self.total = self.total.wrapping_add(1);
        target >= 0xfe000000 || self.high == 0 || (target >= self.low && target < self.high)
    }
    pub fn call_trace(&self) -> [u32; 16] {
        self.trace
    }
    pub fn call_counters(&self) -> (u32, u64) {
        (self.index, self.total)
    }
    // The original logger tracks only the first 16 distinct non-code targets,
    // increments wrapping hit counts, and emits at powers of ten (1 included).
    pub fn call_noncode_values(&mut self, target: u32) -> Option<[u64; 3]> {
        let mut slot = 0u32;
        while slot < self.used && self.seen[slot as usize] != target {
            slot += 1;
        }
        if slot == self.used {
            if self.used == 16 {
                return None;
            }
            self.seen[slot as usize] = target;
            self.hits[slot as usize] = 0;
            self.used += 1;
        }
        self.hits[slot as usize] = self.hits[slot as usize].wrapping_add(1);
        let mut reduced = self.hits[slot as usize];
        while reduced >= 10 && reduced % 10 == 0 {
            reduced /= 10;
        }
        if reduced != 1 {
            return None;
        }
        Some([u64::from(target), self.hits[slot as usize], self.total])
    }
    // Physical ring plus its index preserves both the original trace ordering
    // and zero entries; the host can render the original nonzero recent list.
    pub fn call_missing_values(&self, target: u32) -> [u64; 19] {
        let mut words = [0u64; 19];
        words[0] = u64::from(target);
        words[1] = self.total;
        words[2] = u64::from(self.index);
        for index in 0..16 {
            words[index + 3] = u64::from(self.trace[index]);
        }
        words
    }
    pub fn call_emit_noncode(&mut self, target: u32) -> Result<Option<u64>, i32> {
        if let Some(values) = self.call_noncode_values(target) {
            return Ok(Some(gamepkg::diagnostic(2, 0x544b0001, &values)?));
        }
        Ok(None)
    }
    pub fn call_emit_missing(&self, target: u32) -> Result<u64, i32> {
        gamepkg::diagnostic(2, 0x544b0002, &self.call_missing_values(target))
    }
}

fn call_word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}
fn call_wide(bytes: &[u8], at: usize) -> u64 {
    u64::from_le_bytes([
        bytes[at],
        bytes[at + 1],
        bytes[at + 2],
        bytes[at + 3],
        bytes[at + 4],
        bytes[at + 5],
        bytes[at + 6],
        bytes[at + 7],
    ])
}
impl CallState {
    // Fixed 288-byte LE value record, independent of native struct layout.
    pub fn call_encode(&self) -> std::vec::Vec<u8> {
        let mut bytes = std::vec::Vec::new();
        bytes.resize(288, 0u8);
        bytes[..4].copy_from_slice(&u32::to_le_bytes(self.low));
        bytes[4..8].copy_from_slice(&u32::to_le_bytes(self.high));
        for index in 0..16 {
            let at = 8 + index * 4;
            bytes[at..at + 4].copy_from_slice(&u32::to_le_bytes(self.trace[index]));
            let at = 88 + index * 4;
            bytes[at..at + 4].copy_from_slice(&u32::to_le_bytes(self.seen[index]));
            let at = 152 + index * 8;
            bytes[at..at + 8].copy_from_slice(&u64::to_le_bytes(self.hits[index]));
        }
        bytes[72..76].copy_from_slice(&u32::to_le_bytes(self.index));
        bytes[76..84].copy_from_slice(&u64::to_le_bytes(self.total));
        bytes[84..88].copy_from_slice(&u32::to_le_bytes(self.used));
        bytes[280..288].copy_from_slice(&u64::to_le_bytes(self.services));
        bytes
    }
    pub fn call_services(&self) -> u64 {
        self.services
    }
    pub fn call_set_services(&mut self, expected: u64, next: u64) -> Result<(), i32> {
        if self.services != expected {
            return Err(-4);
        }
        self.services = next;
        Ok(())
    }
    pub fn call_decode(bytes: &[u8]) -> Result<Self, i32> {
        if bytes.len() != 288 {
            return Err(-1);
        }
        let mut value = Self::call_new(call_word(bytes, 0), call_word(bytes, 4))?;
        value.used = call_word(bytes, 84);
        if value.used > 16 {
            return Err(-1);
        }
        for index in 0..16 {
            value.trace[index] = call_word(bytes, 8 + index * 4);
            value.seen[index] = call_word(bytes, 88 + index * 4);
            value.hits[index] = call_wide(bytes, 152 + index * 8);
            if index >= value.used as usize && (value.seen[index] != 0 || value.hits[index] != 0) {
                return Err(-1);
            }
            if index < value.used as usize {
                for previous in 0..index {
                    if value.seen[previous] == value.seen[index] {
                        return Err(-1);
                    }
                }
            }
        }
        value.services = call_wide(bytes, 280);
        value.index = call_word(bytes, 72);
        value.total = call_wide(bytes, 76);
        Ok(value)
    }
}
