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

// Explicit control-word rounding, independent of the host FP rounding mode.
// The pinned helper returns nonfinite inputs unchanged, including NaN payloads.
pub fn round_integer(value: f64, control: u16) -> f64 {
    let bits = f64::to_bits(value);
    let sign = bits & 0x8000000000000000;
    let magnitude = bits & 0x7fffffffffffffff;
    let exponent = (magnitude >> 52) as i32 - 1023;
    if exponent >= 52 || magnitude == 0 {
        return value;
    }
    let mode = (control >> 10) & 3;
    if exponent < 0 {
        let increment = match mode {
            0 => magnitude > 0x3fe0000000000000,
            1 => sign != 0,
            2 => sign == 0,
            _ => false,
        };
        return f64::from_bits(sign | if increment { 0x3ff0000000000000 } else { 0 });
    }
    let unit = 1u64 << (52 - exponent as u32);
    let remainder = magnitude & (unit - 1);
    let mut rounded = magnitude & !(unit - 1);
    let increment = remainder != 0
        && match mode {
            0 => remainder > unit / 2 || (remainder == unit / 2 && rounded & unit != 0),
            1 => sign != 0,
            2 => sign == 0,
            _ => false,
        };
    if increment {
        rounded += unit;
    }
    f64::from_bits(sign | rounded)
}

// FIST's indefinite integer on nonfinite/out-of-range input is the signed
// minimum of the requested width. Unknown widths fail rather than shifting.
pub fn integer_store(value: f64, control: u16, width: u32) -> Result<i64, i32> {
    if width != 16 && width != 32 && width != 64 {
        return Err(-1);
    }
    let rounded = round_integer(value, control);
    let limit = f64::from_bits(u64::from(1023 + width - 1) << 52);
    if !rounded.is_finite() || rounded < -limit || rounded >= limit {
        return Ok(if width == 64 {
            -9223372036854775807i64 - 1
        } else {
            -(1i64 << (width - 1))
        });
    }
    Ok(rounded as i64)
}
