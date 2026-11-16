// Asset-free NV setting cases, including original alias/store ordering.
pub fn settings_config(mode: u32) -> [u32; 2] {
    [
        mode.wrapping_mul(0x10203045),
        if mode & 1 == 0 { 2 } else { 6 },
    ]
}
pub fn settings_step(mode: u32) -> Result<[u32; 6], i32> {
    if mode >= 96 {
        return Err(-1);
    }
    let indices = [
        0u32, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 0xffffffff,
    ];
    let lengths = [0u32, 3, 4, 4097];
    let mut step = [
        24,
        indices[mode as usize % 20],
        0x11000 + (mode & 3),
        0x12000 + (mode & 3),
        lengths[(mode as usize / 20) % 4],
        0x14000 + (mode & 3),
    ];
    if mode >= 80 {
        match mode {
            80 => {
                step = [24, 7, 0x12000, 0x12000, 4, 0x14000];
            }
            81 => {
                step = [24, 8, 0x11000, 0x12000, 8, 0x12000];
            }
            82 => {
                step = [24, 9, 0x12000, 0x12000, 4, 0x12000];
            }
            83 => {
                step = [24, 0xffffffff, 0x12001, 0x12000, 16, 0x12002];
            }
            84 => {
                step = [24, 0, 0x12000, 0x12001, 4, 0x12002];
            }
            85 => {
                step = [24, 7, 0x12000, 0x12000, 3, 0x12000];
            }
            86 => {
                step = [24, 7, 0xffffffff, 0, 0xffffffff, 0xffffffff];
            }
            87 => {
                step = [24, 0xffffffff, 0x11000, 0xffffffff, 0, 0x14000];
            }
            88 => {
                step = [24, 7, 0x11000, 0x12000, 0xffffffff, 0x14000];
            }
            89 => {
                step = [24, 0xffffffff, 0x13000, 0x12000, 4097, 0x13001];
            }
            90 => {
                step[2] = 0;
                step[4] = 4;
            }
            91 => {
                step[5] = 0;
                step[4] = 4;
            }
            92 => {
                step[2] = 0;
                step[5] = 0;
                step[4] = 4;
            }
            93 => {
                step[3] = 0;
                step[4] = 0;
            }
            94 => {
                step[1] = 0xffffffff;
                step[4] = 0;
            }
            95 => {
                step = [24, 7, 0x18004, 0x18000, 4, 0x18008];
            }
            _ => return Err(-1),
        }
    }
    Ok(step)
}
