// Port-owned value operations for the pinned native functions. No CPU/device
// access, instruction dispatch, native pointers, global state or dependencies.
// round24 matches the pinned double-backed helper in the default FP environment;
// exception flags and nondefault host rounding remain separately unqualified.
pub fn round24(value: f64) -> f64 {
    let bits = f64::to_bits(value);
    let sign = bits & 0x8000000000000000;
    let absolute = bits & 0x7fffffffffffffff;
    let exponent = absolute >> 52;
    let fraction = absolute & 0x000fffffffffffff;
    if exponent == 0x7ff {
        if fraction == 0 {
            return value;
        }
        // The original helper narrows a NaN to f32 then widens it. Preserve
        // sign and the retained payload, quiet it, and discard low payload bits.
        return f64::from_bits(sign | 0x7ff8000000000000 | ((fraction >> 29) << 29));
    }
    let discarded = if exponent == 0 {
        let mut magnitude = fraction;
        let mut precision = 0u32;
        while magnitude != 0 {
            precision += 1;
            magnitude >>= 1;
        }
        precision.saturating_sub(24)
    } else {
        29
    };
    if discarded == 0 {
        return value;
    }
    let unit = 1u64 << discarded;
    let mask = unit - 1;
    let remainder = absolute & mask;
    let mut rounded = absolute & !mask;
    if remainder > unit / 2 || (remainder == unit / 2 && rounded & unit != 0) {
        rounded += unit;
    }
    f64::from_bits(sign | rounded)
}
pub fn parity8(value: u32) -> i32 {
    let mut folded = value & 0xff;
    folded ^= folded >> 4;
    folded ^= folded >> 2;
    folded ^= folded >> 1;
    (!folded & 1) as i32
}
pub fn rotate_carry(
    value: u32,
    count: u32,
    carry: &mut i32,
    width: u32,
    left: bool,
) -> Result<u32, i32> {
    if width != 8 && width != 16 && width != 32 {
        return Err(-1);
    }
    let ring = width + 1;
    let mask = (1u64 << width) - 1;
    let full = (1u64 << ring) - 1;
    let mut extended = (((*carry & 1) as u64) << width) | ((value as u64) & mask);
    let mut steps = count & 31;
    if width < 32 {
        steps %= ring;
    }
    if steps != 0 {
        extended = if left {
            (extended << steps) | (extended >> (ring - steps))
        } else {
            (extended >> steps) | (extended << (ring - steps))
        };
        extended &= full;
    }
    *carry = ((extended >> width) & 1) as i32;
    Ok((extended & mask) as u32)
}
