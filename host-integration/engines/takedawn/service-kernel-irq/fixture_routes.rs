// Controlled finite native handlers, not game/device implementations.
use crate::gate;
use crate::irq_delivery;
use crate::irql;
use crate::memory;
use crate::registers;
pub fn supported_mask() -> u32 {
    7
}
fn route_call(
    queue: &irq_delivery::Interrupts,
    task: &mut irql::Task,
    ram: &mut memory::Memory,
    cpu: &mut registers::Registers,
    line: u32,
) -> Result<(), i32> {
    let count = ram.read_u32(0x741000)?;
    if count >= 64 {
        return Err(-1);
    }
    let (_, b) = crate::gamepkg::local_record_snapshot(queue.irq_id(), 384)?;
    let id = u64::from_le_bytes([b[32], b[33], b[34], b[35], b[36], b[37], b[38], b[39]]);
    let lock = gate::Gate::gate_borrow(ram, id)?;
    let values = [
        line,
        cpu.esp,
        u32::from(
            cpu.eax == 0
                && cpu.ebx == 0
                && cpu.ecx == 0
                && cpu.edx == 0
                && cpu.esi == 0
                && cpu.edi == 0,
        ),
        u32::from(task.irql_current(ram)?),
        task.task_values(ram)?[4] as u32,
        lock.gate_state(ram)?[1] as u32,
        u32::from(ram.read_u8(cpu.fs_base + 0x24)?),
        cpu.eflags,
    ];
    let mut bytes = [0u8; 32];
    for lane in 0..8usize {
        let value = u32::to_le_bytes(values[lane]);
        for byte in 0..4usize {
            bytes[lane * 4 + byte] = value[byte];
        }
    }
    ram.write_bytes(0x744000 + count * 32, &bytes)?;
    ram.write_u32(0x741000, count + 1)?;
    cpu.eax = 0x31415926;
    cpu.mm7 = [0x80000000, 0xffffffff];
    cpu.xmm1 = [9, 8, 7, 6];
    cpu.fp_stack[3] = f64::from_bits(0x7ff8000000001234);
    cpu.eflags ^= 0x1234;
    let scenario = ram.read_u32(0x741024)?;
    if line == 0 && count == 0 && (scenario == 7 || scenario == 12) {
        let command = ram.read_u64(0x741040)?;
        let response = ram.read_u64(0x741048)?;
        if scenario == 7 {
            crate::gamepkg::local_mailbox_send(command, &u64::to_le_bytes(0))?;
            let mut b = [0u8; 24];
            if crate::gamepkg::local_mailbox_receive(response, 1000, &mut b)? != 24
                || u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]) != 3
            {
                return Err(-3);
            }
        } else {
            crate::gamepkg::local_mailbox_send(
                response,
                &[
                    u64::to_le_bytes(2),
                    u64::to_le_bytes(0),
                    u64::to_le_bytes(0),
                ]
                .concat(),
            )?;
            let mut b = [0u8; 8];
            crate::gamepkg::local_mailbox_receive(command, 1000, &mut b)?;
            return Err(-3);
        }
    }
    if ram.read_u32(0x741024)? == 1 {
        cpu.fs_base = 0x740100;
    }
    if ram.read_u32(0x741024)? == 6 {
        crate::gamepkg::local_record_release(ram.services_handle()?)?;
    }
    if ram.read_u32(0x741024)? == 2 {
        return Err(-5);
    }
    task.irql_raise(ram, cpu, 2)?;
    Ok(())
}
pub fn vblank(
    queue: &irq_delivery::Interrupts,
    task: &mut irql::Task,
    ram: &mut memory::Memory,
    cpu: &mut registers::Registers,
) -> Result<(), i32> {
    route_call(queue, task, ram, cpu, 0)
}
pub fn apu(
    queue: &irq_delivery::Interrupts,
    task: &mut irql::Task,
    ram: &mut memory::Memory,
    cpu: &mut registers::Registers,
) -> Result<(), i32> {
    route_call(queue, task, ram, cpu, 1)
}
pub fn ohci(
    queue: &irq_delivery::Interrupts,
    task: &mut irql::Task,
    ram: &mut memory::Memory,
    cpu: &mut registers::Registers,
) -> Result<(), i32> {
    route_call(queue, task, ram, cpu, 2)
}
