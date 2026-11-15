// Port-owned value helpers for the pinned generated vector routines.
// Arithmetic uses the explicit portable NaN rule documented in README.md.
// No platform intrinsics, unions, raw memory or instruction dispatch.
use crate::memory;
pub fn zero() -> [u32; 4] {
    [0; 4]
}
pub fn scalar(value: f32) -> [u32; 4] {
    [f32::to_bits(value), 0, 0, 0]
}
pub fn load(guest_ram: &memory::Memory, address: u32) -> Result<[u32; 4], i32> {
    let mut words = [0u32; 4];
    for index in 0..4 {
        words[index] = guest_ram.read_u32(address.wrapping_add((index as u32) * 4))?;
    }
    Ok(words)
}
pub fn store(guest_ram: &mut memory::Memory, address: u32, words: [u32; 4]) -> Result<(), i32> {
    for index in 0..4 {
        guest_ram.write_u32(address.wrapping_add((index as u32) * 4), words[index])?;
    }
    Ok(())
}
pub fn word(words: [u32; 4], index: usize) -> Result<u32, i32> {
    if index >= 4 {
        return Err(-1);
    }
    Ok(words[index])
}
pub fn set_word(words: &mut [u32; 4], index: usize, value: u32) -> Result<(), i32> {
    if index >= 4 {
        return Err(-1);
    }
    words[index] = value;
    Ok(())
}
pub fn float(words: [u32; 4], index: usize) -> Result<f32, i32> {
    Ok(f32::from_bits(word(words, index)?))
}
pub fn set_float(words: &mut [u32; 4], index: usize, value: f32) -> Result<(), i32> {
    set_word(words, index, f32::to_bits(value))
}
pub fn signed(words: [u32; 4], index: usize) -> Result<i32, i32> {
    Ok(word(words, index)? as i32)
}
pub fn set_signed(words: &mut [u32; 4], index: usize, value: i32) -> Result<(), i32> {
    set_word(words, index, value as u32)
}
fn vector_nan(first: u32, second: u32) -> Option<u32> {
    // Explicit portable rule: quiet the first NaN operand, retaining its sign
    // and payload. Rust arithmetic otherwise permits target-dependent payload
    // selection. Helpers test this rule independently of the pinned baseline.
    for bits in [first, second] {
        if bits & 0x7f800000 == 0x7f800000 && bits & 0x007fffff != 0 {
            return Some(bits | 0x00400000);
        }
    }
    None
}
pub fn add_float(first: f32, second: f32) -> f32 {
    match vector_nan(f32::to_bits(first), f32::to_bits(second)) {
        Some(bits) => f32::from_bits(bits),
        None => first + second,
    }
}
pub fn subtract_float(first: f32, second: f32) -> f32 {
    match vector_nan(f32::to_bits(first), f32::to_bits(second)) {
        Some(bits) => f32::from_bits(bits),
        None => first - second,
    }
}
pub fn multiply_float(first: f32, second: f32) -> f32 {
    match vector_nan(f32::to_bits(first), f32::to_bits(second)) {
        Some(bits) => f32::from_bits(bits),
        None => first * second,
    }
}
pub fn add(first: [u32; 4], second: [u32; 4]) -> [u32; 4] {
    let mut words = [0u32; 4];
    for index in 0..4 {
        words[index] = f32::to_bits(add_float(
            f32::from_bits(first[index]),
            f32::from_bits(second[index]),
        ));
    }
    words
}
pub fn subtract(first: [u32; 4], second: [u32; 4]) -> [u32; 4] {
    let mut words = [0u32; 4];
    for index in 0..4 {
        words[index] = f32::to_bits(subtract_float(
            f32::from_bits(first[index]),
            f32::from_bits(second[index]),
        ));
    }
    words
}
pub fn multiply(first: [u32; 4], second: [u32; 4]) -> [u32; 4] {
    let mut words = [0u32; 4];
    for index in 0..4 {
        words[index] = f32::to_bits(multiply_float(
            f32::from_bits(first[index]),
            f32::from_bits(second[index]),
        ));
    }
    words
}
pub fn xor(first: [u32; 4], second: [u32; 4]) -> [u32; 4] {
    let mut words = [0u32; 4];
    for index in 0..4 {
        words[index] = first[index] ^ second[index];
    }
    words
}
pub fn shuffle(first: [u32; 4], second: [u32; 4], immediate: u32) -> [u32; 4] {
    [
        first[(immediate & 3) as usize],
        first[((immediate >> 2) & 3) as usize],
        second[((immediate >> 4) & 3) as usize],
        second[((immediate >> 6) & 3) as usize],
    ]
}
