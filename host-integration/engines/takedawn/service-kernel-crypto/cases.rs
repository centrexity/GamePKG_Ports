// Asset-free original RC4/HMAC bridge corpus.
pub fn crypto_steps(mode: u32) -> Result<std::vec::Vec<[u32; 8]>, i32> {
    if mode >= 96 {
        return Err(-1);
    }
    let context = 0x11000u32 + (mode & 3);
    let mut steps = std::vec::Vec::new();
    if mode < 48 {
        let keys = [
            0u32, 1, 2, 3, 5, 16, 31, 63, 64, 65, 127, 255, 256, 257, 65537, 0xffffffff,
        ];
        let lengths = [
            0u32, 1, 2, 15, 16, 17, 255, 256, 257, 511, 512, 513, 4095, 4096, 4097, 8192,
        ];
        if mode < 24 {
            steps.extend([
                [338, context, keys[mode as usize % 16], 0x20000, 0, 0, 0, 0],
                [
                    339,
                    context,
                    lengths[mode as usize % 16],
                    0x22000,
                    0,
                    0,
                    0,
                    0,
                ],
                [339, context, 127, 0x25000, 0, 0, 0, 0],
            ]);
        } else {
            match mode {
                24 => steps.extend([[338, context, 16, context, 0, 0, 0, 0]]),
                25 => steps.extend([[338, context, 2, context + 256, 0, 0, 0, 0]]),
                26 => steps.extend([
                    [338, context, 16, 0x20000, 0, 0, 0, 0],
                    [339, context, 256, context, 0, 0, 0, 0],
                ]),
                27 => steps.extend([
                    [338, context, 16, 0x20000, 0, 0, 0, 0],
                    [339, context, 2, context + 256, 0, 0, 0, 0],
                ]),
                28 => steps.extend([
                    [338, context, 16, 0x20000, 0, 0, 0, 0],
                    [339, context, 512, context - 40, 0, 0, 0, 0],
                ]),
                29 => steps.extend([[338, context, 257, context - 20, 0, 0, 0, 0]]),
                30 => steps.extend([
                    [338, 0, 0xffffffff, 0xffffffff, 0, 0, 0, 0],
                    [339, 0xffffffff, 0, 0xffffffff, 0, 0, 0, 0],
                    [338, 0xffffffff, 8, 0, 0, 0, 0, 0],
                    [339, 0, 0xffffffff, 0xffffffff, 0, 0, 0, 0],
                ]),
                31 => steps.extend([[338, context, 0xffffffff, 0x20000, 0, 0, 0, 0]]),
                _ => steps.extend([[339, context, (mode - 31) * 17, context - 32, 0, 0, 0, 0]]),
            }
        }
    } else {
        let index = mode - 48;
        let keys = [
            0u32, 1, 20, 63, 64, 65, 127, 128, 129, 4095, 4096, 4097, 65535, 65536, 65537, 131071,
        ];
        let lengths = [0u32, 1, 55, 56, 63, 64, 65, 127, 4095, 4096, 4097, 65535];
        let mut step = [
            340u32,
            0x20000,
            keys[index as usize % 16],
            0x21000,
            lengths[index as usize % 12],
            0x24000,
            lengths[(index as usize + 3) % 12],
            0x12000,
        ];
        if index >= 24 && index < 32 {
            step[7] = if index & 1 == 0 {
                step[1]
            } else {
                step[3] + (index & 3)
            };
        }
        match index {
            32 => {
                step[1] = 0;
                step[2] = 0xffffffff;
                step[7] = 0xffffffff;
            }
            33 => {
                step[1] = 0xffffffff;
                step[2] = 0xffffffff;
                step[7] = 0;
            }
            34 => {
                step[3] = 0;
                step[4] = 0xffffffff;
            }
            35 => {
                step[5] = 0;
                step[6] = 0xffffffff;
            }
            36 => {
                step[1] = 0xffffffff;
                step[2] = 0;
            }
            37 => {
                step[3] = 0xffffffff;
                step[4] = 0;
                step[5] = 0xffffffff;
                step[6] = 0;
            }
            _ => {}
        }
        steps.extend([step]);
        if index % 5 == 0 {
            steps.extend([step]);
        }
    }
    Ok(steps)
}
