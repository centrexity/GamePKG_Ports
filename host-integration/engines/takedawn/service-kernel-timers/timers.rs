// Port-owned original 32-slot timer table. All locks, records and clocks are SDK services.
use crate::gamepkg;
use crate::kernel_pool;
use crate::memory;
use crate::queue;
use crate::timer_clock;
pub struct Timers {
    root: u64,
    services: u64,
    record: u64,
    lock: u64,
    queue: u64,
    worker: u64,
    creator: u64,
    owned: bool,
}
pub struct Scan {
    services: u64,
    lock: u64,
    queue: u64,
    root: u64,
    record: u64,
    worker: u64,
    now: i64,
    index: u32,
    fired: u32,
    next: i64,
    failed: bool,
}
fn timer_word(bytes: &[u8], at: usize) -> Result<u64, i32> {
    let b = bytes.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}
fn timer_u32(bytes: &[u8], at: usize) -> Result<u32, i32> {
    let b = bytes.get(at..at + 4).ok_or(-1)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}
fn timer_put(bytes: &mut [u8], at: usize, value: u64) -> Result<(), i32> {
    let b = bytes.get_mut(at..at + 8).ok_or(-1)?;
    b.copy_from_slice(&u64::to_le_bytes(value));
    Ok(())
}
fn timer_put32(bytes: &mut [u8], at: usize, value: u32) -> Result<(), i32> {
    let b = bytes.get_mut(at..at + 4).ok_or(-1)?;
    b.copy_from_slice(&u32::to_le_bytes(value));
    Ok(())
}
fn timer_owner(ram: &memory::Memory, services: u64) -> Result<(), i32> {
    if services == 0 || ram.services_handle()? != services {
        return Err(-4);
    }
    let values = kernel_pool::kernel_service_values(ram);
    if ram.services_handle()? != services {
        return Err(-4);
    }
    values?;
    if gamepkg::local_task_status()?.1 {
        return Err(-4);
    }
    Ok(())
}
pub fn timer_delay_ms(due: i64) -> i64 {
    if due < 0 {
        due.wrapping_neg() / 10000
    } else {
        0
    }
}
impl Timers {
    pub fn timers_new(ram: &memory::Memory, pending: &queue::Queue) -> Result<Self, i32> {
        let services = ram.services_handle()?;
        timer_owner(ram, services)?;
        pending.queue_state(ram)?;
        let mut reply = [0u8; 8];
        if gamepkg::local_call(21, &[], &mut reply)? != 8 {
            return Err(-3);
        }
        let lock = u64::from_le_bytes(reply);
        let mut bytes = std::vec::Vec::new();
        bytes.resize(832, 0);
        for (at, value) in [
            (0usize, 0x31524d54u64),
            (8, ram.call_handle()),
            (16, services),
            (24, lock),
            (32, pending.queue_id()),
        ] {
            timer_put(&mut bytes, at, value)?;
        }
        let record = match gamepkg::local_record_create(&bytes) {
            Ok(value) => value,
            Err(code) => {
                let _ = gamepkg::local_call(25, &u64::to_le_bytes(lock), &mut []);
                return Err(code);
            }
        };
        Ok(Self {
            root: ram.call_handle(),
            services,
            record,
            lock,
            queue: pending.queue_id(),
            worker: 0,
            creator: gamepkg::local_task_status()?.0,
            owned: true,
        })
    }
    pub fn timers_id(&self) -> u64 {
        self.record
    }
    pub fn timers_closed(&self) -> bool {
        self.record == 0 && self.lock == 0
    }
    pub fn timers_borrow(ram: &memory::Memory, record: u64) -> Result<Self, i32> {
        let (_, bytes) = gamepkg::local_record_snapshot(record, 832)?;
        if bytes.len() != 832 {
            return Err(-1);
        }
        let view = Self {
            root: ram.call_handle(),
            services: ram.services_handle()?,
            record,
            lock: timer_word(&bytes, 24)?,
            queue: timer_word(&bytes, 32)?,
            worker: 0,
            creator: 0,
            owned: false,
        };
        view.timers_snapshot(ram)?;
        match gamepkg::local_mutex_lock(view.lock, 0) {
            Ok(()) => view.timers_unlock()?,
            Err(-2) => {}
            Err(code) => return Err(code),
        }
        Ok(view)
    }
    fn timers_snapshot(&self, ram: &memory::Memory) -> Result<(u64, std::vec::Vec<u8>), i32> {
        if self.record == 0 || self.lock == 0 || self.root != ram.call_handle() {
            return Err(-4);
        }
        timer_owner(ram, self.services)?;
        let (version, bytes) = gamepkg::local_record_snapshot(self.record, 832)?;
        if bytes.len() != 832
            || timer_word(&bytes, 0)? != 0x31524d54
            || timer_word(&bytes, 48)? > 1
            || timer_word(&bytes, 56)? != 0
            || (timer_word(&bytes, 40)? == 0) != (timer_word(&bytes, 48)? == 0)
        {
            return Err(-1);
        }
        if timer_word(&bytes, 8)? != self.root
            || timer_word(&bytes, 16)? != self.services
            || timer_word(&bytes, 24)? != self.lock
            || timer_word(&bytes, 32)? != self.queue
            || self.queue == 0
        {
            return Err(-4);
        }
        for slot in 0..32usize {
            if timer_u32(&bytes, 64 + slot * 24 + 20)? != 0 {
                return Err(-1);
            }
        }
        Ok((version, bytes))
    }
    fn timers_locked(&self, ram: &memory::Memory) -> Result<(u64, std::vec::Vec<u8>), i32> {
        self.timers_snapshot(ram)?;
        gamepkg::local_mutex_lock(self.lock, 1000)?;
        match self.timers_snapshot(ram) {
            Ok(value) => Ok(value),
            Err(code) => {
                let _ = self.timers_unlock();
                Err(code)
            }
        }
    }
    fn timers_unlock(&self) -> Result<(), i32> {
        if gamepkg::local_call(24, &u64::to_le_bytes(self.lock), &mut [])? != 0 {
            return Err(-3);
        }
        Ok(())
    }
    fn timers_publish(&self, version: u64, bytes: &[u8]) -> Result<(), i32> {
        match gamepkg::local_record_compare_exchange(self.record, version, bytes) {
            Ok(_) => Ok(()),
            Err(-2) => Err(-3),
            Err(code) => Err(code),
        }
    }
    fn timers_creator(&self, ram: &memory::Memory) -> Result<(), i32> {
        let (task, cancelled) = gamepkg::local_task_status()?;
        if !self.owned
            || self.lock == 0
            || self.root != ram.call_handle()
            || self.creator != task
            || cancelled
        {
            return Err(-4);
        }
        Ok(())
    }
    // Publish the same actual suspended worker already assigned to the DPC queue.
    pub fn timers_start(
        &mut self,
        ram: &memory::Memory,
        pending: &queue::Queue,
    ) -> Result<(), i32> {
        self.timers_creator(ram)?;
        let binding = pending.queue_worker_binding(ram)?;
        if binding[0] != self.queue || self.worker != 0 {
            return Err(-4);
        }
        if gamepkg::local_task_finished(binding[1])? {
            return Err(-1);
        }
        self.worker = binding[1];
        let (version, mut bytes) = self.timers_locked(ram)?;
        let result = self.timers_start_locked(version, &mut bytes, binding[1]);
        let unlocked = self.timers_unlock();
        result?;
        unlocked?;
        self.worker = binding[1];
        Ok(())
    }
    fn timers_start_locked(&self, version: u64, bytes: &mut [u8], worker: u64) -> Result<(), i32> {
        if timer_word(bytes, 48)? != 0 {
            return Err(-1);
        }
        timer_put(bytes, 40, worker)?;
        timer_put(bytes, 48, 1)?;
        self.timers_publish(version, bytes)
    }
    // [was_set, stored]. Even a full table's caller must reset its shadow event
    // after this unlocked operation, as in the original bridge.
    pub fn timer_set(
        &self,
        ram: &mut memory::Memory,
        timer: u32,
        due: i64,
        period: i32,
        dpc: u32,
    ) -> Result<[u64; 2], i32> {
        let (version, mut bytes) = self.timers_locked(ram)?;
        let result = self.timer_set_locked(ram, version, &mut bytes, timer, due, period, dpc);
        let unlocked = self.timers_unlock();
        let value = result?;
        unlocked?;
        Ok(value)
    }
    fn timer_set_locked(
        &self,
        ram: &mut memory::Memory,
        version: u64,
        bytes: &mut [u8],
        timer: u32,
        due: i64,
        period: i32,
        dpc: u32,
    ) -> Result<[u64; 2], i32> {
        if timer_word(bytes, 48)? == 0 {
            return Err(-5);
        }
        let mut free = 32usize;
        let mut was = 0u64;
        for slot in 0..32usize {
            let guest = timer_u32(bytes, 64 + slot * 24)?;
            if guest == timer {
                free = slot;
                was = 1;
                break;
            }
            if guest == 0 && free == 32 {
                free = slot;
            }
        }
        if free == 32 {
            return Ok([was, 0]);
        }
        // The clock is sampled only for an available slot, while still locked.
        let now = timer_clock::timer_now_ms(ram)?;
        let at = 64 + free * 24;
        timer_put32(bytes, at, timer)?;
        timer_put32(bytes, at + 4, dpc)?;
        timer_put(bytes, at + 8, now.wrapping_add(timer_delay_ms(due)) as u64)?;
        timer_put32(bytes, at + 16, period as u32)?;
        self.timers_publish(version, bytes)?;
        Ok([was, 1])
    }
    pub fn timer_cancel(&self, ram: &memory::Memory, timer: u32) -> Result<bool, i32> {
        if timer_word(&self.timers_snapshot(ram)?.1, 48)? == 0 {
            return Ok(false);
        }
        let (version, mut bytes) = self.timers_locked(ram)?;
        let result = self.timer_cancel_locked(version, &mut bytes, timer);
        let unlocked = self.timers_unlock();
        let value = result?;
        unlocked?;
        Ok(value)
    }
    fn timer_cancel_locked(&self, version: u64, bytes: &mut [u8], timer: u32) -> Result<bool, i32> {
        let mut was = false;
        for slot in 0..32usize {
            let at = 64 + slot * 24;
            if timer_u32(bytes, at)? == timer {
                timer_put32(bytes, at, 0)?;
                was = true;
            }
        }
        self.timers_publish(version, bytes)?;
        Ok(was)
    }
    pub fn timers_state(&self, ram: &memory::Memory) -> Result<std::vec::Vec<u8>, i32> {
        Ok(self.timers_snapshot(ram)?.1)
    }
    pub fn timer_worker_binding(&self, ram: &memory::Memory) -> Result<[u64; 2], i32> {
        let (_, bytes) = self.timers_snapshot(ram)?;
        if timer_word(&bytes, 48)? == 0 {
            return Err(-5);
        }
        Ok([self.queue, timer_word(&bytes, 40)?])
    }
    pub fn timer_fire_begin(&self, ram: &memory::Memory, now: i64) -> Result<Scan, i32> {
        let (_, bytes) = self.timers_snapshot(ram)?;
        let (worker, cancelled) = gamepkg::local_task_status()?;
        if cancelled || timer_word(&bytes, 48)? == 0 || timer_word(&bytes, 40)? != worker {
            return Err(-4);
        }
        Ok(Scan {
            services: self.services,
            lock: self.lock,
            queue: self.queue,
            root: self.root,
            record: self.record,
            worker,
            now,
            index: 0,
            fired: 0,
            next: -1,
            failed: false,
        })
    }
    pub fn timer_fire_next(
        &self,
        ram: &memory::Memory,
        scan: &mut Scan,
    ) -> Result<Option<[u32; 2]>, i32> {
        scan.timer_scan_owner(ram, self.record)?;
        let result = self.timer_fire_next_inner(ram, scan);
        if result.is_err() {
            scan.failed = true;
        }
        result
    }
    fn timer_fire_next_inner(
        &self,
        ram: &memory::Memory,
        scan: &mut Scan,
    ) -> Result<Option<[u32; 2]>, i32> {
        while scan.index < 32 {
            let (version, mut bytes) = self.timers_locked(ram)?;
            let result = self.timer_fire_slot(version, &mut bytes, scan);
            let unlocked = self.timers_unlock();
            let value = result?;
            unlocked?;
            scan.index += 1;
            if let Some(entry) = value {
                scan.fired += 1;
                return Ok(Some(entry));
            }
        }
        Ok(None)
    }
    fn timer_fire_slot(
        &self,
        version: u64,
        bytes: &mut [u8],
        scan: &mut Scan,
    ) -> Result<Option<[u32; 2]>, i32> {
        if timer_word(bytes, 48)? == 0 || timer_word(bytes, 40)? != scan.worker {
            return Err(-4);
        }
        let at = 64 + scan.index as usize * 24;
        let timer = timer_u32(bytes, at)?;
        let due = timer_word(bytes, at + 8)? as i64;
        if timer == 0 || scan.now < due {
            if timer != 0 && (scan.next < 0 || due < scan.next) {
                scan.next = due;
            }
            return Ok(None);
        }
        let dpc = timer_u32(bytes, at + 4)?;
        let period = timer_u32(bytes, at + 16)? as i32;
        if period > 0 {
            let next = scan.now.wrapping_add(i64::from(period));
            timer_put(bytes, at + 8, next as u64)?;
            if scan.next < 0 || next < scan.next {
                scan.next = next;
            }
        } else {
            timer_put32(bytes, at, 0)?;
        }
        self.timers_publish(version, bytes)?;
        Ok(Some([timer, dpc]))
    }
    pub fn timers_close(
        &mut self,
        ram: &memory::Memory,
        joined: Option<&queue::Joined>,
    ) -> Result<(), i32> {
        if self.timers_closed() {
            return Ok(());
        }
        self.timers_creator(ram)?;
        if self.worker != 0 {
            joined.ok_or(-2)?.joined_for(ram, self.queue, self.worker)?;
        }
        let validation = timer_owner(ram, self.services);
        let mut missing = false;
        if self.record != 0 {
            gamepkg::local_mutex_lock(self.lock, 0)?;
            let released = gamepkg::local_record_release(self.record);
            let unlocked = self.timers_unlock();
            match released {
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
        gamepkg::local_call(25, &u64::to_le_bytes(self.lock), &mut [])?;
        self.lock = 0;
        self.owned = false;
        validation?;
        if missing {
            return Err(-4);
        }
        Ok(())
    }
}
impl Scan {
    fn timer_scan_owner(&self, ram: &memory::Memory, record: u64) -> Result<(), i32> {
        if self.root != ram.call_handle()
            || self.record != record
            || gamepkg::local_task_status()?.0 != self.worker
        {
            return Err(-4);
        }
        let view = Timers {
            root: self.root,
            services: self.services,
            record: self.record,
            lock: self.lock,
            queue: self.queue,
            worker: 0,
            creator: 0,
            owned: false,
        };
        let (_, bytes) = view.timers_snapshot(ram)?;
        if timer_word(&bytes, 40)? != self.worker || timer_word(&bytes, 48)? == 0 {
            return Err(-4);
        }
        if self.failed {
            return Err(-3);
        }
        Ok(())
    }
    pub fn timer_scan_result(&self, ram: &memory::Memory) -> Result<[u64; 2], i32> {
        self.timer_scan_owner(ram, self.record)?;
        if self.index != 32 {
            return Err(-2);
        }
        Ok([u64::from(self.fired), self.next as u64])
    }
    pub fn timer_scan_abort(&mut self, ram: &memory::Memory) -> Result<(), i32> {
        self.timer_scan_owner(ram, self.record)?;
        self.failed = true;
        Ok(())
    }
}
