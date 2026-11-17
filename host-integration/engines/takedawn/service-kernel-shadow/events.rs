// Checked game facade; one implementation and shadow map for direct and imported calls.
use crate::memory;
use crate::shadow;
pub fn event_replace(
    map: &shadow::Shadow,
    ram: &memory::Memory,
    guest: u32,
    manual: bool,
    signal: bool,
) -> Result<u64, i32> {
    crate::kernel_pool::event_replace(map, ram, guest, manual, signal)
}
pub fn event_lookup(
    map: &shadow::Shadow,
    ram: &memory::Memory,
    guest: u32,
) -> Result<Option<u64>, i32> {
    crate::kernel_pool::event_lookup(map, ram, guest)
}
pub fn event_for(
    map: &shadow::Shadow,
    ram: &memory::Memory,
    guest: u32,
) -> Result<Option<u64>, i32> {
    crate::kernel_pool::event_for(map, ram, guest)
}
pub fn event_initialize(
    map: &shadow::Shadow,
    ram: &mut memory::Memory,
    guest: u32,
    kind: u32,
    initial: u32,
) -> Result<(), i32> {
    crate::kernel_pool::event_initialize(map, ram, guest, kind, initial)
}
pub fn timer_initialize(
    map: &shadow::Shadow,
    ram: &mut memory::Memory,
    guest: u32,
    kind: u32,
) -> Result<(), i32> {
    crate::kernel_pool::timer_initialize(map, ram, guest, kind)
}
pub fn event_set(map: &shadow::Shadow, ram: &memory::Memory, guest: u32) -> Result<bool, i32> {
    crate::kernel_pool::event_set(map, ram, guest)
}
pub fn event_reset(map: &shadow::Shadow, ram: &memory::Memory, guest: u32) -> Result<bool, i32> {
    crate::kernel_pool::event_reset(map, ram, guest)
}
pub fn event_wait(
    map: &shadow::Shadow,
    ram: &memory::Memory,
    guest: u32,
    milliseconds: u32,
) -> Result<(), i32> {
    crate::kernel_pool::event_wait(map, ram, guest, milliseconds)
}
pub fn timer_reset_after_set(
    map: &shadow::Shadow,
    ram: &memory::Memory,
    guest: u32,
) -> Result<bool, i32> {
    crate::kernel_pool::timer_reset_after_set(map, ram, guest)
}
pub fn timer_signal_after_callback(
    map: &shadow::Shadow,
    ram: &mut memory::Memory,
    guest: u32,
) -> Result<bool, i32> {
    crate::kernel_pool::timer_signal_after_callback(map, ram, guest)
}
pub fn event_remove(map: &shadow::Shadow, ram: &memory::Memory, guest: u32) -> Result<bool, i32> {
    crate::kernel_pool::event_remove(map, ram, guest)
}
pub fn event_close_all(map: &shadow::Shadow, ram: &memory::Memory) -> Result<(), i32> {
    crate::kernel_pool::event_close_all(map, ram)
}
