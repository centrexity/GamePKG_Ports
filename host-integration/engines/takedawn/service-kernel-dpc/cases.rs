pub fn queue_case(index: u32) -> [u32; 4] {
    let base = 0x740100u32;
    if index < 63 {
        return [0, base + index * 32, index.wrapping_mul(0x1020304), !index];
    }
    if index == 63 {
        return [0, base + 63 * 32, 0xfedcba98, 0x87654321];
    }
    if index == 64 {
        return [0, base, 1, 2];
    }
    if index == 65 {
        return [1, base, 0, 0];
    }
    if index == 66 {
        return [0, base + 63 * 32, 3, 4];
    }
    if index == 67 {
        return [2, 0, 0, 0];
    }
    if index == 68 {
        return [0, base + 63 * 32, 3, 4];
    }
    if index < 134 {
        return [2, 0, 0, 0];
    }
    let seed = index.wrapping_mul(1664525).wrapping_add(1013904223);
    let dpc = if index % 37 == 0 {
        0
    } else {
        base + ((seed >> 7) % 64) * 32
    };
    match index % 7 {
        0 | 1 | 2 => [0, dpc, seed, seed.rotate_left(17)],
        3 | 4 => [1, dpc, 0, 0],
        _ => [2, 0, 0, 0],
    }
}
pub fn queue_hash(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash = (hash ^ u64::from(*byte)).wrapping_mul(1099511628211);
    }
    hash
}
