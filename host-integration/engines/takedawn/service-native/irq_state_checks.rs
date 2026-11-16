// Checked game-source preservation controls, with no native reference hooks.
use crate::registers;
fn irq_sample(index: u32) -> std::vec::Vec<u8> {
    let mut seed = u64::from(index) ^ 0x6427d8fe351ab902;
    let mut bytes = std::vec::Vec::new();
    bytes.resize(316, 0u8);
    for index in 0..316usize {
        seed ^= seed.wrapping_shl(13);
        seed ^= seed.wrapping_shr(7);
        seed ^= seed.wrapping_shl(17);
        bytes[index] = seed as u8;
    }
    let top = u32::to_le_bytes(index % 8);
    for lane in 0..4usize {
        bytes[40 + lane] = top[lane];
    }
    bytes
}
pub fn verify() -> Result<(), i32> {
    let mut state = registers::Registers::new();
    for index in 0..4096u32 {
        let before = irq_sample(index);
        let changed = irq_sample(index + 0x10000);
        state.decode(&before)?;
        state.irq_prepare(0x12340000 + index);
        let mut expected = before.clone();
        for at in [0usize, 4, 8, 12, 16, 20] {
            for lane in 0..4usize {
                expected[at + lane] = 0;
            }
        }
        let stack = u32::to_le_bytes(0x12340000 + index);
        for lane in 0..4usize {
            expected[28 + lane] = stack[lane];
        }
        if state.encode() != expected {
            return Err(-3);
        }
        state.decode(&changed)?;
        state.irq_restore(&before)?;
        for lane in 0..4usize {
            expected[240 + lane] = changed[240 + lane];
        }
        for at in [0usize, 4, 8, 12, 16, 20, 28] {
            for lane in 0..4usize {
                expected[at + lane] = before[at + lane];
            }
        }
        if state.encode() != expected {
            return Err(-3);
        }
        let previous = state.encode();
        let mut invalid = before;
        invalid[40] = 8;
        if state.irq_restore(&invalid) != Err(-1) || state.encode() != previous {
            return Err(-3);
        }
    }
    Ok(())
}
