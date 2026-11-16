pub fn stacks_case(index: u32) -> [u32; 3] {
    let bytes = [
        0u32,
        65536,
        196608,
        262144,
        1048576,
        8 * 1048576,
        5 * 262144 + 65536,
        7 * 262144,
    ][index as usize % 8];
    [
        if index & 8 == 0 { 0x780000 } else { 0x1000000 },
        bytes,
        index.wrapping_mul(1664525).wrapping_add(1013904223),
    ]
}
pub fn stacks_operation(index: u32, step: u32) -> [u32; 2] {
    let random = index
        .wrapping_mul(1664525)
        .wrapping_add(step.wrapping_mul(1013904223));
    if step < 32 {
        [0, 0]
    } else if (64..96).contains(&step) {
        [1, (step - 64) % 16]
    } else {
        [random & 1, (random >> 16) % 16]
    }
}
