// Controlled fixture clock only; production uses ../clock.rs and host services.
use crate::memory;
pub fn dpc_host_now(ram: &mut memory::Memory) -> Result<u64, i32> {
    let n = ram.read_u32(0x741070)?;
    ram.write_u32(0x741070, n.wrapping_add(1))?;
    Ok(ram
        .read_u64(0x741060)?
        .wrapping_add(u64::from(n).wrapping_mul(ram.read_u64(0x741068)?)))
}
pub fn dpc_guest_now(ram: &mut memory::Memory) -> Result<u64, i32> {
    if ram.read_u32(0x7410a0)? == 3 {
        return Err(-4);
    }
    let n = ram.read_u32(0x741088)?;
    ram.write_u32(0x741088, n.wrapping_add(1))?;
    Ok(ram
        .read_u64(0x741078)?
        .wrapping_add(u64::from(n).wrapping_mul(ram.read_u64(0x741080)?)))
}
