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
    Ok(())
}
