// Port-owned guest worker slices, never native pointers or executable memory.
use crate::gamepkg;
use crate::kernel_pool;
use crate::memory;
pub struct Pool {
    root: u64,
    services: u64,
    record: u64,
    creator: u64,
    base: u32,
    bytes: u32,
    owned: bool,
}
pub struct Lease {
    root: u64,
    services: u64,
    record: u64,
    actor: u64,
    base: u32,
    bytes: u32,
    slot: u32,
    generation: u64,
    active: bool,
}
fn stacks_word(b: &[u8], at: usize) -> Result<u64, i32> {
    let s = b.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
    ]))
}
fn stacks_put(b: &mut [u8], at: usize, value: u64) -> Result<(), i32> {
    let s = b.get_mut(at..at + 8).ok_or(-1)?;
    let value = u64::to_le_bytes(value);
    for lane in 0..8usize {
        s[lane] = value[lane];
    }
    Ok(())
}
fn stacks_fit(base: u32, bytes: u32) -> Result<u32, i32> {
    if base < 0x780000
        || base % 65536 != 0
        || bytes > 8 * 1048576
        || bytes % 65536 != 0
        || base.checked_add(bytes).ok_or(-1)? > 0x4000000
    {
        return Err(-1);
    }
    Ok((bytes / 262144).min(16))
}
fn stacks_live(ram: &memory::Memory, root: u64, services: u64) -> Result<(), i32> {
    if root != ram.call_handle() || services == 0 || ram.services_handle()? != services {
        return Err(-4);
    }
    let value = kernel_pool::kernel_service_values(ram);
    if ram.services_handle()? != services {
        return Err(-4);
    }
    value?;
    if gamepkg::local_task_status()?.1 {
        return Err(-4);
    }
    Ok(())
}
fn stacks_snapshot(
    root: u64,
    services: u64,
    record: u64,
    base: u32,
    bytes: u32,
    allow_closing: bool,
) -> Result<(u64, std::vec::Vec<u8>), i32> {
    let (version, b) = gamepkg::local_record_snapshot(record, 576)?;
    if b.len() != 576
        || stacks_word(&b, 0)? != 0x31534b57
        || stacks_word(&b, 48)? > 1
        || stacks_word(&b, 56)? != 0
    {
        return Err(-1);
    }
    if stacks_word(&b, 8)? != root
        || stacks_word(&b, 16)? != services
        || stacks_word(&b, 24)? != u64::from(base)
        || stacks_word(&b, 32)? != u64::from(bytes)
    {
        return Err(-4);
    }
    let fit = stacks_fit(base, bytes)?;
    if stacks_word(&b, 40)? != u64::from(fit) {
        return Err(-1);
    }
    for slot in 0..16usize {
        let at = 64 + slot * 32;
        let used = stacks_word(&b, at)?;
        if used > 1
            || stacks_word(&b, at + 24)? != 0
            || (used != 0 && (slot >= fit as usize || stacks_word(&b, at + 16)? == 0))
            || (used == 0 && stacks_word(&b, at + 8)? != 0)
        {
            return Err(-1);
        }
    }
    if stacks_word(&b, 48)? != 0 && !allow_closing {
        return Err(-4);
    }
    Ok((version, b))
}
impl Pool {
    // Pass the reviewed Bootstrap stack base/size, not a host allocation.
    pub fn stacks_new(ram: &memory::Memory, base: u32, bytes: u32) -> Result<Self, i32> {
        let fit = stacks_fit(base, bytes)?;
        let root = ram.call_handle();
        let services = ram.services_handle()?;
        stacks_live(ram, root, services)?;
        let creator = gamepkg::local_task_status()?.0;
        let mut b = std::vec::Vec::new();
        b.resize(576, 0u8);
        for (at, value) in [
            (0usize, 0x31534b57u64),
            (8, root),
            (16, services),
            (24, u64::from(base)),
            (32, u64::from(bytes)),
            (40, u64::from(fit)),
        ] {
            stacks_put(&mut b, at, value)?;
        }
        let record = gamepkg::local_record_create(&b)?;
        if let Err(code) = stacks_live(ram, root, services) {
            if gamepkg::local_record_release(record).is_err() {
                return Err(-3);
            }
            return Err(code);
        }
        Ok(Self {
            root,
            services,
            record,
            creator,
            base,
            bytes,
            owned: true,
        })
    }
    pub fn stacks_id(&self) -> u64 {
        self.record
    }
    pub fn stacks_closed(&self) -> bool {
        self.record == 0
    }
    pub fn stacks_borrow(ram: &memory::Memory, record: u64) -> Result<Self, i32> {
        let (_, b) = gamepkg::local_record_snapshot(record, 576)?;
        if b.len() != 576
            || stacks_word(&b, 24)? > u32::MAX as u64
            || stacks_word(&b, 32)? > u32::MAX as u64
        {
            return Err(-1);
        }
        let root = ram.call_handle();
        let services = ram.services_handle()?;
        let base = stacks_word(&b, 24)? as u32;
        let bytes = stacks_word(&b, 32)? as u32;
        stacks_live(ram, root, services)?;
        stacks_snapshot(root, services, record, base, bytes, false)?;
        Ok(Self {
            root,
            services,
            record,
            creator: 0,
            base,
            bytes,
            owned: false,
        })
    }
    pub fn stacks_usage(&self, ram: &memory::Memory) -> Result<[u32; 2], i32> {
        stacks_live(ram, self.root, self.services)?;
        let (_, b) = stacks_snapshot(
            self.root,
            self.services,
            self.record,
            self.base,
            self.bytes,
            false,
        )?;
        let mut count = 0u32;
        for slot in 0..16usize {
            count += stacks_word(&b, 64 + slot * 32)? as u32;
        }
        Ok([count, stacks_fit(self.base, self.bytes)?])
    }
    pub fn stacks_allocate(&self, ram: &memory::Memory) -> Result<Option<Lease>, i32> {
        stacks_live(ram, self.root, self.services)?;
        let actor = gamepkg::local_task_status()?.0;
        for _ in 0..64u32 {
            let (version, mut b) = stacks_snapshot(
                self.root,
                self.services,
                self.record,
                self.base,
                self.bytes,
                false,
            )?;
            let fit = stacks_fit(self.base, self.bytes)?;
            let mut chosen = 16u32;
            for slot in 0..fit {
                if stacks_word(&b, 64 + slot as usize * 32)? == 0 {
                    chosen = slot;
                    break;
                }
            }
            if chosen == 16 {
                stacks_live(ram, self.root, self.services)?;
                return Ok(None);
            }
            let at = 64 + chosen as usize * 32;
            let generation = stacks_word(&b, at + 16)?.checked_add(1).ok_or(-3)?;
            stacks_put(&mut b, at, 1)?;
            stacks_put(&mut b, at + 8, actor)?;
            stacks_put(&mut b, at + 16, generation)?;
            match gamepkg::local_record_compare_exchange(self.record, version, &b) {
                Ok(_) => {
                    let mut lease = Lease {
                        root: self.root,
                        services: self.services,
                        record: self.record,
                        actor,
                        base: self.base,
                        bytes: self.bytes,
                        slot: chosen,
                        generation,
                        active: true,
                    };
                    if let Err(code) = stacks_live(ram, self.root, self.services) {
                        let _ = lease.stack_release(ram);
                        if !lease.stack_released() {
                            return Err(-3);
                        }
                        return Err(code);
                    }
                    return Ok(Some(lease));
                }
                Err(-2) => memory::Memory::call_retry_pause()?,
                Err(code) => return Err(code),
            }
        }
        Err(-2)
    }
    pub fn stacks_close(&mut self, ram: &memory::Memory) -> Result<(), i32> {
        if !self.owned
            || ram.call_handle() != self.root
            || gamepkg::local_task_status()?.0 != self.creator
        {
            if self.record == 0
                && ram.call_handle() == self.root
                && gamepkg::local_task_status()?.0 == self.creator
            {
                return Ok(());
            }
            return Err(-4);
        }
        let validation = stacks_live(ram, self.root, self.services);
        for _ in 0..64u32 {
            let (version, mut b) = stacks_snapshot(
                self.root,
                self.services,
                self.record,
                self.base,
                self.bytes,
                true,
            )?;
            for slot in 0..16usize {
                if stacks_word(&b, 64 + slot * 32)? != 0 {
                    return Err(-2);
                }
            }
            if stacks_word(&b, 48)? == 1 {
                gamepkg::local_record_release(self.record)?;
                self.record = 0;
                self.owned = false;
                validation?;
                return Ok(());
            }
            stacks_put(&mut b, 48, 1)?;
            match gamepkg::local_record_compare_exchange(self.record, version, &b) {
                Ok(_) => {
                    gamepkg::local_record_release(self.record)?;
                    self.record = 0;
                    self.owned = false;
                    validation?;
                    return Ok(());
                }
                Err(-2) => {} // Cleanup retries use only the existing record.
                Err(code) => return Err(code),
            }
        }
        Err(-2)
    }
}
impl Lease {
    // A copied token is usable only by the actual owning task and generation.
    pub fn stack_adopt(
        ram: &memory::Memory,
        record: u64,
        slot: u32,
        generation: u64,
    ) -> Result<Self, i32> {
        if slot >= 16 || generation == 0 {
            return Err(-1);
        }
        let pool = Pool::stacks_borrow(ram, record)?;
        let value = Self {
            root: pool.root,
            services: pool.services,
            record,
            actor: gamepkg::local_task_status()?.0,
            base: pool.base,
            bytes: pool.bytes,
            slot,
            generation,
            active: true,
        };
        value.stack_validate(ram, false)?;
        Ok(value)
    }
    pub fn stack_identity(&self, ram: &memory::Memory) -> Result<[u64; 3], i32> {
        self.stack_validate(ram, false)?;
        Ok([self.record, u64::from(self.slot), self.generation])
    }
    fn stack_validate(
        &self,
        ram: &memory::Memory,
        cleanup: bool,
    ) -> Result<(u64, std::vec::Vec<u8>), i32> {
        if !self.active
            || self.slot >= 16
            || ram.call_handle() != self.root
            || gamepkg::local_task_status()?.0 != self.actor
        {
            return Err(-4);
        }
        if !cleanup {
            stacks_live(ram, self.root, self.services)?;
        }
        let (version, b) = stacks_snapshot(
            self.root,
            self.services,
            self.record,
            self.base,
            self.bytes,
            false,
        )?;
        let at = 64 + self.slot as usize * 32;
        if stacks_word(&b, at)? != 1
            || stacks_word(&b, at + 8)? != self.actor
            || stacks_word(&b, at + 16)? != self.generation
        {
            return Err(-4);
        }
        Ok((version, b))
    }
    pub fn stack_slot(&self, ram: &memory::Memory) -> Result<u32, i32> {
        self.stack_validate(ram, false)?;
        Ok(self.slot)
    }
    pub fn stack_guest_top(&self, ram: &memory::Memory) -> Result<u32, i32> {
        self.stack_validate(ram, false)?;
        self.base
            .checked_add((self.slot + 1) * 262144)
            .ok_or(-1)?
            .checked_sub(16)
            .ok_or(-1)
    }
    pub fn stack_release(&mut self, ram: &memory::Memory) -> Result<(), i32> {
        if ram.call_handle() != self.root || gamepkg::local_task_status()?.0 != self.actor {
            return Err(-4);
        }
        if !self.active {
            return Ok(());
        }
        let validation = stacks_live(ram, self.root, self.services);
        for _ in 0..64u32 {
            let (version, mut b) = self.stack_validate(ram, true)?;
            let at = 64 + self.slot as usize * 32;
            stacks_put(&mut b, at, 0)?;
            stacks_put(&mut b, at + 8, 0)?;
            match gamepkg::local_record_compare_exchange(self.record, version, &b) {
                Ok(_) => {
                    self.active = false;
                    validation?;
                    return Ok(());
                }
                Err(-2) => {} // Cleanup retries use only the existing record.
                Err(code) => return Err(code),
            }
        }
        Err(-2)
    }
    pub fn stack_released(&self) -> bool {
        !self.active
    }
}
