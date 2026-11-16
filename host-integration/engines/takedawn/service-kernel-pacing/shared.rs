// Root-bound scheduling state shared by reviewed game tasks through copied records.
use crate::gamepkg;
use crate::kernel_pool;
use crate::memory;
use crate::pacing;
use crate::wait_service;
pub struct Schedule {
    root: u64,
    services: u64,
    record: u64,
    creator: u64,
    assigned: u64,
    joined: bool,
    owned: bool,
}
fn schedule_u64(bytes: &[u8], at: usize) -> Result<u64, i32> {
    let b = bytes.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}
fn schedule_owner(ram: &memory::Memory, expected: u64) -> Result<(), i32> {
    if expected == 0 || ram.services_handle()? != expected {
        return Err(-4);
    }
    let result = kernel_pool::kernel_service_values(ram);
    if ram.services_handle()? != expected {
        return Err(-4);
    }
    result?;
    if gamepkg::local_task_status()?.1 {
        return Err(-4);
    }
    Ok(())
}
impl Schedule {
    pub fn schedule_new(ram: &memory::Memory, mode: i32) -> Result<Self, i32> {
        let initial = pacing::Pacing::pacing_new(mode)?;
        let services = ram.services_handle()?;
        schedule_owner(ram, services)?;
        let creator = gamepkg::local_task_status()?.0;
        let mut bytes = std::vec::Vec::new();
        for value in [0x31534350u64, ram.call_handle(), services, 0, 0] {
            bytes.extend(u64::to_le_bytes(value));
        }
        bytes.extend(initial.pacing_encode());
        let record = gamepkg::local_record_create(&bytes)?;
        Ok(Self {
            root: ram.call_handle(),
            services,
            record,
            creator,
            assigned: 0,
            joined: false,
            owned: true,
        })
    }
    pub fn schedule_id(&self) -> u64 {
        self.record
    }
    pub fn schedule_closed(&self) -> bool {
        self.record == 0
    }
    pub fn schedule_borrow(ram: &memory::Memory, record: u64) -> Result<Self, i32> {
        let owner = Self {
            root: ram.call_handle(),
            services: ram.services_handle()?,
            record,
            creator: 0,
            assigned: 0,
            joined: false,
            owned: false,
        };
        owner.schedule_snapshot(ram)?;
        Ok(owner)
    }
    fn schedule_creator(&self, ram: &memory::Memory) -> Result<(), i32> {
        let (task, cancelled) = gamepkg::local_task_status()?;
        if !self.owned
            || self.record == 0
            || ram.call_handle() != self.root
            || task != self.creator
            || cancelled
        {
            return Err(-4);
        }
        Ok(())
    }
    fn schedule_snapshot(
        &self,
        ram: &memory::Memory,
    ) -> Result<(u64, std::vec::Vec<u8>, pacing::Pacing), i32> {
        if self.record == 0 || ram.call_handle() != self.root {
            return Err(-4);
        }
        schedule_owner(ram, self.services)?;
        let (version, bytes) = gamepkg::local_record_snapshot(self.record, 144)?;
        if bytes.len() != 144 || schedule_u64(&bytes, 0)? != 0x31534350 {
            return Err(-1);
        }
        if schedule_u64(&bytes, 8)? != self.root || schedule_u64(&bytes, 16)? != self.services {
            return Err(-4);
        }
        let task = schedule_u64(&bytes, 24)?;
        let claimed = schedule_u64(&bytes, 32)?;
        if claimed > 1 || (claimed == 0 && task != 0) || (claimed == 1 && task == 0) {
            return Err(-1);
        }
        let state = pacing::Pacing::pacing_decode(&bytes[40..])?;
        Ok((version, bytes, state))
    }
    // Publish the actual reviewed task ID before resuming its suspended entry.
    pub fn schedule_assign(&mut self, ram: &memory::Memory, task: u64) -> Result<(), i32> {
        self.schedule_creator(ram)?;
        if task == 0 || self.assigned != 0 || gamepkg::local_task_finished(task)? {
            return Err(-1);
        }
        for _ in 0..64u32 {
            let (version, mut bytes, _) = self.schedule_snapshot(ram)?;
            if schedule_u64(&bytes, 32)? != 0 {
                return Err(-1);
            }
            bytes[24..32].copy_from_slice(&u64::to_le_bytes(task));
            bytes[32..40].copy_from_slice(&u64::to_le_bytes(1));
            match gamepkg::local_record_compare_exchange(self.record, version, &bytes) {
                Ok(_) => {
                    self.assigned = task;
                    return Ok(());
                }
                Err(-2) => memory::Memory::call_retry_pause()?,
                Err(code) => return Err(code),
            }
        }
        Err(-2)
    }
    // Only the assigned worker advances the schedule. Display reporters may
    // concurrently update refresh state; bounded CAS retries retain both updates.
    pub fn schedule_step(&self, ram: &memory::Memory) -> Result<[u64; 4], i32> {
        let (task, cancelled) = gamepkg::local_task_status()?;
        if cancelled {
            return Err(-4);
        }
        let (_, role, _) = self.schedule_snapshot(ram)?;
        if schedule_u64(&role, 32)? != 1 || schedule_u64(&role, 24)? != task {
            return Err(-4);
        }
        let sampled = wait_service::guest_now_ns(ram);
        schedule_owner(ram, self.services)?;
        let now = sampled?;
        for _ in 0..64u32 {
            let (version, mut bytes, mut state) = self.schedule_snapshot(ram)?;
            if schedule_u64(&bytes, 32)? != 1 || schedule_u64(&bytes, 24)? != task {
                return Err(-4);
            }
            let value = state.pacing_step(now)?;
            bytes.truncate(40);
            bytes.extend(state.pacing_encode());
            match gamepkg::local_record_compare_exchange(self.record, version, &bytes) {
                Ok(_) => return Ok([now, value[0], value[1], value[2]]),
                Err(-2) => memory::Memory::call_retry_pause()?,
                Err(code) => return Err(code),
            }
        }
        Err(-2)
    }
    fn schedule_update(
        &self,
        ram: &memory::Memory,
        operation: u32,
        first: u64,
        second: u64,
    ) -> Result<u32, i32> {
        if operation > 3 || (operation == 0 && first > 1) {
            return Err(-1);
        }
        for _ in 0..64u32 {
            let (version, mut bytes, mut state) = self.schedule_snapshot(ram)?;
            let result = match operation {
                0 => {
                    state.pacing_auto(first != 0);
                    0
                }
                1 => state.pacing_refresh(first, second),
                2 => {
                    state.pacing_relearn();
                    0
                }
                3 => u32::from(state.pacing_rate_changed()),
                _ => return Err(-1),
            };
            bytes.truncate(40);
            bytes.extend(state.pacing_encode());
            match gamepkg::local_record_compare_exchange(self.record, version, &bytes) {
                Ok(_) => return Ok(result),
                Err(-2) => memory::Memory::call_retry_pause()?,
                Err(code) => return Err(code),
            }
        }
        Err(-2)
    }
    pub fn schedule_auto(&self, ram: &memory::Memory, on: bool) -> Result<(), i32> {
        self.schedule_update(ram, 0, u64::from(on), 0)?;
        Ok(())
    }
    pub fn schedule_refresh(
        &self,
        ram: &memory::Memory,
        when: u64,
        period: u64,
    ) -> Result<u32, i32> {
        self.schedule_update(ram, 1, when, period)
    }
    pub fn schedule_relearn(&self, ram: &memory::Memory) -> Result<(), i32> {
        self.schedule_update(ram, 2, 0, 0)?;
        Ok(())
    }
    pub fn schedule_changed(&self, ram: &memory::Memory) -> Result<bool, i32> {
        Ok(self.schedule_update(ram, 3, 0, 0)? != 0)
    }
    pub fn schedule_state(&self, ram: &memory::Memory) -> Result<std::vec::Vec<u8>, i32> {
        Ok(self.schedule_snapshot(ram)?.2.pacing_encode())
    }
    // Consume the assigned task's actual nonblocking join, even after config
    // revocation. A caller cannot supply an unverified completion claim.
    pub fn schedule_join(&mut self, ram: &memory::Memory) -> Result<u64, i32> {
        self.schedule_creator(ram)?;
        if self.assigned == 0 || self.joined {
            return Err(-1);
        }
        let mut reply = [0u8; 8];
        if gamepkg::local_call(11, &u64::to_le_bytes(self.assigned), &mut reply)? != 8 {
            return Err(-3);
        }
        self.joined = true;
        Ok(u64::from_le_bytes(reply))
    }
    pub fn schedule_close(&mut self, ram: &memory::Memory) -> Result<(), i32> {
        if self.record == 0 {
            return Ok(());
        }
        self.schedule_creator(ram)?;
        if self.assigned != 0 && !self.joined {
            return Err(-2);
        }
        let validation = schedule_owner(ram, self.services);
        gamepkg::local_record_release(self.record)?;
        self.record = 0;
        self.owned = false;
        validation
    }
}
