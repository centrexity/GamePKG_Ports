use crate::cases;
use crate::gamepkg;
use crate::gate;
use crate::irql;
use crate::memory;
use crate::registers;
fn probe_need(ok: bool) -> Result<(), i32> {
    if ok {
        Ok(())
    } else {
        Err(-3)
    }
}
pub fn probe_case(
    ram: &mut memory::Memory,
    window: u64,
    index: u32,
) -> Result<std::vec::Vec<u8>, i32> {
    gamepkg::local_memory_fill(window, 0, 65536, 0xa5)?;
    ram.write_u32(0x741000, 0)?;
    ram.write_u8(0x741008, (index & 1) as u8)?;
    let mut owner = irql::Controller::controller_new(ram)?;
    let mut task = owner.controller_task(ram)?;
    probe_need(task.task_key() == owner.controller_task(ram)?.task_key())?;
    let (version, mut state) = gamepkg::local_record_snapshot(owner.controller_id(), 2176)?;
    let transitions = if index % 31 == 0 { u32::MAX - 3 } else { 0 };
    let value = u64::to_le_bytes(u64::from(transitions));
    for lane in 0..8usize {
        state[48 + lane] = value[lane];
    }
    gamepkg::local_record_compare_exchange(owner.controller_id(), version, &state)?;
    let mut cpu = registers::Registers::new();
    cpu.fs_base = if index & 4 == 0 { 0 } else { 0x740000 };
    cpu.eflags = index.wrapping_mul(1664525).wrapping_add(1013904223);
    cpu.mm3 = [index, !index];
    cpu.xmm7 = [index, 1, 2, 3];
    let original = cpu.encode();
    let mut results = std::vec::Vec::new();
    results.resize(160, 0u8);
    let mut saved = 0i32;
    for step in 0..20u32 {
        let input = cases::case_operation(index, step);
        let result = match input[0] {
            0 => u64::from(task.irql_raise(ram, &mut cpu, input[1] as u8)?),
            1 => {
                task.irql_lower(ram, &mut cpu, input[1] as u8)?;
                0
            }
            2 => {
                task.irql_disable(ram, &mut cpu)?;
                0
            }
            3 => {
                task.irql_enable_after_shadow(ram, &mut cpu)?;
                0
            }
            4 => u64::from(task.irql_to_dpc(ram, &mut cpu)?),
            5 => {
                task.irql_gate_enter(ram, 0)?;
                0
            }
            6 => {
                task.irql_gate_leave(ram, &mut cpu)?;
                0
            }
            7 => {
                task.irql_set_isr(ram, input[1] != 0)?;
                0
            }
            8 => {
                saved = task.irql_enter_interrupt(ram, &mut cpu, input[1])?;
                saved as u64
            }
            9 => {
                task.irql_leave_interrupt(ram, &mut cpu, saved)?;
                0
            }
            _ => return Err(-1),
        };
        let value = u64::to_le_bytes(result);
        for lane in 0..8usize {
            results[step as usize * 8 + lane] = value[lane];
        }
    }
    let mut expected_cpu = original;
    let flag = u32::to_le_bytes(cpu.eflags);
    for lane in 0..4usize {
        expected_cpu[244 + lane] = flag[lane];
    }
    probe_need(cpu.encode() == expected_cpu)?;
    let mut captured_ram = std::vec::Vec::new();
    captured_ram.resize(65536, 0u8);
    ram.read_bytes(0x740000, &mut captured_ram)?;
    let v = task.task_values(ram)?;
    let counts = owner.controller_counts(ram)?;
    let lock = gate::Gate::gate_borrow(ram, owner.controller_gate())?;
    let values = [
        cases::case_hash(&captured_ram),
        cases::case_hash(&results),
        v[0],
        v[1],
        v[2],
        v[3],
        v[4],
        v[5],
        counts[0],
        counts[1],
        counts[2],
        lock.gate_state(ram)?[1],
        u64::from(cpu.eflags),
        u64::from(task.irql_blocks_dpcs(ram)?),
        u64::from(task.irql_interrupts_enabled(ram)?),
        u64::from(ram.read_u32(0x741000)?),
    ];
    let mut bytes = std::vec::Vec::new();
    bytes.resize(128, 0u8);
    for lane in 0..16usize {
        let value = u64::to_le_bytes(values[lane]);
        for byte in 0..8usize {
            bytes[lane * 8 + byte] = value[byte];
        }
    }
    probe_need(counts[0] == 0 && counts[1] == 0)?;
    owner.controller_close(ram)?;
    probe_need(owner.controller_closed() && task.task_values(ram) == Err(-4))?;
    Ok(bytes)
}
