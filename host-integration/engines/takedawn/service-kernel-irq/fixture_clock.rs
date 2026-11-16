// Maintainer-controlled clock; production clock.rs uses the host counter.
use crate::gamepkg;
use crate::memory;
pub fn now_ns(ram: &memory::Memory) -> Result<u64, i32> {
    if ram.read_u32(0x741028)? == 4 {
        let backing = ram.memory_writable_word_backing(0x741034)?;
        let value = u32::from(ram.read_u8(0x740024)?);
        if gamepkg::local_call(
            6,
            &[
                u64::to_le_bytes(backing[0]).as_slice(),
                &u64::to_le_bytes(backing[1]),
                &u32::to_le_bytes(value),
            ]
            .concat(),
            &mut [],
        )? != 0
        {
            return Err(-3);
        }
    }
    ram.read_u64(0x741008)
}
