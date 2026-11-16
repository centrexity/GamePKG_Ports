// Original timer-owned pending-DPC drain, through checked port primitives.
use crate::callback;
use crate::dpc_clock;
use crate::dpc_observer;
use crate::gamepkg;
use crate::irql;
use crate::memory;
use crate::queue;
use crate::registers;
use crate::stacks;
pub struct Drain {
    invoker: callback::Callback,
    pending: u64,
    guard: u64,
    lag: i32,
    last_ns: u64,
    profile: i32,
}
pub fn drain_guard_ns(microseconds: i64) -> u64 {
    if microseconds <= 0 {
        0
    } else if microseconds >= 4000 {
        4000000
    } else {
        microseconds as u64 * 1000
    }
}
fn drain_word(bytes: &[u8], at: usize) -> Result<u64, i32> {
    let b = bytes.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}
impl Drain {
    // Construct/retain once on the assigned actual timer, not once per pass.
    // Defaults from the original are guard_us=1200, lag_ms=0.
    pub fn drain_new(
        ram: &memory::Memory,
        pending: &queue::Queue,
        controller: &irql::Controller,
        pool: &stacks::Pool,
        guard_us: i64,
        lag_ms: i32,
        profile: i32,
    ) -> Result<Self, i32> {
        let invoker = callback::Callback::callback_new(ram, pending, controller, pool)?;
        let bytes = pending.queue_state(ram)?;
        if gamepkg::local_task_status()? != (drain_word(&bytes, 32)?, false) {
            return Err(-4);
        }
        Ok(Self {
            invoker,
            pending: pending.queue_id(),
            guard: drain_guard_ns(guard_us),
            lag: lag_ms,
            last_ns: 0,
            profile,
        })
    }
    pub fn drain_last_host_ns(&self) -> u64 {
        self.last_ns
    }
    // [still queued, ran as original wrapping 32-bit counter]. No new execution
    // cap is imposed; cancellation and session limits remain host-controlled.
    pub fn drain_before(
        &mut self,
        ram: &mut memory::Memory,
        task: &mut irql::Task,
        cpu: &mut registers::Registers,
        lease: &stacks::Lease,
        due: u64,
    ) -> Result<[u64; 2], i32> {
        self.invoker.callback_ready(ram, task, cpu, lease)?;
        let result = self.drain_loop(ram, task, cpu, lease, due);
        if let Err(code) = result {
            // No IRQ safe point/acknowledgement on a failed dequeue/callback.
            if task.irql_abort(ram).is_err() {
                return Err(-3);
            }
            return Err(code);
        }
        result
    }
    fn drain_loop(
        &mut self,
        ram: &mut memory::Memory,
        task: &mut irql::Task,
        cpu: &mut registers::Registers,
        lease: &stacks::Lease,
        due: u64,
    ) -> Result<[u64; 2], i32> {
        let pending = queue::Queue::queue_borrow(ram, self.pending)?;
        if self.lag > 0 {
            let now = dpc_clock::dpc_host_now(ram)?;
            if now.wrapping_sub(self.last_ns) < self.lag as u64 * 1000000 {
                return Ok([u64::from(pending.queue_nonempty(ram)?), 0]);
            }
            self.last_ns = now;
        }
        let guard = if due == 0 { 0 } else { self.guard };
        let mut ran = 0u32;
        loop {
            if guard != 0 && pending.queue_nonempty(ram)? {
                let now = dpc_clock::dpc_guest_now(ram)?;
                if now < due && now.wrapping_add(guard) >= due {
                    return Ok([1, u64::from(ran)]);
                }
            }
            match task.irql_gate_enter(ram, 0) {
                Ok(()) => {}
                Err(-2) => {
                    dpc_observer::dpc_drain_busy(task, ram, cpu)?;
                    return Ok([u64::from(pending.queue_nonempty(ram)?), u64::from(ran)]);
                }
                Err(code) => return Err(code),
            }
            let item = self.drain_one(&pending, ram, task, cpu, lease)?;
            if let Some(executed) = item {
                if executed {
                    ran = ran.wrapping_add(1);
                }
                task.irql_gate_leave(ram, cpu)?;
            } else {
                task.irql_gate_leave(ram, cpu)?;
                return Ok([0, u64::from(ran)]);
            }
        }
    }
    fn drain_one(
        &self,
        pending: &queue::Queue,
        ram: &mut memory::Memory,
        task: &mut irql::Task,
        cpu: &mut registers::Registers,
        lease: &stacks::Lease,
    ) -> Result<Option<bool>, i32> {
        let mut observation = match pending.queue_take_begin(ram)? {
            Some(value) => value,
            None => return Ok(None),
        };
        let observed = self.drain_publish(&mut observation, ram, task, cpu);
        let closed = observation.take_abort();
        if closed.is_err() || !observation.take_closed() {
            return Err(-3);
        }
        let item = observed?;
        if item[0] == 0 {
            return Ok(Some(false));
        }
        if item[3] != 0 {
            dpc_observer::dpc_drain_latency(task, ram, cpu, item[3])?;
        }
        let ran = self.invoker.callback_run(
            ram,
            task,
            cpu,
            lease,
            item[0] as u32,
            item[1] as u32,
            item[2] as u32,
            self.profile,
        )?;
        Ok(Some(ran))
    }
    fn drain_publish(
        &self,
        observation: &mut queue::Take,
        ram: &mut memory::Memory,
        task: &mut irql::Task,
        cpu: &mut registers::Registers,
    ) -> Result<[u64; 4], i32> {
        let depth = observation.take_depth(ram)?;
        // This observation is still locked and the old Inserted byte is intact.
        dpc_observer::dpc_drain_depth(task, ram, cpu, depth)?;
        observation.take_publish(ram)
    }
}
