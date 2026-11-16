// Port-owned periodic KeTickCount writer; all clocks/tasks/events/RAM are host services.
use crate::gamepkg;
use crate::kernel_data;
use crate::kernel_pool;
use crate::memory;

pub struct Tick {
    root: u64,
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
    // On failure this owner retains any partially-created resources. Call
    // tick_close until terminal before dropping data/root or reusing the owner.
    pub fn tick_start(
        &mut self,
        ram: &memory::Memory,
        data: &kernel_data::Data,
        entry: u32,
    ) -> Result<(), i32> {
        if self.root != 0 {
            return Err(-1);
        }
        let binding = data.data_tick_binding(ram)?;
        let services = ram.services_handle()?;
        kernel_pool::kernel_service_values(ram)?;
        self.root = binding[0];
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
        let mut bytes = std::vec::Vec::new();
        for value in [
            0x314b4354u64,
            self.root,
            binding[1],
            binding[2],
            self.stop,
            0,
            services,
            0,
        ] {
            bytes.extend(u64::to_le_bytes(value));
        }
        self.record = gamepkg::local_record_create(&bytes)?;
        let request = [
            u32::to_le_bytes(entry).as_slice(),
            &u64::to_le_bytes(self.record),
            &u32::to_le_bytes(1),
        ]
        .concat();
        if gamepkg::local_call(10, &request, &mut reply)? != 8 {
            return Err(-3);
        }
        self.task = u64::from_le_bytes(reply);
        let (version, _) = gamepkg::local_record_snapshot(self.record, 64)?;
        bytes[40..48].copy_from_slice(&u64::to_le_bytes(self.task));
        gamepkg::local_record_compare_exchange(self.record, version, &bytes)?;
        let mut resumed = [0u8; 4];
        if gamepkg::local_call(13, &u64::to_le_bytes(self.task), &mut resumed)? != 4
            || resumed != u32::to_le_bytes(1)
        {
            return Err(-3);
        }
        self.started = true;
        Ok(())
    }
    // Stop is a request, not a barrier. A write already accepted may complete.
    pub fn tick_stop(&self, ram: &memory::Memory) -> Result<(), i32> {
        if self.root == 0 {
            return Ok(());
        }
        if ram.call_handle() != self.root {
            return Err(-4);
        }
        if self.stop != 0 {
            tick_signal(self.stop)?;
        }
        Ok(())
    }
    pub fn tick_finished(&self, ram: &memory::Memory) -> Result<bool, i32> {
        if self.root == 0 {
            return Ok(true);
        }
        if ram.call_handle() != self.root {
            return Err(-4);
        }
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
            if !self.started {
                gamepkg::local_call(12, &u64::to_le_bytes(self.task), &mut [])?;
            }
            let mut reply = [0u8; 8];
            if gamepkg::local_call(11, &u64::to_le_bytes(self.task), &mut reply)? != 8 {
                return Err(-3);
            }
            let result = u64::from_le_bytes(reply);
            self.result = if result == 0 {
                0
            } else if result <= 6 {
                -(result as i32)
            } else {
                -3
            };
            self.task = 0;
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
        let settings = kernel_pool::kernel_service_values(&ram)?;
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
