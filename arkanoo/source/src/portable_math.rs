// Platform-independent f32 trig for rasterized effects. In particular, some
// 32-bit CRTs lose phase for the large angles produced by a u64 frame counter.
// Reduce with the 256 binary fractional bits of 2/pi before evaluating fixed
// Taylor polynomials on [-pi/4, pi/4]. No platform libm, crates or native-endian
// representation is used. All indexing is bounded by the f32 exponent range.
fn word_at(words: &[u64; 5], shift: u32) -> u64 {
    let index = (shift / 64) as usize;
    let offset = shift % 64;
    let mut value = words[index] >> offset;
    if offset != 0 && index + 1 < 5 {
        value |= words[index + 1] << (64 - offset);
    }
    value
}
pub fn sin_cos(value: f32) -> [f32; 2] {
    let bits = value.to_bits();
    let absolute = bits & 0x7fff_ffff;
    if absolute >= 0x7f80_0000 {
        return [f32::NAN, f32::NAN];
    }
    let mut angle = f32::from_bits(absolute) as f64;
    let mut quadrant = 0u64;
    if angle > core::f64::consts::FRAC_PI_4 {
        let constants = [
            0xfe51_63ab_debb_c561u64,
            0xdb62_9599_3c43_9041,
            0xfc27_57d1_f534_ddc0,
            0xa2f9_836e_4e44_1529,
        ];
        let mantissa = u128::from((absolute & 0x007f_ffff) | 0x0080_0000);
        let mut product = [0u64; 5];
        let mut carry = 0u128;
        for index in 0..4 {
            let wide = u128::from(constants[index]) * mantissa + carry;
            product[index] = wide as u64;
            carry = wide >> 64;
        }
        product[4] = carry as u64;
        // x = mantissa * 2^(exponent-23); the fraction starts at this bit.
        let shift = 406 - ((absolute >> 23) & 0xff);
        let fraction = word_at(&product, shift - 64);
        let round_up = fraction >> 63;
        quadrant = (word_at(&product, shift) + round_up) & 3;
        let magnitude = if round_up != 0 {
            (!fraction).wrapping_add(1)
        } else {
            fraction
        };
        angle = magnitude as f64 * (1.0 / 18446744073709551616.0) * core::f64::consts::FRAC_PI_2;
        if round_up != 0 {
            angle = -angle;
        }
    }
    let square = angle * angle;
    let sine = angle
        + angle
            * square
            * (-1.0 / 6.0
                + square
                    * (1.0 / 120.0
                        + square
                            * (-1.0 / 5040.0
                                + square
                                    * (1.0 / 362880.0
                                        + square
                                            * (-1.0 / 39916800.0
                                                + square
                                                    * (1.0 / 6227020800.0
                                                        + square
                                                            * (-1.0 / 1307674368000.0
                                                                + square / 355687428096000.0)))))));
    let cosine = 1.0
        + square
            * (-1.0 / 2.0
                + square
                    * (1.0 / 24.0
                        + square
                            * (-1.0 / 720.0
                                + square
                                    * (1.0 / 40320.0
                                        + square
                                            * (-1.0 / 3628800.0
                                                + square
                                                    * (1.0 / 479001600.0
                                                        + square
                                                            * (-1.0 / 87178291200.0
                                                                + square / 20922789888000.0)))))));
    let mut s = match quadrant {
        0 => sine,
        1 => cosine,
        2 => -sine,
        _ => -cosine,
    };
    let c = match quadrant {
        0 => cosine,
        1 => -sine,
        2 => -cosine,
        _ => sine,
    };
    if bits >> 31 != 0 {
        s = -s;
    }
    [s as f32, c as f32]
}

// Scalar callers use the same fixed reduction as paired effects.
pub fn sin(value: f32) -> f32 {
    sin_cos(value)[0]
}
pub fn cos(value: f32) -> f32 {
    sin_cos(value)[1]
}
