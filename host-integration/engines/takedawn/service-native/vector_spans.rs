// Fixed replay regions. Import tail wrappers use the last region for the
// original image slot. All other case layouts retain the default region.
pub fn vector_import_cell(address: u32) -> Option<u32> {
    match address {
        0x1d7012 => Some(0x36b7c0),
        0x1d7018 => Some(0x36b7c4),
        0x1d701e => Some(0x36b7c8),
        0x1d7024 => Some(0x36b7d8),
        0x1cff7a => Some(0x36b820),
        _ => None,
    }
}

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
        (vector_import_cell(address).unwrap_or(0x36c0e0), 64),
    ]
}
