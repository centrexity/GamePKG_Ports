use crate::gamepkg;
use crate::memory;
use crate::queue;
use crate::registers;
pub fn drain_count(group: u32) -> u32 {
    [0u32, 1, 3, 5][(group / 24) as usize]
}
pub fn drain_guard(group: u32) -> i64 {
    [0i64, 1, 1200, 4000, 4001, -2][(group % 6) as usize]
}
pub fn drain_lag(group: u32) -> i32 {
    [0i32, 1, 10, -1][((group / 6) % 4) as usize]
}
pub fn drain_busy_case(group: u32, step: u32) -> bool {
    step == 1 && group % 7 == 0
}
pub fn drain_profile(group: u32) -> u32 {
    (group >> 1) & 1
}
pub fn drain_initial(top: u32) -> registers::Registers {
    let mut cpu = registers::Registers::new();
    cpu.esp = top;
    cpu.fs_base = 0x740000;
    cpu.eax = 0xa5a50001;
    cpu.edx = 0xabcdef01;
    cpu.esi = 0xffffffff;
    cpu.xmm7 = [8, 7, 6, 5];
    cpu.mm3 = [0x80000000, 0x12345678];
    cpu
}
pub fn drain_hash(bytes: &[u8]) -> u64 {
    let mut hash = 14695981039346656037u64;
    for byte in bytes {
        hash = (hash ^ u64::from(*byte)).wrapping_mul(1099511628211);
    }
    hash
}
fn case_put(bytes: &mut [u8], at: usize, value: u64) {
    bytes[at..at + 8].copy_from_slice(&u64::to_le_bytes(value));
}
pub fn drain_seed(
    group: u32,
    ram: &mut memory::Memory,
    pending: &queue::Queue,
    top: u32,
) -> Result<registers::Registers, i32> {
    let mut empty = std::vec::Vec::new();
    empty.resize(524288, 0);
    ram.write_bytes(0x740000, &empty)?;
    let (version, mut bytes) = gamepkg::local_record_snapshot(pending.queue_id(), 1616)?;
    let position = u64::from((group * 17) % 64);
    case_put(&mut bytes, 56, position);
    case_put(&mut bytes, 64, position);
    for lane in 80..1616usize {
        bytes[lane] = 0;
    }
    gamepkg::local_record_compare_exchange(pending.queue_id(), version, &bytes)?;
    ram.write_u32(0x741004, drain_profile(group))?;
    ram.write_u32(0x741010, (group >> 2) & 3)?;
    ram.write_u32(0x741050, u32::from(group % 5 == 0))?;
    for slot in 0..drain_count(group) {
        let dpc = 0x740100 + slot * 32;
        ram.write_u32(
            dpc + 12,
            if slot == 2 {
                0
            } else if slot & 1 == 0 {
                0x1100
            } else {
                0x2200
            },
        )?;
        ram.write_u32(dpc + 16, 0xa54e12b8u32.wrapping_add(group * 17 + slot))?;
        if pending.queue_insert(
            ram,
            dpc,
            0x11223344 ^ slot,
            0xff004400u32.wrapping_sub(slot * 31),
        )? != [1, 0]
        {
            return Err(-3);
        }
    }
    if drain_count(group) >= 3 && !pending.queue_remove(ram, 0x740120)? {
        return Err(-3);
    }
    if group % 3 == 0 {
        let (version, mut b) = gamepkg::local_record_snapshot(pending.queue_id(), 1616)?;
        for slot in 0..drain_count(group) {
            let at = 80 + ((position + u64::from(slot)) % 64) as usize * 24;
            case_put(&mut b, at + 16, 1 + u64::from(slot) * 100);
        }
        gamepkg::local_record_compare_exchange(pending.queue_id(), version, &b)?;
    }
    Ok(drain_initial(top))
}
pub fn drain_step_values(group: u32, step: u32) -> [u64; 4] {
    let guard = crate::draining::drain_guard_ns(drain_guard(group));
    let base = if group % 11 == 0 && guard > 1 {
        u64::MAX - guard / 2
    } else {
        100000000
    };
    let due = if group % 11 == 0 && guard > 1 {
        u64::MAX
    } else {
        base + guard + 1
    };
    match step {
        0 => [0, base, 0, base.wrapping_add(guard)],
        1 => [10000000, base, 1, due],
        2 => [10000001, due, 0, due],
        _ => [100000000, 0, 0, 0],
    }
}
pub fn drain_step_seed(group: u32, step: u32, ram: &mut memory::Memory) -> Result<u64, i32> {
    let values = drain_step_values(group, step);
    ram.write_u64(0x741060, values[0])?;
    ram.write_u64(0x741068, 0)?;
    ram.write_u32(0x741070, 0)?;
    ram.write_u64(0x741078, values[1])?;
    ram.write_u64(0x741080, values[2])?;
    ram.write_u32(0x741088, 0)?;
    Ok(values[3])
}
