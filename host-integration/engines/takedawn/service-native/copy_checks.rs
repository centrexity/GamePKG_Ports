// Asset-free tests, admitted as game code and run in the real confined child.
use crate::memory;
pub fn verify(ram: &mut memory::Memory, window: u64) -> Result<(), i32> {
    // One service copy larger than the ordinary 1 MiB transfer ceiling.
    let length = 1048593u32;
    let mut offset = 0u32;
    while offset < length {
        let count = (length - offset).min(65536);
        let mut bytes = std::vec::Vec::new();
        bytes.resize(count as usize, 0u8);
        for index in 0..count {
            bytes[index as usize] = ((u64::from(offset + index) * 37) % 251) as u8;
        }
        ram.write_bytes(0x300000 + offset, &bytes)?;
        offset += count;
    }
    ram.copy_nonoverlapping(0x500000, 0x300000, length)?;
    offset = 0;
    while offset < length {
        let count = (length - offset).min(65536);
        let mut bytes = std::vec::Vec::new();
        bytes.resize(count as usize, 0u8);
        ram.read_bytes(0x500000 + offset, &mut bytes)?;
        for index in 0..count {
            if bytes[index as usize] != ((u64::from(offset + index) * 37) % 251) as u8 {
                return Err(-3);
            }
        }
        offset += count;
    }
    // Reject overlapping physical backing even through numerically disjoint
    // guest aliases; rejection cannot alter the destination sentinel.
    let mut alias = ram.call_view();
    alias.attach(0, 67108864, window, 0, true)?;
    alias.attach(0x80000000, 67108864, window, 0, true)?;
    alias.attach(0x90000000, 4096, window, 0, false)?;
    let original = ram.read_u32(0)?;
    ram.write_u32(0, 0x12345678)?;
    if alias.copy_nonoverlapping(0x80000000, 0, 4) != Err(-1)
        || alias.copy_nonoverlapping(1, 0, 4) != Err(-1)
        || alias.copy_nonoverlapping(0, 1, 4) != Err(-1)
        || alias.copy_nonoverlapping(0, 0, 4) != Err(-1)
        || alias.copy_nonoverlapping(0x90000000, 0x1000, 4) != Err(-4)
        || alias.copy_nonoverlapping(0xfffffffe, 0, 4) != Err(-1)
        || alias.copy_nonoverlapping(0, 0, 67108865) != Err(-1)
        || alias.copy_nonoverlapping(0x70000000, 0, 4) != Err(-4)
        || ram.read_u32(0)? != 0x12345678
        || memory::guest_address(None) != Err(-1)
        || memory::guest_position(Some(0)) != Ok(0)
        || memory::guest_offset(Some(0xffffffff), 1) != Ok(4294967296)
        || memory::guest_offset(Some(0xffffffff), 2) != Err(-1)
        || memory::guest_offset(Some(0), 0x80000000) != Err(-1)
    {
        return Err(-3);
    }
    ram.write_u32(0, original)?;
    verify_segments(ram, window)?;
    Ok(())
}

fn verify_segments(ram: &mut memory::Memory, window: u64) -> Result<(), i32> {
    let saved_low = ram.read_u32(0)?;
    let saved_high = ram.read_u32(4094)?;
    let mut view = ram.call_view();
    view.attach(0x80000002, 2, window, 0, true)?;
    view.attach(0x80000000, 2, window, 4094, true)?;
    view.write_u32(0x80000000, 0x12345678)?;
    if ram.read_u16(4094)? != 0x5678
        || ram.read_u16(0)? != 0x1234
        || view.read_u32(0x80000000)? != 0x12345678
    {
        return Err(-3);
    }
    let mut result = [0x5au8; 5];
    if view.read_bytes(0x80000000, &mut result) != Err(-4)
        || result != [0x5a; 5]
        || view.write_bytes(0x80000000, &[9; 5]) != Err(-4)
        || view.read_u32(0x80000000)? != 0x12345678
    {
        return Err(-3);
    }
    let mut readonly = ram.call_view();
    readonly.attach(0x80000000, 2, window, 4094, true)?;
    readonly.attach(0x80000002, 2, window, 0, false)?;
    if readonly.write_u32(0x80000000, 0xffffffff) != Err(-4)
        || readonly.read_u32(0x80000000)? != 0x12345678
    {
        return Err(-3);
    }
    let mut upper = ram.call_view();
    upper.attach(0xfffffffe, 2, window, 0, true)?;
    if upper.read_u32(0xfffffffe) != Err(-1)
        || upper.write_u32(0xfffffffe, 0xffffffff) != Err(-1)
        || view.read_u32(0x80000000)? != 0x12345678
    {
        return Err(-3);
    }
    let mut saved = [0u8; 32];
    ram.read_bytes(0x3000, &mut saved)?;
    let mut fragmented = ram.call_view();
    for number in 0..32u32 {
        fragmented.attach(
            0x90000000 + number,
            1,
            window,
            u64::from(0x3000 + number),
            true,
        )?;
    }
    let mut payload = [0u8; 32];
    for number in 0..32usize {
        payload[number] = number as u8;
    }
    fragmented.write_bytes(0x90000000, &payload)?;
    let mut observed = [0u8; 32];
    fragmented.read_bytes(0x90000000, &mut observed)?;
    if observed != payload || fragmented.attach(0x90000020, 1, window, 0, true) != Err(-1) {
        return Err(-3);
    }
    ram.read_bytes(0x3000, &mut observed)?;
    if observed != payload {
        return Err(-3);
    }
    ram.write_bytes(0x3000, &saved)?;
    ram.write_u32(0, saved_low)?;
    ram.write_u32(4094, saved_high)?;
    Ok(())
}
