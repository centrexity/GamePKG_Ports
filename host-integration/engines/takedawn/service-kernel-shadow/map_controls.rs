use crate::gamepkg;
use crate::memory;
use crate::shadow;
fn shadow_need(value: bool) -> Result<(), i32> {
    if value {
        Ok(())
    } else {
        Err(-3)
    }
}
fn shadow_control_put(bytes: &mut [u8], at: usize, value: u64) -> Result<(), i32> {
    if at.checked_add(8).ok_or(-1)? > bytes.len() {
        return Err(-1);
    }
    let value = u64::to_le_bytes(value);
    for lane in 0..8usize {
        bytes[at + lane] = value[lane];
    }
    Ok(())
}
pub fn map_controls(ram: &memory::Memory, map_view: &shadow::Shadow) -> Result<(), i32> {
    let mut guard = map_view.shadow_lock(ram)?;
    shadow_need(
        guard.shadow_insert(ram, 33, 77)? == None && guard.shadow_remove(ram, 33)? == Some(77),
    )?;
    guard.shadow_guard_close(ram)?;
    let (_, original) = gamepkg::local_record_snapshot(map_view.shadow_id(), 152)?;
    for offset in [0usize, 40] {
        let mut bytes = original.clone();
        bytes[offset] = 0xff;
        let record = gamepkg::local_record_create(&bytes)?;
        shadow_need(shadow::Shadow::shadow_borrow(ram, record).err() == Some(-1))?;
        gamepkg::local_record_release(record)?;
        shadow_need(shadow::Shadow::shadow_borrow(ram, record).err() == Some(-4))?;
    }
    for offset in [8usize, 16, 24] {
        let mut bytes = original.clone();
        shadow_control_put(&mut bytes, offset, 0)?;
        let record = gamepkg::local_record_create(&bytes)?;
        shadow_need(shadow::Shadow::shadow_borrow(ram, record).err() == Some(-4))?;
        gamepkg::local_record_release(record)?;
    }
    let mut event = [0u8; 8];
    gamepkg::local_call(
        20,
        &[u32::to_le_bytes(1), u32::to_le_bytes(1)].concat(),
        &mut event,
    )?;
    let event = u64::from_le_bytes(event);
    let mut bytes = original.clone();
    shadow_control_put(&mut bytes, 24, event)?;
    let record = gamepkg::local_record_create(&bytes)?;
    shadow_need(
        shadow::Shadow::shadow_borrow(ram, record).err() == Some(-1)
            && gamepkg::local_event_state(event)?.signaled,
    )?;
    gamepkg::local_record_release(record)?;
    gamepkg::local_call(25, &u64::to_le_bytes(event), &mut [])?;
    // A copied root cannot adopt pages bound to the original root record.
    let record = gamepkg::local_record_create(&original)?;
    let borrowed = shadow::Shadow::shadow_borrow(ram, record)?;
    let mut bad = borrowed.shadow_lock(ram)?;
    shadow_need(bad.shadow_lookup(ram, 33) == Err(-4))?;
    bad.shadow_guard_close(ram)?;
    gamepkg::local_record_release(record)?;
    // Overflow cannot wrap the live-guard ledger and allow early owner close.
    let mut bytes = original.clone();
    shadow_control_put(&mut bytes, 32, u64::MAX)?;
    let record = gamepkg::local_record_create(&bytes)?;
    let borrowed = shadow::Shadow::shadow_borrow(ram, record)?;
    shadow_need(borrowed.shadow_lock(ram).err() == Some(-2))?;
    gamepkg::local_record_release(record)?;
    // Gapped or duplicate page lists are schema errors, not capabilities.
    let mut bytes = original.clone();
    let page = [
        bytes[48], bytes[49], bytes[50], bytes[51], bytes[52], bytes[53], bytes[54], bytes[55],
    ];
    for lane in 0..8usize {
        bytes[56 + lane] = page[lane];
    }
    let record = gamepkg::local_record_create(&bytes)?;
    shadow_need(shadow::Shadow::shadow_borrow(ram, record).err() == Some(-1))?;
    gamepkg::local_record_release(record)?;
    let mut bytes = original.clone();
    shadow_control_put(&mut bytes, 48, 0)?;
    for lane in 0..8usize {
        bytes[56 + lane] = page[lane];
    }
    let record = gamepkg::local_record_create(&bytes)?;
    shadow_need(shadow::Shadow::shadow_borrow(ram, record).err() == Some(-1))?;
    gamepkg::local_record_release(record)?;
    Ok(())
}
