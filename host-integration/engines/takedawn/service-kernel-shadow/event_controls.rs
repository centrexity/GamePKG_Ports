use crate::events;
use crate::gamepkg;
use crate::memory;
use crate::shadow;
fn event_need(value: bool) -> Result<(), i32> {
    if value {
        Ok(())
    } else {
        Err(-3)
    }
}
fn event_release(id: u64) -> Result<(), i32> {
    gamepkg::local_call(25, &u64::to_le_bytes(id), &mut [])?;
    Ok(())
}
fn event_quota(map_view: &shadow::Shadow, ram: &mut memory::Memory) -> Result<(), i32> {
    let guest = 0x740100;
    events::event_initialize(map_view, ram, guest, 0, 1)?;
    let old = events::event_lookup(map_view, ram, guest)?.ok_or(-3)?;
    let state = gamepkg::local_event_state(old)?;
    let mut guard = map_view.shadow_lock(ram)?;
    let live = guard.shadow_entries(ram)?.len();
    guard.shadow_guard_close(ram)?;
    let mut extras = std::vec::Vec::new();
    loop {
        let mut out = [0u8; 8];
        match gamepkg::local_call(20, &[0u8; 8], &mut out) {
            Ok(8) => extras.push(u64::from_le_bytes(out)),
            Err(-2) => break,
            Ok(_) => return Err(-3),
            Err(code) => return Err(code),
        }
    }
    event_need(extras.len() + live + 1 == 128)?;
    let mut before = [0u8; 40];
    ram.read_bytes(guest, &mut before)?;
    event_need(events::event_initialize(map_view, ram, guest, 1, 0) == Err(-2))?;
    let mut after = [0u8; 40];
    ram.read_bytes(guest, &mut after)?;
    event_need(after == before)?;
    event_need(events::timer_initialize(map_view, ram, guest, 2) == Err(-2))?;
    ram.read_bytes(guest, &mut after)?;
    let mut expected = [0u8; 40];
    expected[0] = 8;
    expected[2] = 40;
    event_need(
        after == expected
            && events::event_lookup(map_view, ram, guest)? == Some(old)
            && gamepkg::local_event_state(old)? == state,
    )?;
    // A non-event header never allocates an event, even at the exact sync quota.
    ram.fill(0x74f000, 0, 16)?;
    ram.write_u8(0x74f000, 2)?;
    event_need(
        events::event_for(map_view, ram, 0x74f000)? == None
            && events::event_wait(map_view, ram, 0x74f000, 0) == Err(-5),
    )?;
    for id in extras {
        event_release(id)?;
    }
    Ok(())
}
fn event_readonly(
    map_view: &shadow::Shadow,
    ram: &mut memory::Memory,
    window: u64,
) -> Result<(), i32> {
    let guest = 0x740100;
    let old = events::event_lookup(map_view, ram, guest)?.ok_or(-3)?;
    let mut readonly = ram.call_view();
    readonly.attach(0x740000, 65536, window, 0, false)?;
    let mut before = [0u8; 40];
    ram.read_bytes(guest, &mut before)?;
    event_need(events::event_initialize(map_view, &mut readonly, guest, 0, 1) == Err(-4))?;
    let fresh = events::event_lookup(map_view, ram, guest)?.ok_or(-3)?;
    let mut after = [0u8; 40];
    ram.read_bytes(guest, &mut after)?;
    event_need(
        fresh != old && after == before && gamepkg::local_event_state(old).err() == Some(-4),
    )?;
    event_need(
        events::timer_initialize(map_view, &mut readonly, guest, 1) == Err(-4)
            && events::event_lookup(map_view, ram, guest)? == Some(fresh),
    )?;
    ram.write_u32(guest + 4, 9)?;
    events::event_reset(map_view, ram, guest)?;
    event_need(events::event_set(map_view, ram, guest)? && ram.read_u32(guest + 4)? == 9)?;
    event_need(
        events::timer_reset_after_set(map_view, ram, guest)?
            && ram.read_u32(guest + 4)? == 9
            && !gamepkg::local_event_state(fresh)?.signaled,
    )?;
    event_need(
        events::timer_signal_after_callback(map_view, &mut readonly, guest) == Err(-4)
            && gamepkg::local_event_state(fresh)?.signaled
            && ram.read_u32(guest + 4)? == 9,
    )?;
    events::timer_signal_after_callback(map_view, ram, guest)?;
    event_need(ram.read_u32(guest + 4)? == 1)?;
    Ok(())
}
fn event_wrong_kind(map_view: &shadow::Shadow, ram: &memory::Memory) -> Result<(), i32> {
    let mut mutex = [0u8; 8];
    gamepkg::local_call(21, &[], &mut mutex)?;
    let mutex = u64::from_le_bytes(mutex);
    let mut guard = map_view.shadow_lock(ram)?;
    event_need(guard.shadow_insert(ram, 0x74ff00, mutex)? == None)?;
    guard.shadow_guard_close(ram)?;
    event_need(
        events::event_replace(map_view, ram, 0x74ff00, true, true) == Err(-1)
            && events::event_remove(map_view, ram, 0x74ff00) == Err(-1),
    )?;
    gamepkg::local_mutex_lock(mutex, 0)?;
    gamepkg::local_call(24, &u64::to_le_bytes(mutex), &mut [])?;
    let mut guard = map_view.shadow_lock(ram)?;
    event_need(guard.shadow_remove(ram, 0x74ff00)? == Some(mutex))?;
    guard.shadow_guard_close(ram)?;
    event_release(mutex)?;
    let stale = events::event_replace(map_view, ram, 0x74ff00, false, true)?;
    event_release(stale)?;
    event_need(
        events::event_lookup(map_view, ram, 0x74ff00) == Err(-4)
            && events::event_remove(map_view, ram, 0x74ff00) == Err(-4)
            && events::event_lookup(map_view, ram, 0x74ff00)? == None,
    )?;
    Ok(())
}
pub fn event_controls(
    map_view: &shadow::Shadow,
    ram: &mut memory::Memory,
    window: u64,
) -> Result<(), i32> {
    event_quota(map_view, ram)?;
    event_readonly(map_view, ram, window)?;
    event_wrong_kind(map_view, ram)?;
    event_quota(map_view, ram)?;
    event_need(
        events::event_for(map_view, ram, 0xfffff000) == Err(-4)
            && events::event_initialize(map_view, ram, u32::MAX, 0, 0) == Err(-1)
            && events::timer_initialize(map_view, ram, u32::MAX, 0) == Err(-1),
    )?;
    events::timer_initialize(map_view, ram, 0, 0)?;
    Ok(())
}
