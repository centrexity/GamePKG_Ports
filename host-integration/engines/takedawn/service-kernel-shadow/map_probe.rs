use crate::cases;
use crate::gamepkg;
use crate::kernel_pool;
use crate::memory;
use crate::shadow;
fn map_need(value: bool) -> Result<(), i32> {
    if value {
        Ok(())
    } else {
        Err(-3)
    }
}
pub fn map_probe() -> Result<std::vec::Vec<u8>, i32> {
    let ram = memory::Memory::new()?;
    let services = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2)?;
    let mut map = shadow::Shadow::shadow_new(&ram)?;
    crate::map_controls::map_controls(&ram, &map)?;
    let mut first = map.shadow_lock(&ram)?;
    let mut nested = map.shadow_lock(&ram)?;
    map_need(map.shadow_close(&ram) == Err(-2))?;
    nested.shadow_guard_close(&ram)?;
    first.shadow_guard_close(&ram)?;
    map_need(first.shadow_guard_closed() && first.shadow_lookup(&ram, 1) == Err(-4))?;
    first.shadow_guard_close(&ram)?;
    let other = memory::Memory::new()?;
    map_need(shadow::Shadow::shadow_borrow(&other, map.shadow_id()).err() == Some(-4))?;
    gamepkg::local_record_release(other.call_handle())?;
    let mut guard = map.shadow_lock(&ram)?;
    map_need(
        guard.shadow_lookup(&ram, 0)? == None
            && guard.shadow_remove(&ram, 0)? == None
            && guard.shadow_insert(&ram, 0, 1) == Err(-1)
            && guard.shadow_insert(&ram, 1, 0) == Err(-1),
    )?;
    for index in 0..4096u32 {
        map_need(
            guard.shadow_insert(&ram, 0x1000 + index * 4, 0x100000000u64 + u64::from(index))?
                == None,
        )?;
    }
    map_need(
        guard.shadow_entries(&ram)?.len() == 4096
            && gamepkg::local_event_state(0x100000000).err() == Some(-4),
    )?;
    let mut output = std::vec::Vec::new();
    let mut closed = 0u64;
    let mut trace = 14695981039346656037u64;
    for index in 0..512u32 {
        let args = cases::shadow_case(index);
        let guest = args[1] as u32;
        let token = args[2];
        let before = guard.shadow_lookup(&ram, guest)?;
        match args[0] {
            0 => {
                if let Some(old) = before {
                    if old != token {
                        closed += 1;
                        trace = cases::shadow_trace(trace, old);
                    }
                }
                match guard.shadow_insert(&ram, guest, token) {
                    Ok(old) => map_need(old == before)?,
                    Err(-2) => {
                        map_need(before == None)?;
                        closed += 1;
                        trace = cases::shadow_trace(trace, token);
                    }
                    Err(code) => return Err(code),
                }
            }
            2 => {
                if let Some(old) = before {
                    closed += 1;
                    trace = cases::shadow_trace(trace, old);
                }
                map_need(guard.shadow_remove(&ram, guest)? == before)?;
            }
            _ => {}
        }
        for value in [
            before.unwrap_or(0),
            guard.shadow_lookup(&ram, guest)?.unwrap_or(0),
            closed,
            trace,
        ] {
            output.extend(u64::to_le_bytes(value));
        }
        if index % 64 == 63 {
            let slots = guard.shadow_slots(&ram)?;
            map_need(slots.len() == 4096)?;
            for slot in slots {
                output.extend(u32::to_le_bytes(slot[0] as u32));
                output.extend(u64::to_le_bytes(slot[1]));
            }
        }
    }
    guard.shadow_guard_close(&ram)?;
    map_need(map.shadow_close(&ram) == Err(-2))?;
    services.service_close(&ram)?;
    map_need(map.shadow_lock(&ram).err() == Some(-4))?;
    let mut cleanup = map.shadow_cleanup_lock(&ram)?;
    map_need(cleanup.shadow_insert(&ram, 7, 7) == Err(-4))?;
    for entry in cleanup.shadow_entries(&ram)? {
        map_need(cleanup.shadow_remove(&ram, entry[0] as u32)? == Some(entry[1]))?;
    }
    map_need(map.shadow_close(&ram) == Err(-2))?;
    cleanup.shadow_guard_close(&ram)?;
    map_need(map.shadow_close(&ram) == Err(-4) && map.shadow_closed())?;
    map.shadow_close(&ram)?;
    gamepkg::local_record_release(ram.call_handle())?;
    Ok(output)
}
