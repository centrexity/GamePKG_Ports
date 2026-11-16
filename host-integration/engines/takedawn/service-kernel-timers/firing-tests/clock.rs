// Deterministic composition fixture only. Production uses ../clock.rs.
use crate::memory;
pub fn timer_now_ms(ram: &mut memory::Memory) -> Result<i64, i32> {
    ram.call_snapshot()?;
    Ok(1000)
}
