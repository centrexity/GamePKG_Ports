// Production clock provider: host-owned counters and shared guest skew only.
use crate::memory;
pub fn dpc_host_now(ram: &mut memory::Memory) -> Result<u64, i32> {
    crate::wait_service::host_now_ns(ram)
}
pub fn dpc_guest_now(ram: &mut memory::Memory) -> Result<u64, i32> {
    crate::wait_service::guest_now_ns(ram)
}
