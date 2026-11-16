// Controlled finite native routine; never a production title handler.
use crate::dpc_observer;
use crate::events;
use crate::gamepkg;
use crate::irql;
use crate::memory;
use crate::registers;
use crate::shadow;
use crate::timers;
fn firing_need(value: bool) -> Result<(), i32> {
    if value {
        Ok(())
    } else {
        Err(-3)
    }
}
pub fn dpc_route_supported(routine: u32) -> bool {
    routine == 0x1100
}
pub fn dpc_route_call(
    task: &mut irql::Task,
    ram: &mut memory::Memory,
    cpu: &mut registers::Registers,
    routine: u32,
) -> Result<(), i32> {
    if !dpc_route_supported(routine) {
        return Err(-5);
    }
    dpc_observer::callback_trace(task, ram, cpu, 3)?;
    let table = timers::Timers::timers_borrow(ram, ram.read_u64(0x741080)?)?;
    let map_view = shadow::Shadow::shadow_borrow(ram, ram.read_u64(0x741088)?)?;
    let bytes = table.timers_state(ram)?;
    firing_need(bytes[64..68] == [0u8; 4])?;
    let id = events::event_lookup(&map_view, ram, 0x740500)?.ok_or(-3)?;
    firing_need(!gamepkg::local_event_state(id)?.signaled && ram.read_u32(0x740504)? == 9)?;
    // The foreign producer must acquire the actual mutex while native code runs.
    let mut request = std::vec::Vec::new();
    request.extend(u64::to_le_bytes(1));
    request.resize(512, 0);
    gamepkg::local_mailbox_send(ram.read_u64(0x7410a8)?, &request)?;
    let mut ack = [0u8; 8];
    firing_need(
        gamepkg::local_mailbox_receive(ram.read_u64(0x7410a0)?, 1000, &mut ack)? == 8
            && ack == [0u8; 8],
    )?;
    let mode = ram.read_u32(0x7410b0)?;
    if mode == 1 {
        events::timer_initialize(&map_view, ram, 0x740500, 1)?;
        firing_need(gamepkg::local_event_state(id).err() == Some(-4))?;
        table.timer_set(ram, 0x740500, -100000, 10, 0x740100)?;
        events::timer_reset_after_set(&map_view, ram, 0x740500)?;
        table.timer_set(ram, 0x740540, 0, 0, 0x740120)?;
        table.timer_cancel(ram, 0x740580)?;
    }
    if mode == 8 {
        firing_need(events::event_remove(&map_view, ram, 0x740500)?)?;
    }
    let dpc = ram.read_u32(cpu.esp + 4)?;
    let context = ram.read_u32(cpu.esp + 8)?;
    firing_need(
        ram.read_u32(cpu.esp)? == 0
            && dpc == 0x740100
            && context == 0xa54e12b8
            && ram.read_u32(cpu.esp + 12)? == 0
            && ram.read_u32(cpu.esp + 16)? == 0
            && ram.read_u32(dpc + 20)? == 0
            && ram.read_u32(dpc + 24)? == 0,
    )?;
    cpu.eax ^= routine ^ context;
    cpu.edx = 0;
    cpu.esi = 0;
    cpu.xmm7 = [0, 0, context, dpc];
    cpu.mm3 = [routine, context];
    cpu.eflags ^= 0x844;
    cpu.fp_cmp = -1;
    cpu.fp_cc = 0x4000;
    if mode == 3 {
        return Err(-5);
    }
    cpu.esp = cpu.esp.wrapping_add(20);
    Ok(())
}
