// Checked system-thread request preparation and lifecycle. The caller selects
// an audited Plan entry and a native body present in the compiled dispatch.
use crate::dispatch;
use crate::gamepkg;
use crate::kernel_pool;
use crate::kernel_startup;
use crate::layout;
use crate::memory;
use crate::registers;
use crate::targets;
use crate::thread_tib;

const SYSTEM_THREAD_KIND: u32 = 8;
const SYSTEM_THREAD_RECORD: u64 = 0x31525453;

pub struct ThreadLaunch {
    root: u64,
    creator: u64,
    task: u64,
    joined: Option<u64>,
    argument: u64,
    allocation: u32,
    token: u32,
    guard: Option<kernel_pool::HandleGuard>,
    published: bool,
    resumed: bool,
}

fn system_request_word(frame: &[u8], offset: usize) -> Result<u32, i32> {
    let bytes = frame.get(offset..offset + 4).ok_or(-1)?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn system_record_word(payload: &[u8], lane: usize) -> Result<u64, i32> {
    let offset = lane.checked_mul(8).ok_or(-1)?;
    let part = payload.get(offset..offset + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        part[0], part[1], part[2], part[3], part[4], part[5], part[6], part[7],
    ]))
}

impl ThreadLaunch {
    pub fn thread_launch_new(ram: &memory::Memory) -> Result<Self, i32> {
        ram.call_snapshot()?;
        let (creator, cancelled) = gamepkg::local_task_status()?;
        if cancelled {
            return Err(-4);
        }
        Ok(Self {
            root: ram.call_handle(),
            creator,
            task: 0,
            joined: None,
            argument: 0,
            allocation: 0,
            token: 0,
            guard: None,
            published: false,
            resumed: false,
        })
    }

    fn thread_check_owner(&self, ram: &memory::Memory) -> Result<(), i32> {
        if ram.call_handle() != self.root || gamepkg::local_task_status()?.0 != self.creator {
            return Err(-4);
        }
        Ok(())
    }

    fn thread_unlock(&mut self, ram: &memory::Memory) -> Result<(), i32> {
        if let Some(held) = self.guard.as_mut() {
            held.handles_guard_close(ram)?;
            self.guard = None;
        }
        Ok(())
    }

    pub fn thread_launch_frame(
        &mut self,
        owner: &mut kernel_startup::Startup,
        ram: &mut memory::Memory,
        cpu: &mut registers::Registers,
        route: [u32; 2],
        tls: [u32; 2],
    ) -> Result<(), i32> {
        self.thread_check_owner(ram)?;
        let ids = owner.startup_ids(ram)?;
        if self.argument != 0
            || self.task != 0
            || self.allocation != 0
            || self.joined.is_some()
            || gamepkg::local_task_status()?.1
            || !cpu.valid()
            || cpu.esp == 0
        {
            return Err(-4);
        }
        let next = cpu.esp.checked_add(44).ok_or(-1)?;
        let mut frame = [0u8; 44];
        ram.read_bytes(cpu.esp, &mut frame)?;
        let handle_at = system_request_word(&frame, 4)?;
        let size = system_request_word(&frame, 12)?;
        let id_at = system_request_word(&frame, 20)?;
        let suspend = system_request_word(&frame, 32)?;
        let target = system_request_word(&frame, 40)?;
        if handle_at == 0 || target == 0 || suspend > 1 {
            return Err(-1);
        }
        if system_request_word(&frame, 8)? != 0
            || system_request_word(&frame, 16)? != tls[1]
            || system_request_word(&frame, 36)? != 0
            || target != route[1]
            || targets::original_target(target) >> 24 != 1
        {
            return Err(-5);
        }
        let stack_bytes = if size == 0 { 65536 } else { size };
        if !(4096..=262144).contains(&stack_bytes) || stack_bytes & 15 != 0 {
            return Err(-5);
        }
        ram.memory_check_write(handle_at, 4)?;
        if id_at != 0 {
            ram.memory_check_write(id_at, 4)?;
            if u64::from(handle_at) < u64::from(id_at) + 4
                && u64::from(id_at) < u64::from(handle_at) + 4
            {
                return Err(-1);
            }
        }
        let tls_bytes = thread_tib::thread_tib_bytes(tls[0], tls[1])?;
        let mut system_heap = ram.heap_take()?;
        let heap_result = system_heap.heap_alloc(stack_bytes + tls_bytes, 16);
        ram.heap_put(system_heap);
        self.allocation = heap_result?;
        if self.allocation == 0 {
            return Err(-2);
        }
        let stack_top = self.allocation.checked_add(stack_bytes).ok_or(-1)?;
        thread_tib::thread_tib_apply(ram, stack_top, tls[0], tls[1])?;
        let context = ram.thread_context();
        let mut launch = std::vec::Vec::new();
        for item in [
            SYSTEM_THREAD_RECORD,
            0,
            context[0],
            context[1],
            context[2],
            context[3],
            ids[2],
            u64::from(target),
            u64::from(stack_top),
            u64::from(system_request_word(&frame, 24)?),
            u64::from(system_request_word(&frame, 28)?),
        ] {
            launch.extend(u64::to_le_bytes(item));
        }
        self.argument = gamepkg::local_record_create(&launch)?;
        self.task = owner.startup_spawn(ram, route[0], self.argument)?;
        let (version, _) = gamepkg::local_record_snapshot(self.argument, 88)?;
        let bound = u64::to_le_bytes(self.task);
        for index in 0..8 {
            launch[8 + index] = bound[index];
        }
        gamepkg::local_record_compare_exchange(self.argument, version, &launch)?;

        let table = owner.startup_thread_table(ram)?;
        self.guard = Some(table.handles_lock(ram)?);
        self.token = self.guard.as_ref().ok_or(-3)?.handle_guard_insert(
            ram,
            self.task,
            SYSTEM_THREAD_KIND,
            true,
        )?;
        self.thread_unlock(ram)?;
        let token_bytes = u32::to_le_bytes(self.token);
        let guest_id = u32::to_le_bytes(system_record_value(&launch, 1)?);
        if id_at == 0 {
            ram.write_bytes(handle_at, &token_bytes)?;
        } else {
            ram.memory_write_disjoint([(handle_at, &token_bytes), (id_at, &guest_id), (0, &[])])?;
        }
        self.published = true;
        if suspend == 0 {
            self.thread_resume(owner, ram, self.token)?;
        }
        ram.kernel_set_caller(system_request_word(&frame, 0)?);
        cpu.esp = next;
        cpu.eax = 0;
        Ok(())
    }

    pub fn thread_resume(
        &mut self,
        owner: &kernel_startup::Startup,
        ram: &memory::Memory,
        token: u32,
    ) -> Result<(), i32> {
        self.thread_check_owner(ram)?;
        if !self.published || token != self.token || self.resumed || self.joined.is_some() {
            return Err(-4);
        }
        let table = owner.startup_thread_table(ram)?;
        self.guard = Some(table.handles_lock(ram)?);
        let identity = self
            .guard
            .as_ref()
            .ok_or(-3)?
            .handle_guard_lookup(ram, token);
        self.thread_unlock(ram)?;
        if identity? != Some([self.task, u64::from(SYSTEM_THREAD_KIND)]) {
            return Err(-4);
        }
        let mut resume = [0u8; 4];
        if gamepkg::local_call(13, &u64::to_le_bytes(self.task), &mut resume)? != 4
            || u32::from_le_bytes(resume) != 1
        {
            return Err(-3);
        }
        self.resumed = true;
        Ok(())
    }

    // A suspended or unpublished job is cancelled before joining. A running
    // job must finish before stack, TIB, argument or handle storage is freed.
    pub fn thread_drain(
        &mut self,
        owner: &mut kernel_startup::Startup,
        ram: &mut memory::Memory,
    ) -> Result<u64, i32> {
        self.thread_check_owner(ram)?;
        self.thread_unlock(ram)?;
        if self.task != 0 && self.joined.is_none() {
            if !self.resumed {
                gamepkg::local_call(12, &u64::to_le_bytes(self.task), &mut [])?;
            }
            let joined = owner.startup_join_task(ram, self.task)?;
            self.joined = Some(joined.result());
        }
        if self.token != 0 {
            let table = owner.startup_thread_table(ram)?;
            self.guard = Some(table.handles_cleanup_lock(ram)?);
            let held = self.guard.as_ref().ok_or(-3)?;
            if held.handle_guard_lookup(ram, self.token)?
                != Some([self.task, u64::from(SYSTEM_THREAD_KIND)])
            {
                return Err(-4);
            }
            if held.handle_guard_remove(ram, self.token)?
                != Some([self.task, u64::from(SYSTEM_THREAD_KIND)])
            {
                return Err(-3);
            }
            self.token = 0;
            self.thread_unlock(ram)?;
        }
        if self.argument != 0 {
            gamepkg::local_record_release(self.argument)?;
            self.argument = 0;
        }
        if self.allocation != 0 {
            let mut system_heap = ram.heap_take()?;
            let released = system_heap.heap_release_block(self.allocation);
            ram.heap_put(system_heap);
            released?;
            self.allocation = 0;
        }
        let result = self.joined.unwrap_or(0);
        Ok(result)
    }
}

pub fn system_thread_execute(argument: u64) -> Result<u32, i32> {
    let (_, record) = gamepkg::local_record_snapshot(argument, 88)?;
    let (actor, cancelled) = gamepkg::local_task_status()?;
    if cancelled
        || actor == 0
        || record.len() != 88
        || system_record_word(&record, 0)? != SYSTEM_THREAD_RECORD
        || system_record_word(&record, 1)? != actor
    {
        return Err(-4);
    }
    let source = memory::Memory::with_context([
        system_record_word(&record, 2)?,
        system_record_word(&record, 3)?,
        system_record_word(&record, 4)?,
        system_record_word(&record, 5)?,
    ])?;
    let view = layout::Layout::layout_services(&source)?;
    let mut ram = view.layout_view(&source)?;
    let bound =
        kernel_pool::KernelContext::kernel_context_borrow(&ram, system_record_word(&record, 6)?)?;
    bound.kernel_context_registry(&ram)?;
    let target = system_record_value(&record, 7)?;
    if targets::original_target(target) >> 24 != 1 {
        return Err(-5);
    }
    let stack_top = system_record_value(&record, 8)?;
    let first = system_record_value(&record, 9)?;
    let second = system_record_value(&record, 10)?;
    let mut cpu = registers::Registers::new();
    cpu.esp = stack_top.checked_sub(12).ok_or(-1)?;
    cpu.fs_base = stack_top;
    let call_frame = [
        u32::to_le_bytes(0).as_slice(),
        &u32::to_le_bytes(first),
        &u32::to_le_bytes(second),
    ]
    .concat();
    ram.write_bytes(cpu.esp, &call_frame)?;
    let outcome = dispatch::invoke(&mut cpu, &mut ram, target);
    let close = ram.heap_close();
    outcome?;
    close?;
    if cpu.esp != stack_top {
        return Err(-3);
    }
    Ok(cpu.eax)
}

fn system_record_value(payload: &[u8], lane: usize) -> Result<u32, i32> {
    let value = system_record_word(payload, lane)?;
    if value > u64::from(u32::MAX) {
        return Err(-1);
    }
    Ok(value as u32)
}

pub fn system_thread_fixture_entry(argument: u64) -> u64 {
    match system_thread_execute(argument) {
        Ok(exit) => u64::from(exit),
        Err(status) => status as i64 as u64,
    }
}

fn system_fixture_need(phase: i32, observed: bool) -> Result<(), i32> {
    if observed {
        Ok(())
    } else {
        Err(-phase)
    }
}

fn system_fixture_drain(
    launch: &mut ThreadLaunch,
    owner: &mut kernel_startup::Startup,
    ram: &mut memory::Memory,
) -> Result<u64, i32> {
    for _ in 0..4096 {
        match launch.thread_drain(owner, ram) {
            Ok(value) => return Ok(value),
            Err(-2) => memory::Memory::call_retry_pause()?,
            Err(status) => return Err(status),
        }
    }
    Err(-2)
}

pub fn system_thread_fixture(
    owner: &mut kernel_startup::Startup,
    ram: &mut memory::Memory,
) -> Result<(), i32> {
    let target = 0x155c0;
    let route = [8, target];
    let parent_stack = ram.read_u32(0x4008)?.checked_add(0x6000).ok_or(-1)?;
    let output = parent_stack + 64;
    let thread_id = output + 4;
    let template = parent_stack + 256;
    let tls = [template, 148];
    ram.fill(template, 0x5c, 148)?;
    ram.write_u32(output, 0xfaabcc13)?;
    ram.write_u32(thread_id, 0xfafef00d)?;
    let arguments = [
        0x0e000432, output, 0, 65536, 148, thread_id, 0x13572468, 0x24681357, 1, 0, target,
    ];
    for (index, argument_value) in arguments.iter().enumerate() {
        ram.write_u32(parent_stack + index as u32 * 4, *argument_value)?;
    }
    let job_count = owner.startup_jobs();
    let mut launch = ThreadLaunch::thread_launch_new(ram)?;
    let mut cpu = registers::Registers::new();
    cpu.esp = parent_stack;
    cpu.eax = 0x977d12f3;
    cpu.ebx = 0x2057beef;
    // A guest-selected target outside the audited route fails before spawn.
    ram.write_u32(parent_stack + 40, target + 1)?;
    system_fixture_need(
        1060,
        launch.thread_launch_frame(owner, ram, &mut cpu, route, tls) == Err(-5)
            && owner.startup_jobs() == job_count
            && ram.read_u32(output)? == 0xfaabcc13
            && cpu.esp == parent_stack
            && cpu.eax == 0x977d12f3,
    )?;
    ram.write_u32(parent_stack + 40, target)?;
    launch.thread_launch_frame(owner, ram, &mut cpu, route, tls)?;
    let token = ram.read_u32(output)?;
    let job = u64::from(ram.read_u32(thread_id)?);
    let tib = launch.allocation + 65536;
    system_fixture_need(
        1061,
        token == launch.token
            && job == launch.task
            && owner.startup_jobs() == job_count + 1
            && !gamepkg::local_task_finished(job)?
            && cpu.esp == parent_stack + 44
            && cpu.eax == 0
            && cpu.ebx == 0x2057beef
            && ram.kernel_caller() == 0x0e000432
            && ram.read_u32(tib)? == u32::MAX
            && ram.read_u32(tib + 4)? == tib + 64 + 148
            && ram.read_u8(tib + 68)? == 0x5c
            && ram.read_u32(tib + 64 + 148)? == 0,
    )?;
    system_fixture_need(1062, launch.thread_resume(owner, ram, token + 1) == Err(-4))?;
    launch.thread_resume(owner, ram, token)?;
    system_fixture_need(
        1063,
        launch.thread_resume(owner, ram, token) == Err(-4)
            && owner.startup_close(&mut ram.call_view()) == Err(-2),
    )?;
    let exit = system_fixture_drain(&mut launch, owner, ram)?;
    system_fixture_need(
        1064,
        exit == 1
            && gamepkg::local_task_finished(job) == Err(-4)
            && owner.startup_jobs() == job_count
            && launch.allocation == 0
            && launch.argument == 0
            && launch.token == 0,
    )?;
    // The generated body returns with stdcall cleanup. Both guest parameters
    // were written to its inherited stack before native dispatch.
    system_fixture_need(
        1065,
        ram.read_u32(tib - 8)? == 0x13572468 && ram.read_u32(tib - 4)? == 0x24681357,
    )?;
    // Reproduce the owned start request's unsuspended flag. The only
    // generated body used here is the reviewed short stdcall fixture route.
    let mut immediate = ThreadLaunch::thread_launch_new(ram)?;
    cpu.esp = parent_stack;
    ram.write_u32(parent_stack + 32, 0)?;
    immediate.thread_launch_frame(owner, ram, &mut cpu, route, tls)?;
    system_fixture_need(
        1066,
        immediate.resumed && system_fixture_drain(&mut immediate, owner, ram)? == 1,
    )?;

    let mut cancelled = ThreadLaunch::thread_launch_new(ram)?;
    cpu.esp = parent_stack;
    ram.write_u32(parent_stack + 32, 1)?;
    cancelled.thread_launch_frame(owner, ram, &mut cpu, route, tls)?;
    let suspended_job = cancelled.task;
    system_fixture_need(
        1067,
        system_fixture_drain(&mut cancelled, owner, ram)? == 0
            && gamepkg::local_task_finished(suspended_job) == Err(-4)
            && owner.startup_jobs() == job_count,
    )?;
    gamepkg::diagnostic(1, 0x544b4c00, &[255, 65536, 148, 3])?;
    Ok(())
}
