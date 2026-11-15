// Port-owned ANSI debug-service behavior from pinned recomp_trace.c.
// Host logging is generic GamePKG service I/O; no console or format strings.
use crate::gamepkg;
use crate::memory;
fn append(output: &mut [u8], position: &mut u32, bytes: &[u8]) -> Result<(), i32> {
    let end = position.checked_add(bytes.len() as u32).ok_or(-1)?;
    if end as usize > output.len() {
        return Err(-1);
    }
    output[*position as usize..end as usize].copy_from_slice(bytes);
    *position = end;
    Ok(())
}
fn decimal(output: &mut [u8], position: &mut u32, mut value: u32) -> Result<(), i32> {
    let mut digits = [0u8; 10];
    let mut count = 0u32;
    loop {
        digits[count as usize] = b'0' + (value % 10) as u8;
        count += 1;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    while count != 0 {
        count -= 1;
        append(output, position, &[digits[count as usize]])?;
    }
    Ok(())
}
fn hexadecimal(output: &mut [u8], position: &mut u32, value: u32) -> Result<(), i32> {
    let mut shift = 28u32;
    loop {
        let digit = (value >> shift) & 15;
        let byte = if digit < 10 {
            b'0' + digit as u8
        } else {
            b'A' + (digit - 10) as u8
        };
        append(output, position, &[byte])?;
        if shift == 0 {
            break;
        }
        shift -= 4;
    }
    Ok(())
}
pub fn service(ram: &mut memory::Memory, service_code: u32, argument: u32) -> Result<(), i32> {
    // At most u16::MAX counted bytes, eight prefix bytes and one newline.
    // Fixed length avoids a growable per-byte allocation path in game objects.
    let mut output = std::vec::Vec::new();
    output.resize(65544, 0u8);
    let mut position = 0u32;
    if service_code != 1 {
        append(&mut output, &mut position, b"[GUEST] DebugService ")?;
        decimal(&mut output, &mut position, service_code)?;
        append(&mut output, &mut position, b" (arg 0x")?;
        hexadecimal(&mut output, &mut position, argument)?;
        append(&mut output, &mut position, b"), ignored\n")?;
        gamepkg::diagnostic_bytes(2, 0x544b0005, &output[..position as usize])?;
        return Ok(());
    }
    if argument == 0 {
        return Ok(());
    }
    let length = u32::from(ram.read_u16(argument)?);
    let buffer = ram.read_u32(argument.checked_add(4).ok_or(-1)?)?;
    if buffer == 0 || length == 0 {
        return Ok(());
    }
    append(&mut output, &mut position, b"[GUEST] ")?;
    let mut index = 0u32;
    while index < length {
        let byte = ram.read_u8(buffer.checked_add(index).ok_or(-1)?)?;
        if byte == 0 {
            break;
        }
        append(&mut output, &mut position, &[byte])?;
        index += 1;
    }
    // Native printf stops at NUL, but the original newline test still reads
    // the final counted byte, even when that byte was not printed.
    let tail = ram.read_u8(buffer.checked_add(length - 1).ok_or(-1)?)?;
    if tail != b'\n' {
        append(&mut output, &mut position, b"\n")?;
    }
    gamepkg::diagnostic_bytes(1, 0x544b0004, &output[..position as usize])?;
    Ok(())
}
