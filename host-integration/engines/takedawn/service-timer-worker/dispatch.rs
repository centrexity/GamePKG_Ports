// Original ordered timer-owned DPC drain then timer firing. No device fallback.
use crate::callback;
use crate::drain;
use crate::firing;
use crate::gamepkg;
use crate::irql;
use crate::memory;
use crate::production_clock;
use crate::queue;
use crate::shadow;
use crate::stacks;
use crate::timers;
use crate::worker_setup;
pub struct Dispatch {
    root: u64,
    actor: u64,
    stack: [u64; 3],
    table: timers::Timers,
    map: shadow::Shadow,
    invoker: callback::Callback,
    pending: drain::Drain,
    task: irql::Task,
    profile: i32,
    failed: bool,
    closed: bool,
}
impl Dispatch {
    pub fn dispatch_new(
        worker: &mut worker_setup::Worker,
        pending_id: u64,
        control_id: u64,
        pool_id: u64,
        table_id: u64,
        map_id: u64,
        guard_us: i64,
        lag_ms: i32,
        profile: i32,
    ) -> Result<Self, i32> {
        let (ram, cpu, lease) = worker.worker_parts()?;
        let identity = lease.stack_identity(ram)?;
        if identity[0] != pool_id {
            return Err(-4);
        }
        let pending = queue::Queue::queue_borrow(ram, pending_id)?;
        let controller = irql::Controller::controller_borrow(ram, control_id)?;
        let pool = stacks::Pool::stacks_borrow(ram, pool_id)?;
        let table = timers::Timers::timers_borrow(ram, table_id)?;
        let map = shadow::Shadow::shadow_borrow(ram, map_id)?;
        let (actor, cancelled) = gamepkg::local_task_status()?;
        if cancelled || table.timer_worker_binding(ram)? != [pending_id, actor] {
            return Err(-4);
        }
        let invoker = callback::Callback::callback_new(ram, &pending, &controller, &pool)?;
        let retained =
            drain::Drain::drain_new(ram, &pending, &controller, &pool, guard_us, lag_ms, profile)?;
        // Validate all copied owners before registering this task with IRQL.
        let task = controller.controller_task(ram)?;
        if let Err(code) = invoker.callback_ready(ram, &task, cpu, lease) {
            if task.irql_abort(ram).is_err() {
                return Err(-3);
            }
            return Err(code);
        }
        Ok(Self {
            root: ram.call_handle(),
            actor,
            stack: identity,
            table,
            map,
            invoker,
            pending: retained,
            task,
            profile,
            failed: false,
            closed: false,
        })
    }
    // The checked IRQ layer can attach its typed queue before the first pass.
    // Only this worker's retained context is exposed; no callback value or
    // native IRQ object crosses this port-side composition boundary.
    pub fn dispatch_actor<'a>(
        &'a mut self,
        worker: &'a mut worker_setup::Worker,
    ) -> Result<(&'a mut memory::Memory, &'a mut irql::Task), i32> {
        if self.failed || self.closed {
            return Err(-4);
        }
        let (ram, _, lease) = worker.worker_parts()?;
        let retained_key = lease.stack_identity(ram)?;
        if ram.call_handle() != self.root
            || retained_key != self.stack
            || gamepkg::local_task_status()? != (self.actor, false)
        {
            return Err(-4);
        }
        self.task.task_values(ram)?;
        Ok((ram, &mut self.task))
    }
    // [still queued, DPCs run, timers fired, next timer due as i64 bits]. The
    // outer loop has already performed its vblank tick. Clock sampling follows
    // drain, as in kernel_timer_loop_ns; caller performs SpinWake and pacing.
    pub fn dispatch_pass(
        &mut self,
        worker: &mut worker_setup::Worker,
        vblank_next: u64,
        warp: bool,
    ) -> Result<[u64; 4], i32> {
        if self.failed || self.closed {
            return Err(-4);
        }
        let (ram, cpu, lease) = worker.worker_parts()?;
        if ram.call_handle() != self.root
            || lease.stack_identity(ram)? != self.stack
            || gamepkg::local_task_status()? != (self.actor, false)
        {
            return Err(-4);
        }
        self.invoker.callback_ready(ram, &self.task, cpu, lease)?;
        let result = self.dispatch_work(ram, cpu, lease, if warp { 0 } else { vblank_next });
        if let Err(code) = result {
            // No completed-pass claim or replay of a published dequeue/callback.
            self.failed = true;
            if self.task.irql_abort(ram).is_err() {
                return Err(-3);
            }
            return Err(code);
        }
        result
    }
    fn dispatch_work(
        &mut self,
        ram: &mut memory::Memory,
        cpu: &mut crate::registers::Registers,
        lease: &stacks::Lease,
        edge: u64,
    ) -> Result<[u64; 4], i32> {
        let drained = self
            .pending
            .drain_before(ram, &mut self.task, cpu, lease, edge)?;
        let now = production_clock::timer_now_ms(ram)?;
        let fired = firing::timer_fire_pass(
            &self.table,
            &self.map,
            &self.invoker,
            ram,
            &mut self.task,
            cpu,
            lease,
            now,
            self.profile,
        )?;
        Ok([drained[0], drained[1], fired[0], fired[1]])
    }
    pub fn dispatch_finish(&mut self, worker: &mut worker_setup::Worker) -> Result<(), i32> {
        let (ram, identity) = worker.worker_cleanup()?;
        if ram.call_handle() != self.root
            || identity != self.stack
            || gamepkg::local_task_status()?.0 != self.actor
        {
            return Err(-4);
        }
        if !self.closed {
            // Abort first, including cancellation/revocation. Do not free the
            // retained guest stack while this actor may still own a gate.
            self.task.irql_abort(ram)?;
            self.closed = true;
            self.failed = true;
        }
        worker.worker_finish()
    }
    pub fn dispatch_closed(&self) -> bool {
        self.closed
    }
}
