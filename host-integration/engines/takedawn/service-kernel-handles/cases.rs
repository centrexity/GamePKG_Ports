// Checked deterministic inputs, shared by the maintainer oracle and game probe.
pub fn handle_case(index: u32) -> [u64; 3] {
    let seed = index.wrapping_mul(1664525).wrapping_add(1013904223);
    let slot = seed % 16383 + 1;
    match index % 4 {
        0 => [0, 0x100000000 + u64::from(slot - 1), 0],
        1 => [2, u64::from(0x48000000 | slot), 0],
        2 => [0, 0x200000000 + u64::from(index), 1],
        _ => [1, u64::from(0x48000000 | slot), 0],
    }
}
