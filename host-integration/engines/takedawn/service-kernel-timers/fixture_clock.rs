// Controlled test clock only. Production uses clock.rs and the host counter.
use crate::memory;
pub fn timer_now_ms(ram: &mut memory::Memory) -> Result<i64, i32> {
    let mode = ram.read_u32(0x740010)?;
    if mode == 2 {
        return Err(-4);
    }
    if mode == 1 {
        let record = ram.read_u64(0x740018)?;
        let (version, bytes) = crate::gamepkg::local_record_snapshot(record, 832)?;
        crate::gamepkg::local_record_compare_exchange(record, version, &bytes)?;
    }
    let reads = ram.read_u64(0x740008)?;
    ram.write_u64(0x740008, reads.wrapping_add(1))?;
    Ok(ram.read_u64(0x740000)? as i64)
}
