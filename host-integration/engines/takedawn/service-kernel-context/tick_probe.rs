// Checked integration fixtures, not title entry or a timer/DPC scheduler.
use crate::gamepkg;
use crate::kernel_data;
use crate::kernel_pool;
use crate::kernel_startup;
use crate::memory;
use crate::tick_service;
use crate::tick_startup;
fn clock_need(ok: bool) -> Result<(), i32> {
    if ok {
        Ok(())
    } else {
        Err(-3)
    }
}
fn clock_pause() -> Result<(), i32> {
    memory::Memory::call_retry_pause()
}
fn clock_finished(clock: &tick_service::Tick, ram: &memory::Memory) -> Result<(), i32> {
    for _ in 0..4096u32 {
        if clock.tick_finished(ram)? {
            return Ok(());
        }
        clock_pause()?;
    }
    Err(-2)
}
fn clock_join(
    clock: &mut tick_service::Tick,
    ram: &memory::Memory,
) -> Result<gamepkg::LocalTaskJoin, i32> {
    for _ in 0..4096u32 {
        match clock.tick_join_task(ram) {
            Err(-2) => clock_pause()?,
            result => return result,
        }
    }
    Err(-2)
}
fn clock_word(ram: &memory::Memory, data: &kernel_data::Data) -> Result<u32, i32> {
    ram.read_u32(data.data_address(ram, 156)?)
}
fn clock_observe(ram: &memory::Memory, data: &kernel_data::Data, skew: u64) -> Result<(), i32> {
    let before = gamepkg::local_performance_sample()?;
    let low = tick_service::tick_value(before.uptime_ms, skew);
    for _ in 0..4096u32 {
        let tick_snapshot = clock_word(ram, data)?;
        let after = gamepkg::local_performance_sample()?;
        let span = after.uptime_ms.checked_sub(before.uptime_ms).ok_or(-3)?;
        if span <= u64::from(u32::MAX) && u64::from(tick_snapshot.wrapping_sub(low)) <= span {
            return Ok(());
        }
        clock_pause()?;
    }
    Err(-2)
}
fn clock_unrelated() -> Result<gamepkg::LocalTaskJoin, i32> {
    let mut request = std::vec::Vec::new();
    request.extend(u32::to_le_bytes(2));
    request.extend(u64::to_le_bytes(0));
    request.extend(u32::to_le_bytes(0));
    let mut bytes = [0u8; 8];
    clock_need(gamepkg::local_call(10, &request, &mut bytes)? == 8)?;
    let task = u64::from_le_bytes(bytes);
    for _ in 0..4096u32 {
        match gamepkg::local_task_join(task) {
            Err(-2) => clock_pause()?,
            result => return result,
        }
    }
    Err(-2)
}
pub fn startup_clock_case(
    owner: &mut kernel_startup::Startup,
    ram: &mut memory::Memory,
    data: &kernel_data::Data,
    mode: u8,
) -> Result<(), i32> {
    let mut clock = tick_service::Tick::tick_new();
    let base = data.data_base(ram)?;
    let mut before = std::vec::Vec::new();
    before.resize(4096, 0u8);
    ram.read_bytes(base, &mut before)?;
    let ids = owner.startup_ids(ram)?;
    if mode == 26 || mode == 27 {
        if mode == 26 {
            clock.tick_allocate(ram, data)?;
            clock_need(
                clock.tick_task(ram)? == 0
                    && clock.tick_resume(ram) == Err(-1)
                    && clock.tick_argument(ram)? != 0,
            )?;
        } else {
            clock_need(
                tick_startup::startup_tick(owner, &mut clock, ram, data, u32::MAX) == Err(-4)
                    && clock.tick_task(ram)? == 0
                    && !clock.tick_closed(),
            )?;
        }
        clock_need(owner.startup_jobs() == 0)?;
        clock.tick_close(ram)?;
        clock.tick_close(ram)?;
        let mut after = std::vec::Vec::new();
        after.resize(4096, 0u8);
        ram.read_bytes(base, &mut after)?;
        clock_need(before == after)?;
    } else if mode == 28 || mode == 29 {
        clock.tick_allocate(ram, data)?;
        let argument = clock.tick_argument(ram)?;
        let task = owner.startup_spawn(ram, 2, argument)?;
        if mode == 28 {
            let (version, mut bytes) = gamepkg::local_record_snapshot(argument, 64)?;
            bytes[56] = 1;
            gamepkg::local_record_compare_exchange(argument, version, &bytes)?;
            clock_need(
                clock.tick_assign(ram, task) == Err(-4)
                    && clock.tick_task(ram)? == task
                    && clock.tick_resume(ram) == Err(-4),
            )?;
        } else {
            let foreign = memory::Memory::new()?;
            clock_need(
                clock.tick_argument(&foreign) == Err(-4)
                    && clock.tick_assign(&foreign, task) == Err(-4)
                    && clock.tick_task(ram)? == 0,
            )?;
            clock.tick_assign(ram, task)?;
            clock_need(
                clock.tick_resume(&foreign) == Err(-4)
                    && clock.tick_stop(&foreign) == Err(-4)
                    && clock.tick_close(&foreign) == Err(-4),
            )?;
            gamepkg::local_record_release(foreign.call_handle())?;
        }
        clock_need(
            !gamepkg::local_task_finished(task)?
                && owner.startup_jobs() == 1
                && owner.startup_close(ram) == Err(-2),
        )?;
        let mut after = std::vec::Vec::new();
        after.resize(4096, 0u8);
        ram.read_bytes(base, &mut after)?;
        clock_need(before == after)?;
        let joined = clock_join(&mut clock, ram)?;
        clock_need(joined.result() == 0 && owner.startup_close(ram) == Err(-2))?;
        owner.startup_accept_join(ram, &joined)?;
        clock.tick_close(ram)?;
        clock_need(gamepkg::local_record_snapshot(argument, 64) == Err(-4))?;
    } else {
        tick_startup::startup_tick(owner, &mut clock, ram, data, 2)?;
        let task = clock.tick_task(ram)?;
        clock_need(
            task != 0
                && owner.startup_jobs() == 1
                && !clock.tick_closed()
                && owner.startup_close(ram) == Err(-2),
        )?;
        clock_observe(ram, data, 0)?;
        kernel_pool::kernel_advance_services(ram, 7000000000)?;
        clock_observe(ram, data, 7000000000)?;
        clock_need(kernel_pool::kernel_tsc_state(ram)? == [0, 0, 0])?;
        let unrelated = clock_unrelated()?;
        clock_need(
            unrelated.result() == 4
                && clock.tick_accept_join(ram, &unrelated) == Err(-4)
                && owner.startup_accept_join(ram, &unrelated) == Err(-4)
                && clock.tick_task(ram)? == task
                && owner.startup_jobs() == 1,
        )?;
        if mode == 30 {
            ram.services_link(ids[0], 0)?;
        }
        if mode == 31 {
            gamepkg::local_call(12, &u64::to_le_bytes(task), &mut [])?;
        }
        if mode == 30 || mode == 31 {
            clock_finished(&clock, ram)?;
        } else {
            clock.tick_stop(ram)?;
            clock_finished(&clock, ram)?;
        }
        clock_need(owner.startup_close(ram) == Err(-2) && !clock.tick_closed())?;
        let joined = if mode == 25 {
            let proof = owner.startup_join_task(ram, task)?;
            // A consumed job has not yet been witnessed by the clock owner.
            clock_need(clock.tick_close(ram) == Err(-4) && !clock.tick_closed())?;
            clock.tick_accept_join(ram, &proof)?;
            proof
        } else {
            let proof = clock_join(&mut clock, ram)?;
            clock_need(owner.startup_close(ram) == Err(-2))?;
            owner.startup_accept_join(ram, &proof)?;
            proof
        };
        gamepkg::diagnostic(1, 0x544b1710, &[u64::from(mode), joined.result()])?;
        clock_need(
            joined.task_id() == task
                && joined.result() == if mode == 30 || mode == 31 { 4 } else { 0 }
                && clock.tick_task(ram)? == 0
                && owner.startup_jobs() == 0
                && clock.tick_accept_join(ram, &joined) == Err(-4)
                && owner.startup_accept_join(ram, &joined) == Err(-4),
        )?;
        let expected = if mode == 30 || mode == 31 {
            Err(-4)
        } else {
            Ok(())
        };
        clock_need(clock.tick_close(ram) == expected && clock.tick_closed())?;
        // Preserve the existing Tick one-shot terminal-error contract.
        clock.tick_close(ram)?;
        let stopped = clock_word(ram, data)?;
        clock_pause()?;
        clock_pause()?;
        clock_need(clock_word(ram, data)? == stopped)?;
        let mut after = std::vec::Vec::new();
        after.resize(4096, 0u8);
        ram.read_bytes(base, &mut after)?;
        clock_need(after[..32] == before[..32] && after[36..] == before[36..])?;
    }
    clock_need(clock.tick_closed() && owner.startup_jobs() == 0)?;
    let expected = if mode == 30 { Err(-4) } else { Ok(()) };
    clock_need(owner.startup_close(ram) == expected && owner.startup_closed())?;
    for id in ids {
        match gamepkg::local_record_snapshot(id, 4096) {
            Err(-4) => {}
            _ => return Err(-3),
        }
    }
    gamepkg::diagnostic(1, 0x544b0f00, &[u64::from(mode), 0, 5, 0])?;
    Ok(())
}
