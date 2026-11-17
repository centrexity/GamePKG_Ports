use crate::gamepkg;
use crate::kernel_pool;
use crate::memory;
pub struct Fixture {
    pub ram: memory::Memory,
    pub table: kernel_pool::Handles,
    pub events: kernel_pool::Events,
    pub semaphores: kernel_pool::Semaphores,
    pub services: kernel_pool::KernelServices,
}
pub fn mixed_word(bytes: &[u8], at: usize) -> Result<u64, i32> {
    let part = bytes.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        part[0], part[1], part[2], part[3], part[4], part[5], part[6], part[7],
    ]))
}
pub fn mixed_need(code: i32, value: bool) -> Result<(), i32> {
    if value {
        Ok(())
    } else {
        Err(-code)
    }
}
fn mixed_commit(window: u64, bytes: u64) -> Result<(), i32> {
    let states = gamepkg::local_memory_page_states(window, 0, (bytes / 4096) as u32)?;
    let mut ready = 0usize;
    for state in &states {
        if *state == 3 {
            ready += 1;
        } else if *state != 0 {
            return Err(-4);
        }
    }
    if ready == states.len() {
        return Ok(());
    }
    if ready != 0 {
        return Err(-4);
    }
    gamepkg::local_call(
        2,
        &[
            u64::to_le_bytes(window).as_slice(),
            &u64::to_le_bytes(0),
            &u64::to_le_bytes(bytes),
            &u32::to_le_bytes(2),
        ]
        .concat(),
        &mut [],
    )?;
    Ok(())
}
impl Fixture {
    pub fn mixed_fixture_new() -> Result<Self, i32> {
        let mut ram = memory::Memory::new()?;
        let services = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2)?;
        let mut info = [0u8; 64];
        mixed_need(701, gamepkg::local_call(1, &[], &mut info)? == 64)?;
        let main = mixed_word(&info, 0)?;
        let scratch = mixed_word(&info, 48)?;
        mixed_commit(main, 4096)?;
        mixed_commit(scratch, 1048576)?;
        ram.attach(0x10000, 4096, main, 0, true)?;
        let mut table = kernel_pool::Handles::handles_new(&ram)?;
        table.handles_bind(&ram, scratch, 0)?;
        let events = kernel_pool::Events::events_new(&ram, &table)?;
        let semaphores = kernel_pool::Semaphores::semaphores_new(&ram, &table)?;
        Ok(Self {
            ram,
            table,
            events,
            semaphores,
            services,
        })
    }
    pub fn mixed_event(&mut self, manual: bool, initial: u32) -> Result<[u64; 2], i32> {
        let mut incoming = self.events.events_begin(&self.ram, &self.table)?;
        let created = incoming.event_create(&self.ram, if manual { 0 } else { 1 }, initial);
        if let Err(code) = created {
            incoming.event_creation_close(&self.ram)?;
            return Err(code);
        }
        let id = incoming.event_creation_id();
        let result = incoming.event_publish(&mut self.ram, 0x10000);
        let closed = incoming.event_creation_close(&self.ram);
        let token = result?;
        closed?;
        Ok([id, u64::from(token)])
    }
    pub fn mixed_semaphore(&mut self, initial: u32, maximum: u32) -> Result<[u64; 2], i32> {
        let mut incoming = self.semaphores.semaphores_begin(&self.ram, &self.table)?;
        let created = incoming.semaphore_create(&self.ram, initial, maximum);
        if let Err(code) = created {
            incoming.semaphore_creation_close(&self.ram)?;
            return Err(code);
        }
        let id = incoming.semaphore_creation_id();
        let result = incoming.semaphore_publish(&mut self.ram, 0x10000);
        let closed = incoming.semaphore_creation_close(&self.ram);
        let token = result?;
        closed?;
        Ok([id, u64::from(token)])
    }
    pub fn mixed_fixture_close(&mut self) -> Result<(), i32> {
        self.semaphores.semaphores_close(&self.ram, &self.table)?;
        self.events.events_close(&self.ram, &self.table)?;
        self.table.handles_close(&self.ram)?;
        self.services.service_close(&self.ram)?;
        gamepkg::local_record_release(self.ram.call_handle())
    }
}
