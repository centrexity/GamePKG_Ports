pub fn timer_case(index: u32) -> [u64; 6] {
    let seed = index.wrapping_mul(1664525).wrapping_add(1013904223);
    let mode = if index < 40 {
        0
    } else {
        match index % 5 {
            0 | 1 | 2 => 0,
            3 => 1,
            _ => 2,
        }
    };
    let timer = if index < 40 {
        0x1000 + index * 40
    } else if index % 37 == 0 {
        0
    } else {
        0x1000 + ((seed >> 3) % 48) * 40
    };
    let due = if index % 13 == 0 {
        0x123456789i64
    } else {
        -i64::from(seed % 50000)
    };
    let period = match index % 6 {
        0 => 0i32,
        1 => -1,
        2 => 1,
        3 => 10,
        4 => i32::MAX,
        _ => i32::MIN,
    };
    [
        mode,
        u64::from(timer),
        due as u64,
        u64::from(period as u32),
        if index % 7 == 0 {
            0
        } else {
            u64::from(0x2000 + index * 4)
        },
        u64::from(1000 + index * 3),
    ]
}
pub fn timer_hash(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash = (hash ^ u64::from(*byte)).wrapping_mul(1099511628211);
    }
    hash
}

pub fn timer_clock_case(index: u32) -> [u64; 3] {
    let frequency = match index % 4 {
        0 => 1000000000u64,
        1 => 10000000,
        2 => 3,
        _ => 7,
    };
    let fraction = match (index / 4) % 4 {
        0 => frequency - 1,
        1 => 0,
        2 => 1,
        _ => frequency / 2,
    };
    [
        frequency * u64::from(index / 16) + fraction,
        frequency,
        999999 - u64::from(index) * 1003,
    ]
}
