// Finite checked synthetic native routes. Never supplied as real title handlers.
use crate::dpc_observer;
use crate::irql;
use crate::memory;
use crate::registers;
pub fn dpc_route_supported(routine: u32) -> bool {
    routine == 0x1100 || routine == 0x2200
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
    if ram.read_u32(cpu.esp)? != 0 {
        return Err(-3);
    }
    let dpc = ram.read_u32(cpu.esp + 4)?;
    let context = ram.read_u32(cpu.esp + 8)?;
    let first = ram.read_u32(cpu.esp + 12)?;
    let second = ram.read_u32(cpu.esp + 16)?;
    if ram.read_u32(dpc + 20)? != first || ram.read_u32(dpc + 24)? != second {
        return Err(-3);
    }
    cpu.eax ^= routine ^ context;
    cpu.edx = first;
    cpu.esi = second;
    cpu.xmm7 = [first, second, context, dpc];
    cpu.mm3 = [routine, context];
    cpu.eflags ^= 0x844;
    cpu.fp_cmp = -1;
    cpu.fp_cc = 0x4000;
    let mode = ram.read_u32(0x741010)?;
    if mode >= 2 {
        cpu.fs_base = 0x742000;
    }
    if mode == 1 {
        task.irql_lower(ram, cpu, 0)?;
    }
    if mode == 2 {
        task.irql_raise(ram, cpu, 4)?;
    }
    if mode == 3 {
        task.irql_lower(ram, cpu, 1)?;
    }
    ram.write_u32(0x744000, cpu.eax)?;
    if ram.read_u32(0x741030)? == 1 {
        return Err(-5);
    }
    if ram.read_u32(0x741030)? != 2 {
        cpu.esp = cpu.esp.wrapping_add(20);
    }
    Ok(())
}
