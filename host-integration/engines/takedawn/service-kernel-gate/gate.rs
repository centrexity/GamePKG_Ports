// Port-owned single-CPU gate. Actual recursive ownership is enforced by SDK26.
use crate::gamepkg;
use crate::kernel_pool;
use crate::memory;
pub struct Gate {
    root: u64,
    services: u64,
    record: u64,
    mutex: u64,
    creator: u64,
    owned: bool,
}
fn gate_word(bytes: &[u8], at: usize) -> Result<u64, i32> {
    let b = bytes.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}
fn gate_put(bytes: &mut [u8], at: usize, value: u64) -> Result<(), i32> {
    if at.checked_add(8).ok_or(-1)? > bytes.len() {
        return Err(-1);
    }
    let value = u64::to_le_bytes(value);
    for lane in 0..8usize {
        bytes[at + lane] = value[lane];
    }
    Ok(())
}
fn gate_unlocked(mutex: u64) -> Result<(), i32> {
    if gamepkg::local_call(24, &u64::to_le_bytes(mutex), &mut [])? != 0 {
        return Err(-3);
    }
    Ok(())
}
impl Gate {
    pub fn gate_new(ram: &memory::Memory) -> Result<Self, i32> {
        let (creator, cancelled) = gamepkg::local_task_status()?;
        if cancelled {
            return Err(-4);
        }
        let services = ram.services_handle()?;
        if services == 0 {
            return Err(-4);
        }
        kernel_pool::kernel_service_values(ram)?;
        let mut reply = [0u8; 8];
        if gamepkg::local_call(21, &[], &mut reply)? != 8 {
            return Err(-3);
        }
        let mutex = u64::from_le_bytes(reply);
        let bytes = [
            u64::to_le_bytes(0x31544147),
            u64::to_le_bytes(ram.call_handle()),
            u64::to_le_bytes(services),
            u64::to_le_bytes(mutex),
            u64::to_le_bytes(0),
            u64::to_le_bytes(0),
            u64::to_le_bytes(0),
            u64::to_le_bytes(0),
        ]
        .concat();
        let record = match gamepkg::local_record_create(&bytes) {
            Ok(id) => id,
            Err(code) => {
                let _ = gamepkg::local_call(25, &u64::to_le_bytes(mutex), &mut []);
                return Err(code);
            }
        };
        let created = Self {
            root: ram.call_handle(),
            services,
            record,
            mutex,
            creator,
            owned: true,
        };
        if let Err(code) = created.gate_live(ram) {
            let _ = gamepkg::local_record_release(record);
            let _ = gamepkg::local_call(25, &u64::to_le_bytes(mutex), &mut []);
            return Err(code);
        }
        Ok(created)
    }
    pub fn gate_id(&self) -> u64 {
        self.record
    }
    pub fn gate_closed(&self) -> bool {
        self.record == 0 && self.mutex == 0
    }
    fn gate_live(&self, ram: &memory::Memory) -> Result<(), i32> {
        if ram.call_handle() != self.root
            || ram.services_handle()? != self.services
            || self.services == 0
        {
            return Err(-4);
        }
        let result = kernel_pool::kernel_service_values(ram);
        if ram.services_handle()? != self.services {
            return Err(-4);
        }
        result?;
        if gamepkg::local_task_status()?.1 {
            return Err(-4);
        }
        Ok(())
    }
    fn gate_raw(&self, ram: &memory::Memory) -> Result<(u64, std::vec::Vec<u8>), i32> {
        if self.record == 0 || self.mutex == 0 || ram.call_handle() != self.root {
            return Err(-4);
        }
        let (version, bytes) = gamepkg::local_record_snapshot(self.record, 64)?;
        if bytes.len() != 64
            || gate_word(&bytes, 0)? != 0x31544147
            || gate_word(&bytes, 48)? != 0
            || gate_word(&bytes, 56)? != 0
            || gate_word(&bytes, 40)? > u32::MAX as u64
        {
            return Err(-1);
        }
        if gate_word(&bytes, 8)? != self.root
            || gate_word(&bytes, 16)? != self.services
            || gate_word(&bytes, 24)? != self.mutex
        {
            return Err(-4);
        }
        if gate_word(&bytes, 40)? == 0 && gate_word(&bytes, 32)? != 0 {
            return Err(-1);
        }
        Ok((version, bytes))
    }
    pub fn gate_borrow(ram: &memory::Memory, record: u64) -> Result<Self, i32> {
        let (_, bytes) = gamepkg::local_record_snapshot(record, 64)?;
        if bytes.len() != 64 {
            return Err(-1);
        }
        let view = Self {
            root: ram.call_handle(),
            services: ram.services_handle()?,
            record,
            mutex: gate_word(&bytes, 24)?,
            creator: 0,
            owned: false,
        };
        view.gate_live(ram)?;
        view.gate_raw(ram)?;
        match gamepkg::local_mutex_lock(view.mutex, 0) {
            Ok(()) => gate_unlocked(view.mutex)?,
            Err(-2) => {}
            Err(code) => return Err(code),
        }
        Ok(view)
    }
    pub fn gate_state(&self, ram: &memory::Memory) -> Result<[u64; 2], i32> {
        self.gate_live(ram)?;
        let (_, bytes) = self.gate_raw(ram)?;
        Ok([gate_word(&bytes, 32)?, gate_word(&bytes, 40)?])
    }
    fn gate_publish(&self, version: u64, bytes: &[u8]) -> Result<(), i32> {
        match gamepkg::local_record_compare_exchange(self.record, version, bytes) {
            Ok(_) => Ok(()),
            Err(-2) => Err(-3),
            Err(code) => Err(code),
        }
    }
    pub fn gate_enter(&self, ram: &memory::Memory, milliseconds: u32) -> Result<(), i32> {
        self.gate_live(ram)?;
        self.gate_raw(ram)?;
        gamepkg::local_mutex_lock(self.mutex, milliseconds)?;
        let result = self.gate_enter_locked(ram);
        if result.is_err() {
            let _ = gate_unlocked(self.mutex);
        }
        result
    }
    fn gate_enter_locked(&self, ram: &memory::Memory) -> Result<(), i32> {
        self.gate_live(ram)?;
        let (version, mut bytes) = self.gate_raw(ram)?;
        let task = gamepkg::local_task_status()?.0;
        let depth = gate_word(&bytes, 40)?;
        if depth != 0 && gate_word(&bytes, 32)? != task {
            return Err(-3);
        }
        let next = depth.checked_add(1).ok_or(-3)?;
        if next > u32::MAX as u64 {
            return Err(-3);
        }
        gate_put(&mut bytes, 32, task)?;
        gate_put(&mut bytes, 40, next)?;
        self.gate_publish(version, &bytes)
    }
    // Raw release only. The forthcoming IRQ adapter must run its safe point before
    // this call and its release point afterwards; do not use this as GateLeave.
    pub fn gate_leave_raw(&self, ram: &memory::Memory) -> Result<(), i32> {
        let (task, _) = gamepkg::local_task_status()?;
        let (version, mut bytes) = self.gate_raw(ram)?;
        let depth = gate_word(&bytes, 40)?;
        if depth == 0 || gate_word(&bytes, 32)? != task {
            return Err(-4);
        }
        gate_put(&mut bytes, 40, depth - 1)?;
        if depth == 1 {
            gate_put(&mut bytes, 32, 0)?;
        }
        self.gate_publish(version, &bytes)?;
        gate_unlocked(self.mutex)
    }
    fn gate_release_record(&self, ram: &memory::Memory) -> Result<(), i32> {
        let (_, bytes) = self.gate_raw(ram)?;
        if gate_word(&bytes, 40)? != 0 {
            return Err(-2);
        }
        gamepkg::local_record_release(self.record)
    }
    pub fn gate_close(&mut self, ram: &memory::Memory) -> Result<(), i32> {
        if self.gate_closed() {
            return Ok(());
        }
        if !self.owned
            || gamepkg::local_task_status()?.0 != self.creator
            || ram.call_handle() != self.root
        {
            return Err(-4);
        }
        let validation = self.gate_live(ram);
        let mut missing = false;
        if self.record != 0 {
            gamepkg::local_mutex_lock(self.mutex, 0)?;
            let result = self.gate_release_record(ram);
            let unlocked = gate_unlocked(self.mutex);
            match result {
                Ok(()) => self.record = 0,
                Err(-4) => {
                    self.record = 0;
                    missing = true;
                }
                Err(code) => {
                    let _ = unlocked;
                    return Err(code);
                }
            }
            unlocked?;
        }
        gamepkg::local_call(25, &u64::to_le_bytes(self.mutex), &mut [])?;
        self.mutex = 0;
        self.owned = false;
        validation?;
        if missing {
            return Err(-4);
        }
        Ok(())
    }
}
