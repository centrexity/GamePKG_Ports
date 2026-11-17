use crate::event_import_probe;
use crate::gamepkg;
use crate::kernel_pool;
use crate::layout;
use crate::memory;
pub struct Fixture {
    pub ram: memory::Memory,
    pub table: kernel_pool::Handles,
    pub domain: kernel_pool::Events,
    context: kernel_pool::KernelContext,
    registry: kernel_pool::Connections,
    shadow: kernel_pool::Shadow,
    services: kernel_pool::KernelServices,
}
impl Fixture {
    pub fn fixture_new() -> Result<Self, i32> {
        let root = memory::Memory::new()?;
        let topology = layout::Layout::layout_services(&root)?;
        let banks = topology.layout_banks();
        let scratch = topology.layout_scratch()?;
        for (window, bytes) in [(banks[0], 67108864u64), (scratch, 1048576)] {
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
        }
        let mut ram = topology.layout_view(&root)?;
        ram.write_u32(0x4008, 0x780000)?;
        let services = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2)?;
        let registry = kernel_pool::Connections::connections_new(&ram)?;
        let shadow = kernel_pool::Shadow::shadow_new(&ram)?;
        let mut table = kernel_pool::Handles::handles_new(&ram)?;
        table.handles_bind(&ram, scratch, 0)?;
        let domain = kernel_pool::Events::events_new(&ram, &table)?;
        let context = kernel_pool::KernelContext::kernel_context_new_with_objects(
            &ram,
            &registry,
            &shadow,
            table.handles_id(),
            domain.events_id(),
        )?;
        let mut plan = [0u8; 44];
        for (at, value) in [
            (0usize, 0x344c504bu32),
            (4, 2),
            (16, 0x20000),
            (20, 2),
            (32, 189),
            (36, 225),
        ] {
            let encoded = u32::to_le_bytes(value);
            for lane in 0..4usize {
                plan[at + lane] = encoded[lane];
            }
        }
        let encoded = u64::to_le_bytes(ram.call_handle());
        for lane in 0..8usize {
            plan[8 + lane] = encoded[lane];
        }
        let encoded = u64::to_le_bytes(context.kernel_context_id());
        for lane in 0..8usize {
            plan[24 + lane] = encoded[lane];
        }
        ram.write_u32(0x20000, 0xfe000000)?;
        ram.write_u32(0x20004, 0xfe000004)?;
        ram.kernel_install(gamepkg::local_record_create(&plan)?)?;
        Ok(Self {
            ram,
            table,
            domain,
            context,
            registry,
            shadow,
            services,
        })
    }
    pub fn fixture_close(&mut self) -> Result<(), i32> {
        self.ram.kernel_clear()?;
        self.context.kernel_context_close(&self.ram)?;
        self.domain.events_close(&self.ram, &self.table)?;
        self.table.handles_close(&self.ram)?;
        kernel_pool::event_close_all(&self.shadow, &self.ram)?;
        self.shadow.shadow_close(&self.ram)?;
        self.registry.connections_close(&self.ram)?;
        self.services.service_close(&self.ram)?;
        gamepkg::local_record_release(self.ram.call_handle())
    }
}
pub fn event_import_run() -> Result<std::vec::Vec<u8>, i32> {
    let mut fixture = Fixture::fixture_new()?;
    event_import_probe::event_import_controls(
        &mut fixture.ram,
        &fixture.table,
        &fixture.domain,
        [0, 1],
    )?;
    let vectors = event_import_probe::event_import_vectors(
        &mut fixture.ram,
        &fixture.table,
        &fixture.domain,
        [0, 1],
    )?;
    fixture.fixture_close()?;
    Ok(vectors)
}
