// Checked synthetic-image startup/lifetime tests, not title routines.
use crate::calls_runtime;
use crate::gamepkg;
use crate::kernel_data;
use crate::kernel_pool;
use crate::kernel_startup;
use crate::layout;
use crate::memory;
use crate::registers;
use crate::tick_probe;
use crate::timer_probe;
use crate::xbe;
fn startup_need(ok: bool) -> Result<(), i32> {
    if ok {
        Ok(())
    } else {
        Err(-3)
    }
}
fn startup_long(bytes: &[u8], at: usize) -> Result<u64, i32> {
    let p = bytes.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        p[0], p[1], p[2], p[3], p[4], p[5], p[6], p[7],
    ]))
}
fn startup_resume(task: u64) -> Result<(), i32> {
    let mut output = [0u8; 4];
    startup_need(
        gamepkg::local_call(13, &u64::to_le_bytes(task), &mut output)? == 4
            && u32::from_le_bytes(output) == 1,
    )
}
fn startup_wait(task: u64) -> Result<(), i32> {
    let pause = gamepkg::local_mailbox_create(1, 1)?;
    let mut byte = [0u8; 1];
    while !gamepkg::local_task_finished(task)? {
        startup_need(gamepkg::local_mailbox_receive(pause, 1, &mut byte) == Err(-2))?;
    }
    gamepkg::local_mailbox_release(pause)
}
fn startup_external(argument: u64) -> Result<u64, i32> {
    let mut bytes = std::vec::Vec::new();
    bytes.extend(u32::to_le_bytes(1));
    bytes.extend(u64::to_le_bytes(argument));
    bytes.extend(u32::to_le_bytes(1));
    let mut output = [0u8; 8];
    startup_need(gamepkg::local_call(10, &bytes, &mut output)? == 8)?;
    Ok(u64::from_le_bytes(output))
}
pub fn startup_worker(argument: u64) -> u64 {
    match startup_worker_run(argument) {
        Ok(()) => 0,
        Err(code) => code as i64 as u64,
    }
}
fn startup_worker_run(argument: u64) -> Result<(), i32> {
    let (_, bytes) = gamepkg::local_record_snapshot(argument, 32)?;
    startup_need(bytes.len() == 32)?;
    let mut ids = [0u64; 4];
    for at in 0..4usize {
        ids[at] = startup_long(&bytes, at * 8)?;
    }
    let source = memory::Memory::with_context(ids)?;
    let topology = layout::Layout::layout_current(&source)?;
    let mut ram = topology.layout_view(&source)?;
    let mut cpu = registers::Registers::new();
    cpu.esp = 0x20000;
    cpu.eax = 0xdeadbeef;
    cpu.ebx = 0x17391531;
    cpu.mm7 = [0x11223344, 0x89abcdef];
    let before = cpu.encode();
    ram.write_u32(cpu.esp, 0x76543210)?;
    ram.write_u32(cpu.esp + 4, 0x1d000)?;
    calls_runtime::invoke_call(&mut cpu, &mut ram, 0xfe000010, 0x20000)?;
    let mut expected = registers::Registers::new();
    expected.decode(&before)?;
    expected.eax = 1;
    expected.esp = 0x20008;
    startup_need(cpu.encode() == expected.encode() && ram.kernel_caller() == 0x76543210)?;
    let (_, map) = gamepkg::local_record_snapshot(ram.kernel_handle(), 1548)?;
    let binding = kernel_pool::KernelContext::kernel_context_borrow(&ram, startup_long(&map, 24)?)?;
    let registry = binding.kernel_context_registry(&ram)?;
    startup_need(registry.connection_get(&ram, 3)? == 0x1d000)
}
fn startup_jobs_test(
    owner: &mut kernel_startup::Startup,
    ram: &mut memory::Memory,
) -> Result<(), i32> {
    let mut bytes = std::vec::Vec::new();
    for id in ram.thread_context() {
        bytes.extend(u64::to_le_bytes(id));
    }
    let argument = gamepkg::local_record_create(&bytes)?;
    startup_need(
        owner.startup_spawn(ram, u32::MAX, argument) == Err(-4) && owner.startup_jobs() == 0,
    )?;
    let first = owner.startup_spawn(ram, 1, argument)?;
    let second = owner.startup_spawn(ram, 1, argument)?;
    startup_need(
        owner.startup_jobs() == 2
            && owner.startup_track(ram, first) == Err(-1)
            && !gamepkg::local_task_finished(first)?,
    )?;
    match owner.startup_join_task(ram, first) {
        Err(-2) => {}
        _ => return Err(-3),
    }
    let ids = owner.startup_ids(ram)?;
    startup_need(
        owner.startup_close(ram) == Err(-2)
            && owner.startup_phase() == 2
            && owner.startup_ids(ram)? == ids,
    )?;
    let attached = startup_external(argument)?;
    owner.startup_track(ram, attached)?;
    startup_resume(attached)?;
    startup_wait(attached)?;
    let attached_join = gamepkg::local_task_join(attached)?;
    startup_need(attached_join.result() == 0)?;
    owner.startup_accept_join(ram, &attached_join)?;
    startup_need(
        owner.startup_accept_join(ram, &attached_join) == Err(-4) && owner.startup_jobs() == 2,
    )?;
    let unrelated = startup_external(argument)?;
    startup_resume(unrelated)?;
    startup_wait(unrelated)?;
    let unrelated_join = gamepkg::local_task_join(unrelated)?;
    startup_need(
        unrelated_join.result() == 0
            && owner.startup_accept_join(ram, &unrelated_join) == Err(-4)
            && owner.startup_jobs() == 2,
    )?;
    // Each worker uses the same guest argument frame, so run sequentially.
    startup_resume(first)?;
    startup_wait(first)?;
    startup_need(owner.startup_close(ram) == Err(-2) && owner.startup_ids(ram)? == ids)?;
    let first_join = owner.startup_join_task(ram, first)?;
    startup_need(first_join.result() == 0 && owner.startup_jobs() == 1)?;
    startup_resume(second)?;
    startup_wait(second)?;
    let second_join = gamepkg::local_task_join(second)?;
    // Consumed outside the owner but not yet witnessed: still not closeable.
    startup_need(second_join.result() == 0 && owner.startup_close(ram) == Err(-2))?;
    owner.startup_accept_join(ram, &second_join)?;
    startup_need(
        owner.startup_jobs() == 0 && owner.startup_accept_join(ram, &second_join) == Err(-4),
    )?;
    gamepkg::local_record_release(argument)
}
pub fn startup_image(
    ram: &mut memory::Memory,
    image: &xbe::Image,
    data: &kernel_data::Data,
    mode: u8,
    shape: [u32; 14],
) -> Result<(), i32> {
    let mut owner = kernel_startup::Startup::startup_new(ram)?;
    let mut idle_startup = if mode >= 32 {
        Some(kernel_startup::Startup::startup_new(ram)?)
    } else {
        None
    };
    startup_need(
        owner.startup_phase() == 0 && !owner.startup_closed() && owner.startup_ids(ram) == Err(-4),
    )?;
    let mut foreign = memory::Memory::new()?;
    startup_need(
        owner.startup_prepare(&mut foreign, image, data, &[], 0x10000, 2) == Err(-4)
            && owner.startup_close(&mut foreign) == Err(-4)
            && owner.startup_phase() == 0,
    )?;
    gamepkg::local_record_release(foreign.call_handle())?;
    let table = image.image_thunk();
    let mut before = std::vec::Vec::new();
    before.resize(64 + 379 * 4, 0u8);
    ram.read_bytes(table - 64, &mut before)?;
    if mode == 14 {
        ram.call_bounds(0, 0)?;
    }
    let mut held = std::vec::Vec::new();
    let mut filled = 0usize;
    if mode >= 19 && mode <= 21 || mode >= 44 && mode <= 47 {
        for _ in 0..64usize {
            match gamepkg::local_record_create(&[0u8; 8]) {
                Ok(id) => held.push(id),
                Err(-2) => break,
                Err(code) => return Err(code),
            }
        }
        filled = held.len();
        match gamepkg::local_record_create(&[0u8; 8]) {
            Err(-2) => {}
            _ => return Err(-3),
        }
        for _ in 0..usize::from(if mode >= 44 { mode - 43 } else { mode - 18 }) {
            gamepkg::local_record_release(held.pop().ok_or(-3)?)?;
        }
    }
    let expected = if mode >= 19 && mode <= 21 || mode >= 44 && mode <= 48 {
        -2
    } else if mode == 13 || mode == 23 || mode == 49 {
        -5
    } else if mode == 14 || mode == 15 || mode == 17 {
        -1
    } else {
        0
    };
    if mode == 23 || mode == 49 {
        let mut missing = std::vec::Vec::new();
        for index in 0..378u32 {
            let current = ram.read_u32(table + index * 4)?;
            if current == 0 {
                break;
            }
            if current & 0x80000000 != 0 {
                let ordinal = current & 0x7fffffff;
                if data.data_address(ram, ordinal)? == 0
                    // This startup still creates KIC1: timer initialization
                    // needs the explicit KIC2 shadow binding, even though its
                    // argument classification is now implemented.
                    && (kernel_pool::kernel_supported(ordinal).is_err() || mode == 23 && ordinal == 113)
                {
                    missing.push(u64::from(ordinal));
                    if missing.len() == 32 {
                        gamepkg::diagnostic(1, 0x544b0f10, &missing)?;
                        missing.clear();
                    }
                }
            }
        }
        if !missing.is_empty() {
            gamepkg::diagnostic(1, 0x544b0f10, &missing)?;
        }
    }
    let sync_held = if mode == 48 {
        startup_fill_sync()?
    } else {
        std::vec::Vec::new()
    };
    let prepared = if mode >= 42 {
        owner.startup_prepare_with_shadow(ram, image, data, &[], 0x10000, 2)
    } else {
        owner.startup_prepare(
            ram,
            image,
            data,
            &[],
            0x10000,
            if mode == 15 { 3 } else { 2 },
        )
    };
    if expected != 0 {
        startup_need(
            prepared == Err(expected)
                && owner.startup_phase() == 3
                && owner.startup_error() == expected
                && owner.startup_ids(ram) == Err(-4)
                && ram.kernel_handle() == 0,
        )?;
        let mut after = std::vec::Vec::new();
        after.resize(64 + 379 * 4, 0u8);
        ram.read_bytes(table - 64, &mut after)?;
        startup_need(before == after)?;
        startup_need(owner.startup_prepare(ram, image, data, &[], 0x10000, 2) == Err(-1))?;
        owner.startup_close(ram)?;
        owner.startup_close(ram)?;
        startup_need(owner.startup_closed() && ram.services_handle()? == 0)?;
        if mode >= 19 && mode <= 21 || mode >= 44 && mode <= 47 {
            for id in held {
                gamepkg::local_record_release(id)?;
            }
            let mut recovered = std::vec::Vec::new();
            for _ in 0..filled {
                recovered.push(gamepkg::local_record_create(&[0u8; 8])?);
            }
            match gamepkg::local_record_create(&[0u8; 8]) {
                Err(-2) => {}
                _ => return Err(-3),
            }
            for id in recovered {
                gamepkg::local_record_release(id)?;
            }
        }
        for id in sync_held {
            gamepkg::local_call(25, &u64::to_le_bytes(id), &mut [])?;
        }
        if mode >= 44 {
            let recovered_sync = startup_fill_sync()?;
            for id in recovered_sync {
                gamepkg::local_call(25, &u64::to_le_bytes(id), &mut [])?;
            }
        }
        gamepkg::diagnostic(
            1,
            0x544b0f00,
            &[u64::from(mode), expected as i64 as u64, 5, 0],
        )?;
        return Ok(());
    }
    prepared?;
    let ids = owner.startup_ids(ram)?;
    startup_need(
        owner.startup_phase() == 2
            && owner.startup_error() == 0
            && owner.startup_prepare(ram, image, data, &[], 0x10000, 2) == Err(-1)
            && owner.startup_ids(ram)? == ids,
    )?;
    if mode == 50 {
        let mut other_shadow = kernel_pool::Shadow::shadow_new(ram)?;
        let (version, mut changed) = gamepkg::local_record_snapshot(ids[2], 72)?;
        let original = changed.clone();
        let replacement = u64::to_le_bytes(other_shadow.shadow_id());
        for lane in 0..8usize {
            changed[32 + lane] = replacement[lane];
        }
        gamepkg::local_record_compare_exchange(ids[2], version, &changed)?;
        startup_need(
            owner.startup_ids(ram) == Err(-4)
                && owner.startup_shadow(ram).err() == Some(-4)
                && owner.startup_spawn(ram, 3, 0) == Err(-4)
                && owner.startup_jobs() == 0,
        )?;
        let (version, _) = gamepkg::local_record_snapshot(ids[2], 72)?;
        gamepkg::local_record_compare_exchange(ids[2], version, &original)?;
        other_shadow.shadow_close(ram)?;
    }
    if mode >= 32 {
        return timer_probe::startup_timer_case(
            &mut owner,
            idle_startup.as_mut().ok_or(-3)?,
            ram,
            data,
            mode,
            shape,
        );
    }
    if mode >= 24 {
        return tick_probe::startup_clock_case(&mut owner, ram, data, mode);
    }
    let mut state = registers::Registers::new();
    state.esp = 0x21000;
    ram.write_u32(state.esp, 0x12345678)?;
    let args = [0x1d000u32, 0x1100, 0x89abcdef, 3, 16, 1, 0];
    for at in 0..7usize {
        ram.write_u32(state.esp + 4 + at as u32 * 4, args[at])?;
    }
    calls_runtime::invoke_call(&mut state, ram, 0xfe00000c, 0x21000)?;
    startup_need(state.eax == 0 && state.esp == 0x21020)?;
    if mode == 12 {
        // A same-root view carrying somebody else's map cannot destroy ours.
        let other_map = gamepkg::local_record_create(&[0u8; 8])?;
        let mut wrong = memory::Memory::with_call_handle(ram.call_handle())?;
        wrong.kernel_install(other_map)?;
        startup_need(
            owner.startup_close(&mut wrong) == Err(-4)
                && !owner.startup_closed()
                && ram.kernel_handle() == ids[3],
        )?;
        wrong.kernel_clear()?;
        // Closing has begun, so the owner does not become ready again.
        startup_need(owner.startup_ids(ram) == Err(-4))?;
        owner.startup_close(ram)?;
    } else if mode == 22 {
        let (_, original) = gamepkg::local_record_snapshot(ids[3], 1548)?;
        for at in [0usize, 8, 31] {
            let (version, mut changed) = gamepkg::local_record_snapshot(ids[3], 1548)?;
            changed[at] ^= 1;
            gamepkg::local_record_compare_exchange(ids[3], version, &changed)?;
            startup_need(
                owner.startup_ids(ram) == Err(-4)
                    && owner.startup_spawn(ram, 1, 0) == Err(-4)
                    && owner.startup_jobs() == 0,
            )?;
            let (version, _) = gamepkg::local_record_snapshot(ids[3], 1548)?;
            gamepkg::local_record_compare_exchange(ids[3], version, &original)?;
            startup_need(owner.startup_ids(ram)? == ids)?;
        }
        owner.startup_close(ram)?;
    } else if mode == 18 {
        startup_jobs_test(&mut owner, ram)?;
        owner.startup_close(ram)?;
        owner.startup_close(ram)?;
    } else {
        startup_jobs_test(&mut owner, ram)?;
        ram.services_link(ids[0], 0)?;
        startup_need(
            owner.startup_ids(ram) == Err(-4)
                && owner.startup_close(ram) == Err(-4)
                && owner.startup_closed()
                && owner.startup_close(ram) == Err(-4),
        )?;
    }
    for id in ids {
        match gamepkg::local_record_snapshot(id, 4096) {
            Err(-4) => {}
            _ => return Err(-3),
        }
    }
    startup_need(
        owner.startup_closed()
            && ram.kernel_handle() == 0
            && ram.services_handle()? == 0
            && owner.startup_ids(ram) == Err(-4)
            && owner.startup_jobs() == 0,
    )?;
    if mode == 12 {
        owner.startup_close(ram)?;
    }
    gamepkg::diagnostic(1, 0x544b0f00, &[u64::from(mode), 0, 5, 0])?;
    Ok(())
}

fn startup_fill_sync() -> Result<std::vec::Vec<u64>, i32> {
    let mut held_sync = std::vec::Vec::new();
    for _ in 0..128usize {
        let mut bytes = [0u8; 8];
        startup_need(gamepkg::local_call(20, &[0u8; 8], &mut bytes)? == 8)?;
        held_sync.push(u64::from_le_bytes(bytes));
    }
    startup_need(gamepkg::local_call(20, &[0u8; 8], &mut [0u8; 8]) == Err(-2))?;
    Ok(held_sync)
}
