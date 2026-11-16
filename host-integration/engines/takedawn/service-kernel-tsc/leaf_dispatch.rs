use crate::generated;
use crate::memory;
use crate::registers;
// Exact reviewed leaves, with no callable pointers or generic instruction loop.
pub fn invoke(
    state: &mut registers::Registers,
    ram: &mut memory::Memory,
    address: u32,
) -> Result<(), i32> {
    match address {
        0x001d20ac => generated::sub_001D20AC(state, ram),
        0x001fc960 => generated::sub_001FC960(state, ram),
        0x00356ac0 => generated::sub_00356AC0(state, ram),
        0x001fc930 => generated::sub_001FC930(state, ram),
        0x001fc980 => generated::sub_001FC980(state, ram),
        _ => Err(-5),
    }
}
