// Public checked port facade. The shared import bridge owns the same type.
pub use crate::kernel_pool::Connections;
use crate::memory;
// The bridge writes exactly 44 guest bytes, not a host-shaped KINTERRUPT.
pub fn interrupt_initialize(
    ram: &mut memory::Memory,
    guest: u32,
    routine: u32,
    context: u32,
    vector_index: u32,
) -> Result<bool, i32> {
    ram.call_snapshot()?;
    if guest == 0 {
        return Ok(false);
    }
    guest.checked_add(44).ok_or(-1)?;
    ram.fill(guest, 0, 44)?;
    ram.write_u32(guest, routine)?;
    ram.write_u32(guest + 4, context)?;
    ram.write_u32(guest + 8, vector_index)?;
    Ok(true)
}
pub fn interrupt_vector(ram: &mut memory::Memory, level: u32, irql_guest: u32) -> Result<u32, i32> {
    ram.call_snapshot()?;
    if irql_guest != 0 {
        ram.write_u8(irql_guest, level as u8)?;
    }
    Ok(level)
}
