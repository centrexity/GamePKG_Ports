use crate::controls;
use crate::gamepkg;
use crate::kernel_pool;
use crate::memory;
use crate::setup;
fn mixed_set(id: u64) -> Result<(), i32> {
    gamepkg::local_call(
        22,
        &[u64::to_le_bytes(id).as_slice(), &u32::to_le_bytes(1)].concat(),
        &mut [],
    )?;
    Ok(())
}
pub fn mixed_concurrency() -> Result<(), i32> {
    let mut f = setup::Fixture::mixed_fixture_new()?;
    for mode in 0..3u64 {
        let event = f.mixed_event(false, 0)?;
        let sem = f.mixed_semaphore(1, 2)?;
        let mut bytes = [0u8; 8];
        gamepkg::local_call(20, &[1u8, 0, 0, 0, 0, 0, 0, 0], &mut bytes)?;
        let ready = u64::from_le_bytes(bytes);
        let args = [
            u64::to_le_bytes(f.ram.call_handle()).as_slice(),
            &u64::to_le_bytes(f.table.handles_id()),
            &u64::to_le_bytes(f.events.events_id()),
            &u64::to_le_bytes(f.semaphores.semaphores_id()),
            &u64::to_le_bytes(event[1]),
            &u64::to_le_bytes(sem[1]),
            &u64::to_le_bytes(ready),
            &u64::to_le_bytes(mode),
        ]
        .concat();
        let argument = gamepkg::local_record_create(&args)?;
        gamepkg::local_call(
            10,
            &[
                u32::to_le_bytes(0).as_slice(),
                &u64::to_le_bytes(argument),
                &u32::to_le_bytes(0),
            ]
            .concat(),
            &mut bytes,
        )?;
        let task = u64::from_le_bytes(bytes);
        gamepkg::local_event_wait(ready, 1000)?;
        setup::mixed_need(
            740,
            controls::mixed_graph(f.events.events_id())?[0] == 1
                && controls::mixed_graph(f.semaphores.semaphores_id())?[0] == 1,
        )?;
        setup::mixed_need(
            741,
            f.events.events_remove(&f.ram, &f.table, event[1] as u32) == Err(-2)
                && f.semaphores
                    .semaphores_remove(&f.ram, &f.table, sem[1] as u32)
                    == Err(-2),
        )?;
        // These typed operations require the same table in another actual task.
        // They prove the worker did not retain its table guard across the host wait.
        if mode == 0 {
            f.events
                .events_set(&f.ram, &f.table, event[1] as u32, true)?;
        } else if mode == 1 {
            gamepkg::local_call(12, &u64::to_le_bytes(task), &mut [])?;
        } else {
            f.ram.services_link(f.services.service_handle(), 0)?;
            mixed_set(event[0])?;
        }
        let mut value = None;
        for _ in 0..4096u32 {
            match gamepkg::local_task_join(task) {
                Ok(proof) => {
                    value = Some(proof.result());
                    break;
                }
                Err(-2) => memory::Memory::call_retry_pause()?,
                Err(code) => return Err(code),
            }
        }
        setup::mixed_need(
            742,
            value == Some(if mode == 0 { 0 } else { (-4i64) as u64 }),
        )?;
        if mode == 2 {
            f.ram.services_link(0, f.services.service_handle())?;
        }
        setup::mixed_need(
            743,
            controls::mixed_graph(f.events.events_id())? == [0, 1]
                && controls::mixed_graph(f.semaphores.semaphores_id())? == [0, 1],
        )?;
        if mode == 0 {
            setup::mixed_need(
                744,
                !gamepkg::local_event_state(event[0])?.signaled
                    && gamepkg::local_semaphore_state(sem[0])?.count == 0,
            )?;
        }
        if mode == 1 {
            setup::mixed_need(
                745,
                !gamepkg::local_event_state(event[0])?.signaled
                    && gamepkg::local_semaphore_state(sem[0])?.count == 1,
            )?;
        }
        f.events.events_remove(&f.ram, &f.table, event[1] as u32)?;
        f.semaphores
            .semaphores_remove(&f.ram, &f.table, sem[1] as u32)?;
        gamepkg::local_record_release(argument)?;
        gamepkg::local_call(25, &u64::to_le_bytes(ready), &mut [])?;
    }
    f.mixed_fixture_close()
}
fn mixed_actor(argument: u64) -> Result<i32, i32> {
    let (_, b) = gamepkg::local_record_snapshot(argument, 64)?;
    setup::mixed_need(746, b.len() == 64)?;
    let ram = memory::Memory::with_call_handle(setup::mixed_word(&b, 0)?)?;
    let table = kernel_pool::Handles::handles_borrow(&ram, setup::mixed_word(&b, 8)?)?;
    let events = kernel_pool::Events::events_borrow(&ram, &table, setup::mixed_word(&b, 16)?)?;
    let sems =
        kernel_pool::Semaphores::semaphores_borrow(&ram, &table, setup::mixed_word(&b, 24)?)?;
    let mut mixed = crate::mixed::MixedWait::mixed_wait_new(&ram, &table, &events, &sems)?;
    mixed.mixed_wait_prepare(
        &ram,
        &[
            setup::mixed_word(&b, 32)? as u32,
            setup::mixed_word(&b, 40)? as u32,
        ],
    )?;
    mixed_set(setup::mixed_word(&b, 48)?)?;
    let result = mixed.mixed_wait(&ram, true, 1000);
    mixed.mixed_wait_close(&ram)?;
    setup::mixed_need(747, mixed.mixed_wait_closed())?;
    Ok(match result {
        Ok(value) => value.index as i32,
        Err(code) => code,
    })
}
pub fn mixed_worker(argument: u64) -> u64 {
    match mixed_actor(argument) {
        Ok(code) | Err(code) => code as i64 as u64,
    }
}
