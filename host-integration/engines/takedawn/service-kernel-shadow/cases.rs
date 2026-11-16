pub fn shadow_case(index: u32) -> [u64; 3] {
    let seed = index.wrapping_mul(1664525).wrapping_add(1013904223);
    let mode = if index < 5 {
        match index {
            0 => 2,
            1 | 2 | 4 => 0,
            _ => 1,
        }
    } else {
        index % 3
    };
    let guest = match index {
        0 => 0x1000 + 17 * 4,
        1 => 0x1000 + 4095 * 4,
        2 | 3 => 0x1000 + 9000 * 4,
        4 => 0x1000 + 9001 * 4,
        _ => 0x1000 + ((seed >> 5) % 4160) * 4,
    };
    [
        u64::from(mode),
        u64::from(guest),
        0x8000000100000000u64 + u64::from(index),
    ]
}
pub fn shadow_trace(mut hash: u64, token: u64) -> u64 {
    for byte in u64::to_le_bytes(token) {
        hash = (hash ^ u64::from(byte)).wrapping_mul(1099511628211);
    }
    hash
}
pub fn event_case(index: u32) -> [u32; 4] {
    let seed = index.wrapping_mul(1664525).wrapping_add(1013904223);
    if index < 16 {
        return [0, index, index % 4, index % 3];
    }
    if index < 32 {
        return [1, index - 16, index % 4, 0];
    }
    [
        (index - 32) % 8,
        (seed >> 4) % 16,
        (seed >> 6) % 4,
        index % 3,
    ]
}
