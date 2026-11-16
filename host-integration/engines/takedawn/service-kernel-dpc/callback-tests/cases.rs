use crate::memory;
use crate::registers;
pub fn callback_params(index: u32) -> [u32; 6] {
    [
        if index & 2 == 0 { 0x740100 } else { 0 },
        if index & 4 != 0 {
            0
        } else if index & 32 != 0 {
            0x2200
        } else {
            0x1100
        },
        0xa54e12b8u32.wrapping_add(index * 17),
        0x11223344 ^ index,
        0xff004400u32.wrapping_sub(index * 31),
        (-37i32).wrapping_add(index as i32) as u32,
    ]
}
pub fn callback_initial(top: u32) -> registers::Registers {
    let mut cpu = registers::Registers::new();
    cpu.eax = 0xa5a50001;
    cpu.edx = 0xabcdef01;
    cpu.esi = 0xffffffff;
    cpu.xmm7 = [8, 7, 6, 5];
    cpu.mm3 = [0x80000000, 0x12345678];
    cpu.esp = top;
    cpu.fs_base = 0x740000;
    cpu.eflags = 0x202;
    cpu
}
pub fn callback_seed(
    index: u32,
    ram: &mut memory::Memory,
    top: u32,
) -> Result<registers::Registers, i32> {
    let mut empty = std::vec::Vec::new();
    empty.resize(524288, 0);
    ram.write_bytes(0x740000, &empty)?;
    let params = callback_params(index);
    ram.write_u32(0x74010c, params[1])?;
    ram.write_u32(0x740110, params[2])?;
    ram.write_u32(0x741004, index & 1)?;
    ram.write_u32(0x741010, (index >> 3) & 3)?;
    Ok(callback_initial(top))
}
pub fn callback_old(index: u32) -> u8 {
    [0u8, 1, 2, 4][((index >> 6) & 3) as usize]
}
pub fn callback_hash(bytes: &[u8]) -> u64 {
    let mut hash = 14695981039346656037u64;
    for byte in bytes {
        hash = (hash ^ u64::from(*byte)).wrapping_mul(1099511628211);
    }
    hash
}
