// Deterministic asset-free SHA kernel-call corpus, shared by both witnesses.
pub fn sha_steps(mode: u32) -> Result<std::vec::Vec<[u32; 4]>, i32> {
    if mode >= 64 {
        return Err(-1);
    }
    let lengths = [
        0u32, 1, 2, 55, 56, 57, 63, 64, 65, 119, 120, 127, 128, 129, 255, 256, 257, 4095, 4096,
        4097, 65535, 65536, 65537, 131071,
    ];
    let mut steps = std::vec::Vec::new();
    let context = 0x11000u32 + if mode & 1 == 0 { 0 } else { 3 };
    if mode < 36 || mode >= 56 {
        steps.extend([[335, context, 0, 0]]);
    }
    if mode < 24 {
        steps.extend([[336, context, 0x20000, lengths[mode as usize]]]);
    } else if mode < 36 {
        for index in 0..(mode - 23) {
            steps.extend([[336, context, 0x20000 + index * 97, 63 + index % 3]]);
        }
    } else if mode < 56 {
        steps.extend([[336, context, 0x20000 + mode * 3, (mode - 36) * 63]]);
    } else {
        match mode {
            56 => steps.extend([
                [335, 0, 0, 0],
                [336, 0, 0xffffffff, 0xffffffff],
                [337, 0, 0xffffffff, 0],
            ]),
            57 => steps.extend([
                [336, 0xffffffff, 0, 9],
                [336, 0xffffffff, 0xffffffff, 0],
                [337, 0xffffffff, 0, 0],
            ]),
            58 => steps.extend([[336, context, context + 20, 8]]),
            59 => steps.extend([[336, context, context, 20]]),
            60 => steps.extend([[336, context, context - 36, 128]]),
            61 => steps.extend([[336, context, context - 64, 256]]),
            62 => steps.extend([[336, context, 0x20000, 127], [337, context, context, 0]]),
            63 => steps.extend([[336, context, 0x20000, 56], [337, context, context + 3, 0]]),
            _ => return Err(-1),
        }
    }
    steps.extend([[337, context, 0x12000, 0]]);
    // Final is intentionally repeatable on the mutated context, like original.
    if mode % 7 == 0 {
        steps.extend([[337, context, 0x12020, 0]]);
    }
    Ok(steps)
}
