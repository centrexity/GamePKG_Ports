// Port-owned host_time conversion and one-sleeper notification semantics.
// Every actual counter read, wait and wake uses the supplied GamePKG SDK.
use crate::gamepkg;
use crate::kernel_pool;
use crate::memory;

pub fn host_time_value(counter: u64, frequency: u64) -> Result<u64, i32> {
    if counter > i64::MAX as u64 || frequency == 0 || frequency > i64::MAX as u64 {
        return Err(-1);
    }
    // Original QPC split and unsigned operation order, including u64 wrapping.
    Ok((counter / frequency)
        .wrapping_mul(1000000000)
        .wrapping_add((counter % frequency).wrapping_mul(1000000000) / frequency))
}
pub fn guest_time_value(host_ns: u64, skew_ns: u64) -> u64 {
    host_ns.wrapping_add(skew_ns)
}
// A relative ns request is rounded up into the actual host counter domain.
// This is a validated SDK request, not an emulated clock or guessed frequency.
pub fn sleep_deadline(counter: u64, frequency: u64, ns: u64) -> Result<u64, i32> {
    if counter > i64::MAX as u64 || frequency == 0 || frequency > i64::MAX as u64 || ns > 1000000000
    {
        return Err(-1);
    }
    let counts = (u128::from(ns) * u128::from(frequency)).div_ceil(1000000000);
    let counts = match u64::try_from(counts) {
        Ok(value) => value,
        Err(_) => return Err(-1),
    };
    let deadline = counter.checked_add(counts).ok_or(-1)?;
    if deadline > i64::MAX as u64 {
        return Err(-1);
    }
    Ok(deadline)
}
fn time_owner(ram: &memory::Memory) -> Result<[u64; 2], i32> {
    ram.call_snapshot()?;
    let services = ram.services_handle()?;
    kernel_pool::kernel_service_values(ram)?;
    if ram.services_handle()? != services {
        return Err(-4);
    }
    Ok([ram.call_handle(), services])
}
pub fn host_now_ns(ram: &memory::Memory) -> Result<u64, i32> {
    time_owner(ram)?;
    let sample = gamepkg::local_performance_sample()?;
    host_time_value(sample.counter, sample.frequency)
}
pub fn guest_now_ns(ram: &memory::Memory) -> Result<u64, i32> {
    // The original samples host time before loading shared skew.
    let host = host_now_ns(ram)?;
    let skew = kernel_pool::kernel_service_values(ram)?[2];
    Ok(guest_time_value(host, skew))
}
pub fn sleep_wake(ram: &memory::Memory) -> Result<u64, i32> {
    time_owner(ram)?;
    gamepkg::local_clock_wake()
}
pub struct Sleeper {
    root: u64,
    services: u64,
    task: u64,
    frequency: u64,
    observed: u64,
}
impl Sleeper {
    // Initialize in the one timer worker before enabling queue producers.
    // No native object, event, record, task or callback is allocated here.
    pub fn sleep_new(ram: &memory::Memory) -> Result<Self, i32> {
        let owner = time_owner(ram)?;
        let (task, cancelled) = gamepkg::local_task_status()?;
        if cancelled {
            return Err(-4);
        }
        let clock = gamepkg::local_clock_state()?;
        Ok(Self {
            root: owner[0],
            services: owner[1],
            task,
            frequency: clock.sample.frequency,
            observed: clock.generation,
        })
    }
    fn sleep_check(&self, ram: &memory::Memory) -> Result<(), i32> {
        if ram.call_handle() != self.root {
            return Err(-4);
        }
        if ram.services_handle()? != self.services {
            return Err(-4);
        }
        // Recheck the pinned association even on a failed values read. Closing
        // between the first root read and the KSV read must remain revocation,
        // not a new/unconfigured service (-5), and must not acknowledge a wake.
        let values = kernel_pool::kernel_service_values(ram);
        if ram.services_handle()? != self.services {
            return Err(-4);
        }
        values?;
        let (task, cancelled) = gamepkg::local_task_status()?;
        if task != self.task || cancelled {
            return Err(-4);
        }
        Ok(())
    }
    fn sleep_state(&self, ram: &memory::Memory) -> Result<gamepkg::LocalClockState, i32> {
        self.sleep_check(ram)?;
        let clock = gamepkg::local_clock_state()?;
        if clock.sample.frequency != self.frequency || clock.generation < self.observed {
            return Err(-3);
        }
        Ok(clock)
    }
    // Like the original pending flag, this observes without consuming a wake.
    pub fn sleep_pending(&self, ram: &memory::Memory) -> Result<bool, i32> {
        Ok(self.sleep_state(ram)?.generation != self.observed)
    }
    pub fn sleep_observed(&self) -> u64 {
        self.observed
    }
    // 0 = elapsed deadline; 1 = one or more coalesced wakes consumed.
    // Zero ns consumes an existing wake, without blocking. A newly read state
    // never replaces observed before waiting: that would lose a queued wake.
    pub fn sleep_ns(&mut self, ram: &memory::Memory, ns: u64) -> Result<u32, i32> {
        if ns > 1000000000 {
            return Err(-1);
        }
        let clock = self.sleep_state(ram)?;
        let deadline = sleep_deadline(clock.sample.counter, self.frequency, ns)?;
        let waited = gamepkg::local_clock_wait(deadline, self.frequency, self.observed)?;
        // Do not publish consumption if the root/task was revoked during wait.
        self.sleep_check(ram)?;
        self.observed = waited.state.generation;
        Ok(waited.reason)
    }
}
