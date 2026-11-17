use crate::gamepkg;
use crate::kernel_pool;
use crate::memory;
use crate::setup;
pub fn mixed_graph(domain: u64) -> Result<[u32; 2], i32> {
    let (_, bytes) = gamepkg::local_record_snapshot(domain, 2624)?;
    let mut count = [0u32; 2];
    for slot in 0..32usize {
        if setup::mixed_word(&bytes, 2120 + slot * 16)? == 1 {
            let id = setup::mixed_word(&bytes, 2112 + slot * 16)?;
            let (_, lease) = gamepkg::local_record_snapshot(id, 80)?;
            let state = setup::mixed_word(&lease, 56)?;
            setup::mixed_need(720, state <= 1)?;
            count[state as usize] += 1;
        }
    }
    Ok(count)
}
fn mixed_empty(f: &setup::Fixture) -> Result<(), i32> {
    setup::mixed_need(
        721,
        mixed_graph(f.events.events_id())? == [0, 0]
            && mixed_graph(f.semaphores.semaphores_id())? == [0, 0],
    )
}
fn mixed_unready(f: &setup::Fixture) -> Result<crate::mixed::MixedWait, i32> {
    crate::mixed::MixedWait::mixed_wait_new(&f.ram, &f.table, &f.events, &f.semaphores)
}
pub fn mixed_controls() -> Result<(), i32> {
    mixed_slices()?;
    let mut f = setup::Fixture::mixed_fixture_new()?;
    let event = f.mixed_event(false, 1)?;
    let sem = f.mixed_semaphore(1, 3)?;
    let alias = f
        .events
        .events_alias(&f.ram, &f.table, event[1] as u32, true)?;
    {
        let mut wrong = memory::Memory::new()?;
        let mut lease = mixed_unready(&f)?;
        setup::mixed_need(
            722,
            lease.mixed_wait_prepare(&wrong, &[event[1] as u32]) == Err(-4)
                && lease.mixed_wait_close(&wrong) == Err(-4),
        )?;
        setup::mixed_need(
            723,
            lease.mixed_wait_prepare(&f.ram, &[]) == Err(-1)
                && lease.mixed_wait_prepare(&f.ram, &[event[1] as u32; 65]) == Err(-1),
        )?;
        lease.mixed_wait_close(&f.ram)?;
        setup::mixed_need(724, lease.mixed_wait_close(&wrong) == Err(-4))?;
        wrong.kernel_clear()?;
        gamepkg::local_record_release(wrong.call_handle())?;
    }
    for tokens in [
        [event[1] as u32, alias],
        [sem[1] as u32, sem[1] as u32],
        [event[1] as u32, 0],
        [event[1] as u32, 0xdead0001],
        [event[1] as u32, 0xffffffff],
    ] {
        let mut lease = mixed_unready(&f)?;
        setup::mixed_need(725, lease.mixed_wait_prepare(&f.ram, &tokens).is_err())?;
        mixed_empty(&f)?;
        lease.mixed_wait_close(&f.ram)?;
    }
    let mut guard = f.table.handles_lock(&f.ram)?;
    let unknown = guard.handle_guard_insert(&f.ram, event[0], 2, true)?;
    let forged_sem = guard.handle_guard_insert(&f.ram, event[0], 3, true)?;
    guard.handles_guard_close(&f.ram)?;
    {
        let mut lease = mixed_unready(&f)?;
        setup::mixed_need(726, lease.mixed_wait_prepare(&f.ram, &[unknown]) == Err(-5))?;
        lease.mixed_wait_close(&f.ram)?;
        mixed_empty(&f)?;
    }
    let (_, pristine) = gamepkg::local_record_snapshot(f.semaphores.semaphores_id(), 2624)?;
    let mut changed = pristine.clone();
    changed[64..72].copy_from_slice(&u64::to_le_bytes(event[0]));
    let (version, _) = gamepkg::local_record_snapshot(f.semaphores.semaphores_id(), 2624)?;
    gamepkg::local_record_compare_exchange(f.semaphores.semaphores_id(), version, &changed)?;
    {
        let mut lease = mixed_unready(&f)?;
        setup::mixed_need(
            727,
            lease.mixed_wait_prepare(&f.ram, &[event[1] as u32, forged_sem]) == Err(-1),
        )?;
        lease.mixed_wait_close(&f.ram)?;
        mixed_empty(&f)?;
    }
    let (version, _) = gamepkg::local_record_snapshot(f.semaphores.semaphores_id(), 2624)?;
    gamepkg::local_record_compare_exchange(f.semaphores.semaphores_id(), version, &pristine)?;
    let mut guard = f.table.handles_lock(&f.ram)?;
    guard.handle_guard_remove(&f.ram, unknown)?;
    guard.handle_guard_remove(&f.ram, forged_sem)?;
    guard.handles_guard_close(&f.ram)?;
    {
        let mut foreign = kernel_pool::Events::events_new(&f.ram, &f.table)?;
        let mut lease =
            crate::mixed::MixedWait::mixed_wait_new(&f.ram, &f.table, &foreign, &f.semaphores)?;
        setup::mixed_need(
            728,
            lease.mixed_wait_prepare(&f.ram, &[event[1] as u32, sem[1] as u32]) == Err(-4),
        )?;
        lease.mixed_wait_close(&f.ram)?;
        foreign.events_close(&f.ram, &f.table)?;
        mixed_empty(&f)?;
    }
    f.events.events_remove(&f.ram, &f.table, alias)?;
    {
        let mut lease = mixed_unready(&f)?;
        lease.mixed_wait_prepare(&f.ram, &[event[1] as u32, sem[1] as u32])?;
        let ksv = f.services.service_handle();
        f.ram.services_link(ksv, 0)?;
        setup::mixed_need(
            729,
            lease.mixed_wait(&f.ram, true, 0) == Err(-4)
                && lease.mixed_wait_native_result() == Err(-1),
        )?;
        lease.mixed_wait_close(&f.ram)?;
        f.ram.services_link(0, ksv)?;
        setup::mixed_need(
            730,
            gamepkg::local_event_state(event[0])?.signaled
                && gamepkg::local_semaphore_state(sem[0])?.count == 1,
        )?;
    }
    f.events.events_remove(&f.ram, &f.table, event[1] as u32)?;
    f.semaphores
        .semaphores_remove(&f.ram, &f.table, sem[1] as u32)?;
    mixed_empty(&f)?;
    mixed_guard_retirement(&f)?;
    mixed_exhaustion(&mut f)?;
    mixed_slot_bound(&mut f)?;
    f.mixed_fixture_close()
}
fn mixed_exhaustion(f: &mut setup::Fixture) -> Result<(), i32> {
    let event = f.mixed_event(false, 1)?;
    let sem = f.mixed_semaphore(1, 2)?;
    let mut fillers = [0u64; 64];
    let mut used = 0usize;
    for _ in 0..64usize {
        match gamepkg::local_record_create(&[0u8; 8]) {
            Ok(id) => {
                fillers[used] = id;
                used += 1;
            }
            Err(-2) => break,
            Err(code) => return Err(code),
        }
    }
    setup::mixed_need(731, used > 1)?;
    used -= 1;
    gamepkg::local_record_release(fillers[used])?;
    let mut lease = mixed_unready(f)?;
    setup::mixed_need(
        732,
        lease.mixed_wait_prepare(&f.ram, &[event[1] as u32, sem[1] as u32]) == Err(-2),
    )?;
    setup::mixed_need(
        733,
        mixed_graph(f.events.events_id())? == [1, 0]
            && mixed_graph(f.semaphores.semaphores_id())? == [0, 0]
            && !lease.mixed_wait_closed(),
    )?;
    setup::mixed_need(
        734,
        f.events.events_remove(&f.ram, &f.table, event[1] as u32) == Err(-2)
            && gamepkg::local_event_state(event[0])?.signaled
            && gamepkg::local_semaphore_state(sem[0])?.count == 1,
    )?;
    lease.mixed_wait_close(&f.ram)?;
    setup::mixed_need(
        735,
        lease.mixed_wait_closed() && lease.mixed_wait_native_result() == Err(-1),
    )?;
    for id in &fillers[..used] {
        gamepkg::local_record_release(*id)?;
    }
    f.events.events_remove(&f.ram, &f.table, event[1] as u32)?;
    f.semaphores
        .semaphores_remove(&f.ram, &f.table, sem[1] as u32)?;
    mixed_empty(f)
}
fn mixed_slot_bound(f: &mut setup::Fixture) -> Result<(), i32> {
    let mut items = [[0u64; 2]; 33];
    let mut tokens = [0u32; 33];
    for at in 0..33usize {
        items[at] = f.mixed_event(false, 0)?;
        tokens[at] = items[at][1] as u32;
    }
    let mut lease = mixed_unready(f)?;
    setup::mixed_need(
        736,
        lease.mixed_wait_prepare(&f.ram, &tokens) == Err(-2)
            && mixed_graph(f.events.events_id())? == [32, 0],
    )?;
    lease.mixed_wait_close(&f.ram)?;
    setup::mixed_need(
        737,
        lease.mixed_wait_closed() && mixed_graph(f.events.events_id())? == [0, 32],
    )?;
    for item in items {
        f.events.events_remove(&f.ram, &f.table, item[1] as u32)?;
    }
    mixed_empty(f)
}

pub fn mixed_completion(domain: u64, native: u64, outcome: i32) -> Result<(), i32> {
    let (_, bytes) = gamepkg::local_record_snapshot(domain, 2624)?;
    let mut found = 0u32;
    for slot in 0..32usize {
        if setup::mixed_word(&bytes, 2120 + slot * 16)? == 1 {
            let id = setup::mixed_word(&bytes, 2112 + slot * 16)?;
            let (_, lease) = gamepkg::local_record_snapshot(id, 80)?;
            if setup::mixed_word(&lease, 40)? == native {
                setup::mixed_need(
                    748,
                    setup::mixed_word(&lease, 56)? == 1
                        && setup::mixed_word(&lease, 64)? == outcome as i64 as u64,
                )?;
                found += 1;
            }
        }
    }
    setup::mixed_need(749, found == 1)
}

fn mixed_guard_retirement(f: &setup::Fixture) -> Result<(), i32> {
    // Synthetic damaged-count controls in ordinary checked game code. Restore the
    // fixture's known accounting entry before retry; this is not a forged lease.
    let mut guard = f.table.handles_lock(&f.ram)?;
    let (version, mut bytes) = gamepkg::local_record_snapshot(f.table.handles_id(), 80)?;
    setup::mixed_need(750, setup::mixed_word(&bytes, 32)? == 1)?;
    bytes[32..40].copy_from_slice(&u64::to_le_bytes(0));
    gamepkg::local_record_compare_exchange(f.table.handles_id(), version, &bytes)?;
    setup::mixed_need(
        751,
        guard.handles_guard_close(&f.ram) == Err(-3) && !guard.handles_guard_closed(),
    )?;
    let (version, mut bytes) = gamepkg::local_record_snapshot(f.table.handles_id(), 80)?;
    bytes[32..40].copy_from_slice(&u64::to_le_bytes(1));
    gamepkg::local_record_compare_exchange(f.table.handles_id(), version, &bytes)?;
    let mut other = f.table.handles_lock(&f.ram)?;
    guard.handles_guard_close(&f.ram)?;
    guard.handles_guard_close(&f.ram)?;
    setup::mixed_need(
        752,
        guard.handles_guard_closed() && other.handle_guard_lookup(&f.ram, 0x48000001)?.is_none(),
    )?;
    other.handles_guard_close(&f.ram)?;
    let mut shadow = kernel_pool::Shadow::shadow_new(&f.ram)?;
    let mut guard = shadow.shadow_lock(&f.ram)?;
    let (version, mut bytes) = gamepkg::local_record_snapshot(shadow.shadow_id(), 152)?;
    setup::mixed_need(753, setup::mixed_word(&bytes, 32)? == 1)?;
    bytes[32..40].copy_from_slice(&u64::to_le_bytes(0));
    gamepkg::local_record_compare_exchange(shadow.shadow_id(), version, &bytes)?;
    setup::mixed_need(
        754,
        guard.shadow_guard_close(&f.ram) == Err(-3) && !guard.shadow_guard_closed(),
    )?;
    let (version, mut bytes) = gamepkg::local_record_snapshot(shadow.shadow_id(), 152)?;
    bytes[32..40].copy_from_slice(&u64::to_le_bytes(1));
    gamepkg::local_record_compare_exchange(shadow.shadow_id(), version, &bytes)?;
    let mut other = shadow.shadow_lock(&f.ram)?;
    guard.shadow_guard_close(&f.ram)?;
    guard.shadow_guard_close(&f.ram)?;
    setup::mixed_need(
        755,
        guard.shadow_guard_closed() && other.shadow_lookup(&f.ram, 0x10000)?.is_none(),
    )?;
    other.shadow_guard_close(&f.ram)?;
    shadow.shadow_close(&f.ram)
}

// Ordinary checked game source: no provider, direct platform call or test grant.
fn mixed_slices() -> Result<(), i32> {
    let mut f = setup::Fixture::mixed_fixture_new()?;
    let event = f.mixed_event(false, 0)?;
    let sem = f.mixed_semaphore(1, 2)?;
    {
        let mut lease = mixed_unready(&f)?;
        setup::mixed_need(780, lease.mixed_wait_timeout(&f.ram) == Err(-1))?;
        lease.mixed_wait_prepare(&f.ram, &[event[1] as u32, sem[1] as u32])?;
        setup::mixed_need(781, lease.mixed_wait_timeout(&f.ram) == Err(-1))?;
        for _ in 0..3u32 {
            setup::mixed_need(782, lease.mixed_wait_slice(&f.ram, true, 0) == Err(-2))?;
            setup::mixed_need(
                783,
                mixed_graph(f.events.events_id())? == [1, 0]
                    && mixed_graph(f.semaphores.semaphores_id())? == [1, 0]
                    && lease.mixed_wait_native_result() == Err(-2)
                    && gamepkg::local_semaphore_state(sem[0])?.count == 1,
            )?;
            setup::mixed_need(
                784,
                f.events.events_remove(&f.ram, &f.table, event[1] as u32) == Err(-2)
                    && f.semaphores
                        .semaphores_remove(&f.ram, &f.table, sem[1] as u32)
                        == Err(-2),
            )?;
        }
        setup::mixed_need(
            785,
            lease.mixed_wait_slice(&f.ram, false, 0) == Err(-1)
                && lease.mixed_wait_slice(&f.ram, true, 1001) == Err(-1)
                && lease.mixed_wait(&f.ram, true, 0) == Err(-1),
        )?;
        f.events
            .events_set(&f.ram, &f.table, event[1] as u32, true)?;
        let result = lease.mixed_wait_slice(&f.ram, true, 0)?;
        setup::mixed_need(
            786,
            result.index == 0
                && !result.abandoned
                && !gamepkg::local_event_state(event[0])?.signaled
                && gamepkg::local_semaphore_state(sem[0])?.count == 0,
        )?;
        setup::mixed_need(
            787,
            lease.mixed_wait_slice(&f.ram, true, 0) == Err(-1)
                && lease.mixed_wait_timeout(&f.ram) == Err(-1),
        )?;
        lease.mixed_wait_close(&f.ram)?;
        mixed_completion(f.events.events_id(), event[0], 0)?;
        mixed_completion(f.semaphores.semaphores_id(), sem[0], 0)?;
    }
    {
        let mut lease = mixed_unready(&f)?;
        lease.mixed_wait_prepare(&f.ram, &[event[1] as u32, sem[1] as u32])?;
        setup::mixed_need(788, lease.mixed_wait_slice(&f.ram, false, 0) == Err(-2))?;
        // A now-ready signal must remain untouched by finalizing the guest timeout.
        f.events
            .events_set(&f.ram, &f.table, event[1] as u32, true)?;
        lease.mixed_wait_timeout(&f.ram)?;
        setup::mixed_need(
            789,
            lease.mixed_wait_native_result() == Err(-2)
                && gamepkg::local_event_state(event[0])?.signaled
                && lease.mixed_wait_slice(&f.ram, false, 0) == Err(-1)
                && lease.mixed_wait_timeout(&f.ram) == Err(-1),
        )?;
        lease.mixed_wait_close(&f.ram)?;
        mixed_completion(f.events.events_id(), event[0], -2)?;
        mixed_completion(f.semaphores.semaphores_id(), sem[0], -2)?;
    }
    f.events
        .events_set(&f.ram, &f.table, event[1] as u32, false)?;
    {
        let mut lease = mixed_unready(&f)?;
        lease.mixed_wait_prepare(&f.ram, &[event[1] as u32, sem[1] as u32])?;
        setup::mixed_need(790, lease.mixed_wait_slice(&f.ram, true, 0) == Err(-2))?;
        // Abandonment after a Busy slice is Denied, never fabricated timeout.
        lease.mixed_wait_close(&f.ram)?;
        mixed_completion(f.events.events_id(), event[0], -4)?;
        mixed_completion(f.semaphores.semaphores_id(), sem[0], -4)?;
    }
    {
        let mut lease = mixed_unready(&f)?;
        lease.mixed_wait_prepare(&f.ram, &[event[1] as u32, sem[1] as u32])?;
        setup::mixed_need(791, lease.mixed_wait_slice(&f.ram, true, 0) == Err(-2))?;
        let ksv = f.services.service_handle();
        f.ram.services_link(ksv, 0)?;
        setup::mixed_need(792, lease.mixed_wait_timeout(&f.ram) == Err(-4))?;
        lease.mixed_wait_close(&f.ram)?;
        f.ram.services_link(0, ksv)?;
        mixed_completion(f.events.events_id(), event[0], -4)?;
        mixed_completion(f.semaphores.semaphores_id(), sem[0], -4)?;
    }
    f.events.events_remove(&f.ram, &f.table, event[1] as u32)?;
    f.semaphores
        .semaphores_remove(&f.ram, &f.table, sem[1] as u32)?;
    f.mixed_fixture_close()
}
