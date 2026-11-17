use crate::cases;
use crate::gamepkg;
use crate::handles;
use crate::kernel_pool;
use crate::memory;
fn handle_need(ok: bool) -> Result<(), i32> {
    if ok {
        Ok(())
    } else {
        Err(-3)
    }
}
pub fn handle_probe() -> Result<std::vec::Vec<u8>, i32> {
    let ram = memory::Memory::new()?;
    let services = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2)?;
    let mut inventory = [0u8; 64];
    handle_need(gamepkg::local_call(1, &[], &mut inventory)? == 64)?;
    let window = u64::from_le_bytes([
        inventory[48],
        inventory[49],
        inventory[50],
        inventory[51],
        inventory[52],
        inventory[53],
        inventory[54],
        inventory[55],
    ]);
    let commit = [
        u64::to_le_bytes(window).as_slice(),
        &u64::to_le_bytes(0),
        &u64::to_le_bytes(1048576),
        &u32::to_le_bytes(2),
    ]
    .concat();
    gamepkg::local_call(2, &commit, &mut [])?;
    let mut map = handles::Handles::handles_new(&ram)?;
    handle_need(map.handles_lock(&ram).err() == Some(-4))?;
    handle_need(map.handles_bind(&ram, window, 1) == Err(-1))?;
    handle_need(map.handles_bind(&ram, window | 0x100000000, 0) == Err(-4))?;
    handle_need(map.handles_progress() == [0, 0])?;
    map.handles_bind(&ram, window, 0)?;
    handle_need(map.handles_progress() == [2, 16] && map.handles_bind(&ram, window, 0) == Err(-1))?;
    crate::controls::handle_controls(&ram, &mut map)?;
    let mut guard = map.handles_lock(&ram)?;
    let mut nested = map.handles_lock(&ram)?;
    handle_need(map.handles_close(&ram) == Err(-2))?;
    nested.handles_guard_close(&ram)?;
    for slot in 1..16384u32 {
        handle_need(
            guard.handle_guard_insert(&ram, 0x100000000 + u64::from(slot - 1), 1, false)?
                == 0x48000000 | slot,
        )?;
    }
    handle_need(guard.handle_guard_insert(&ram, 0x900000000, 1, true) == Err(-2))?;
    handle_need(guard.handle_guard_insert(&ram, 0x100000000, 1, false)? == 0x48000001)?;
    handle_need(guard.handle_guard_insert(&ram, 0x100000000, 1, true) == Err(-2))?;
    handle_need(gamepkg::local_event_state(0x100000000).err() == Some(-4))?;
    let mut output = std::vec::Vec::new();
    for index in 0..512u32 {
        let input = cases::handle_case(index);
        let result = if input[0] == 0 {
            match guard.handle_guard_insert(&ram, input[1], 1, input[2] != 0) {
                Ok(token) => u64::from(token),
                Err(-2) => 0,
                Err(code) => return Err(code),
            }
        } else if input[0] == 1 {
            match guard.handle_guard_lookup(&ram, input[1] as u32)? {
                Some(value) => value[0],
                None => 0,
            }
        } else {
            match guard.handle_guard_remove(&ram, input[1] as u32)? {
                Some(value) => value[0],
                None => 0,
            }
        };
        output.extend(u64::to_le_bytes(result));
        let observed = guard.handle_guard_lookup(&ram, 0x48000001 + index % 16383)?;
        output.extend(u64::to_le_bytes(if let Some(value) = observed {
            value[0]
        } else {
            0
        }));
    }
    let slots = guard.handle_guard_slots(&ram)?;
    handle_need(slots.len() == 16383)?;
    for (offset, item) in slots.iter().enumerate() {
        output.extend(u64::to_le_bytes(item[0]));
        if item[1] != 0 {
            handle_need(
                item[1] == 1
                    && guard.handle_guard_remove(&ram, 0x48000001 + offset as u32)? == Some(*item),
            )?;
        }
    }
    guard.handles_guard_close(&ram)?;
    handle_need(
        guard.handles_guard_closed() && guard.handle_guard_lookup(&ram, 0x48000001) == Err(-4),
    )?;
    guard.handles_guard_close(&ram)?;
    map.handles_close(&ram)?;
    handle_need(map.handles_closed())?;
    map.handles_close(&ram)?;
    let mut revoked = handles::Handles::handles_new(&ram)?;
    revoked.handles_bind(&ram, window, 0)?;
    let mut prepared = revoked.handles_lock(&ram)?;
    let token = prepared.handle_guard_insert(&ram, 0x123456789, 1, false)?;
    prepared.handles_guard_close(&ram)?;
    services.service_close(&ram)?;
    handle_need(revoked.handles_lock(&ram).err() == Some(-4))?;
    let mut cleanup = revoked.handles_cleanup_lock(&ram)?;
    handle_need(cleanup.handle_guard_insert(&ram, 7, 1, false) == Err(-4))?;
    handle_need(cleanup.handle_guard_remove(&ram, token)? == Some([0x123456789, 1]))?;
    handle_need(revoked.handles_close(&ram) == Err(-2))?;
    cleanup.handles_guard_close(&ram)?;
    handle_need(revoked.handles_close(&ram) == Err(-4) && revoked.handles_closed())?;
    handle_need(revoked.handles_close(&ram) == Err(-4))?;
    gamepkg::local_record_release(ram.call_handle())?;
    Ok(output)
}
