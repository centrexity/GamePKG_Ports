use crate::registers;
pub fn irq_cpu(index: u32) -> Result<registers::Registers, i32> {
    let mut bytes = std::vec::Vec::new();
    bytes.resize(316, 0u8);
    let mut random = index.wrapping_add(0x9e3779b9);
    for lane in 0..79usize {
        random = random.wrapping_mul(1664525).wrapping_add(1013904223);
        let word = u32::to_le_bytes(random);
        for byte in 0..4usize {
            bytes[lane * 4 + byte] = word[byte];
        }
    }
    let top = u32::to_le_bytes(index & 7);
    let tib_bytes = u32::to_le_bytes(0x740000);
    let flags = u32::to_le_bytes(random | 0x200);
    for lane in 0..4usize {
        bytes[40 + lane] = top[lane];
        bytes[240 + lane] = tib_bytes[lane];
        bytes[244 + lane] = flags[lane];
    }
    let mut cpu = registers::Registers::new();
    cpu.decode(&bytes)?;
    Ok(cpu)
}
pub fn irq_hash(bytes: &[u8]) -> u64 {
    let mut value = 14695981039346656037u64;
    for byte in bytes {
        value = (value ^ u64::from(*byte)).wrapping_mul(1099511628211);
    }
    value
}
