// Fixed replay regions. Two new tail-wrapper cases use the same-sized last
// region at their image import cell. Existing case layouts are unchanged.
pub fn vector_spans(address: u32) -> [(u32, usize); 9] {
    [
        (0x10000, 1024),
        (0x11000, 1024),
        (0x1fe00, 1024),
        (0x3b1680, 64),
        (0x4a1c40, 64),
        (0x463b00, 64),
        (0x3a34a0, 64),
        (0x3c6100, 64),
        (
            match address {
                0x1d7024 => 0x36b7d8,
                0x1cff7a => 0x36b820,
                _ => 0x36c0e0,
            },
            64,
        ),
    ]
}
