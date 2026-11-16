// Native production clock adapter; fixtures may replace this module explicitly.
use crate::memory;
pub fn now_ns(ram: &memory::Memory) -> Result<u64, i32> {
    crate::wait_service::host_now_ns(ram)
}
