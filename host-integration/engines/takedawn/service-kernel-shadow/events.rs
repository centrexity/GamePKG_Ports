// Port-owned event/timer headers and shadow order. Native events remain SDK-owned.
use crate::gamepkg;
use crate::memory;
use crate::shadow;
fn event_signal(id: u64, signal: bool) -> Result<(), i32> {
    if gamepkg::local_call(
        22,
        &[
            u64::to_le_bytes(id).as_slice(),
            &u32::to_le_bytes(u32::from(signal)),
        ]
        .concat(),
        &mut [],
    )? != 0
    {
        return Err(-3);
    }
    Ok(())
}
fn event_release(id: u64) -> Result<(), i32> {
    if gamepkg::local_call(25, &u64::to_le_bytes(id), &mut [])? != 0 {
        return Err(-3);
    }
    Ok(())
}
fn event_native_new(manual: bool, signal: bool) -> Result<u64, i32> {
    let mut out = [0u8; 8];
    if gamepkg::local_call(
        20,
        &[
            u32::to_le_bytes(u32::from(manual)),
            u32::to_le_bytes(u32::from(signal)),
        ]
        .concat(),
        &mut out,
    )? != 8
    {
        return Err(-3);
    }
    let id = u64::from_le_bytes(out);
    Ok(id)
}
fn event_create_locked(
    guard: &shadow::Guard,
    ram: &memory::Memory,
    guest: u32,
    manual: bool,
    signal: bool,
) -> Result<u64, i32> {
    let id = event_native_new(manual, signal)?;
    let result = event_publish_locked(guard, ram, guest, id);
    if let Err(code) = result {
        let _ = event_release(id);
        return Err(code);
    }
    Ok(id)
}
fn event_publish_locked(
    guard: &shadow::Guard,
    ram: &memory::Memory,
    guest: u32,
    id: u64,
) -> Result<(), i32> {
    if let Some(old) = guard.shadow_lookup(ram, guest)? {
        // Never close an unrelated mutex/task token presented as an event.
        gamepkg::local_event_state(old)?;
        event_release(old)?;
    }
    // A failure here is an explicit fatal partial prefix: an old event may
    // already be revoked. The caller must stop/report, not blindly retry.
    guard.shadow_insert(ram, guest, id)?;
    Ok(())
}
pub fn event_replace(
    map: &shadow::Shadow,
    ram: &memory::Memory,
    guest: u32,
    manual: bool,
    signal: bool,
) -> Result<u64, i32> {
    if guest == 0 {
        return Err(-1);
    }
    // The original initializer creates before acquiring the shadow-table lock.
    let id = event_native_new(manual, signal)?;
    let mut guard = match map.shadow_lock(ram) {
        Ok(value) => value,
        Err(code) => {
            let _ = event_release(id);
            return Err(code);
        }
    };
    let result = event_publish_locked(&guard, ram, guest, id);
    let closed = guard.shadow_guard_close(ram);
    if let Err(code) = result {
        let _ = event_release(id);
        return Err(code);
    }
    closed?;
    Ok(id)
}

pub fn event_lookup(
    map: &shadow::Shadow,
    ram: &memory::Memory,
    guest: u32,
) -> Result<Option<u64>, i32> {
    let mut guard = map.shadow_lock(ram)?;
    let result = guard.shadow_lookup(ram, guest);
    let closed = guard.shadow_guard_close(ram);
    let value = result?;
    closed?;
    if let Some(id) = value {
        gamepkg::local_event_state(id)?;
    }
    Ok(value)
}
pub fn event_for(
    map: &shadow::Shadow,
    ram: &memory::Memory,
    guest: u32,
) -> Result<Option<u64>, i32> {
    if let Some(id) = event_lookup(map, ram, guest)? {
        return Ok(Some(id));
    }
    if guest == 0 {
        return Ok(None);
    }
    let mut header = [0u8; 16];
    ram.read_bytes(guest, &mut header)?;
    let kind = header[0];
    if kind > 1 {
        return Ok(None);
    }
    let mut guard = map.shadow_lock(ram)?;
    let result = event_lazy_locked(&guard, ram, guest, kind);
    let closed = guard.shadow_guard_close(ram);
    let id = result?;
    closed?;
    Ok(Some(id))
}
fn event_lazy_locked(
    guard: &shadow::Guard,
    ram: &memory::Memory,
    guest: u32,
    kind: u8,
) -> Result<u64, i32> {
    if let Some(id) = guard.shadow_lookup(ram, guest)? {
        gamepkg::local_event_state(id)?;
        return Ok(id);
    }
    // Original Type is sampled before taking the map lock; SignalState is read
    // only here. Recheck the mapping so concurrent first users share one event.
    let signal = ram.read_u32(guest.checked_add(4).ok_or(-1)?)? != 0;
    event_create_locked(guard, ram, guest, kind == 0, signal)
}
pub fn event_initialize(
    map: &shadow::Shadow,
    ram: &mut memory::Memory,
    guest: u32,
    kind: u32,
    initial: u32,
) -> Result<(), i32> {
    if guest == 0 || guest.checked_add(15).is_none() {
        return Err(-1);
    }
    event_replace(map, ram, guest, kind == 0, initial != 0)?;
    // The mapping is published before the original guest-header writes.
    ram.write_u8(guest, u8::from(kind != 0))?;
    ram.write_u8(guest + 1, 0)?;
    ram.write_u8(guest + 2, 16)?;
    ram.write_u8(guest + 3, 1)?;
    ram.write_u32(guest + 4, u32::from(initial != 0))?;
    ram.write_u32(guest + 8, 0)?;
    ram.write_u32(guest + 12, 0)?;
    Ok(())
}
pub fn timer_initialize(
    map: &shadow::Shadow,
    ram: &mut memory::Memory,
    guest: u32,
    kind: u32,
) -> Result<(), i32> {
    if guest == 0 {
        return Ok(());
    }
    if guest.checked_add(39).is_none() {
        return Err(-1);
    }
    // Original timer bridge writes its 40-byte guest object before allocating
    // an event. A failed allocation leaves that prefix and any old mapping.
    ram.fill(guest, 0, 40)?;
    ram.write_u8(guest, 8 + (kind & 1) as u8)?;
    ram.write_u8(guest + 2, 40)?;
    event_replace(map, ram, guest, kind == 0, false)?;
    Ok(())
}
pub fn event_set(map: &shadow::Shadow, ram: &memory::Memory, guest: u32) -> Result<bool, i32> {
    match event_for(map, ram, guest)? {
        Some(id) => {
            event_signal(id, true)?;
            Ok(true)
        }
        None => Ok(false),
    }
}
pub fn event_reset(map: &shadow::Shadow, ram: &memory::Memory, guest: u32) -> Result<bool, i32> {
    match event_for(map, ram, guest)? {
        Some(id) => {
            event_signal(id, false)?;
            Ok(true)
        }
        None => Ok(false),
    }
}
pub fn event_wait(
    map: &shadow::Shadow,
    ram: &memory::Memory,
    guest: u32,
    milliseconds: u32,
) -> Result<(), i32> {
    let id = event_for(map, ram, guest)?.ok_or(-5)?;
    gamepkg::local_event_wait(id, milliseconds)
}
pub fn timer_reset_after_set(
    map: &shadow::Shadow,
    ram: &memory::Memory,
    guest: u32,
) -> Result<bool, i32> {
    match event_lookup(map, ram, guest)? {
        Some(id) => {
            event_signal(id, false)?;
            Ok(true)
        }
        None => Ok(false),
    }
}
pub fn timer_signal_after_callback(
    map: &shadow::Shadow,
    ram: &mut memory::Memory,
    guest: u32,
) -> Result<bool, i32> {
    match event_lookup(map, ram, guest)? {
        Some(id) => {
            event_signal(id, true)?;
            ram.write_u32(guest.checked_add(4).ok_or(-1)?, 1)?;
            Ok(true)
        }
        None => Ok(false),
    }
}
// Ordinary removal is available to live port callers. Owner-only shutdown
// below retains a deletion lane after KSV3 revocation; neither bypasses kind checks.
pub fn event_remove(map: &shadow::Shadow, ram: &memory::Memory, guest: u32) -> Result<bool, i32> {
    let mut guard = map.shadow_lock(ram)?;
    let result = event_remove_locked(&guard, ram, guest);
    let closed = guard.shadow_guard_close(ram);
    let value = result?;
    closed?;
    Ok(value)
}
fn event_remove_locked(
    guard: &shadow::Guard,
    ram: &memory::Memory,
    guest: u32,
) -> Result<bool, i32> {
    let id = match guard.shadow_lookup(ram, guest)? {
        Some(id) => id,
        None => return Ok(false),
    };
    let missing = match gamepkg::local_event_state(id) {
        Ok(_) => false,
        Err(-4) => true,
        Err(code) => return Err(code),
    };
    if !missing {
        event_release(id)?;
    }
    if guard.shadow_remove(ram, guest)? != Some(id) {
        return Err(-3);
    }
    if missing {
        return Err(-4);
    }
    Ok(true)
}
pub fn event_close_all(map: &shadow::Shadow, ram: &memory::Memory) -> Result<(), i32> {
    let mut guard = map.shadow_cleanup_lock(ram)?;
    let result = event_close_locked(&guard, ram);
    let closed = guard.shadow_guard_close(ram);
    result?;
    closed?;
    Ok(())
}
fn event_close_locked(guard: &shadow::Guard, ram: &memory::Memory) -> Result<(), i32> {
    let entries = guard.shadow_entries(ram)?;
    let mut missing = false;
    for entry in entries {
        match event_remove_locked(guard, ram, entry[0] as u32) {
            Ok(true) => {}
            Err(-4) => missing = true,
            Ok(false) => return Err(-3),
            Err(code) => return Err(code),
        }
    }
    if missing {
        return Err(-4);
    }
    Ok(())
}
