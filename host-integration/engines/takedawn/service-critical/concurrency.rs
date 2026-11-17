use crate::gamepkg;
use crate::kernel_pool;
use crate::memory;
use crate::setup;
fn cs_set(id: u64) -> Result<(), i32> {
    gamepkg::local_call(
        22,
        &[u64::to_le_bytes(id).as_slice(), &u32::to_le_bytes(1)].concat(),
        &mut [],
    )?;
    Ok(())
}
fn cs_event() -> Result<u64, i32> {
    let mut out = [0u8; 8];
    setup::mixed_need(
        810,
        gamepkg::local_call(20, &[1u8, 0, 0, 0, 0, 0, 0, 0], &mut out)? == 8,
    )?;
    Ok(u64::from_le_bytes(out))
}
pub fn critical_concurrency() -> Result<(), i32> {
    let mut f = setup::Fixture::mixed_fixture_new()?;
    let mut map = kernel_pool::Shadow::shadow_new(&f.ram)?;
    let mut domain = kernel_pool::Criticals::criticals_new(&f.ram, &map)?;
    for mode in 0..3u64 {
        kernel_pool::critical_enter(&domain, &map, &f.ram, 0x87654321)?;
        let ready = cs_event()?;
        let gate = cs_event()?;
        let bytes = [
            u64::to_le_bytes(f.ram.call_handle()).as_slice(),
            &u64::to_le_bytes(map.shadow_id()),
            &u64::to_le_bytes(domain.criticals_id()),
            &u64::to_le_bytes(ready),
            &u64::to_le_bytes(gate),
            &u64::to_le_bytes(mode),
        ]
        .concat();
        let args = gamepkg::local_record_create(&bytes)?;
        let mut out = [0u8; 8];
        gamepkg::local_call(
            10,
            &[
                u32::to_le_bytes(0).as_slice(),
                &u64::to_le_bytes(args),
                &u32::to_le_bytes(0),
            ]
            .concat(),
            &mut out,
        )?;
        let task = u64::from_le_bytes(out);
        gamepkg::local_event_wait(ready, 1000)?;
        // Another actual task has published a lease and released the table guard.
        // Close must refuse without marking the still-usable domain closing.
        setup::mixed_need(811, domain.criticals_close(&f.ram, &map) == Err(-2))?;
        if mode == 2 {
            f.ram.services_link(f.services.service_handle(), 0)?;
            cs_set(gate)?;
        } else {
            if mode == 1 {
                gamepkg::local_call(12, &u64::to_le_bytes(task), &mut [])?;
            }
            kernel_pool::critical_leave(&domain, &map, &f.ram, 0x87654321)?;
            cs_set(gate)?;
        }
        let mut result = None;
        for _ in 0..4096u32 {
            match gamepkg::local_task_join(task) {
                Ok(proof) => {
                    result = Some(proof.result());
                    break;
                }
                Err(-2) => memory::Memory::call_retry_pause()?,
                Err(code) => return Err(code),
            }
        }
        setup::mixed_need(
            812,
            result == Some(if mode == 0 { 0 } else { (-4i64) as u64 }),
        )?;
        if mode == 2 {
            f.ram.services_link(0, f.services.service_handle())?;
            kernel_pool::critical_leave(&domain, &map, &f.ram, 0x87654321)?;
        }
        gamepkg::local_record_release(args)?;
        gamepkg::local_sync_release(ready, 1)?;
        gamepkg::local_sync_release(gate, 1)?;
    }
    setup::mixed_need(813, domain.criticals_counts(&f.ram, &map)? == [6, 4])?;
    domain.criticals_close(&f.ram, &map)?;
    map.shadow_close(&f.ram)?;
    f.mixed_fixture_close()
}
fn cs_actor(argument: u64) -> Result<i32, i32> {
    let (_, bytes) = gamepkg::local_record_snapshot(argument, 48)?;
    setup::mixed_need(814, bytes.len() == 48)?;
    let ram = memory::Memory::with_call_handle(setup::mixed_word(&bytes, 0)?)?;
    let map = kernel_pool::Shadow::shadow_borrow(&ram, setup::mixed_word(&bytes, 8)?)?;
    let domain =
        kernel_pool::Criticals::criticals_borrow(&ram, &map, setup::mixed_word(&bytes, 16)?)?;
    setup::mixed_need(
        815,
        kernel_pool::critical_leave(&domain, &map, &ram, 0x87654321) == Err(-4),
    )?;
    let mut wait = domain.criticals_wait_begin(&ram, &map, 0x87654321)?;
    wait.critical_wait_prepare(&ram)?;
    setup::mixed_need(816, wait.critical_wait_prepare(&ram) == Err(-1))?;
    for _ in 0..2u32 {
        setup::mixed_need(
            817,
            wait.critical_wait_slice(&ram, 0) == Err(-2) && wait.critical_wait_pending(),
        )?;
    }
    cs_set(setup::mixed_word(&bytes, 24)?)?;
    let _ = gamepkg::local_event_wait(setup::mixed_word(&bytes, 32)?, 1000);
    let result = wait.critical_wait_slice(&ram, 0);
    setup::mixed_need(
        818,
        !wait.critical_wait_pending() && wait.critical_wait_slice(&ram, 0) == Err(-1),
    )?;
    wait.critical_wait_close(&ram)?;
    setup::mixed_need(819, wait.critical_wait_closed())?;
    if result.is_ok() {
        kernel_pool::critical_leave(&domain, &map, &ram, 0x87654321)?;
    }
    Ok(match result {
        Ok(()) => 0,
        Err(code) => code,
    })
}
pub fn critical_worker(argument: u64) -> u64 {
    match cs_actor(argument) {
        Ok(code) | Err(code) => code as i64 as u64,
    }
}
