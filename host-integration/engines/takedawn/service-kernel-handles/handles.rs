// Checked game metadata over bounded GamePKG scratch. No native handle authority.
use crate::gamepkg;
use crate::kernel_pool;
use crate::memory;
pub struct Handles {
    root: u64,
    services: u64,
    record: u64,
    lock: u64,
    creator: u64,
    window: u64,
    offset: u64,
    initialized: u64,
    phase: u64,
    close_error: i32,
}
pub struct HandleGuard {
    root: u64,
    services: u64,
    record: u64,
    lock: u64,
    task: u64,
    window: u64,
    offset: u64,
    held: bool,
    cleanup: bool,
}
fn handles_word(b: &[u8], at: usize) -> Result<u64, i32> {
    let p = b.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        p[0], p[1], p[2], p[3], p[4], p[5], p[6], p[7],
    ]))
}
fn handles_u32(b: &[u8], at: usize) -> Result<u32, i32> {
    let p = b.get(at..at + 4).ok_or(-1)?;
    Ok(u32::from_le_bytes([p[0], p[1], p[2], p[3]]))
}
fn handles_put(b: &mut [u8], at: usize, value: u64) -> Result<(), i32> {
    let p = b.get_mut(at..at + 8).ok_or(-1)?;
    p.copy_from_slice(&u64::to_le_bytes(value));
    Ok(())
}
fn handles_put32(b: &mut [u8], at: usize, value: u32) -> Result<(), i32> {
    let p = b.get_mut(at..at + 4).ok_or(-1)?;
    p.copy_from_slice(&u32::to_le_bytes(value));
    Ok(())
}
fn handles_owner(ram: &memory::Memory, services: u64) -> Result<(), i32> {
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
fn handles_scratch() -> Result<u64, i32> {
    let mut b = [0u8; 64];
    if gamepkg::local_call(1, &[], &mut b)? != 64 {
        return Err(-5);
    }
    let mut ids = [0u64; 4];
    for index in 0..4usize {
        ids[index] = handles_word(&b, index * 16)?;
        if ids[index] == 0
            || handles_word(&b, index * 16 + 8)? != if index < 2 { 67108864 } else { 1048576 }
        {
            return Err(-1);
        }
        for before in 0..index {
            if ids[before] == ids[index] {
                return Err(-1);
            }
        }
    }
    Ok(ids[3])
}
fn handles_publish(record: u64, version: u64, b: &[u8]) -> Result<(), i32> {
    match gamepkg::local_record_compare_exchange(record, version, b) {
        Ok(_) => Ok(()),
        Err(-2) => Err(-3),
        Err(code) => Err(code),
    }
}
fn handles_root(
    record: u64,
    root: u64,
    services: u64,
    lock: u64,
) -> Result<(u64, std::vec::Vec<u8>), i32> {
    let (version, b) = gamepkg::local_record_snapshot(record, 80)?;
    if b.len() != 80
        || handles_word(&b, 0)? != 0x32525448
        || handles_word(&b, 40)? > 3
        || handles_word(&b, 64)? > 16
        || handles_word(&b, 72)? != 0
    {
        return Err(-1);
    }
    if root == 0
        || services == 0
        || lock == 0
        || handles_word(&b, 8)? != root
        || handles_word(&b, 16)? != services
        || handles_word(&b, 24)? != lock
    {
        return Err(-4);
    }
    let phase = handles_word(&b, 40)?;
    let window = handles_word(&b, 48)?;
    let offset = handles_word(&b, 56)?;
    let count = handles_word(&b, 64)?;
    if phase == 0 {
        if window != 0 || offset != 0 || count != 0 {
            return Err(-1);
        }
    } else {
        if window == 0 || offset % 262144 != 0 || offset > 786432 {
            return Err(-1);
        }
        if phase == 2 && count != 16 {
            return Err(-1);
        }
    }
    Ok((version, b))
}
fn handles_unlock(lock: u64) -> Result<(), i32> {
    if gamepkg::local_call(24, &u64::to_le_bytes(lock), &mut [])? != 0 {
        return Err(-3);
    }
    Ok(())
}
fn handles_read(window: u64, offset: u64) -> Result<std::vec::Vec<u8>, i32> {
    let mut output = std::vec::Vec::new();
    output.resize(16384, 0);
    let request = [
        u64::to_le_bytes(window).as_slice(),
        &u64::to_le_bytes(offset),
        &u32::to_le_bytes(16384),
    ]
    .concat();
    if gamepkg::local_call(5, &request, &mut output)? != output.len() {
        return Err(-3);
    }
    Ok(output)
}
fn handles_page(
    window: u64,
    offset: u64,
    record: u64,
    index: usize,
) -> Result<std::vec::Vec<u8>, i32> {
    if index >= 16 {
        return Err(-1);
    }
    let b = handles_read(window, offset + index as u64 * 16384)?;
    if handles_u32(&b, 0)? != 0x32505448
        || handles_u32(&b, 4)? != index as u32
        || handles_word(&b, 24)? != 0
    {
        return Err(-1);
    }
    if handles_word(&b, 8)? != record {
        return Err(-4);
    }
    let count = if index == 15 { 1023usize } else { 1024 };
    for slot in 0..count {
        if (handles_u32(&b, 32 + slot * 12)? == 0) != (handles_word(&b, 36 + slot * 12)? == 0) {
            return Err(-1);
        }
    }
    for value in &b[32 + count * 12..] {
        if *value != 0 {
            return Err(-1);
        }
    }
    Ok(b)
}
fn handles_replace(
    window: u64,
    offset: u64,
    index: usize,
    old: &[u8],
    replacement: &[u8],
) -> Result<(), i32> {
    if gamepkg::local_memory_compare_exchange(
        window,
        offset + index as u64 * 16384,
        old,
        replacement,
    )? {
        Ok(())
    } else {
        Err(-3)
    }
}
fn handles_decode(token: u32) -> Result<[usize; 2], i32> {
    let number = token & 0x00ffffff;
    if token & 0xff000000 != 0x48000000 || number == 0 || number >= 16384 {
        return Err(-1);
    }
    let offset = (number - 1) as usize;
    Ok([offset / 1024, offset % 1024])
}
impl Handles {
    // Allocated, unbound owner. Bind once; failures retain explicit close obligations.
    pub fn handles_new(ram: &memory::Memory) -> Result<Self, i32> {
        let services = ram.services_handle()?;
        handles_owner(ram, services)?;
        let creator = gamepkg::local_task_status()?.0;
        let mut out = [0u8; 8];
        if gamepkg::local_call(21, &[], &mut out)? != 8 {
            return Err(-3);
        }
        let lock = u64::from_le_bytes(out);
        let mut b = [0u8; 80];
        for (at, value) in [
            (0usize, 0x32525448u64),
            (8, ram.call_handle()),
            (16, services),
            (24, lock),
        ] {
            handles_put(&mut b, at, value)?;
        }
        let record = match gamepkg::local_record_create(&b) {
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
            creator,
            window: 0,
            offset: 0,
            initialized: 0,
            phase: 0,
            close_error: 0,
        })
    }
    fn handles_creator(&self, ram: &memory::Memory) -> Result<(), i32> {
        if ram.call_handle() != self.root || gamepkg::local_task_status()?.0 != self.creator {
            return Err(-4);
        }
        Ok(())
    }
    pub fn handles_id(&self) -> u64 {
        self.record
    }
    pub fn handles_closed(&self) -> bool {
        self.record == 0 && self.lock == 0
    }
    pub fn handles_progress(&self) -> [u64; 2] {
        [self.phase, self.initialized]
    }
    pub fn handles_bind(
        &mut self,
        ram: &memory::Memory,
        window: u64,
        offset: u64,
    ) -> Result<(), i32> {
        self.handles_creator(ram)?;
        handles_owner(ram, self.services)?;
        if self.phase != 0
            || self.window != 0
            || window == 0
            || offset % 262144 != 0
            || offset > 786432
        {
            return Err(-1);
        }
        if window != handles_scratch()? {
            return Err(-4);
        }
        let states = gamepkg::local_memory_page_states(window, offset, 64)?;
        if states.len() != 64 {
            return Err(-1);
        }
        for state in states {
            if state != 3 {
                return Err(-4);
            }
        }
        let (version, mut b) = handles_root(self.record, self.root, self.services, self.lock)?;
        if handles_word(&b, 40)? != 0 || handles_word(&b, 32)? != 0 {
            return Err(-4);
        }
        handles_put(&mut b, 40, 1)?;
        handles_put(&mut b, 48, window)?;
        handles_put(&mut b, 56, offset)?;
        handles_publish(self.record, version, &b)?;
        self.window = window;
        self.offset = offset;
        self.phase = 1;
        let mut handles_zero = std::vec::Vec::new();
        handles_zero.resize(16384, 0u8);
        for index in 0..16usize {
            handles_owner(ram, self.services)?;
            let mut page = handles_zero.clone();
            handles_put32(&mut page, 0, 0x32505448)?;
            handles_put32(&mut page, 4, index as u32)?;
            handles_put(&mut page, 8, self.record)?;
            if !gamepkg::local_memory_compare_exchange(
                window,
                offset + index as u64 * 16384,
                &handles_zero,
                &page,
            )? {
                return Err(-2);
            }
            // Retain actual accepted RAM prefix before any later fallible publication.
            self.initialized = index as u64 + 1;
            let (version, mut b) = handles_root(self.record, self.root, self.services, self.lock)?;
            if handles_word(&b, 40)? != 1
                || handles_word(&b, 48)? != window
                || handles_word(&b, 56)? != offset
                || handles_word(&b, 64)? + 1 != self.initialized
            {
                return Err(-4);
            }
            handles_put(&mut b, 64, self.initialized)?;
            handles_publish(self.record, version, &b)?;
        }
        handles_owner(ram, self.services)?;
        let (version, mut b) = handles_root(self.record, self.root, self.services, self.lock)?;
        if handles_word(&b, 64)? != 16 || handles_word(&b, 40)? != 1 {
            return Err(-4);
        }
        handles_put(&mut b, 40, 2)?;
        handles_publish(self.record, version, &b)?;
        self.phase = 2;
        Ok(())
    }
    pub fn handles_borrow(ram: &memory::Memory, record: u64) -> Result<Self, i32> {
        let (_, b) = gamepkg::local_record_snapshot(record, 80)?;
        if b.len() != 80 {
            return Err(-1);
        }
        let view = Self {
            root: ram.call_handle(),
            services: ram.services_handle()?,
            record,
            lock: handles_word(&b, 24)?,
            creator: u64::MAX,
            window: handles_word(&b, 48)?,
            offset: handles_word(&b, 56)?,
            initialized: 16,
            phase: 2,
            close_error: 0,
        };
        if view.window != handles_scratch()? {
            return Err(-4);
        }
        view.handles_snapshot(ram)?;
        match gamepkg::local_mutex_lock(view.lock, 0) {
            Ok(()) => handles_unlock(view.lock)?,
            Err(-2) => {}
            Err(code) => return Err(code),
        }
        Ok(view)
    }
    fn handles_snapshot(&self, ram: &memory::Memory) -> Result<(u64, std::vec::Vec<u8>), i32> {
        if self.record == 0 || ram.call_handle() != self.root {
            return Err(-4);
        }
        handles_owner(ram, self.services)?;
        let result = handles_root(self.record, self.root, self.services, self.lock)?;
        if self.phase != 2
            || handles_word(&result.1, 40)? != 2
            || handles_word(&result.1, 48)? != self.window
            || handles_word(&result.1, 56)? != self.offset
        {
            return Err(-4);
        }
        Ok(result)
    }
    pub fn handles_lock(&self, ram: &memory::Memory) -> Result<HandleGuard, i32> {
        self.handles_snapshot(ram)?;
        self.handles_enter(ram, false)
    }
    pub fn handles_cleanup_lock(&self, ram: &memory::Memory) -> Result<HandleGuard, i32> {
        self.handles_creator(ram)?;
        self.handles_enter(ram, true)
    }
    fn handles_enter(&self, ram: &memory::Memory, cleanup: bool) -> Result<HandleGuard, i32> {
        let task = gamepkg::local_task_status()?.0;
        gamepkg::local_mutex_lock(self.lock, 1000)?;
        let outcome = self.handles_enter_locked(ram, cleanup);
        if let Err(code) = outcome {
            let _ = handles_unlock(self.lock);
            return Err(code);
        }
        Ok(HandleGuard {
            root: self.root,
            services: self.services,
            record: self.record,
            lock: self.lock,
            task,
            window: self.window,
            offset: self.offset,
            held: true,
            cleanup,
        })
    }
    fn handles_enter_locked(&self, ram: &memory::Memory, cleanup: bool) -> Result<(), i32> {
        let (version, mut b) = if cleanup {
            handles_root(self.record, self.root, self.services, self.lock)?
        } else {
            self.handles_snapshot(ram)?
        };
        if ram.call_handle() != self.root
            || self.phase != 2
            || handles_word(&b, 40)? != 2
            || handles_word(&b, 48)? != self.window
            || handles_word(&b, 56)? != self.offset
        {
            return Err(-4);
        }
        let count = handles_word(&b, 32)?.checked_add(1).ok_or(-2)?;
        handles_put(&mut b, 32, count)?;
        handles_publish(self.record, version, &b)
    }
    pub fn handles_close(&mut self, ram: &memory::Memory) -> Result<(), i32> {
        self.handles_creator(ram)?;
        if self.handles_closed() {
            return if self.close_error == 0 {
                Ok(())
            } else {
                Err(self.close_error)
            };
        }
        let validation = handles_owner(ram, self.services);
        gamepkg::local_mutex_lock(self.lock, 0)?;
        let outcome = self.handles_close_locked();
        let unlocked = handles_unlock(self.lock);
        outcome?;
        unlocked?;
        if gamepkg::local_call(25, &u64::to_le_bytes(self.lock), &mut [])? != 0 {
            return Err(-3);
        }
        self.lock = 0;
        if let Err(code) = validation {
            self.close_error = code;
        }
        if self.close_error == 0 {
            Ok(())
        } else {
            Err(self.close_error)
        }
    }
    fn handles_close_locked(&mut self) -> Result<(), i32> {
        if self.record == 0 {
            return Ok(());
        }
        if self.phase != 3 {
            let (version, mut b) = handles_root(self.record, self.root, self.services, self.lock)?;
            if handles_word(&b, 32)? != 0 {
                return Err(-2);
            }
            if handles_word(&b, 40)? != self.phase
                || handles_word(&b, 48)? != self.window
                || handles_word(&b, 56)? != self.offset
                || handles_word(&b, 64)? > self.initialized
            {
                return Err(-4);
            }
            for index in 0..self.initialized as usize {
                let page = handles_page(self.window, self.offset, self.record, index)?;
                for slot in 0..if index == 15 { 1023usize } else { 1024 } {
                    if handles_u32(&page, 32 + slot * 12)? != 0 {
                        return Err(-2);
                    }
                }
            }
            handles_put(&mut b, 40, 3)?;
            handles_publish(self.record, version, &b)?;
            self.phase = 3;
        }
        // Reverse order retains the first-page claim until all other pages clear.
        while self.initialized != 0 {
            let index = (self.initialized - 1) as usize;
            let page = handles_page(self.window, self.offset, self.record, index)?;
            for slot in 0..if index == 15 { 1023usize } else { 1024 } {
                if handles_u32(&page, 32 + slot * 12)? != 0 {
                    return Err(-2);
                }
            }
            let mut handles_zero = std::vec::Vec::new();
            handles_zero.resize(16384, 0u8);
            handles_replace(self.window, self.offset, index, &page, &handles_zero)?;
            self.initialized -= 1;
        }
        match gamepkg::local_record_release(self.record) {
            Ok(()) | Err(-4) => {}
            Err(code) => return Err(code),
        }
        self.record = 0;
        Ok(())
    }
}
impl HandleGuard {
    fn handles_guard_owner(&self, ram: &memory::Memory) -> Result<(), i32> {
        if !self.held
            || ram.call_handle() != self.root
            || gamepkg::local_task_status()?.0 != self.task
        {
            return Err(-4);
        }
        Ok(())
    }
    fn handles_guard_snapshot(
        &self,
        ram: &memory::Memory,
    ) -> Result<(u64, std::vec::Vec<u8>), i32> {
        self.handles_guard_owner(ram)?;
        if !self.cleanup {
            handles_owner(ram, self.services)?;
        }
        let result = handles_root(self.record, self.root, self.services, self.lock)?;
        if handles_word(&result.1, 40)? != 2
            || handles_word(&result.1, 32)? == 0
            || handles_word(&result.1, 48)? != self.window
            || handles_word(&result.1, 56)? != self.offset
        {
            return Err(-4);
        }
        Ok(result)
    }
    pub fn handle_guard_insert(
        &self,
        ram: &memory::Memory,
        object: u64,
        kind: u32,
        fresh: bool,
    ) -> Result<u32, i32> {
        self.handles_guard_snapshot(ram)?;
        if self.cleanup {
            return Err(-4);
        }
        if object == 0 || kind == 0 {
            return Err(-1);
        }
        let mut free = [16usize, 0];
        for index in 0..16usize {
            let page = handles_page(self.window, self.offset, self.record, index)?;
            for slot in 0..if index == 15 { 1023usize } else { 1024 } {
                let occupied = handles_u32(&page, 32 + slot * 12)?;
                let value = handles_word(&page, 36 + slot * 12)?;
                if !fresh && occupied == kind && value == object {
                    return Ok(0x48000001 + (index * 1024 + slot) as u32);
                }
                if occupied == 0 && free[0] == 16 {
                    free = [index, slot];
                }
            }
        }
        if free[0] == 16 {
            return Err(-2);
        }
        let index = free[0];
        let slot = free[1];
        self.handles_guard_snapshot(ram)?;
        let old = handles_page(self.window, self.offset, self.record, index)?;
        if handles_u32(&old, 32 + slot * 12)? != 0 {
            return Err(-3);
        }
        let mut next = old.clone();
        handles_put(
            &mut next,
            16,
            handles_word(&old, 16)?.checked_add(1).ok_or(-3)?,
        )?;
        handles_put32(&mut next, 32 + slot * 12, kind)?;
        handles_put(&mut next, 36 + slot * 12, object)?;
        handles_replace(self.window, self.offset, index, &old, &next)?;
        Ok(0x48000001 + (index * 1024 + slot) as u32)
    }
    pub fn handle_guard_lookup(
        &self,
        ram: &memory::Memory,
        token: u32,
    ) -> Result<Option<[u64; 2]>, i32> {
        self.handles_guard_snapshot(ram)?;
        let at = handles_decode(token)?;
        let b = handles_page(self.window, self.offset, self.record, at[0])?;
        let kind = handles_u32(&b, 32 + at[1] * 12)?;
        if kind == 0 {
            Ok(None)
        } else {
            Ok(Some([handles_word(&b, 36 + at[1] * 12)?, u64::from(kind)]))
        }
    }
    pub fn handle_guard_remove(
        &self,
        ram: &memory::Memory,
        token: u32,
    ) -> Result<Option<[u64; 2]>, i32> {
        self.handles_guard_snapshot(ram)?;
        let at = handles_decode(token)?;
        let old = handles_page(self.window, self.offset, self.record, at[0])?;
        let kind = handles_u32(&old, 32 + at[1] * 12)?;
        if kind == 0 {
            return Ok(None);
        }
        let value = handles_word(&old, 36 + at[1] * 12)?;
        let mut next = old.clone();
        handles_put(
            &mut next,
            16,
            handles_word(&old, 16)?.checked_add(1).ok_or(-3)?,
        )?;
        handles_put32(&mut next, 32 + at[1] * 12, 0)?;
        handles_put(&mut next, 36 + at[1] * 12, 0)?;
        handles_replace(self.window, self.offset, at[0], &old, &next)?;
        Ok(Some([value, u64::from(kind)]))
    }
    pub fn handle_guard_slots(&self, ram: &memory::Memory) -> Result<std::vec::Vec<[u64; 2]>, i32> {
        self.handles_guard_snapshot(ram)?;
        let mut result = std::vec::Vec::new();
        for index in 0..16usize {
            let b = handles_page(self.window, self.offset, self.record, index)?;
            for slot in 0..if index == 15 { 1023usize } else { 1024 } {
                result.push([
                    handles_word(&b, 36 + slot * 12)?,
                    u64::from(handles_u32(&b, 32 + slot * 12)?),
                ]);
            }
        }
        Ok(result)
    }
    pub fn handle_guard_references(
        &self,
        ram: &memory::Memory,
        object: u64,
        kind: u32,
    ) -> Result<u32, i32> {
        if object == 0 || kind == 0 {
            return Err(-1);
        }
        let mut count = 0u32;
        for item in self.handle_guard_slots(ram)? {
            if item == [object, u64::from(kind)] {
                count += 1;
            }
        }
        Ok(count)
    }
    pub fn handles_guard_close(&mut self, ram: &memory::Memory) -> Result<(), i32> {
        if !self.held {
            return Ok(());
        }
        self.handles_guard_owner(ram)?;
        let outcome = self.handles_leave();
        let unlocked = handles_unlock(self.lock);
        if unlocked.is_ok() {
            self.held = false;
        }
        outcome?;
        unlocked
    }
    fn handles_leave(&self) -> Result<(), i32> {
        let (version, mut b) = handles_root(self.record, self.root, self.services, self.lock)?;
        let count = handles_word(&b, 32)?;
        if count == 0 {
            return Err(-3);
        }
        handles_put(&mut b, 32, count - 1)?;
        handles_publish(self.record, version, &b)
    }
    pub fn handles_guard_closed(&self) -> bool {
        !self.held
    }
}
