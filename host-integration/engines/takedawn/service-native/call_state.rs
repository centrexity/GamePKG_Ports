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
}
impl CallState {
    pub fn call_new(low: u32, high: u32) -> Result<Self, i32> {
        if high != 0 && low > high {
            return Err(-1);
        }
        Ok(Self {
            low,
            high,
            trace: [0; 16],
            index: 0,
            total: 0,
            seen: [0; 16],
            hits: [0; 16],
            used: 0,
        })
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
