pub fn wait_case(index: u32) -> [u64; 3] {
    let edges = [
        0,
        1,
        2,
        999999999,
        1000000000,
        4294967295,
        4294967296,
        i64::MAX as u64,
    ];
    let rates = [
        1,
        3,
        10000000,
        1000000000,
        4294967295,
        4294967296,
        1u64 << 62,
        i64::MAX as u64,
    ];
    let seed = u64::from(index)
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    let counter = if index < 512 {
        edges[(index & 7) as usize]
    } else {
        seed & i64::MAX as u64
    };
    let frequency = rates[((index >> 3) & 7) as usize];
    let skew = if index < 512 {
        edges[((index >> 6) & 7) as usize]
    } else {
        seed.rotate_left(29)
    };
    [counter, frequency, skew]
}
