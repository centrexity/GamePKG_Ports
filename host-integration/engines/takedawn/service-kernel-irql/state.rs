// Port-owned IRQL bookkeeping, one atomic record for counts and task state.
use crate::gamepkg;
use crate::gate;
use crate::kernel_pool;
use crate::memory;
pub struct Controller {
    root: u64,
    services: u64,
    record: u64,
    raw_gate_record: u64,
    creator: u64,
    owned: bool,
    lock: gate::Gate,
}
pub struct Task {
    root: u64,
    services: u64,
    controller: u64,
    task: u64,
    raw_gate_record: u64,
    slot: usize,
    lock: gate::Gate,
}
fn state_word(b: &[u8], at: usize) -> Result<u64, i32> {
    let b = b.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}
fn state_put(b: &mut [u8], at: usize, value: u64) -> Result<(), i32> {
    if at.checked_add(8).ok_or(-1)? > b.len() {
        return Err(-1);
    }
    let v = u64::to_le_bytes(value);
    for lane in 0..8usize {
        b[at + lane] = v[lane];
    }
    Ok(())
}
fn state_live(ram: &memory::Memory, root: u64, services: u64) -> Result<(), i32> {
    if root != ram.call_handle() || services == 0 || ram.services_handle()? != services {
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
fn state_snapshot(
    root: u64,
    services: u64,
    record: u64,
    raw_gate_record: u64,
) -> Result<(u64, std::vec::Vec<u8>), i32> {
    let (version, b) = gamepkg::local_record_snapshot(record, 2176)?;
    if b.len() != 2176
        || state_word(&b, 0)? != 0x314c5149
        || state_word(&b, 56)? > 1
        || state_word(&b, 32)? > 33
        || state_word(&b, 40)? > 33
        || state_word(&b, 48)? > u32::MAX as u64
    {
        return Err(-1);
    }
    if state_word(&b, 8)? != root
        || state_word(&b, 16)? != services
        || state_word(&b, 24)? != raw_gate_record
    {
        return Err(-4);
    }
    for slot in 0..33usize {
        let at = 64 + slot * 64;
        let task = state_word(&b, at)?;
        let used = state_word(&b, at + 8)?;
        if used > 15 || (used != 0 && used & 1 == 0) || state_word(&b, at + 16)? > 255 {
            return Err(-1);
        }
        for lane in [24usize, 32, 40, 48, 56] {
            if state_word(&b, at + lane)? > 1 {
                return Err(-1);
            }
        }
        if used == 0 {
            for lane in 0..8usize {
                if state_word(&b, at + lane * 8)? != 0 {
                    return Err(-1);
                }
            }
        } else {
            for prior in 0..slot {
                if state_word(&b, 64 + prior * 64 + 8)? != 0
                    && state_word(&b, 64 + prior * 64)? == task
                {
                    return Err(-1);
                }
            }
        }
    }
    let mut raised = 0u64;
    let mut masked = 0u64;
    for slot in 0..33usize {
        let status = state_word(&b, 72 + slot * 64)?;
        raised += u64::from(status & 2 != 0);
        masked += u64::from(status & 4 != 0);
    }
    if raised != state_word(&b, 32)? || masked != state_word(&b, 40)? {
        return Err(-1);
    }
    Ok((version, b))
}
impl Controller {
    pub fn controller_new(ram: &memory::Memory) -> Result<Self, i32> {
        let (creator, cancel) = gamepkg::local_task_status()?;
        if cancel {
            return Err(-4);
        }
        let root = ram.call_handle();
        let services = ram.services_handle()?;
        state_live(ram, root, services)?;
        let mut lock = gate::Gate::gate_new(ram)?;
        let raw_gate_record = lock.gate_id();
        let mut b = std::vec::Vec::new();
        b.resize(2176, 0);
        for (at, v) in [
            (0usize, 0x314c5149u64),
            (8, root),
            (16, services),
            (24, raw_gate_record),
        ] {
            state_put(&mut b, at, v)?;
        }
        let record = match gamepkg::local_record_create(&b) {
            Ok(id) => id,
            Err(code) => {
                let _ = lock.gate_close(ram);
                return Err(code);
            }
        };
        let owner = Self {
            root,
            services,
            record,
            raw_gate_record,
            creator,
            owned: true,
            lock,
        };
        if let Err(code) = state_live(ram, root, services) {
            let _ = gamepkg::local_record_release(record);
            let mut lock = owner.lock;
            let _ = lock.gate_close(ram);
            return Err(code);
        }
        Ok(owner)
    }
    pub fn controller_id(&self) -> u64 {
        self.record
    }
    pub fn controller_gate(&self) -> u64 {
        self.raw_gate_record
    }
    pub fn controller_closed(&self) -> bool {
        self.record == 0 && self.lock.gate_closed()
    }
    pub fn controller_borrow(ram: &memory::Memory, record: u64) -> Result<Self, i32> {
        let (_, b) = gamepkg::local_record_snapshot(record, 2176)?;
        if b.len() != 2176 {
            return Err(-1);
        }
        let root = ram.call_handle();
        let services = ram.services_handle()?;
        let raw_gate_record = state_word(&b, 24)?;
        state_live(ram, root, services)?;
        state_snapshot(root, services, record, raw_gate_record)?;
        if state_word(&b, 56)? != 0 {
            return Err(-4);
        }
        Ok(Self {
            root,
            services,
            record,
            raw_gate_record,
            creator: 0,
            owned: false,
            lock: gate::Gate::gate_borrow(ram, raw_gate_record)?,
        })
    }
    pub fn controller_counts(&self, ram: &memory::Memory) -> Result<[u64; 3], i32> {
        state_live(ram, self.root, self.services)?;
        let (_, b) = state_snapshot(self.root, self.services, self.record, self.raw_gate_record)?;
        if state_word(&b, 56)? != 0 {
            return Err(-4);
        }
        Ok([
            state_word(&b, 32)?,
            state_word(&b, 40)?,
            state_word(&b, 48)?,
        ])
    }
    pub fn controller_task(&self, ram: &memory::Memory) -> Result<Task, i32> {
        state_live(ram, self.root, self.services)?;
        let task = gamepkg::local_task_status()?.0;
        for _ in 0..64u32 {
            let (version, mut b) =
                state_snapshot(self.root, self.services, self.record, self.raw_gate_record)?;
            if state_word(&b, 56)? != 0 {
                return Err(-4);
            }
            let mut empty = 33usize;
            for slot in 0..33usize {
                let at = 64 + slot * 64;
                let used = state_word(&b, at + 8)?;
                if used != 0 && state_word(&b, at)? == task {
                    if used & 8 != 0 {
                        return Err(-4);
                    }
                    return Ok(Task {
                        root: self.root,
                        services: self.services,
                        controller: self.record,
                        task,
                        raw_gate_record: self.raw_gate_record,
                        slot,
                        lock: gate::Gate::gate_borrow(ram, self.raw_gate_record)?,
                    });
                }
                if used == 0 && empty == 33 {
                    empty = slot;
                }
            }
            if empty == 33 {
                return Err(-2);
            }
            state_put(&mut b, 64 + empty * 64, task)?;
            state_put(&mut b, 72 + empty * 64, 1)?;
            match gamepkg::local_record_compare_exchange(self.record, version, &b) {
                Ok(_) => {
                    return Ok(Task {
                        root: self.root,
                        services: self.services,
                        controller: self.record,
                        task,
                        raw_gate_record: self.raw_gate_record,
                        slot: empty,
                        lock: gate::Gate::gate_borrow(ram, self.raw_gate_record)?,
                    })
                }
                Err(-2) => memory::Memory::call_retry_pause()?,
                Err(code) => return Err(code),
            }
        }
        Err(-2)
    }
    fn controller_mutex(&self) -> Result<u64, i32> {
        let (_, b) = gamepkg::local_record_snapshot(self.raw_gate_record, 64)?;
        if b.len() != 64
            || state_word(&b, 0)? != 0x31544147
            || state_word(&b, 8)? != self.root
            || state_word(&b, 16)? != self.services
        {
            return Err(-4);
        }
        Ok(state_word(&b, 24)?)
    }
    fn controller_revoke_locked(&self) -> Result<(), i32> {
        let (version, mut b) =
            state_snapshot(self.root, self.services, self.record, self.raw_gate_record)?;
        if state_word(&b, 32)? != 0 || state_word(&b, 40)? != 0 {
            return Err(-2);
        }
        for slot in 0..33usize {
            for lane in [24usize, 32, 40, 48, 56] {
                if state_word(&b, 64 + slot * 64 + lane)? != 0 {
                    return Err(-2);
                }
            }
        }
        let (_, gate_bytes) = gamepkg::local_record_snapshot(self.raw_gate_record, 64)?;
        if state_word(&gate_bytes, 40)? != 0 {
            return Err(-2);
        }
        if state_word(&b, 56)? == 0 {
            state_put(&mut b, 56, 1)?;
            gamepkg::local_record_compare_exchange(self.record, version, &b)?;
        }
        gamepkg::local_record_release(self.record)
    }
    pub fn controller_close(&mut self, ram: &memory::Memory) -> Result<(), i32> {
        if self.controller_closed() {
            return Ok(());
        }
        if !self.owned
            || gamepkg::local_task_status()?.0 != self.creator
            || ram.call_handle() != self.root
        {
            return Err(-4);
        }
        let validation = state_live(ram, self.root, self.services);
        if self.record != 0 {
            let mutex = self.controller_mutex()?;
            gamepkg::local_mutex_lock(mutex, 0)?;
            let result = self.controller_revoke_locked();
            let unlocked = gamepkg::local_call(24, &u64::to_le_bytes(mutex), &mut []);
            result?;
            self.record = 0;
            if unlocked? != 0 {
                return Err(-3);
            }
        }
        let result = self.lock.gate_close(ram);
        if self.lock.gate_closed() {
            self.raw_gate_record = 0;
            self.owned = false;
        }
        result?;
        validation?;
        Ok(())
    }
}
impl Task {
    pub fn task_key(&self) -> [u64; 2] {
        [self.controller, self.task]
    }
    fn task_snapshot(
        &self,
        ram: &memory::Memory,
        cleanup: bool,
    ) -> Result<(u64, std::vec::Vec<u8>), i32> {
        if self.slot >= 33
            || ram.call_handle() != self.root
            || gamepkg::local_task_status()?.0 != self.task
        {
            return Err(-4);
        }
        if !cleanup {
            state_live(ram, self.root, self.services)?;
        }
        let (version, b) = state_snapshot(
            self.root,
            self.services,
            self.controller,
            self.raw_gate_record,
        )?;
        if state_word(&b, 56)? != 0
            || state_word(&b, 64 + self.slot * 64 + 8)? & 1 == 0
            || state_word(&b, 64 + self.slot * 64)? != self.task
        {
            return Err(-4);
        }
        if !cleanup && state_word(&b, 72 + self.slot * 64)? & 8 != 0 {
            return Err(-4);
        }
        Ok((version, b))
    }
    pub fn task_values(&self, ram: &memory::Memory) -> Result<[u64; 6], i32> {
        let (_, b) = self.task_snapshot(ram, false)?;
        let at = 80 + self.slot * 64;
        Ok([
            state_word(&b, at)?,
            state_word(&b, at + 8)?,
            state_word(&b, at + 16)?,
            state_word(&b, at + 24)?,
            state_word(&b, at + 32)?,
            state_word(&b, at + 40)?,
        ])
    }
}
impl Task {
    fn task_commit(&self, version: u64, bytes: &[u8]) -> Result<(), i32> {
        match gamepkg::local_record_compare_exchange(self.controller, version, bytes) {
            Ok(_) => Ok(()),
            Err(code) => Err(code),
        }
    }
    fn task_set(
        &self,
        ram: &memory::Memory,
        lane: usize,
        value: u64,
        cleanup: bool,
    ) -> Result<(), i32> {
        if ![16usize, 24, 32, 40, 48, 56].contains(&lane)
            || (lane == 16 && value > 255)
            || (lane != 16 && value > 1)
        {
            return Err(-1);
        }
        for _ in 0..64u32 {
            let (version, mut b) = self.task_snapshot(ram, cleanup)?;
            state_put(&mut b, 64 + self.slot * 64 + lane, value)?;
            match self.task_commit(version, &b) {
                Ok(()) => return Ok(()),
                Err(-2) => memory::Memory::call_retry_pause()?,
                Err(code) => return Err(code),
            }
        }
        Err(-2)
    }
    fn task_counts(
        &self,
        ram: &memory::Memory,
        raised: i32,
        masked: i32,
        transition: bool,
        cleanup: bool,
    ) -> Result<(), i32> {
        if !(-1..=1).contains(&raised) || !(-1..=1).contains(&masked) {
            return Err(-1);
        }
        for _ in 0..64u32 {
            let (version, mut b) = self.task_snapshot(ram, cleanup)?;
            let status = state_word(&b, 72 + self.slot * 64)?;
            if (raised > 0 && status & 2 != 0)
                || (raised < 0 && status & 2 == 0)
                || (masked > 0 && status & 4 != 0)
                || (masked < 0 && status & 4 == 0)
            {
                return Err(-3);
            }
            let mut updated = status;
            if raised > 0 {
                updated |= 2;
            } else if raised < 0 {
                updated &= !2u64;
            }
            if masked > 0 {
                updated |= 4;
            } else if masked < 0 {
                updated &= !4u64;
            }
            state_put(&mut b, 72 + self.slot * 64, updated)?;
            let raised_value = state_word(&b, 32)? as i32 + raised;
            let masked_value = state_word(&b, 40)? as i32 + masked;
            if !(0..=33).contains(&raised_value) || !(0..=33).contains(&masked_value) {
                return Err(-3);
            }
            state_put(&mut b, 32, raised_value as u64)?;
            state_put(&mut b, 40, masked_value as u64)?;
            if transition {
                let next = (state_word(&b, 48)? as u32).wrapping_add(1);
                state_put(&mut b, 48, u64::from(next))?;
            }
            match self.task_commit(version, &b) {
                Ok(()) => return Ok(()),
                Err(-2) => memory::Memory::call_retry_pause()?,
                Err(code) => return Err(code),
            }
        }
        Err(-2)
    }
    pub fn irql_current(&self, ram: &memory::Memory) -> Result<u8, i32> {
        Ok(self.task_values(ram)?[0] as u8)
    }
    pub fn irql_blocks_dpcs(&self, ram: &memory::Memory) -> Result<bool, i32> {
        let v = self.task_values(ram)?;
        Ok(v[1] != 0 || v[2] != 0 || v[0] >= 2)
    }
    pub fn irql_set_isr(&self, ram: &memory::Memory, inside: bool) -> Result<(), i32> {
        self.task_set(ram, 48, u64::from(inside), false)
    }
    fn irql_publish(
        &mut self,
        ram: &mut memory::Memory,
        cpu: &mut crate::registers::Registers,
    ) -> Result<(), i32> {
        let v = self.task_values(ram)?;
        if cpu.fs_base != 0 {
            let address = cpu.fs_base.checked_add(0x24).ok_or(-1)?;
            ram.write_u8(address, v[0] as u8)?;
        }
        if v[5] != 0 {
            self.task_set(ram, 56, 0, false)?;
            crate::irq_hooks::release_point(self, ram, cpu)?;
        }
        Ok(())
    }
    fn irql_track(
        &mut self,
        ram: &mut memory::Memory,
        cpu: &mut crate::registers::Registers,
        level_next: u8,
        gated: bool,
    ) -> Result<(), i32> {
        let v = self.task_values(ram)?;
        let now = level_next >= 2;
        if now == (v[1] != 0) {
            return Ok(());
        }
        if !now && v[2] != 0 {
            crate::irq_hooks::safe_point(self, ram, cpu)?;
        }
        self.task_set(ram, 24, u64::from(now), false)?;
        if now && gated {
            if let Err(code) = self.lock.gate_enter(ram, 1000) {
                let _ = self.task_set(ram, 24, 0, true);
                return Err(code);
            }
            self.task_set(ram, 32, 1, false)?;
        }
        self.task_counts(ram, if now { 1 } else { -1 }, 0, true, false)?;
        if !now && v[2] != 0 {
            self.task_set(ram, 32, 0, false)?;
            crate::irq_hooks::release_window(self, ram, cpu)?;
            self.lock.gate_leave_raw(ram)?;
            if self.lock.gate_state(ram)?[1] == 0 {
                self.task_set(ram, 56, 1, false)?;
            }
        }
        Ok(())
    }
    pub fn irql_raise(
        &mut self,
        ram: &mut memory::Memory,
        cpu: &mut crate::registers::Registers,
        level_next: u8,
    ) -> Result<u8, i32> {
        let old = self.irql_current(ram)?;
        if level_next < old {
            crate::irq_hooks::warn(self, ram, 0, old, level_next)?;
        }
        self.irql_track(ram, cpu, level_next, true)?;
        self.task_set(ram, 16, u64::from(level_next), false)?;
        self.irql_publish(ram, cpu)?;
        Ok(old)
    }
    pub fn irql_lower(
        &mut self,
        ram: &mut memory::Memory,
        cpu: &mut crate::registers::Registers,
        level_next: u8,
    ) -> Result<(), i32> {
        let old = self.irql_current(ram)?;
        if level_next > old {
            crate::irq_hooks::warn(self, ram, 1, old, level_next)?;
        } else {
            self.irql_track(ram, cpu, level_next, true)?;
        }
        self.task_set(ram, 16, u64::from(level_next), false)?;
        self.irql_publish(ram, cpu)
    }
    pub fn irql_enter_interrupt(
        &mut self,
        ram: &mut memory::Memory,
        cpu: &mut crate::registers::Registers,
        level: i32,
    ) -> Result<i32, i32> {
        let saved = self.irql_current(ram)?;
        if saved != level as u8 {
            self.irql_track(ram, cpu, level as u8, level <= 2)?;
            self.task_set(ram, 16, u64::from(level as u8), false)?;
            self.irql_publish(ram, cpu)?;
        }
        Ok(i32::from(saved))
    }
    pub fn irql_leave_interrupt(
        &mut self,
        ram: &mut memory::Memory,
        cpu: &mut crate::registers::Registers,
        saved: i32,
    ) -> Result<(), i32> {
        if self.irql_current(ram)? != saved as u8 {
            self.irql_track(ram, cpu, saved as u8, true)?;
            self.task_set(ram, 16, u64::from(saved as u8), false)?;
            self.irql_publish(ram, cpu)?;
        }
        crate::irq_hooks::spin_wake(self, ram)
    }
    pub fn irql_gate_enter(&self, ram: &memory::Memory, milliseconds: u32) -> Result<(), i32> {
        self.task_snapshot(ram, false)?;
        self.lock.gate_enter(ram, milliseconds)
    }
    pub fn irql_gate_leave(
        &mut self,
        ram: &mut memory::Memory,
        cpu: &mut crate::registers::Registers,
    ) -> Result<(), i32> {
        self.task_snapshot(ram, false)?;
        crate::irq_hooks::safe_point(self, ram, cpu)?;
        self.lock.gate_leave_raw(ram)?;
        if self.lock.gate_state(ram)?[1] == 0 {
            crate::irq_hooks::release_point(self, ram, cpu)?;
        }
        Ok(())
    }
    pub fn irql_disable(
        &mut self,
        ram: &mut memory::Memory,
        cpu: &mut crate::registers::Registers,
    ) -> Result<(), i32> {
        if self.task_values(ram)?[3] == 0 {
            self.lock.gate_enter(ram, 1000)?;
            self.task_set(ram, 40, 1, false)?;
            self.task_counts(ram, 0, 1, false, false)?;
        }
        cpu.eflags &= !0x200u32;
        Ok(())
    }
    // The generated caller invokes this only after the STI following-instruction shadow.
    pub fn irql_enable_after_shadow(
        &mut self,
        ram: &mut memory::Memory,
        cpu: &mut crate::registers::Registers,
    ) -> Result<(), i32> {
        self.task_snapshot(ram, false)?;
        cpu.eflags |= 0x200u32;
        if self.task_values(ram)?[3] != 0 {
            self.task_set(ram, 40, 0, false)?;
            self.task_counts(ram, 0, -1, false, false)?;
            self.irql_gate_leave(ram, cpu)?;
        }
        Ok(())
    }
}

impl Task {
    pub fn irql_to_dpc(
        &mut self,
        ram: &mut memory::Memory,
        cpu: &mut crate::registers::Registers,
    ) -> Result<u8, i32> {
        let old = self.irql_current(ram)?;
        self.irql_track(ram, cpu, 2, true)?;
        self.task_set(ram, 16, 2, false)?;
        self.irql_publish(ram, cpu)?;
        Ok(old)
    }
    pub fn irql_interrupts_enabled(&self, ram: &memory::Memory) -> Result<bool, i32> {
        Ok(self.task_values(ram)?[3] == 0)
    }
}
impl Task {
    // Error/stop cleanup only. It never acknowledges pending IRQs or reports a
    // callback as executed. Re-entry is permanently denied for this actor.
    pub fn irql_abort(&self, ram: &memory::Memory) -> Result<(), i32> {
        let mut cleared = false;
        for _ in 0..64u32 {
            let (version, mut b) = self.task_snapshot(ram, true)?;
            let at = 64 + self.slot * 64;
            let status = state_word(&b, at + 8)?;
            let raised = state_word(&b, 32)?
                .checked_sub(u64::from(status & 2 != 0))
                .ok_or(-3)?;
            let masked = state_word(&b, 40)?
                .checked_sub(u64::from(status & 4 != 0))
                .ok_or(-3)?;
            state_put(&mut b, 32, raised)?;
            state_put(&mut b, 40, masked)?;
            state_put(&mut b, at + 8, 9)?;
            for lane in [24usize, 32, 40, 48, 56] {
                state_put(&mut b, at + lane, 0)?;
            }
            match self.task_commit(version, &b) {
                Ok(()) => {
                    cleared = true;
                    break;
                }
                Err(-2) => memory::Memory::call_retry_pause()?,
                Err(code) => return Err(code),
            }
        }
        if !cleared {
            return Err(-2);
        }
        for _ in 0..64u32 {
            let (_, b) = gamepkg::local_record_snapshot(self.raw_gate_record, 64)?;
            if b.len() != 64
                || state_word(&b, 0)? != 0x31544147
                || state_word(&b, 8)? != self.root
                || state_word(&b, 16)? != self.services
            {
                return Err(-4);
            }
            if state_word(&b, 40)? == 0 || state_word(&b, 32)? != self.task {
                return Ok(());
            }
            self.lock.gate_leave_raw(ram)?;
        }
        let (_, b) = gamepkg::local_record_snapshot(self.raw_gate_record, 64)?;
        if state_word(&b, 40)? != 0 && state_word(&b, 32)? == self.task {
            return Err(-2);
        }
        Ok(())
    }
}
