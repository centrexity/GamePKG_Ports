// Checked port IRQ coalescing and native callback composition; no host callbacks.
use crate::gamepkg;
use crate::gate;
use crate::irql;
use crate::kernel_pool;
use crate::memory;
use crate::registers;
use crate::stacks;
pub struct Interrupts {
    root: u64,
    services: u64,
    record: u64,
    controller: u64,
    gate_record: u64,
    stack_record: u64,
    creator: u64,
    owned: bool,
}
fn irq_word(b: &[u8], at: usize) -> Result<u64, i32> {
    let v = b.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7],
    ]))
}
fn irq_put(b: &mut [u8], at: usize, value: u64) -> Result<(), i32> {
    let v = b.get_mut(at..at + 8).ok_or(-1)?;
    let value = u64::to_le_bytes(value);
    for lane in 0..8usize {
        v[lane] = value[lane];
    }
    Ok(())
}
fn irq_increment(b: &mut [u8], at: usize) -> Result<(), i32> {
    let next = (irq_word(b, at)? as u32).wrapping_add(1);
    irq_put(b, at, u64::from(next))
}
fn irq_live(ram: &memory::Memory, root: u64, services: u64) -> Result<(), i32> {
    if ram.call_handle() != root || services == 0 || ram.services_handle()? != services {
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
// Xbox/Windows LONG is signed32 on both supported Windows architectures.
// Use its exact conversion on every target; wider long comparisons can narrow incorrectly.
pub fn irq_wait_maximum(now: u64, posted: u64, previous: u32) -> Result<u32, i32> {
    if previous > i32::MAX as u32 {
        return Err(-1);
    }
    let elapsed = (now.wrapping_sub(posted) / 1000) as u32 as i32;
    Ok(if elapsed > previous as i32 {
        elapsed as u32
    } else {
        previous
    })
}
impl Interrupts {
    pub fn irq_new(
        ram: &memory::Memory,
        controller: &irql::Controller,
        pool: &stacks::Pool,
        handlers: u32,
        safe: bool,
        force: bool,
        wait_ms: i32,
        prompt: bool,
    ) -> Result<Self, i32> {
        let root = ram.call_handle();
        let services = ram.services_handle()?;
        irq_live(ram, root, services)?;
        if handlers > 7 {
            return Err(-1);
        }
        if handlers & !crate::irq_routes::supported_mask() != 0 {
            return Err(-5);
        }
        controller.controller_counts(ram)?;
        pool.stacks_usage(ram)?;
        let mut b = std::vec::Vec::new();
        b.resize(384, 0u8);
        for (at, value) in [
            (0usize, 0x31515249u64),
            (8, root),
            (16, services),
            (24, controller.controller_id()),
            (32, controller.controller_gate()),
            (40, pool.stacks_id()),
            (48, u64::from(handlers)),
            (64, u64::from(safe)),
            (72, u64::from(force)),
            (80, (wait_ms as u64).wrapping_mul(1000000)),
            (88, u64::from(prompt)),
        ] {
            irq_put(&mut b, at, value)?;
        }
        let record = gamepkg::local_record_create(&b)?;
        if let Err(code) = irq_live(ram, root, services) {
            if gamepkg::local_record_release(record).is_err() {
                return Err(-3);
            }
            return Err(code);
        }
        Ok(Self {
            root,
            services,
            record,
            controller: controller.controller_id(),
            gate_record: controller.controller_gate(),
            stack_record: pool.stacks_id(),
            creator: gamepkg::local_task_status()?.0,
            owned: true,
        })
    }
    pub fn irq_id(&self) -> u64 {
        self.record
    }
    pub fn irq_closed(&self) -> bool {
        self.record == 0
    }
    fn irq_snapshot(
        &self,
        ram: &memory::Memory,
        cleanup: bool,
        closing: bool,
    ) -> Result<(u64, std::vec::Vec<u8>), i32> {
        if ram.call_handle() != self.root {
            return Err(-4);
        }
        if !cleanup {
            irq_live(ram, self.root, self.services)?;
        }
        let (version, b) = gamepkg::local_record_snapshot(self.record, 384)?;
        if b.len() != 384
            || irq_word(&b, 0)? != 0x31515249
            || irq_word(&b, 48)? > 7
            || irq_word(&b, 56)? > 7
            || irq_word(&b, 64)? > 1
            || irq_word(&b, 72)? > 1
            || irq_word(&b, 88)? > 1
            || irq_word(&b, 104)? > u32::MAX as u64
            || irq_word(&b, 112)? > 1
            || irq_word(&b, 120)? > u32::MAX as u64
            || irq_word(&b, 128)? > 33
        {
            return Err(-1);
        }
        if irq_word(&b, 8)? != self.root
            || irq_word(&b, 16)? != self.services
            || irq_word(&b, 24)? != self.controller
            || irq_word(&b, 32)? != self.gate_record
            || irq_word(&b, 40)? != self.stack_record
        {
            return Err(-4);
        }
        for at in [136usize, 144, 152, 160, 168, 176, 184] {
            if irq_word(&b, at)? != 0 {
                return Err(-1);
            }
        }
        for line in 0..3usize {
            for field in 1..7usize {
                if irq_word(&b, 192 + line * 64 + field * 8)? > u32::MAX as u64 {
                    return Err(-1);
                }
            }
            if irq_word(&b, 240 + line * 64)? > i32::MAX as u64 {
                return Err(-1);
            }
            if irq_word(&b, 248 + line * 64)? != 0 {
                return Err(-1);
            }
        }
        if !closing && irq_word(&b, 112)? != 0 {
            return Err(-4);
        }
        Ok((version, b))
    }
    pub fn irq_borrow(ram: &memory::Memory, record: u64) -> Result<Self, i32> {
        let (_, b) = gamepkg::local_record_snapshot(record, 384)?;
        if b.len() != 384 {
            return Err(-1);
        }
        let view = Self {
            root: ram.call_handle(),
            services: ram.services_handle()?,
            record,
            controller: irq_word(&b, 24)?,
            gate_record: irq_word(&b, 32)?,
            stack_record: irq_word(&b, 40)?,
            creator: 0,
            owned: false,
        };
        view.irq_snapshot(ram, false, false)?;
        let control = irql::Controller::controller_borrow(ram, view.controller)?;
        if control.controller_gate() != view.gate_record {
            return Err(-4);
        }
        stacks::Pool::stacks_borrow(ram, view.stack_record)?;
        Ok(view)
    }
    pub fn irq_attach_task(&self, ram: &memory::Memory, task: &mut irql::Task) -> Result<(), i32> {
        self.irq_snapshot(ram, false, false)?;
        if task.task_key()[0] != self.controller {
            return Err(-4);
        }
        task.irql_hook_bind(ram, self.record)
    }
    fn irq_task(&self, ram: &memory::Memory, task: &irql::Task) -> Result<(), i32> {
        self.irq_snapshot(ram, false, false)?;
        if task.task_key()[0] != self.controller || task.irql_hook_context(ram)? != self.record {
            return Err(-4);
        }
        task.task_values(ram)?;
        Ok(())
    }
    // sub_001D3F2F at 0x001d3fce: CLI, MOV [ESI], STI, PUSH EBX.
    // The pending IRQ release can run only after the PUSH writes its stack word.
    pub fn irq_thread_patch(
        &self,
        ram: &mut memory::Memory,
        task: &mut irql::Task,
        cpu: &mut registers::Registers,
    ) -> Result<u32, i32> {
        self.irq_task(ram, task)?;
        task.irql_disable(ram, cpu)?;
        ram.write_u32(cpu.esi, 0x28c5048d)?;
        let call_esp = cpu.esp;
        cpu.esp = call_esp.wrapping_sub(4);
        ram.write_u32(cpu.esp, cpu.ebx)?;
        task.irql_enable_after_shadow(ram, cpu)?;
        Ok(call_esp)
    }
    pub fn irq_pending(&self, ram: &memory::Memory) -> Result<u32, i32> {
        let (_, b) = self.irq_snapshot(ram, false, false)?;
        Ok(irq_word(&b, 56)? as u32)
    }
    fn irq_active_change(
        &self,
        ram: &memory::Memory,
        delta: i32,
        cleanup: bool,
    ) -> Result<(), i32> {
        if delta != 1 && delta != -1 {
            return Err(-1);
        }
        for _ in 0..64u32 {
            let (version, mut b) = self.irq_snapshot(ram, cleanup, false)?;
            let value = irq_word(&b, 128)? as i32 + delta;
            if !(0..=33).contains(&value) {
                return Err(-3);
            }
            irq_put(&mut b, 128, value as u64)?;
            match gamepkg::local_record_compare_exchange(self.record, version, &b) {
                Ok(_) => return Ok(()),
                Err(-2) => {
                    if !cleanup {
                        memory::Memory::call_retry_pause()?;
                    }
                }
                Err(code) => return Err(code),
            }
        }
        Err(-2)
    }
    fn irq_count(&self, ram: &memory::Memory, at: usize) -> Result<(), i32> {
        for _ in 0..64u32 {
            let (version, mut b) = self.irq_snapshot(ram, false, false)?;
            irq_increment(&mut b, at)?;
            match gamepkg::local_record_compare_exchange(self.record, version, &b) {
                Ok(_) => return Ok(()),
                Err(-2) => memory::Memory::call_retry_pause()?,
                Err(code) => return Err(code),
            }
        }
        Err(-2)
    }
    fn irq_note_wait(&self, ram: &memory::Memory, line: u32) -> Result<(), i32> {
        if line >= 3 {
            return Err(-1);
        }
        let now = crate::irq_clock::now_ns(ram)?;
        for _ in 0..64u32 {
            let (version, mut b) = self.irq_snapshot(ram, false, false)?;
            let at = 192 + line as usize * 64;
            let previous = irq_word(&b, at + 48)? as u32;
            let maximum = irq_wait_maximum(now, irq_word(&b, at)?, previous)?;
            if maximum == previous {
                return Ok(());
            }
            irq_put(&mut b, at + 48, u64::from(maximum))?;
            match gamepkg::local_record_compare_exchange(self.record, version, &b) {
                Ok(_) => return Ok(()),
                Err(-2) => memory::Memory::call_retry_pause()?,
                Err(code) => return Err(code),
            }
        }
        Err(-2)
    }
    fn irq_restore_pending(
        &self,
        ram: &memory::Memory,
        mask: u32,
        refused: bool,
    ) -> Result<(), i32> {
        if mask > 7 {
            return Err(-1);
        }
        for _ in 0..64u32 {
            let (version, mut b) = self.irq_snapshot(ram, true, false)?;
            let pending = irq_word(&b, 56)?;
            irq_put(&mut b, 56, pending | u64::from(mask))?;
            if refused {
                irq_increment(&mut b, 104)?;
            }
            match gamepkg::local_record_compare_exchange(self.record, version, &b) {
                Ok(_) => return Ok(()),
                Err(-2) => {} // Cleanup cannot depend on allocating a wait event.
                Err(code) => return Err(code),
            }
        }
        Err(-2)
    }
    fn irq_take_pending(&self, ram: &memory::Memory) -> Result<u32, i32> {
        for _ in 0..64u32 {
            let (version, mut b) = self.irq_snapshot(ram, false, false)?;
            let pending = irq_word(&b, 56)? as u32;
            if pending == 0 {
                return Ok(0);
            }
            irq_put(&mut b, 56, 0)?;
            match gamepkg::local_record_compare_exchange(self.record, version, &b) {
                Ok(_) => return Ok(pending),
                Err(-2) => memory::Memory::call_retry_pause()?,
                Err(code) => return Err(code),
            }
        }
        Err(-2)
    }
    fn irq_dispatch(
        &self,
        ram: &mut memory::Memory,
        task: &mut irql::Task,
        cpu: &mut registers::Registers,
        line: u32,
    ) -> Result<(), i32> {
        task.irql_set_isr(ram, true)?;
        task.irql_enter_interrupt(ram, cpu, 16)?;
        match line {
            0 => crate::irq_routes::vblank(self, task, ram, cpu),
            1 => crate::irq_routes::apu(self, task, ram, cpu),
            2 => crate::irq_routes::ohci(self, task, ram, cpu),
            _ => Err(-1),
        }
    }
    fn irq_run_line(
        &self,
        ram: &mut memory::Memory,
        task: &mut irql::Task,
        cpu: &mut registers::Registers,
        line: u32,
    ) -> Result<bool, i32> {
        if line >= 3 {
            return Err(-1);
        }
        self.irq_task(ram, task)?;
        let (_, b) = self.irq_snapshot(ram, false, false)?;
        if irq_word(&b, 48)? & (1u64 << line) == 0 {
            return Ok(true);
        }
        let pool = stacks::Pool::stacks_borrow(ram, self.stack_record)?;
        let mut lease = match pool.stacks_allocate(ram)? {
            Some(lease) => lease,
            None => return Ok(false),
        };
        if let Err(code) = self.irq_active_change(ram, 1, false) {
            let _ = lease.stack_release(ram);
            if !lease.stack_released() {
                return Err(-3);
            }
            return Err(code);
        }
        let prepared = self.irq_prepare_line(ram, task, cpu, line, &lease);
        let freed = lease.stack_release(ram);
        let active = self.irq_active_change(ram, -1, true);
        let result = prepared.and(freed).and(active);
        if let Err(code) = result {
            if task.irql_abort(ram).is_err() {
                return Err(-3);
            }
            return Err(code);
        }
        self.irq_count(ram, 192 + line as usize * 64 + 16)?;
        Ok(true)
    }
    fn irq_prepare_line(
        &self,
        ram: &mut memory::Memory,
        task: &mut irql::Task,
        cpu: &mut registers::Registers,
        line: u32,
        lease: &stacks::Lease,
    ) -> Result<(), i32> {
        self.irq_note_wait(ram, line)?;
        let top = lease.stack_guest_top(ram)?;
        let saved_priority = task.irql_current(ram)?;
        let saved = cpu.encode();
        cpu.irq_prepare(top);
        let called = self.irq_dispatch(ram, task, cpu, line);
        let restored = task.irql_leave_interrupt(ram, cpu, i32::from(saved_priority));
        let cleared = task.irql_set_isr(ram, false);
        let cpu_restored = cpu.irq_restore(&saved);
        called.and(restored).and(cleared).and(cpu_restored)
    }
    fn irq_run_pending(
        &self,
        ram: &mut memory::Memory,
        task: &mut irql::Task,
        cpu: &mut registers::Registers,
    ) -> Result<(), i32> {
        loop {
            let mut pending = self.irq_take_pending(ram)?;
            if pending == 0 {
                return Ok(());
            }
            for line in 0..3u32 {
                let bit = 1u32 << line;
                if pending & bit == 0 {
                    continue;
                }
                pending &= !bit;
                match self.irq_run_line(ram, task, cpu, line) {
                    Ok(true) => {}
                    Ok(false) => {
                        self.irq_restore_pending(ram, pending | bit, true)?;
                        return Ok(());
                    }
                    Err(code) => {
                        if self.irq_restore_pending(ram, pending | bit, false).is_err() {
                            return Err(-3);
                        }
                        return Err(code);
                    }
                }
            }
        }
    }
    pub fn irq_safe_point(
        &self,
        ram: &mut memory::Memory,
        task: &mut irql::Task,
        cpu: &mut registers::Registers,
    ) -> Result<(), i32> {
        self.irq_task(ram, task)?;
        if self.irq_pending(ram)? == 0 {
            return Ok(());
        }
        let lock = gate::Gate::gate_borrow(ram, self.gate_record)?;
        let owner = lock.gate_state(ram)?;
        if owner[1] == 0 || owner[0] != task.task_key()[1] {
            return Ok(());
        }
        let v = task.task_values(ram)?;
        if v[3] != 0 || v[4] != 0 || v[0] > 2 {
            self.irq_count(ram, 104)?;
            return Ok(());
        }
        self.irq_run_pending(ram, task, cpu)
    }
    pub fn irq_release_point(
        &self,
        ram: &mut memory::Memory,
        task: &mut irql::Task,
        cpu: &mut registers::Registers,
    ) -> Result<(), i32> {
        self.irq_task(ram, task)?;
        let v = task.task_values(ram)?;
        if v[3] != 0 || v[4] != 0 || v[0] > 2 {
            return Ok(());
        }
        if self.irq_pending(ram)? != 0 {
            let lock = gate::Gate::gate_borrow(ram, self.gate_record)?;
            match lock.gate_enter(ram, 0) {
                Ok(()) => {
                    let result = self.irq_run_pending(ram, task, cpu);
                    let unlocked = lock.gate_leave_raw(ram);
                    result?;
                    unlocked?;
                }
                Err(-2) => {}
                Err(code) => return Err(code),
            }
        }
        crate::irq_observer::dpc_release(self, task, ram)
    }
}
impl Interrupts {
    fn irq_clear_line(&self, ram: &memory::Memory, line: u32) -> Result<bool, i32> {
        if line >= 3 {
            return Err(-1);
        }
        let bit = 1u64 << line;
        for _ in 0..64u32 {
            let (version, mut b) = self.irq_snapshot(ram, false, false)?;
            let pending = irq_word(&b, 56)?;
            if pending & bit == 0 {
                return Ok(false);
            }
            irq_put(&mut b, 56, pending & !bit)?;
            match gamepkg::local_record_compare_exchange(self.record, version, &b) {
                Ok(_) => return Ok(true),
                Err(-2) => memory::Memory::call_retry_pause()?,
                Err(code) => return Err(code),
            }
        }
        Err(-2)
    }
    fn irq_mark_post(&self, ram: &memory::Memory, line: u32, now: u64) -> Result<bool, i32> {
        if line >= 3 {
            return Err(-1);
        }
        let bit = 1u64 << line;
        for _ in 0..64u32 {
            let (version, mut b) = self.irq_snapshot(ram, false, false)?;
            let pending = irq_word(&b, 56)?;
            if pending & bit != 0 {
                return Ok(true);
            }
            irq_put(&mut b, 56, pending | bit)?;
            irq_put(&mut b, 192 + line as usize * 64, now)?;
            irq_increment(&mut b, 200 + line as usize * 64)?;
            match gamepkg::local_record_compare_exchange(self.record, version, &b) {
                Ok(_) => return Ok(false),
                Err(-2) => memory::Memory::call_retry_pause()?,
                Err(code) => return Err(code),
            }
        }
        Err(-2)
    }
    fn irq_wait_check(&self, ram: &memory::Memory, now: u64) -> Result<(), i32> {
        for _ in 0..64u32 {
            let (version, mut b) = self.irq_snapshot(ram, false, false)?;
            let pending = irq_word(&b, 56)?;
            if pending == 0 {
                return Ok(());
            }
            let mut oldest = 0u64;
            for line in 0..3usize {
                if pending & (1u64 << line) != 0 {
                    let posted = irq_word(&b, 192 + line * 64)?;
                    if oldest == 0 || posted < oldest {
                        oldest = posted;
                    }
                }
            }
            let last = irq_word(&b, 96)?;
            if oldest == 0
                || now.wrapping_sub(oldest) < irq_word(&b, 80)?
                || (last >= oldest && now.wrapping_sub(last) < 1000000000)
            {
                return Ok(());
            }
            irq_put(&mut b, 96, now)?;
            irq_increment(&mut b, 120)?;
            let refused = irq_word(&b, 104)?;
            match gamepkg::local_record_compare_exchange(self.record, version, &b) {
                Ok(_) => {
                    let _ = crate::irq_observer::irq_wait_notice(
                        self,
                        ram,
                        pending as u32,
                        oldest,
                        now,
                        refused as u32,
                    );
                    return Ok(());
                }
                Err(-2) => memory::Memory::call_retry_pause()?,
                Err(code) => return Err(code),
            }
        }
        Err(-2)
    }
    fn irq_force_line(
        &self,
        ram: &memory::Memory,
        line: u32,
        now: u64,
        was: bool,
    ) -> Result<bool, i32> {
        if !was {
            return Ok(false);
        }
        let bit = 1u64 << line;
        for _ in 0..64u32 {
            let (version, mut b) = self.irq_snapshot(ram, false, false)?;
            if irq_word(&b, 72)? == 0
                || irq_word(&b, 56)? & bit == 0
                || now.wrapping_sub(irq_word(&b, 192 + line as usize * 64)?) < irq_word(&b, 80)?
            {
                return Ok(false);
            }
            let control = irql::Controller::controller_borrow(ram, self.controller)?;
            if control.controller_counts(ram)?[1] != 0 {
                return Ok(false);
            }
            let pending = irq_word(&b, 56)?;
            irq_put(&mut b, 56, pending & !bit)?;
            irq_increment(&mut b, 224 + line as usize * 64)?;
            match gamepkg::local_record_compare_exchange(self.record, version, &b) {
                Ok(_) => return Ok(true),
                Err(-2) => memory::Memory::call_retry_pause()?,
                Err(code) => return Err(code),
            }
        }
        Err(-2)
    }
    fn irq_post_gated(&self, ram: &memory::Memory, line: u32, second: bool) -> Result<i32, i32> {
        if second {
            if !self.irq_clear_line(ram, line)? {
                return Ok(0);
            }
            self.irq_count(ram, 216 + line as usize * 64)?;
            self.irq_note_wait(ram, line)?;
            return Ok(1);
        }
        let (_, b) = self.irq_snapshot(ram, false, false)?;
        if irq_word(&b, 64)? != 0 && self.irq_clear_line(ram, line)? {
            self.irq_note_wait(ram, line)?;
        }
        Ok(1)
    }
    fn irq_post_acquired(
        &self,
        ram: &mut memory::Memory,
        task: &mut irql::Task,
        cpu: &mut registers::Registers,
        line: u32,
        second: bool,
        lock: &gate::Gate,
    ) -> Result<i32, i32> {
        match self.irq_post_gated(ram, line, second) {
            Ok(0) => {
                task.irql_gate_leave(ram, cpu)?;
                Ok(0)
            }
            Ok(value) => Ok(value),
            Err(code) => {
                let pending = self.irq_restore_pending(ram, 1u32 << line, false);
                let unlocked = lock.gate_leave_raw(ram);
                if pending.is_err() || unlocked.is_err() {
                    return Err(-3);
                }
                Err(code)
            }
        }
    }
    // 0 Posted, 1 Gated (caller owns one additional raw hold), 2 Ungated.
    pub fn irq_post(
        &self,
        ram: &mut memory::Memory,
        task: &mut irql::Task,
        cpu: &mut registers::Registers,
        line: u32,
    ) -> Result<i32, i32> {
        if line >= 3 {
            return Err(-1);
        }
        self.irq_task(ram, task)?;
        let lock = gate::Gate::gate_borrow(ram, self.gate_record)?;
        if task.task_values(ram)?[3] == 0 {
            match lock.gate_enter(ram, 0) {
                Ok(()) => return self.irq_post_acquired(ram, task, cpu, line, false, &lock),
                Err(-2) => {}
                Err(code) => return Err(code),
            }
        }
        let (_, b) = self.irq_snapshot(ram, false, false)?;
        let control = irql::Controller::controller_borrow(ram, self.controller)?;
        if irq_word(&b, 64)? == 0 && control.controller_counts(ram)?[1] == 0 {
            return Ok(2);
        }
        crate::irq_observer::post_window(self, task, ram, cpu, line)?;
        let now = crate::irq_clock::now_ns(ram)?;
        let was = self.irq_mark_post(ram, line, now)?;
        if task.task_values(ram)?[3] == 0 {
            match lock.gate_enter(ram, 0) {
                Ok(()) => return self.irq_post_acquired(ram, task, cpu, line, true, &lock),
                Err(-2) => {}
                Err(code) => return Err(code),
            }
        }
        self.irq_wait_check(ram, now)?;
        if self.irq_force_line(ram, line, now, was)? {
            return Ok(2);
        }
        Ok(0)
    }
    pub fn irq_note_ungated(&self, ram: &memory::Memory, line: u32) -> Result<(), i32> {
        if line >= 3 {
            return Err(-1);
        }
        self.irq_snapshot(ram, false, false)?;
        let lock = gate::Gate::gate_borrow(ram, self.gate_record)?;
        if lock.gate_state(ram)?[1] != 0 {
            self.irq_count(ram, 232 + line as usize * 64)?;
        }
        Ok(())
    }
    pub fn irq_stats(&self, ram: &memory::Memory, reset: bool) -> Result<[u32; 20], i32> {
        for _ in 0..64u32 {
            let (version, mut b) = self.irq_snapshot(ram, false, false)?;
            let mut result = [0u32; 20];
            for field in 0..6usize {
                for line in 0..3usize {
                    let at = 200 + line * 64 + field * 8;
                    result[field * 3 + line] = irq_word(&b, at)? as u32;
                    if reset {
                        irq_put(&mut b, at, 0)?;
                    }
                }
            }
            result[18] = irq_word(&b, 120)? as u32;
            result[19] = irq_word(&b, 104)? as u32;
            if !reset {
                return Ok(result);
            }
            irq_put(&mut b, 120, 0)?;
            irq_put(&mut b, 104, 0)?;
            match gamepkg::local_record_compare_exchange(self.record, version, &b) {
                Ok(_) => return Ok(result),
                Err(-2) => memory::Memory::call_retry_pause()?,
                Err(code) => return Err(code),
            }
        }
        Err(-2)
    }
    fn irq_close_locked(&self, ram: &memory::Memory, discard: bool) -> Result<u32, i32> {
        for _ in 0..64u32 {
            let (version, mut b) = self.irq_snapshot(ram, true, true)?;
            let pending = irq_word(&b, 56)? as u32;
            if irq_word(&b, 128)? != 0 || (!discard && pending != 0) {
                return Err(-2);
            }
            let (_, raw) = gamepkg::local_record_snapshot(self.gate_record, 64)?;
            if irq_word(&raw, 40)? != 0 {
                return Err(-2);
            }
            if irq_word(&b, 112)? != 0 {
                gamepkg::local_record_release(self.record)?;
                return Ok(pending);
            }
            irq_put(&mut b, 112, 1)?;
            match gamepkg::local_record_compare_exchange(self.record, version, &b) {
                Ok(_) => {
                    gamepkg::local_record_release(self.record)?;
                    return Ok(pending);
                }
                Err(-2) => {} // Cleanup cannot depend on allocating a wait event.
                Err(code) => return Err(code),
            }
        }
        Err(-2)
    }
    // discard is explicit session shutdown; returned bits were cancelled, not run.
    pub fn irq_close(&mut self, ram: &memory::Memory, discard: bool) -> Result<u32, i32> {
        if ram.call_handle() != self.root || gamepkg::local_task_status()?.0 != self.creator {
            return Err(-4);
        }
        if self.record == 0 {
            return Ok(0);
        }
        if !self.owned {
            return Err(-4);
        }
        let validation = irq_live(ram, self.root, self.services);
        let (_, raw) = gamepkg::local_record_snapshot(self.gate_record, 64)?;
        if raw.len() != 64
            || irq_word(&raw, 0)? != 0x31544147
            || irq_word(&raw, 8)? != self.root
            || irq_word(&raw, 16)? != self.services
        {
            return Err(-4);
        }
        let mutex = irq_word(&raw, 24)?;
        gamepkg::local_mutex_lock(mutex, 0)?;
        let result = self.irq_close_locked(ram, discard);
        let unlocked = gamepkg::local_call(24, &u64::to_le_bytes(mutex), &mut []);
        let pending = result?;
        self.record = 0;
        self.owned = false;
        if unlocked? != 0 {
            return Err(-3);
        }
        validation?;
        Ok(pending)
    }
}
