// Port-owned original deferred-work ring. Native locks and memory remain SDK services.
use crate::gamepkg;
use crate::kernel_pool;
use crate::memory;
use crate::wait_service;
pub struct Queue {
    root: u64,
    services: u64,
    record: u64,
    lock: u64,
    stats: bool,
    creator: u64,
    worker: u64,
    joined: bool,
    owned: bool,
}
// An actual consuming join, witnessed by the owning queue. Other components
// borrowing the same timer worker can close without a second native join.
pub struct Joined {
    root: u64,
    services: u64,
    queue: u64,
    worker: u64,
    creator: u64,
}
impl Joined {
    pub fn joined_for(
        &self,
        ram: &memory::Memory,
        queue_identity: u64,
        worker: u64,
    ) -> Result<(), i32> {
        if self.root != ram.call_handle()
            || self.queue != queue_identity
            || self.worker != worker
            || worker == 0
            || self.services == 0
            || gamepkg::local_task_status()?.0 != self.creator
        {
            return Err(-4);
        }
        Ok(())
    }
}
// A single assigned consumer's still-locked dequeue observation. No native
// pointer, callback, new SDK object or permission is represented by this value.
pub struct Take {
    root: u64,
    services: u64,
    record: u64,
    lock: u64,
    worker: u64,
    version: u64,
    bytes: std::vec::Vec<u8>,
    held: bool,
}
fn queue_u64(bytes: &[u8], at: usize) -> Result<u64, i32> {
    let b = bytes.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}
fn queue_u32(bytes: &[u8], at: usize) -> Result<u32, i32> {
    let b = bytes.get(at..at + 4).ok_or(-1)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}
fn queue_put(bytes: &mut [u8], at: usize, value: u64) -> Result<(), i32> {
    if at.checked_add(8).ok_or(-1)? > bytes.len() {
        return Err(-1);
    }
    let value = u64::to_le_bytes(value);
    for lane in 0..8usize {
        bytes[at + lane] = value[lane];
    }
    Ok(())
}
fn queue_put32(bytes: &mut [u8], at: usize, value: u32) -> Result<(), i32> {
    if at.checked_add(4).ok_or(-1)? > bytes.len() {
        return Err(-1);
    }
    let value = u32::to_le_bytes(value);
    for lane in 0..4usize {
        bytes[at + lane] = value[lane];
    }
    Ok(())
}
fn queue_owner(ram: &memory::Memory, services: u64) -> Result<(), i32> {
    if services == 0 || ram.services_handle()? != services {
        return Err(-4);
    }
    let result = kernel_pool::kernel_service_values(ram);
    if ram.services_handle()? != services {
        return Err(-4);
    }
    result?;
    if gamepkg::local_task_status()?.1 {
        return Err(-4);
    }
    Ok(())
}
impl Queue {
    pub fn queue_new(ram: &memory::Memory, stats: bool) -> Result<Self, i32> {
        let services = ram.services_handle()?;
        queue_owner(ram, services)?;
        let mut reply = [0u8; 8];
        if gamepkg::local_call(21, &[], &mut reply)? != 8 {
            return Err(-3);
        }
        let lock = u64::from_le_bytes(reply);
        let mut bytes = std::vec::Vec::new();
        bytes.resize(1616, 0);
        for (at, value) in [
            (0usize, 0x31435044u64),
            (8, ram.call_handle()),
            (16, services),
            (24, lock),
            (48, u64::from(stats)),
        ] {
            queue_put(&mut bytes, at, value)?;
        }
        let record = match gamepkg::local_record_create(&bytes) {
            Ok(id) => id,
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
            stats,
            creator: gamepkg::local_task_status()?.0,
            worker: 0,
            joined: false,
            owned: true,
        })
    }
    pub fn queue_id(&self) -> u64 {
        self.record
    }
    pub fn queue_closed(&self) -> bool {
        self.record == 0 && self.lock == 0
    }
    pub fn queue_borrow(ram: &memory::Memory, record: u64) -> Result<Self, i32> {
        let (_, bytes) = gamepkg::local_record_snapshot(record, 1616)?;
        if bytes.len() != 1616 {
            return Err(-1);
        }
        let view = Self {
            root: ram.call_handle(),
            services: ram.services_handle()?,
            record,
            lock: queue_u64(&bytes, 24)?,
            stats: queue_u64(&bytes, 48)? != 0,
            creator: 0,
            worker: 0,
            joined: false,
            owned: false,
        };
        view.queue_snapshot(ram)?;
        // Busy is still a correctly typed mutex. An event must never be accepted.
        match gamepkg::local_mutex_lock(view.lock, 0) {
            Ok(()) => view.queue_unlock()?,
            Err(-2) => {}
            Err(code) => return Err(code),
        }
        Ok(view)
    }
    fn queue_snapshot(&self, ram: &memory::Memory) -> Result<(u64, std::vec::Vec<u8>), i32> {
        if self.record == 0 || self.lock == 0 || ram.call_handle() != self.root {
            return Err(-4);
        }
        queue_owner(ram, self.services)?;
        let (version, bytes) = gamepkg::local_record_snapshot(self.record, 1616)?;
        if bytes.len() != 1616
            || queue_u64(&bytes, 0)? != 0x31435044
            || queue_u64(&bytes, 72)? != 0
            || queue_u64(&bytes, 48)? > 1
            || queue_u64(&bytes, 40)? > 1
            || queue_u64(&bytes, 56)? >= 64
            || queue_u64(&bytes, 64)? >= 64
        {
            return Err(-1);
        }
        if queue_u64(&bytes, 8)? != self.root
            || queue_u64(&bytes, 16)? != self.services
            || queue_u64(&bytes, 24)? != self.lock
            || (queue_u64(&bytes, 48)? != 0) != self.stats
        {
            return Err(-4);
        }
        if (queue_u64(&bytes, 32)? == 0) != (queue_u64(&bytes, 40)? == 0) {
            return Err(-1);
        }
        for slot in 0..64usize {
            if queue_u32(&bytes, 80 + slot * 24 + 12)? != 0 {
                return Err(-1);
            }
        }
        Ok((version, bytes))
    }
    fn queue_locked(&self, ram: &memory::Memory) -> Result<(u64, std::vec::Vec<u8>), i32> {
        self.queue_snapshot(ram)?;
        gamepkg::local_mutex_lock(self.lock, 1000)?;
        match self.queue_snapshot(ram) {
            Ok(value) => Ok(value),
            Err(code) => {
                let _ = self.queue_unlock();
                Err(code)
            }
        }
    }
    fn queue_unlock(&self) -> Result<(), i32> {
        if gamepkg::local_call(24, &u64::to_le_bytes(self.lock), &mut [])? != 0 {
            return Err(-3);
        }
        Ok(())
    }
    fn queue_publish(&self, version: u64, bytes: &[u8]) -> Result<(), i32> {
        // All legitimate writers hold the same mutex: a stale version is a
        // corrupt/interfering publisher, not a retryable ordinary contention.
        match gamepkg::local_record_compare_exchange(self.record, version, bytes) {
            Ok(_) => Ok(()),
            Err(-2) => Err(-3),
            Err(code) => Err(code),
        }
    }
    fn queue_creator(&self, ram: &memory::Memory) -> Result<(), i32> {
        let (task, cancelled) = gamepkg::local_task_status()?;
        if !self.owned
            || self.lock == 0
            || ram.call_handle() != self.root
            || task != self.creator
            || cancelled
        {
            return Err(-4);
        }
        Ok(())
    }
    pub fn queue_assign(&mut self, ram: &memory::Memory, task: u64) -> Result<(), i32> {
        self.queue_creator(ram)?;
        if task == 0 || self.worker != 0 || gamepkg::local_task_finished(task)? {
            return Err(-1);
        }
        let (version, bytes) = self.queue_locked(ram)?;
        let result = self.queue_assign_locked(version, bytes, task);
        let unlocked = self.queue_unlock();
        result?;
        unlocked?;
        self.worker = task;
        Ok(())
    }
    fn queue_assign_locked(
        &self,
        version: u64,
        mut bytes: std::vec::Vec<u8>,
        task: u64,
    ) -> Result<(), i32> {
        if queue_u64(&bytes, 40)? != 0 {
            return Err(-1);
        }
        queue_put(&mut bytes, 32, task)?;
        queue_put(&mut bytes, 40, 1)?;
        self.queue_publish(version, &bytes)
    }
    // Success means queued, not executed. The caller must apply the original
    // DpcQueued/gate-release notification policy; this queue never calls a DPC inline.
    pub fn queue_insert(
        &self,
        ram: &mut memory::Memory,
        dpc: u32,
        first: u32,
        second: u32,
    ) -> Result<[u64; 2], i32> {
        self.queue_snapshot(ram)?;
        if dpc == 0 {
            return Ok([0, 0]);
        }
        let (version, bytes) = self.queue_locked(ram)?;
        let result = self.queue_insert_locked(ram, version, bytes, dpc, first, second);
        let unlocked = self.queue_unlock();
        let value = result?;
        unlocked?;
        Ok(value)
    }
    fn queue_insert_locked(
        &self,
        ram: &mut memory::Memory,
        version: u64,
        mut bytes: std::vec::Vec<u8>,
        dpc: u32,
        first: u32,
        second: u32,
    ) -> Result<[u64; 2], i32> {
        if ram.read_u8(dpc.wrapping_add(2))? != 0 {
            return Ok([0, 0]);
        }
        let head = queue_u64(&bytes, 56)?;
        let tail = queue_u64(&bytes, 64)?;
        let next = (tail + 1) % 64;
        if next == head {
            return Ok([0, 1]);
        }
        let sampled = if self.stats {
            wait_service::host_now_ns(ram)
        } else {
            Ok(0)
        };
        queue_owner(ram, self.services)?;
        let time = sampled?;
        let at = 80 + tail as usize * 24;
        queue_put32(&mut bytes, at, dpc)?;
        queue_put32(&mut bytes, at + 4, first)?;
        queue_put32(&mut bytes, at + 8, second)?;
        queue_put(&mut bytes, at + 16, time)?;
        // Original flag-before-tail publication. A later failed publication is
        // an explicit partial operation, never reported as queued/success.
        ram.write_u8(dpc.wrapping_add(2), 1)?;
        queue_put(&mut bytes, 64, next)?;
        self.queue_publish(version, &bytes)?;
        Ok([1, 0])
    }
    pub fn queue_remove(&self, ram: &mut memory::Memory, dpc: u32) -> Result<bool, i32> {
        self.queue_snapshot(ram)?;
        if dpc == 0 {
            return Ok(false);
        }
        let (version, bytes) = self.queue_locked(ram)?;
        let result = self.queue_remove_locked(ram, version, bytes, dpc);
        let unlocked = self.queue_unlock();
        let value = result?;
        unlocked?;
        Ok(value)
    }
    fn queue_remove_locked(
        &self,
        ram: &mut memory::Memory,
        version: u64,
        mut bytes: std::vec::Vec<u8>,
        dpc: u32,
    ) -> Result<bool, i32> {
        if ram.read_u8(dpc.wrapping_add(2))? == 0 {
            return Ok(false);
        }
        let mut slot = queue_u64(&bytes, 56)?;
        let tail = queue_u64(&bytes, 64)?;
        for _ in 0..63u32 {
            if slot == tail {
                break;
            }
            let at = 80 + slot as usize * 24;
            if queue_u32(&bytes, at)? == dpc {
                queue_put32(&mut bytes, at, 0)?;
            }
            slot = (slot + 1) % 64;
        }
        self.queue_publish(version, &bytes)?;
        ram.write_u8(dpc.wrapping_add(2), 0)?;
        Ok(true)
    }
    // Call only after the original dispatch gate/IRQL logic allows deferred work.
    // A cancelled hole is returned as dpc=0 and still advances the ring head.
    pub fn queue_pop(&self, ram: &mut memory::Memory) -> Result<Option<[u64; 4]>, i32> {
        let (version, bytes) = self.queue_locked(ram)?;
        let result = self.queue_pop_locked(ram, version, bytes);
        let unlocked = self.queue_unlock();
        let value = result?;
        unlocked?;
        Ok(value)
    }
    fn queue_pop_locked(
        &self,
        ram: &mut memory::Memory,
        version: u64,
        mut bytes: std::vec::Vec<u8>,
    ) -> Result<Option<[u64; 4]>, i32> {
        let (task, cancelled) = gamepkg::local_task_status()?;
        if cancelled || queue_u64(&bytes, 40)? != 1 || queue_u64(&bytes, 32)? != task {
            return Err(-4);
        }
        let head = queue_u64(&bytes, 56)?;
        if head == queue_u64(&bytes, 64)? {
            return Ok(None);
        }
        let at = 80 + head as usize * 24;
        let value = [
            u64::from(queue_u32(&bytes, at)?),
            u64::from(queue_u32(&bytes, at + 4)?),
            u64::from(queue_u32(&bytes, at + 8)?),
            queue_u64(&bytes, at + 16)?,
        ];
        queue_put(&mut bytes, 56, (head + 1) % 64)?;
        self.queue_publish(version, &bytes)?;
        if value[0] != 0 {
            ram.write_u8((value[0] as u32).wrapping_add(2), 0)?;
        }
        Ok(Some(value))
    }
    pub fn queue_pending(&self, ram: &memory::Memory) -> Result<bool, i32> {
        let (_, bytes) = self.queue_snapshot(ram)?;
        let (task, _) = gamepkg::local_task_status()?;
        Ok(
            !(queue_u64(&bytes, 40)? == 1 && queue_u64(&bytes, 32)? == task)
                && queue_u64(&bytes, 56)? != queue_u64(&bytes, 64)?,
        )
    }
    // Unlike queue_pending, this also reports entries to the assigned timer.
    // Cancelled holes are entries until consumed, just as head != tail in C.
    pub fn queue_nonempty(&self, ram: &memory::Memory) -> Result<bool, i32> {
        let (_, bytes) = self.queue_snapshot(ram)?;
        Ok(queue_u64(&bytes, 56)? != queue_u64(&bytes, 64)?)
    }
    // The original drain records queue depth while locked, BEFORE head/flag
    // publication. A successful Some transfers exactly this lock depth to Take.
    pub fn queue_take_begin(&self, ram: &memory::Memory) -> Result<Option<Take>, i32> {
        let (_, observed) = self.queue_snapshot(ram)?;
        let (worker, cancelled) = gamepkg::local_task_status()?;
        if cancelled || queue_u64(&observed, 40)? != 1 || queue_u64(&observed, 32)? != worker {
            return Err(-4);
        }
        let (version, bytes) = self.queue_locked(ram)?;
        let result = self.queue_take_prepare(version, bytes);
        match result {
            Ok(Some(value)) => Ok(Some(value)),
            Ok(None) => {
                self.queue_unlock()?;
                Ok(None)
            }
            Err(code) => {
                if self.queue_unlock().is_err() {
                    return Err(-3);
                }
                Err(code)
            }
        }
    }
    fn queue_take_prepare(
        &self,
        version: u64,
        bytes: std::vec::Vec<u8>,
    ) -> Result<Option<Take>, i32> {
        let (worker, cancelled) = gamepkg::local_task_status()?;
        if cancelled || queue_u64(&bytes, 40)? != 1 || queue_u64(&bytes, 32)? != worker {
            return Err(-4);
        }
        if queue_u64(&bytes, 56)? == queue_u64(&bytes, 64)? {
            return Ok(None);
        }
        Ok(Some(Take {
            root: self.root,
            services: self.services,
            record: self.record,
            lock: self.lock,
            worker,
            version,
            bytes,
            held: true,
        }))
    }
    pub fn queue_state(&self, ram: &memory::Memory) -> Result<std::vec::Vec<u8>, i32> {
        Ok(self.queue_snapshot(ram)?.1)
    }
    pub fn queue_join(&mut self, ram: &memory::Memory) -> Result<u64, i32> {
        self.queue_creator(ram)?;
        if self.worker == 0 || self.joined {
            return Err(-1);
        }
        let mut reply = [0u8; 8];
        if gamepkg::local_call(11, &u64::to_le_bytes(self.worker), &mut reply)? != 8 {
            return Err(-3);
        }
        self.joined = true;
        Ok(u64::from_le_bytes(reply))
    }
    pub fn queue_worker_binding(&self, ram: &memory::Memory) -> Result<[u64; 2], i32> {
        let (_, bytes) = self.queue_snapshot(ram)?;
        if queue_u64(&bytes, 40)? == 0 {
            return Err(-5);
        }
        Ok([self.record, queue_u64(&bytes, 32)?])
    }
    pub fn queue_join_witness(&self, ram: &memory::Memory) -> Result<Joined, i32> {
        self.queue_creator(ram)?;
        if !self.joined || self.worker == 0 {
            return Err(-2);
        }
        Ok(Joined {
            root: self.root,
            services: self.services,
            queue: self.record,
            worker: self.worker,
            creator: self.creator,
        })
    }
    pub fn queue_close(&mut self, ram: &memory::Memory) -> Result<(), i32> {
        if self.queue_closed() {
            return Ok(());
        }
        self.queue_creator(ram)?;
        if self.worker != 0 && !self.joined {
            return Err(-2);
        }
        let validation = queue_owner(ram, self.services);
        let mut missing = false;
        if self.record != 0 {
            gamepkg::local_mutex_lock(self.lock, 0)?;
            let released = gamepkg::local_record_release(self.record);
            let unlocked = self.queue_unlock();
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
impl Take {
    fn take_live(&self, ram: &memory::Memory) -> Result<(), i32> {
        if !self.held
            || ram.call_handle() != self.root
            || gamepkg::local_task_status()?.0 != self.worker
        {
            return Err(-4);
        }
        queue_owner(ram, self.services)?;
        let (version, bytes) = gamepkg::local_record_snapshot(self.record, 1616)?;
        // Any interfering/reentrant publisher invalidates this observation;
        // never silently refresh it and acknowledge a different queue entry.
        if version != self.version || bytes != self.bytes {
            return Err(-3);
        }
        Ok(())
    }
    pub fn take_depth(&self, ram: &memory::Memory) -> Result<u32, i32> {
        self.take_live(ram)?;
        let head = queue_u64(&self.bytes, 56)?;
        let tail = queue_u64(&self.bytes, 64)?;
        Ok(((tail + 64 - head) % 64) as u32)
    }
    pub fn take_entry(&self, ram: &memory::Memory) -> Result<[u64; 4], i32> {
        self.take_live(ram)?;
        let at = 80 + queue_u64(&self.bytes, 56)? as usize * 24;
        Ok([
            u64::from(queue_u32(&self.bytes, at)?),
            u64::from(queue_u32(&self.bytes, at + 4)?),
            u64::from(queue_u32(&self.bytes, at + 8)?),
            queue_u64(&self.bytes, at + 16)?,
        ])
    }
    fn take_commit(&self, ram: &mut memory::Memory) -> Result<[u64; 4], i32> {
        let value = self.take_entry(ram)?;
        let mut bytes = self.bytes.clone();
        queue_put(&mut bytes, 56, (queue_u64(&self.bytes, 56)? + 1) % 64)?;
        match gamepkg::local_record_compare_exchange(self.record, self.version, &bytes) {
            Ok(_) => {}
            Err(-2) => return Err(-3),
            Err(code) => return Err(code),
        }
        if value[0] != 0 {
            ram.write_u8((value[0] as u32).wrapping_add(2), 0)?;
        }
        Ok(value)
    }
    // Attempts cleanup on every result, including partial head/flag publication.
    // Check take_closed after an error; failed unlock needs owner cleanup/stop.
    pub fn take_publish(&mut self, ram: &mut memory::Memory) -> Result<[u64; 4], i32> {
        let result = self.take_commit(ram);
        let closed = self.take_abort();
        let value = result?;
        closed?;
        Ok(value)
    }
    // No root/KSV/cancellation precondition for cleanup of an already-owned
    // depth. Another actual task may neither unlock nor mark this value closed.
    pub fn take_abort(&mut self) -> Result<(), i32> {
        if !self.held {
            return Ok(());
        }
        if gamepkg::local_task_status()?.0 != self.worker {
            return Err(-4);
        }
        if gamepkg::local_call(24, &u64::to_le_bytes(self.lock), &mut [])? != 0 {
            return Err(-3);
        }
        self.held = false;
        Ok(())
    }
    pub fn take_closed(&self) -> bool {
        !self.held
    }
}
