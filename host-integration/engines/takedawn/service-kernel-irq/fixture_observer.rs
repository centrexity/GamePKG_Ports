// Controlled observational hooks only, no real timer/DPC/device integration.
use crate::gamepkg;
use crate::irq_delivery;
use crate::irql;
use crate::memory;
use crate::registers;
fn observer_increment(ram: &mut memory::Memory, address: u32) -> Result<(), i32> {
    let value = ram.read_u32(address)?.wrapping_add(1);
    ram.write_u32(address, value)
}
fn observer_pause(ram: &memory::Memory) -> Result<(), i32> {
    let reply = ram.read_u64(0x741048)?;
    let command = ram.read_u64(0x741040)?;
    gamepkg::local_mailbox_send(
        reply,
        &[
            u64::to_le_bytes(2),
            u64::to_le_bytes(0),
            u64::to_le_bytes(0),
        ]
        .concat(),
    )?;
    let mut b = [0u8; 8];
    if gamepkg::local_mailbox_receive(command, 1000, &mut b)? != 8 {
        return Err(-3);
    }
    Ok(())
}
pub fn post_window(
    _queue: &irq_delivery::Interrupts,
    _task: &mut irql::Task,
    ram: &mut memory::Memory,
    _cpu: &mut registers::Registers,
    _line: u32,
) -> Result<(), i32> {
    if ram.read_u32(0x741028)? == 1 {
        observer_pause(ram)?;
    }
    Ok(())
}
// Called only by the explicitly instrumented checked test copy, not irq.rs.
pub fn after_mark_fixture(
    _queue: &irq_delivery::Interrupts,
    _task: &mut irql::Task,
    ram: &mut memory::Memory,
    _cpu: &mut registers::Registers,
    _line: u32,
) -> Result<(), i32> {
    let mode = ram.read_u32(0x741028)?;
    if mode == 2 || mode == 3 {
        observer_pause(ram)?;
    }
    Ok(())
}
pub fn release_window(
    _queue: &irq_delivery::Interrupts,
    _task: &mut irql::Task,
    ram: &mut memory::Memory,
    _cpu: &mut registers::Registers,
) -> Result<(), i32> {
    if ram.read_u32(0x741028)? == 4 {
        gamepkg::local_mailbox_send(ram.read_u64(0x741040)?, &u64::to_le_bytes(0))?;
        let mut b = [0u8; 24];
        if gamepkg::local_mailbox_receive(ram.read_u64(0x741048)?, 1000, &mut b)? != 24
            || u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]) != 3
        {
            return Err(-3);
        }
        let value = u64::from_le_bytes([b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]]);
        ram.write_u32(0x741038, value as u32)?;
    }
    Ok(())
}
pub fn spin_wake(
    _queue: &irq_delivery::Interrupts,
    _task: &mut irql::Task,
    ram: &mut memory::Memory,
) -> Result<(), i32> {
    observer_increment(ram, 0x741010)
}
pub fn warn(
    _queue: &irq_delivery::Interrupts,
    _task: &mut irql::Task,
    ram: &mut memory::Memory,
    _kind: u32,
    _old: u8,
    _level_next: u8,
) -> Result<(), i32> {
    observer_increment(ram, 0x741014)
}
pub fn dpc_release(
    _queue: &irq_delivery::Interrupts,
    _task: &mut irql::Task,
    ram: &mut memory::Memory,
) -> Result<(), i32> {
    observer_increment(ram, 0x741018)
}
pub fn wait(
    _queue: &irq_delivery::Interrupts,
    ram: &memory::Memory,
    _pending: u32,
    _oldest: u64,
    _now: u64,
    _refused: u32,
) -> Result<(), i32> {
    let backing = ram.memory_writable_word_backing(0x741020)?;
    let value = ram.read_u32(0x741020)?.wrapping_add(1);
    if gamepkg::local_call(
        6,
        &[
            u64::to_le_bytes(backing[0]).as_slice(),
            &u64::to_le_bytes(backing[1]),
            &u32::to_le_bytes(value),
        ]
        .concat(),
        &mut [],
    )? != 0
    {
        return Err(-3);
    }
    Ok(())
}

// Checked test-copy fault seam: stale the first cancelled actor's abort CAS once.
pub fn abort_cas_fixture(controller: u64, context: u64) -> Result<(), i32> {
    if !gamepkg::local_task_status()?.1 || context == 0 {
        return Ok(());
    }
    for _ in 0..64u32 {
        let (version, mut b) = gamepkg::local_record_snapshot(context, 384)?;
        if b.len() != 384 {
            return Err(-1);
        }
        let flag = u64::from_le_bytes([b[96], b[97], b[98], b[99], b[100], b[101], b[102], b[103]]);
        if flag != 0 {
            return Ok(());
        }
        b[96] = 1;
        match gamepkg::local_record_compare_exchange(context, version, &b) {
            Ok(_) => {
                let (version, current) = gamepkg::local_record_snapshot(controller, 2176)?;
                gamepkg::local_record_compare_exchange(controller, version, &current)?;
                return Ok(());
            }
            Err(-2) => {}
            Err(code) => return Err(code),
        }
    }
    Err(-2)
}
