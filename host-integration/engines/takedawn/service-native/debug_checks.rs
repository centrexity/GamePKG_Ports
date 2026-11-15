// Checked game source, shared by native references and the confined child.
use crate::debug;
use crate::memory;
pub fn requests() -> [[u32; 2]; 7] {
    [
        [0, 0xffffffff],
        [0xffffffff, 0xabcdef12],
        [1, 0x12000],
        [1, 0x12008],
        [1, 0x12010],
        [1, 0x12018],
        [1, 0x12020],
    ]
}
fn descriptor(ram: &mut memory::Memory, address: u32, length: u16, buffer: u32) -> Result<(), i32> {
    ram.write_u16(address, length)?;
    ram.write_u16(address + 2, 1)?; // MaximumLength is intentionally ignored by the original.
    ram.write_u32(address + 4, buffer)?;
    Ok(())
}
pub fn verify(ram: &mut memory::Memory, window: u64) -> Result<(), i32> {
    ram.write_bytes(0x13000, b"A\0BCD")?;
    ram.write_bytes(0x13008, b"A\0BC\n")?;
    ram.write_bytes(0x13010, &[255u8, 27, 37])?;
    ram.write_bytes(0x13018, b"ABC\n")?;
    ram.fill(0x600000, b'x', 65535)?;
    for record in [
        [0x12000u32, 5, 0x13000],
        [0x12008, 5, 0x13008],
        [0x12010, 3, 0x13010],
        [0x12018, 4, 0x13018],
        [0x12020, 65535, 0x600000],
    ] {
        descriptor(ram, record[0], record[1] as u16, record[2])?;
    }
    let mut readonly = ram.call_view();
    readonly.attach(0, 8388608, window, 0, false)?;
    for request in requests() {
        if request[1] == 0x12018 {
            debug::service(&mut readonly, request[0], request[1])?;
        } else {
            debug::service(ram, request[0], request[1])?;
        }
    }
    // Defined quiet cases and denied reads must not emit partial messages.
    debug::service(ram, 1, 0)?;
    descriptor(ram, 0x12028, 0, 0xffffffff)?;
    debug::service(ram, 1, 0x12028)?;
    descriptor(ram, 0x12028, 1, 0)?;
    debug::service(ram, 1, 0x12028)?;
    descriptor(ram, 0x12028, 2, 0xffffffff)?;
    if debug::service(ram, 1, 0x12028) != Err(-4) || debug::service(ram, 1, 0xffffffff) != Err(-1) {
        return Err(-3);
    }
    descriptor(ram, 0x12028, 2, 0x800000)?;
    if debug::service(&mut readonly, 1, 0x12028) != Err(-4) {
        return Err(-3);
    }
    // An early NUL does not suppress the independent final-counted-byte read.
    descriptor(ram, 0x12028, 3, 0x7ffffe)?;
    ram.write_u8(0x7ffffe, 0)?;
    if debug::service(&mut readonly, 1, 0x12028) != Err(-4) {
        return Err(-3);
    }
    let mut readonly = ram.call_view();
    // A view with no mapping fails before recording anything.
    if debug::service(&mut readonly, 1, 0x12000) != Err(-4) {
        return Err(-3);
    }
    Ok(())
}
