// Calls through the authenticated KIC4 import plan, not a synthesized raw table.
use crate::kernel_pool;
use crate::memory;
use crate::registers;
fn rtl_image_need(value: bool) -> Result<(), i32> {
    if value {
        Ok(())
    } else {
        Err(-601)
    }
}
fn rtl_image_call(
    ram: &mut memory::Memory,
    slot: u32,
    args: &[u32],
    result: u32,
) -> Result<(), i32> {
    let mut cpu = registers::Registers::new();
    cpu.esp = ram.read_u32(0x4008)?.checked_add(0x2000).ok_or(-1)?;
    cpu.eax = 0xdeadbeef;
    cpu.ebx = 0x87654321;
    cpu.edx = 0xa5b6c7d8;
    cpu.eflags = 0x202;
    cpu.mm4 = [0xfedcba98, 0x11223344];
    ram.write_u32(cpu.esp, 0x56781234)?;
    for (index, value) in args.iter().enumerate() {
        ram.write_u32(cpu.esp + 4 + index as u32 * 4, *value)?;
    }
    let mut expected = registers::Registers::new();
    expected.decode(&cpu.encode())?;
    expected.eax = result;
    expected.esp += 4 + args.len() as u32 * 4;
    kernel_pool::kernel_invoke(&mut cpu, ram, slot)?;
    rtl_image_need(cpu.encode() == expected.encode() && ram.kernel_caller() == 0x56781234)
}
pub fn rtl_image_calls(ram: &mut memory::Memory, slots: [u32; 5]) -> Result<[u64; 10], i32> {
    let base = ram.read_u32(0x4008)?;
    base.checked_add(0x6120).ok_or(-1)?;
    let first = base.checked_add(0x5000).ok_or(-1)?;
    let second = first + 16;
    let wide = first + 32;
    let text = base.checked_add(0x6000).ok_or(-1)?;
    let other = text + 16;
    let unicode = text + 32;
    let pattern = text + 257;
    ram.write_bytes(text, &[65u8, 66, 67, 0])?;
    ram.write_bytes(other, &[97u8, 98, 99, 0])?;
    ram.write_bytes(unicode, &[65u8, 0, 66, 0, 0, 0])?;
    rtl_image_call(ram, slots[2], &[first, text], 0)?;
    rtl_image_call(ram, slots[2], &[second, other], 0)?;
    rtl_image_call(ram, slots[3], &[wide, unicode], 0)?;
    rtl_image_need(
        ram.read_u16(first)? == 3
            && ram.read_u16(first + 2)? == 4
            && ram.read_u32(first + 4)? == text
            && ram.read_u16(wide)? == 4
            && ram.read_u16(wide + 2)? == 6
            && ram.read_u32(wide + 4)? == unicode,
    )?;
    rtl_image_call(ram, slots[1], &[first, second, 0x100], 0)?;
    rtl_image_call(ram, slots[1], &[first, second, 0x101], 1)?;
    ram.write_u32(pattern, 0x91abcdef)?;
    ram.write_u32(pattern + 4, 0x91abcdef)?;
    ram.write_u32(pattern + 8, 0)?;
    rtl_image_call(ram, slots[0], &[pattern, 15, 0x91abcdef], 8)?;
    rtl_image_call(ram, slots[0], &[0, u32::MAX, 1], 0)?;
    rtl_image_call(ram, slots[4], &[0x103], 997)?;
    rtl_image_call(ram, slots[4], &[0xc000009a], 317)?;
    Ok([8, 0, 0, 1, 3, 4, 4, 6, 997, 317])
}
