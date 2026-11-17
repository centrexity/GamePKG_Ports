pub fn isr_case(index: u32) -> [u32; 7] {
    let mode = index & 7;
    [
        mode,
        if mode == 0 {
            0
        } else if mode == 6 {
            32 + index
        } else if index & 64 != 0 {
            31
        } else {
            3
        },
        (index >> 3) & 1,
        [0u32, 1, 2, 16][((index >> 4) & 3) as usize],
        0x81230000u32.wrapping_add(index * 197),
        0x55660000 | index,
        0x12340000 | index,
    ]
}
