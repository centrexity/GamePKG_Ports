// Port-owned periodic KeTickCount writer; all clocks/tasks/events/RAM are host services.
use crate::gamepkg;
use crate::kernel_data;
use crate::kernel_pool;
use crate::memory;

pub struct Tick {
    root: u64,
    creator: u64,
    binding: [u64; 3],
    record: u64,
    stop: u64,
    task: u64,
    started: bool,
    result: i32,
}
fn tick_word(bytes: &[u8], at: usize) -> Result<u64, i32> {
    let value = bytes.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        value[0], value[1], value[2], value[3], value[4], value[5], value[6], value[7],
    ]))
}
pub fn tick_value(uptime_ms: u64, skew_ns: u64) -> u32 {
    (uptime_ms as u32).wrapping_add((skew_ns / 1000000) as u32)
}
fn tick_decode(bytes: &[u8]) -> Result<[u64; 6], i32> {
    if bytes.len() != 64 || tick_word(bytes, 0)? != 0x314b4354 || tick_word(bytes, 56)? != 0 {
        return Err(-1);
    }
    let values = [
        tick_word(bytes, 8)?,
        tick_word(bytes, 16)?,
        tick_word(bytes, 24)?,
        tick_word(bytes, 32)?,
        tick_word(bytes, 40)?,
        tick_word(bytes, 48)?,
    ];
    if values[0] == 0
        || values[1] == 0
        || values[2] & 3 != 0
        || values[2].checked_add(4).is_none()
        || values[3] == 0
        || values[4] == 0
        || values[5] == 0
    {
        return Err(-1);
    }
    Ok(values)
}
fn tick_signal(stop: u64) -> Result<(), i32> {
    let request = [u64::to_le_bytes(stop).as_slice(), &u32::to_le_bytes(1)].concat();
    gamepkg::local_call(22, &request, &mut [])?;
    Ok(())
}
fn tick_wait(stop: u64, milliseconds: u32) -> Result<bool, i32> {
    let request = [
        u64::to_le_bytes(stop).as_slice(),
        &u32::to_le_bytes(milliseconds),
    ]
    .concat();
    match gamepkg::local_call(23, &request, &mut []) {
        Ok(0) => Ok(true),
        Err(-2) => Ok(false),
        Ok(_) => Err(-3),
        Err(code) => Err(code),
    }
}
impl Tick {
    pub fn tick_new() -> Self {
        Self {
            root: 0,
            creator: 0,
            binding: [0; 3],
            record: 0,
            stop: 0,
            task: 0,
            started: false,
            result: 0,
        }
    }
    // Unlike task completion, this says all driver-owned resources are gone.
    pub fn tick_closed(&self) -> bool {
        self.root == 0
    }
    fn tick_owner(&self, ram: &memory::Memory) -> Result<(), i32> {
        if self.root != 0
            && (ram.call_handle() != self.root || gamepkg::local_task_status()?.0 != self.creator)
        {
            return Err(-4);
        }
        Ok(())
    }
    fn tick_metadata(&self, task: u64) -> std::vec::Vec<u8> {
        let mut bytes = std::vec::Vec::new();
        for value in [
            0x314b4354u64,
            self.root,
            self.binding[0],
            self.binding[1],
            self.stop,
            task,
            self.binding[2],
            0,
        ] {
            bytes.extend(u64::to_le_bytes(value));
        }
        bytes
    }
    fn tick_validate(&self, ram: &memory::Memory) -> Result<(), i32> {
        self.tick_owner(ram)?;
        if self.root == 0 || self.record == 0 || self.stop == 0 {
            return Err(-1);
        }
        if gamepkg::local_task_status()?.1 || ram.services_handle()? != self.binding[2] {
            return Err(-4);
        }
        kernel_pool::kernel_service_values(ram)?;
        let (_, bytes) = gamepkg::local_record_snapshot(self.record, 64)?;
        if bytes != self.tick_metadata(self.task) {
            return Err(-4);
        }
        Ok(())
    }
    // Allocate immutable startup metadata and stop event, but no worker. A
    // coordinator can now use Startup::startup_spawn to establish its lifetime
    // obligation before assigning and resuming the actual SDK job.
    pub fn tick_allocate(
        &mut self,
        ram: &memory::Memory,
        data: &kernel_data::Data,
    ) -> Result<(), i32> {
        if self.root != 0 {
            return Err(-1);
        }
        let binding = data.data_tick_binding(ram)?;
        let services = ram.services_handle()?;
        kernel_pool::kernel_service_values(ram)?;
        let (creator, cancelled) = gamepkg::local_task_status()?;
        if cancelled {
            return Err(-4);
        }
        self.root = binding[0];
        self.creator = creator;
        self.binding = [binding[1], binding[2], services];
        self.result = 0;
        let mut reply = [0u8; 8];
        if gamepkg::local_call(
            20,
            &[u32::to_le_bytes(1), u32::to_le_bytes(0)].concat(),
            &mut reply,
        )? != 8
        {
            return Err(-3);
        }
        self.stop = u64::from_le_bytes(reply);
        self.record = gamepkg::local_record_create(&self.tick_metadata(0))?;
        Ok(())
    }
    // Opaque argument only. Requires fully allocated, unassigned live state.
    pub fn tick_argument(&self, ram: &memory::Memory) -> Result<u64, i32> {
        self.tick_validate(ram)?;
        if self.task != 0 {
            return Err(-1);
        }
        Ok(self.record)
    }
    // The caller must pass its actual suspended SDK spawn. Retain that cleanup
    // obligation before fallible metadata/liveness checks; failed assignment
    // must still cancel/join the job before releasing the argument record.
    pub fn tick_assign(&mut self, ram: &memory::Memory, task: u64) -> Result<(), i32> {
        self.tick_owner(ram)?;
        if self.root == 0
            || self.record == 0
            || self.stop == 0
            || self.task != 0
            || task == 0
            || task == self.creator
        {
            return Err(-1);
        }
        self.task = task;
        if gamepkg::local_task_finished(task)? {
            return Err(-4);
        }
        if gamepkg::local_task_status()?.1 || ram.services_handle()? != self.binding[2] {
            return Err(-4);
        }
        kernel_pool::kernel_service_values(ram)?;
        let (version, bytes) = gamepkg::local_record_snapshot(self.record, 64)?;
        if bytes != self.tick_metadata(0) {
            return Err(-4);
        }
        gamepkg::local_record_compare_exchange(self.record, version, &self.tick_metadata(task))?;
        Ok(())
    }
    pub fn tick_resume(&mut self, ram: &memory::Memory) -> Result<(), i32> {
        self.tick_validate(ram)?;
        if self.task == 0 || self.started {
            return Err(-1);
        }
        let mut resumed = [0u8; 4];
        if gamepkg::local_call(13, &u64::to_le_bytes(self.task), &mut resumed)? != 4
            || resumed != u32::to_le_bytes(1)
        {
            return Err(-3);
        }
        self.started = true;
        Ok(())
    }
    pub fn tick_task(&self, ram: &memory::Memory) -> Result<u64, i32> {
        self.tick_owner(ram)?;
        Ok(self.task)
    }
    // Compatibility composition for a sole owner. Failure retains all partial
    // resources. Shared coordinators use allocate/argument/spawn/assign/resume.
    pub fn tick_start(
        &mut self,
        ram: &memory::Memory,
        data: &kernel_data::Data,
        entry: u32,
    ) -> Result<(), i32> {
        self.tick_allocate(ram, data)?;
        let request = [
            u32::to_le_bytes(entry).as_slice(),
            &u64::to_le_bytes(self.tick_argument(ram)?),
            &u32::to_le_bytes(1),
        ]
        .concat();
        let mut reply = [0u8; 8];
        if gamepkg::local_call(10, &request, &mut reply)? != 8 {
            return Err(-3);
        }
        self.tick_assign(ram, u64::from_le_bytes(reply))?;
        self.tick_resume(ram)
    }
    // A successful join is shared evidence, not another native object or lease.
    pub fn tick_accept_join(
        &mut self,
        ram: &memory::Memory,
        joined: &gamepkg::LocalTaskJoin,
    ) -> Result<(), i32> {
        self.tick_owner(ram)?;
        if self.task == 0 || joined.task_id() != self.task {
            return Err(-4);
        }
        let result = joined.result();
        self.result = if result == 0 {
            0
        } else if result <= 6 {
            -(result as i32)
        } else {
            -3
        };
        self.task = 0;
        Ok(())
    }
    pub fn tick_join_task(&mut self, ram: &memory::Memory) -> Result<gamepkg::LocalTaskJoin, i32> {
        self.tick_stop(ram)?;
        if self.task == 0 {
            return Err(-1);
        }
        if !self.started {
            gamepkg::local_call(12, &u64::to_le_bytes(self.task), &mut [])?;
        }
        let joined = gamepkg::local_task_join(self.task)?;
        self.tick_accept_join(ram, &joined)?;
        Ok(joined)
    }
    // Stop is a request, not a barrier. A write already accepted may complete.
    pub fn tick_stop(&self, ram: &memory::Memory) -> Result<(), i32> {
        if self.root == 0 {
            return Ok(());
        }
        self.tick_owner(ram)?;
        if self.stop != 0 {
            tick_signal(self.stop)?;
        }
        Ok(())
    }
    pub fn tick_finished(&self, ram: &memory::Memory) -> Result<bool, i32> {
        if self.root == 0 {
            return Ok(true);
        }
        self.tick_owner(ram)?;
        if self.task == 0 {
            return Ok(true);
        }
        gamepkg::local_task_finished(self.task)
    }
    // Nonblocking stop/join/cleanup. Busy retains all live resources; retry
    // outside the window owner. After successful join no worker write remains.
    // A terminal worker error is returned only after its resources are released.
    pub fn tick_close(&mut self, ram: &memory::Memory) -> Result<(), i32> {
        self.tick_stop(ram)?;
        if self.task != 0 {
            self.tick_join_task(ram)?;
        }
        if self.record != 0 {
            gamepkg::local_record_release(self.record)?;
            self.record = 0;
        }
        if self.stop != 0 {
            gamepkg::local_call(25, &u64::to_le_bytes(self.stop), &mut [])?;
            self.stop = 0;
        }
        self.root = 0;
        self.started = false;
        let result = self.result;
        self.result = 0;
        if result == 0 {
            Ok(())
        } else {
            Err(result)
        }
    }
}
fn tick_run(record: u64) -> Result<(), i32> {
    let (_, bytes) = gamepkg::local_record_snapshot(record, 64)?;
    let values = tick_decode(&bytes)?;
    let ram = memory::Memory::with_call_handle(values[0])?;
    let (task, cancelled) = gamepkg::local_task_status()?;
    if task != values[4] || cancelled {
        return Err(-4);
    }
    loop {
        if tick_wait(values[3], 0)? {
            return Ok(());
        }
        let (current, cancelled) = gamepkg::local_task_status()?;
        if current != task || cancelled {
            return Err(-4);
        }
        let (_, current) = gamepkg::local_record_snapshot(record, 64)?;
        if current != bytes || ram.services_handle()? != values[5] {
            return Err(-4);
        }
        let sample = gamepkg::local_performance_sample()?;
        let settings = kernel_pool::kernel_service_values(&ram);
        // A bound configuration revoked/replaced during the fetch is Denied,
        // even if the fetch saw a temporarily unconfigured root (Unsupported).
        if ram.services_handle()? != values[5] {
            return Err(-4);
        }
        let settings = settings?;
        if tick_wait(values[3], 0)? {
            return Ok(());
        }
        let tick = tick_value(sample.uptime_ms, settings[2]);
        let request = [
            u64::to_le_bytes(values[1]).as_slice(),
            &u64::to_le_bytes(values[2]),
            &u32::to_le_bytes(tick),
        ]
        .concat();
        gamepkg::local_call(6, &request, &mut [])?;
        if tick_wait(values[3], 1)? {
            return Ok(());
        }
    }
}
// The bootstrap supplies this exact reviewed integer entry; no game callback.
pub fn tick_worker(record: u64) -> u64 {
    match tick_run(record) {
        Ok(()) => 0,
        Err(code) => code.unsigned_abs() as u64,
    }
}
