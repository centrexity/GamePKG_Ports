use crate::gamepkg;
use crate::memory;
use crate::timers;
fn timer_need(value: bool) -> Result<(), i32> {
    if value {
        Ok(())
    } else {
        Err(-3)
    }
}
pub fn timer_controls(ram: &mut memory::Memory, table: &timers::Timers) -> Result<(), i32> {
    let original = table.timers_state(ram)?;
    // Invalid schemas are rejected before acquiring any alleged lock.
    for at in [0usize, 48, 56, 84] {
        let mut bytes = original.clone();
        bytes[at] = 0xff;
        let record = gamepkg::local_record_create(&bytes)?;
        timer_need(timers::Timers::timers_borrow(ram, record).err() == Some(-1))?;
        gamepkg::local_record_release(record)?;
        timer_need(timers::Timers::timers_borrow(ram, record).err() == Some(-4))?;
    }
    for at in [8usize, 16, 32] {
        let mut bytes = original.clone();
        bytes[at..at + 8].copy_from_slice(&u64::to_le_bytes(0));
        let record = gamepkg::local_record_create(&bytes)?;
        timer_need(timers::Timers::timers_borrow(ram, record).err() == Some(-4))?;
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
    bytes[24..32].copy_from_slice(&u64::to_le_bytes(event));
    let record = gamepkg::local_record_create(&bytes)?;
    timer_need(
        timers::Timers::timers_borrow(ram, record).err() == Some(-1)
            && gamepkg::local_event_state(event)?.signaled,
    )?;
    gamepkg::local_record_release(record)?;
    gamepkg::local_call(25, &u64::to_le_bytes(event), &mut [])?;
    let other = memory::Memory::new()?;
    timer_need(timers::Timers::timers_borrow(&other, table.timers_id()).err() == Some(-4))?;
    gamepkg::local_record_release(other.call_handle())?;
    // Interfering CAS is a permanent prefix error, not ordinary lock contention.
    ram.write_u64(0x740018, table.timers_id())?;
    ram.write_u32(0x740010, 1)?;
    timer_need(
        table.timer_set(ram, 0x1000, -10000, 0, 0) == Err(-3)
            && table.timers_state(ram)? == original,
    )?;
    ram.write_u32(0x740010, 2)?;
    timer_need(
        table.timer_set(ram, 0x1000, -10000, 0, 0) == Err(-4)
            && table.timers_state(ram)? == original,
    )?;
    ram.write_u32(0x740010, 0)?;
    ram.write_u64(0x740008, 0)?;
    let b = original.get(24..32).ok_or(-1)?;
    let lock = u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]);
    gamepkg::local_mutex_lock(lock, 0)?;
    gamepkg::local_call(24, &u64::to_le_bytes(lock), &mut [])?;
    Ok(())
}
