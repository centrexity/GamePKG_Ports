// Production checked startup composition: track before any periodic write.
use crate::kernel_data;
use crate::kernel_startup;
use crate::memory;
use crate::tick_service;

// All partial resources stay in the two caller-owned values on error. Close
// the tick (with shared actual join evidence) before closing kernel startup.
// No callback value, new task entry, SDK operation or host exemption is added.
pub fn startup_tick(
    owner: &mut kernel_startup::Startup,
    clock: &mut tick_service::Tick,
    ram: &memory::Memory,
    data: &kernel_data::Data,
    entry: u32,
) -> Result<(), i32> {
    owner.startup_ids(ram)?;
    clock.tick_allocate(ram, data)?;
    let argument = clock.tick_argument(ram)?;
    let task = owner.startup_spawn(ram, entry, argument)?;
    clock.tick_assign(ram, task)?;
    clock.tick_resume(ram)
}
