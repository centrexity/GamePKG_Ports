// Original port scheduling arithmetic. Display reports are values, not GPU/OS handles.
pub struct Pacing {
    mode: i32,
    lock: i32,
    reference: u64,
    period: u64,
    candidate: u64,
    candidate_n: i32,
    off: i32,
    offrun: i32,
    onrun: i32,
    offsum: u64,
    onsum: u64,
    jitter: i32,
    relearn: i32,
    rate_changed: i32,
    said: i32,
    started: i32,
    epoch: u64,
    count: u64,
}
fn pacing_gcd(mut first: u64, mut second: u64) -> u64 {
    if first < second {
        std::mem::swap(&mut first, &mut second);
    }
    while second >= 2000000 {
        let remainder = first % second;
        if remainder <= second / 8 || second - remainder <= second / 8 {
            return second;
        }
        first = second;
        second = remainder;
    }
    0
}
fn pacing_u64(bytes: &[u8], at: usize) -> u64 {
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
fn pacing_i32(bytes: &[u8], at: usize) -> i32 {
    i32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}
impl Pacing {
    // -1: no explicit setting (presenter auto may select); 0/1: forced off/on.
    pub fn pacing_new(mode: i32) -> Result<Self, i32> {
        if !(-1..=1).contains(&mode) {
            return Err(-1);
        }
        Ok(Self {
            mode,
            lock: if mode < 0 { 0 } else { mode },
            reference: 0,
            period: 0,
            candidate: 0,
            candidate_n: 0,
            off: 0,
            offrun: 0,
            onrun: 0,
            offsum: 0,
            onsum: 0,
            jitter: 0,
            relearn: 0,
            rate_changed: 0,
            said: 0,
            started: 0,
            epoch: 0,
            count: 0,
        })
    }
    pub fn pacing_auto(&mut self, on: bool) {
        if self.mode < 0 {
            self.lock = i32::from(on);
        }
    }
    pub fn pacing_locked(&self) -> bool {
        self.lock != 0
    }
    pub fn pacing_relearn(&mut self) {
        self.relearn = 1;
    }
    pub fn pacing_rate_changed(&mut self) -> bool {
        let changed = self.rate_changed != 0;
        self.rate_changed = 0;
        changed
    }
    // Flags describe the original three diagnostics: bit 0 sustained changed
    // rate; bit 1 consecutive changed rate; bit 2 first irregular stated report.
    // The caller emits bounded GamePKG diagnostics after publishing the update.
    pub fn pacing_refresh(&mut self, when_ns: u64, period_ns: u64) -> u32 {
        let mut previous = self.reference;
        let mut current = self.period;
        let mut diagnostics = 0;
        if !self.pacing_locked() || when_ns == 0 {
            return 0;
        }
        if self.relearn != 0 {
            self.relearn = 0;
            self.reference = 0;
            self.period = 0;
            self.candidate = 0;
            self.candidate_n = 0;
            self.off = 0;
            self.offrun = 0;
            self.onrun = 0;
            self.offsum = 0;
            self.onsum = 0;
            self.jitter = 0;
            previous = 0;
            current = 0;
        }
        if period_ns != 0 {
            if (2000000..=50000000).contains(&period_ns) {
                self.period = period_ns;
            }
        } else if previous != 0 && when_ns > previous && when_ns - previous <= 100000000 {
            let nominal = 1000000000u64 / 60;
            let distance = when_ns - previous;
            if current == 0 {
                if (2000000..=50000000).contains(&distance) {
                    self.period = distance;
                }
            } else {
                let gcd = pacing_gcd(current, distance);
                if gcd == 0 {
                    self.off = self.off.wrapping_add(1);
                    if self.off >= 8 && (2000000..=50000000).contains(&distance) {
                        self.period = distance;
                        self.off = 0;
                    }
                } else {
                    self.off = 0;
                    if gcd.wrapping_add(current / 8) >= current {
                        let periods = distance.wrapping_add(current / 2) / current;
                        self.period =
                            current.wrapping_mul(15).wrapping_add(distance / periods) / 16;
                        self.offrun = 0;
                        if periods >= 2
                            && distance.wrapping_add(nominal / 8) >= nominal
                            && distance <= nominal + nominal / 8
                        {
                            self.onsum = self.onsum.wrapping_add(distance);
                            self.onrun = self.onrun.wrapping_add(1);
                            if self.onrun >= 120 {
                                self.period = self.onsum / (self.onrun as u64);
                                self.rate_changed = 1;
                                self.onrun = 0;
                                self.onsum = 0;
                                diagnostics |= 1;
                            }
                        } else if periods >= 2
                            && (2..=3).contains(&(distance.wrapping_add(nominal / 8) / nominal))
                            && distance.wrapping_add(nominal / 8) % nominal <= nominal / 4
                        {
                            // Original neutral missed-frame/30fps step; keep the accumulated run.
                        } else {
                            self.onrun = 0;
                            self.onsum = 0;
                        }
                    } else if distance.wrapping_add(nominal / 8) >= nominal
                        && distance <= nominal + nominal / 8
                    {
                        self.offsum =
                            (if self.offrun != 0 { self.offsum } else { 0 }).wrapping_add(distance);
                        self.offrun = self.offrun.wrapping_add(1);
                        if self.offrun >= 3 {
                            self.period = self.offsum / (self.offrun as u64);
                            self.rate_changed = 1;
                            self.offrun = 0;
                            self.candidate = 0;
                            self.candidate_n = 0;
                            diagnostics |= 2;
                        }
                    } else {
                        self.offrun = 0;
                        if self.candidate != 0
                            && gcd.wrapping_add(self.candidate / 8) >= self.candidate
                            && gcd <= self.candidate.wrapping_add(self.candidate / 8)
                        {
                            self.candidate_n = self.candidate_n.wrapping_add(1);
                        } else {
                            self.candidate = gcd;
                            self.candidate_n = 1;
                        }
                        if self.candidate_n >= 3 {
                            self.period = self.candidate;
                            self.candidate = 0;
                            self.candidate_n = 0;
                        }
                    }
                }
            }
        }
        current = self.period;
        if period_ns != 0
            && current != 0
            && previous != 0
            && when_ns > previous
            && when_ns - previous <= 100000000
        {
            let remainder = (when_ns - previous) % current;
            if remainder > current / 8 && current - remainder > current / 8 {
                if self.said == 0 {
                    diagnostics |= 4;
                }
                self.said = self.said.wrapping_add(1);
                if self.jitter < 7200 {
                    self.jitter = self.jitter.wrapping_add(240);
                }
            } else if self.jitter != 0 {
                self.jitter = self.jitter.wrapping_sub(1);
            }
        }
        current = self.period;
        if period_ns == 0
            && current != 0
            && previous != 0
            && when_ns > previous
            && when_ns - previous <= 100000000
        {
            let periods = (when_ns - previous).wrapping_add(current / 2) / current;
            let predicted = previous.wrapping_add(periods.wrapping_mul(current));
            if periods != 0 && when_ns > predicted && when_ns - predicted < current / 4 {
                self.reference = predicted.wrapping_add((when_ns - predicted) / 8);
                return diagnostics;
            }
        }
        self.reference = when_ns;
        diagnostics
    }
    fn pacing_grid(&self, now: u64) -> Option<[u64; 2]> {
        let nominal = 1000000000u64 / 60;
        if !self.pacing_locked()
            || self.reference == 0
            || self.period == 0
            || (now > self.reference && now - self.reference > 250000000)
            || self.jitter != 0
            || self.relearn != 0
        {
            return None;
        }
        let multiple = nominal.wrapping_add(self.period / 2) / self.period;
        if multiple == 0 {
            return None;
        }
        let period = multiple.wrapping_mul(self.period);
        if period < nominal - nominal / 100 || period > nominal + nominal / 100 {
            return None;
        }
        Some([self.reference, period])
    }
    fn pacing_due(&self, count: u64) -> u64 {
        self.epoch.wrapping_add(count.wrapping_mul(1000000000) / 60)
    }
    // Return due(0/1), next raw guest-ns edge, restart lateness. No IRQ is raised
    // here: its original side effects remain in the checked timer/device caller.
    pub fn pacing_step(&mut self, now: u64) -> Result<[u64; 3], i32> {
        let mut next_state = Self::pacing_decode(&self.pacing_encode())?;
        let result = next_state.pacing_step_inner(now)?;
        *self = next_state;
        Ok(result)
    }
    fn pacing_step_inner(&mut self, now: u64) -> Result<[u64; 3], i32> {
        if self.started == 0 {
            self.started = 1;
            self.epoch = now;
            self.count = 0;
        }
        let due = self.pacing_due(self.count);
        if now < due {
            return Ok([0, due, 0]);
        }
        let mut late = 0;
        if now - due > 100000000 {
            late = now - due;
            self.epoch = now;
            self.count = 0;
        }
        self.count = self.count.wrapping_add(1);
        let mut next = self.pacing_due(self.count);
        if let Some([reference, period]) = self.pacing_grid(now) {
            let target = due.wrapping_add(period);
            next = if target >= reference {
                reference.wrapping_add(
                    ((target - reference).wrapping_add(self.period / 2) / self.period)
                        .wrapping_mul(self.period),
                )
            } else {
                reference.wrapping_sub(
                    ((reference - target).wrapping_add(self.period / 2) / self.period)
                        .wrapping_mul(self.period),
                )
            };
            if next <= now {
                // Exact count of the original repeated positive-period adds.
                // Refuse arithmetic wrap/nontermination instead of hanging a worker.
                let increments = (now - next) / period;
                let increments = increments.checked_add(1).ok_or(-1)?;
                next = next
                    .checked_add(increments.checked_mul(period).ok_or(-1)?)
                    .ok_or(-1)?;
            }
            self.epoch = next.wrapping_sub(self.count.wrapping_mul(1000000000) / 60);
        }
        Ok([1, next, late])
    }
    // Private fixed-width format only; no native structure layouts or pointers.
    pub fn pacing_encode(&self) -> std::vec::Vec<u8> {
        let mut bytes = std::vec::Vec::new();
        bytes.extend(u32::to_le_bytes(0x31435056));
        for value in [
            self.mode,
            self.lock,
            self.candidate_n,
            self.off,
            self.offrun,
            self.onrun,
            self.jitter,
            self.relearn,
            self.rate_changed,
            self.said,
            self.started,
        ] {
            bytes.extend(i32::to_le_bytes(value));
        }
        for value in [
            self.reference,
            self.period,
            self.candidate,
            self.offsum,
            self.onsum,
            self.epoch,
            self.count,
        ] {
            bytes.extend(u64::to_le_bytes(value));
        }
        bytes
    }
    pub fn pacing_decode(bytes: &[u8]) -> Result<Self, i32> {
        if bytes.len() != 104 || &bytes[..4] != &u32::to_le_bytes(0x31435056) {
            return Err(-1);
        }
        let mode = pacing_i32(bytes, 4);
        let lock = pacing_i32(bytes, 8);
        let candidate_n = pacing_i32(bytes, 12);
        let off = pacing_i32(bytes, 16);
        let offrun = pacing_i32(bytes, 20);
        let onrun = pacing_i32(bytes, 24);
        let jitter = pacing_i32(bytes, 28);
        let relearn = pacing_i32(bytes, 32);
        let rate_changed = pacing_i32(bytes, 36);
        let said = pacing_i32(bytes, 40);
        let started = pacing_i32(bytes, 44);
        if !(-1..=1).contains(&mode)
            || !(0..=1).contains(&lock)
            || (mode >= 0 && mode != lock)
            || !(0..=1).contains(&relearn)
            || !(0..=1).contains(&rate_changed)
            || !(0..=1).contains(&started)
            || candidate_n < 0
            || off < 0
            || offrun < 0
            || onrun < 0
            || jitter < 0
        {
            return Err(-1);
        }
        let reference = pacing_u64(bytes, 48);
        let period = pacing_u64(bytes, 56);
        let candidate = pacing_u64(bytes, 64);
        // Reported periods start in 2..50ms. Original smoothing can move a
        // learned period outside that input interval; d is at most 100ms and
        // a smooth starts at >=2ms, so these are conservative reachable bounds.
        if (period != 0 && !(1875000..=100000000).contains(&period))
            || (candidate != 0 && !(2000000..=100000000).contains(&candidate))
        {
            return Err(-1);
        }
        Ok(Self {
            mode,
            lock,
            reference,
            period,
            candidate,
            candidate_n,
            off,
            offrun,
            onrun,
            offsum: pacing_u64(bytes, 72),
            onsum: pacing_u64(bytes, 80),
            jitter,
            relearn,
            rate_changed,
            said,
            started,
            epoch: pacing_u64(bytes, 88),
            count: pacing_u64(bytes, 96),
        })
    }
}
