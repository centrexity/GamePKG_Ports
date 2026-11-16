use crate::gamepkg;
use crate::kernel_pool;
use crate::memory;
use crate::queue;
use crate::shared;
use crate::timers;
fn join_need(value: bool) -> Result<(), i32> {
    if value {
        Ok(())
    } else {
        Err(-3)
    }
}
fn join_word(bytes: &[u8], at: usize) -> Result<u64, i32> {
    let b = bytes.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}
pub fn join_worker(value: u64) -> u64 {
    value
}
fn join_spawn(value: u64, suspended: bool) -> Result<u64, i32> {
    let mut out = [0u8; 8];
    if gamepkg::local_call(
        10,
        &[
            u32::to_le_bytes(0).as_slice(),
            &u64::to_le_bytes(value),
            &u32::to_le_bytes(u32::from(suspended)),
        ]
        .concat(),
        &mut out,
    )? != 8
    {
        return Err(-3);
    }
    join_word(&out, 0)
}
fn join_finished(job: u64) -> Result<(), i32> {
    let start = gamepkg::local_performance_sample()?;
    while !gamepkg::local_task_finished(job)? {
        let now = gamepkg::local_performance_sample()?;
        if now.frequency != start.frequency
            || now.counter < start.counter
            || now.counter - start.counter > start.frequency * 3
        {
            return Err(-2);
        }
    }
    Ok(())
}
pub fn join_probe() -> Result<std::vec::Vec<u8>, i32> {
    let mut output = std::vec::Vec::new();
    for mode in 0..3u32 {
        let ram = memory::Memory::new()?;
        let services = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2)?;
        let mut first = queue::Queue::queue_new(&ram, false)?;
        let mut second = queue::Queue::queue_new(&ram, false)?;
        let mut schedule = shared::Schedule::schedule_new(&ram, -1)?;
        let mut table = timers::Timers::timers_new(&ram, &first)?;
        let expected = 0xfedcba9876543200u64 + u64::from(mode);
        let job = join_spawn(expected, true)?;
        first.queue_assign(&ram, job)?;
        second.queue_assign(&ram, job)?;
        schedule.schedule_assign(&ram, job)?;
        table.timers_start(&ram, &first)?;
        join_need(
            gamepkg::local_task_join(job).err() == Some(-2)
                && !gamepkg::local_task_finished(job)?
                && first.queue_close(&ram) == Err(-2)
                && schedule.schedule_close(&ram) == Err(-2)
                && table.timers_close(&ram, None) == Err(-2),
        )?;
        let unrelated = join_spawn(37, false)?;
        join_finished(unrelated)?;
        let wrong = gamepkg::local_task_join(unrelated)?;
        join_need(
            first.queue_accept_join(&ram, &wrong) == Err(-4)
                && second.queue_accept_join(&ram, &wrong) == Err(-4)
                && schedule.schedule_accept_join(&ram, &wrong) == Err(-4),
        )?;
        let mut resumed = [0u8; 4];
        gamepkg::local_call(13, &u64::to_le_bytes(job), &mut resumed)?;
        join_finished(job)?;
        // A true completion snapshot cannot replace an actual consuming join.
        join_need(first.queue_close(&ram) == Err(-2) && schedule.schedule_close(&ram) == Err(-2))?;
        let proof = match mode {
            0 => first.queue_join_task(&ram)?,
            1 => schedule.schedule_join_task(&ram)?,
            _ => gamepkg::local_task_join(job)?,
        };
        join_need(
            proof.task_id() == job
                && proof.result() == expected
                && gamepkg::local_task_join(job).err() == Some(-4),
        )?;
        let mut borrowed = queue::Queue::queue_borrow(&ram, first.queue_id())?;
        let mut viewed = shared::Schedule::schedule_borrow(&ram, schedule.schedule_id())?;
        join_need(
            borrowed.queue_accept_join(&ram, &proof) == Err(-4)
                && viewed.schedule_accept_join(&ram, &proof) == Err(-4),
        )?;
        let other = memory::Memory::new()?;
        join_need(
            first.queue_accept_join(&other, &proof) == Err(-4)
                && schedule.schedule_accept_join(&other, &proof) == Err(-4),
        )?;
        gamepkg::local_record_release(other.call_handle())?;
        if mode == 2 {
            services.service_close(&ram)?;
        }
        if mode != 0 {
            first.queue_accept_join(&ram, &proof)?;
        }
        if mode != 1 {
            schedule.schedule_accept_join(&ram, &proof)?;
        }
        second.queue_accept_join(&ram, &proof)?;
        join_need(
            first.queue_accept_join(&ram, &proof) == Err(-1)
                && schedule.schedule_accept_join(&ram, &proof) == Err(-1)
                && first.queue_join(&ram) == Err(-1)
                && schedule.schedule_join(&ram) == Err(-1),
        )?;
        let queue_proof = first.queue_join_witness(&ram)?;
        let first_closed = first.queue_close(&ram);
        let table_closed = table.timers_close(&ram, Some(&queue_proof));
        let second_closed = second.queue_close(&ram);
        let schedule_release_result = schedule.schedule_close(&ram);
        if mode == 2 {
            join_need(
                first_closed == Err(-4)
                    && second_closed == Err(-4)
                    && table_closed == Err(-4)
                    && schedule_release_result == Err(-4),
            )?;
        } else {
            first_closed?;
            second_closed?;
            table_closed?;
            schedule_release_result?;
            services.service_close(&ram)?;
        }
        join_need(
            first.queue_closed()
                && second.queue_closed()
                && table.timers_closed()
                && schedule.schedule_closed(),
        )?;
        output.extend(u64::to_le_bytes(u64::from(mode)));
        output.extend(u64::to_le_bytes(proof.result()));
        gamepkg::local_record_release(ram.call_handle())?;
    }
    Ok(output)
}
