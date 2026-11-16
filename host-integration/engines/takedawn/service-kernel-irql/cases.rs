pub fn case_operation(index: u32, step: u32) -> [i32; 2] {
    match step {
        0 => [0, (index % 256) as i32],
        1 => [0, ((index >> 3) % 256) as i32],
        2 => [1, ((index >> 5) % 256) as i32],
        3 => [1, 0],
        4 | 5 => [2, 0],
        6 => [4, 0],
        7 => [1, 0],
        8 => [3, 0],
        9 => [5, 0],
        10 => [7, 1],
        11 => [8, 16],
        12 => [9, 0],
        13 => [7, 0],
        14 => [6, 0],
        15 => [8, if index & 2 == 0 { -1 } else { -258 }],
        16 => [9, 0],
        17 => [4, 0],
        18 => [4, 0],
        _ => [1, 0],
    }
}
pub fn case_hash(bytes: &[u8]) -> u64 {
    let mut value = 14695981039346656037u64;
    for byte in bytes {
        value = (value ^ u64::from(*byte)).wrapping_mul(1099511628211);
    }
    value
}
