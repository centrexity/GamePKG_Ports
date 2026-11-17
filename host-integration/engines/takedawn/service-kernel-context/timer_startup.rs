// Checked timer-job startup/lifetime coordinator. No task body or device fallback.
use crate::gamepkg;
use crate::kernel_startup;
use crate::memory;
use crate::queue;
use crate::schedule;
use crate::timers;
pub struct Launch {
    root: u64,
    creator: u64,
    lifetime: [u64; 2],
    owners: [u64; 3],
    task: u64,
    queued: bool,
    table_attempted: bool,
    table_bound: bool,
    scheduled: bool,
    resumed: bool,
    joined: bool,
    queue_witnessed: bool,
    schedule_witnessed: bool,
    startup_witnessed: bool,
    closed: bool,
}
impl Launch {
    pub fn launch_new() -> Self {
        Self {
            root: 0,
            creator: 0,
            lifetime: [0; 2],
            owners: [0; 3],
            task: 0,
            queued: false,
            table_attempted: false,
            table_bound: false,
            scheduled: false,
            resumed: false,
            joined: false,
            queue_witnessed: false,
            schedule_witnessed: false,
            startup_witnessed: false,
            closed: false,
        }
    }
    fn launch_owner(&self, ram: &memory::Memory) -> Result<(), i32> {
        if self.root != ram.call_handle() || gamepkg::local_task_status()?.0 != self.creator {
            return Err(-4);
        }
        Ok(())
    }
    fn launch_bindings(
        &self,
        owner: &kernel_startup::Startup,
        ram: &memory::Memory,
        pending: &queue::Queue,
        table: &timers::Timers,
        display_pacing: &schedule::Schedule,
    ) -> Result<(), i32> {
        self.launch_owner(ram)?;
        if owner.startup_lifetime(ram)? != self.lifetime
            || [
                pending.queue_id(),
                table.timers_id(),
                display_pacing.schedule_id(),
            ] != self.owners
        {
            return Err(-4);
        }
        Ok(())
    }
    // Preflight every owner before creating a job. On later error this value
    // retains its exact suspended job; callers keep argument/owners until join.
    pub fn launch_prepare(
        &mut self,
        owner: &mut kernel_startup::Startup,
        ram: &memory::Memory,
        pending: &mut queue::Queue,
        table: &mut timers::Timers,
        display_pacing: &mut schedule::Schedule,
        entry: u32,
        argument: u64,
    ) -> Result<(), i32> {
        if self.root != 0 || self.closed {
            return Err(-1);
        }
        let ids = owner.startup_ids(ram)?;
        pending.queue_start_ready(ram)?;
        table.timers_start_ready(ram, pending)?;
        display_pacing.schedule_start_ready(ram)?;
        gamepkg::local_record_snapshot(argument, 4096)?;
        self.root = ram.call_handle();
        self.creator = gamepkg::local_task_status()?.0;
        self.lifetime = [ids[0], ids[3]];
        self.owners = [
            pending.queue_id(),
            table.timers_id(),
            display_pacing.schedule_id(),
        ];
        self.task = owner.startup_spawn(ram, entry, argument)?;
        pending.queue_assign(ram, self.task)?;
        self.queued = true;
        // Timers retain a private worker obligation before their fallible lock
        // and publication. Record the attempt even if no table row was published.
        self.table_attempted = true;
        table.timers_start(ram, pending)?;
        self.table_bound = true;
        display_pacing.schedule_assign(ram, self.task)?;
        self.scheduled = true;
        Ok(())
    }
    // The reviewed caller prepares its argument and configuration, then resumes.
    // The actual worker must establish its sleeper/IRQ/handlers and signal ready
    // before asynchronous producers are enabled. This helper does not fake that.
    pub fn launch_resume(
        &mut self,
        owner: &kernel_startup::Startup,
        ram: &memory::Memory,
        pending: &queue::Queue,
        table: &timers::Timers,
        display_pacing: &schedule::Schedule,
    ) -> Result<(), i32> {
        self.launch_bindings(owner, ram, pending, table, display_pacing)?;
        owner.startup_ids(ram)?;
        if self.task == 0
            || !self.queued
            || !self.table_bound
            || !self.scheduled
            || self.resumed
            || self.joined
            || self.closed
        {
            return Err(-1);
        }
        if pending.queue_worker_binding(ram)? != [self.owners[0], self.task]
            || table.timer_worker_binding(ram)? != [self.owners[0], self.task]
            || display_pacing.schedule_worker(ram)? != self.task
        {
            return Err(-4);
        }
        let mut reply = [0u8; 4];
        if gamepkg::local_call(13, &u64::to_le_bytes(self.task), &mut reply)? != 4
            || reply != u32::to_le_bytes(1)
        {
            return Err(-3);
        }
        self.resumed = true;
        Ok(())
    }
    pub fn launch_task(&self, ram: &memory::Memory) -> Result<u64, i32> {
        self.launch_owner(ram)?;
        Ok(self.task)
    }
    // queue assigned / table attempted / table bound / schedule assigned /
    // resumed / actual joined / queue witnessed / schedule witnessed / startup witnessed.
    pub fn launch_progress(&self) -> [bool; 9] {
        [
            self.queued,
            self.table_attempted,
            self.table_bound,
            self.scheduled,
            self.resumed,
            self.joined,
            self.queue_witnessed,
            self.schedule_witnessed,
            self.startup_witnessed,
        ]
    }
    pub fn launch_join_task(
        &mut self,
        ram: &memory::Memory,
    ) -> Result<gamepkg::LocalTaskJoin, i32> {
        self.launch_owner(ram)?;
        if self.task == 0 || self.joined || self.closed {
            return Err(-1);
        }
        if !self.resumed {
            gamepkg::local_call(12, &u64::to_le_bytes(self.task), &mut [])?;
        }
        let joined = gamepkg::local_task_join(self.task)?;
        self.launch_accept_join(ram, &joined)?;
        Ok(joined)
    }
    pub fn launch_accept_join(
        &mut self,
        ram: &memory::Memory,
        joined: &gamepkg::LocalTaskJoin,
    ) -> Result<(), i32> {
        self.launch_owner(ram)?;
        if self.task == 0 || joined.task_id() != self.task || self.joined || self.closed {
            return Err(-4);
        }
        self.joined = true;
        Ok(())
    }
    // Retryable forwarding of one real proof, not a second join. Keep the proof
    // on an error; successfully forwarded prefixes are remembered individually.
    pub fn launch_share_join(
        &mut self,
        owner: &mut kernel_startup::Startup,
        ram: &memory::Memory,
        pending: &mut queue::Queue,
        table: &timers::Timers,
        display_pacing: &mut schedule::Schedule,
        joined: &gamepkg::LocalTaskJoin,
    ) -> Result<(), i32> {
        self.launch_bindings(owner, ram, pending, table, display_pacing)?;
        if !self.joined || joined.task_id() != self.task || self.task == 0 || self.closed {
            return Err(-4);
        }
        if self.queued && !self.queue_witnessed {
            match pending.queue_join_witness(ram) {
                Ok(proof) => proof.joined_for(ram, self.owners[0], self.task)?,
                Err(-2) => pending.queue_accept_join(ram, joined)?,
                Err(code) => return Err(code),
            }
            self.queue_witnessed = true;
        }
        if self.scheduled && !self.schedule_witnessed {
            match display_pacing.schedule_joined_task(ram) {
                Ok(task) => {
                    if task != self.task {
                        return Err(-4);
                    }
                }
                Err(-2) => display_pacing.schedule_accept_join(ram, joined)?,
                Err(code) => return Err(code),
            }
            self.schedule_witnessed = true;
        }
        if !self.startup_witnessed {
            if owner.startup_has_task(ram, self.task)? {
                owner.startup_accept_join(ram, joined)?;
            }
            self.startup_witnessed = true;
        }
        Ok(())
    }
    // No argument or subsystem record is released here: those remain caller
    // owned. Closed means the job is joined and every applicable owner witnessed.
    pub fn launch_close(&mut self, ram: &memory::Memory) -> Result<(), i32> {
        self.launch_owner(ram)?;
        if self.closed {
            return Ok(());
        }
        if self.task != 0
            && (!self.joined
                || !self.startup_witnessed
                || self.queued && !self.queue_witnessed
                || self.scheduled && !self.schedule_witnessed)
        {
            return Err(-2);
        }
        self.closed = true;
        Ok(())
    }
    pub fn launch_closed(&self) -> bool {
        self.closed
    }
}
