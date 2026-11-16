// Checked port-side DpcQueued/release policy. Never executes a deferred routine.
use crate::gamepkg;
use crate::gate;
use crate::irql;
use crate::memory;
use crate::queue;
use crate::wait_service;

pub struct Notifier {
    root: u64,
    services: u64,
    queue: u64,
    controller: u64,
    gate: u64,
    consumer: u64,
}
fn notify_word(bytes: &[u8], at: usize) -> Result<u64, i32> {
    let b = bytes.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}
impl Notifier {
    // Publish the assigned suspended timer first. Register its Sleeper before
    // enabling producers. This value owns neither the queue nor the worker.
    pub fn notify_new(
        ram: &memory::Memory,
        pending: &queue::Queue,
        controller: &irql::Controller,
    ) -> Result<Self, i32> {
        let bytes = pending.queue_state(ram)?;
        controller.controller_counts(ram)?;
        if notify_word(&bytes, 40)? != 1 {
            return Err(-5);
        }
        let consumer = notify_word(&bytes, 32)?;
        if gamepkg::local_task_finished(consumer)? {
            return Err(-4);
        }
        Ok(Self {
            root: ram.call_handle(),
            services: ram.services_handle()?,
            queue: pending.queue_id(),
            controller: controller.controller_id(),
            gate: controller.controller_gate(),
            consumer,
        })
    }
    fn notify_live(
        &self,
        ram: &memory::Memory,
        task: &irql::Task,
    ) -> Result<(queue::Queue, [u64; 6], u64), i32> {
        if ram.call_handle() != self.root
            || ram.services_handle()? != self.services
            || task.task_key()[0] != self.controller
        {
            return Err(-4);
        }
        let values = task.task_values(ram)?;
        let controller = irql::Controller::controller_borrow(ram, self.controller)?;
        if controller.controller_gate() != self.gate {
            return Err(-4);
        }
        let pending = queue::Queue::queue_borrow(ram, self.queue)?;
        let bytes = pending.queue_state(ram)?;
        if notify_word(&bytes, 40)? != 1 || notify_word(&bytes, 32)? != self.consumer {
            return Err(-4);
        }
        if gamepkg::local_task_finished(self.consumer)? {
            return Err(-4);
        }
        let state = gate::Gate::gate_borrow(ram, self.gate)?.gate_state(ram)?;
        // Another task's hold is not this thread's original t_gate_depth.
        let depth = if state[0] == task.task_key()[1] {
            state[1]
        } else {
            0
        };
        Ok((pending, values, depth))
    }
    // The original queued hook does not test pending, ISR or current IRQL.
    // In particular an ungated ISR must notify now: it has no gate release.
    pub fn notify_queued(
        &self,
        ram: &memory::Memory,
        task: &irql::Task,
        prompt: bool,
    ) -> Result<bool, i32> {
        let (_, _, depth) = self.notify_live(ram, task)?;
        if depth != 0 || !prompt {
            return Ok(false);
        }
        wait_service::sleep_wake(ram)?;
        Ok(true)
    }
    // Called after original safe-point/raw-unlock/new-PCR publication ordering.
    // The assigned timer's queue_pending is false, avoiding self release kicks.
    pub fn notify_release(
        &self,
        ram: &memory::Memory,
        task: &irql::Task,
        prompt: bool,
    ) -> Result<bool, i32> {
        let (pending, values, depth) = self.notify_live(ram, task)?;
        if depth != 0 || values[4] != 0 || values[0] >= 2 {
            return Ok(false);
        }
        if !pending.queue_pending(ram)? || !prompt {
            return Ok(false);
        }
        wait_service::sleep_wake(ram)?;
        Ok(true)
    }
    // Returns [inserted,full,kicked]. A wake failure after publication is a
    // partial operation: stop/report it, never retry insertion as if unqueued.
    pub fn notify_insert(
        &self,
        ram: &mut memory::Memory,
        task: &irql::Task,
        prompt: bool,
        dpc: u32,
        first: u32,
        second: u32,
    ) -> Result<[u64; 3], i32> {
        let (pending, _, _) = self.notify_live(ram, task)?;
        let result = pending.queue_insert(ram, dpc, first, second)?;
        let kicked = result[0] != 0 && self.notify_queued(ram, task, prompt)?;
        Ok([result[0], result[1], u64::from(kicked)])
    }
}
