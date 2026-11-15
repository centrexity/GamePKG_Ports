// Maintainer harness only. Module paths are supplied by check.py in the wrapper.
use std::{fs::File, io::Write, path::Path};
const SPANS: [(u32, usize); 7] = [
    (0x10000, 1024),
    (0x11000, 1024),
    (0x1fe00, 1024),
    (0x3b1680, 64),
    (0x4a1c40, 64),
    (0x463b00, 64),
    (0x3a34a0, 64),
];
fn reference_case_count(address: u32) -> u32 {
    match address {
        0x13f10 | 0x244df0 => 1024,
        0x1fbae0 => 512,
        0x243450 => 256,
        0x15570 => 128,
        0x13d10 | 0x13e50 => 128,
        _ => 32,
    }
}
fn next_random(seed: &mut u64) -> u64 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    *seed
}
fn initial_state(seed: &mut u64) -> registers::Registers {
    let mut r = registers::Registers::new();
    r.eax = 0x10000 + ((next_random(seed) as u32) & 12);
    r.ecx = 0x11000 + ((next_random(seed) as u32) & 12);
    r.ebx = next_random(seed) as u32;
    r.edx = next_random(seed) as u32;
    r.esi = next_random(seed) as u32;
    r.edi = next_random(seed) as u32;
    r.ebp = next_random(seed) as u32;
    r.seh_ebp = next_random(seed) as u32;
    r.esp = 0x20000 + ((next_random(seed) as u32) & 12);
    r.df = (next_random(seed) & 1) as i32;
    r.fp_top = (next_random(seed) & 7) as i32;
    r.fp_cc = next_random(seed) as u16;
    r.fp_control_word = [0x037f, 0x027f, 0x007f][(next_random(seed) % 3) as usize];
    for v in &mut r.fp_stack {
        *v = f64::from_bits(next_random(seed) & 0xffef_ffff_ffff_ffff);
    }
    for words in [
        &mut r.xmm0,
        &mut r.xmm1,
        &mut r.xmm2,
        &mut r.xmm3,
        &mut r.xmm4,
        &mut r.xmm5,
        &mut r.xmm6,
        &mut r.xmm7,
    ] {
        for word in words {
            *word = next_random(seed) as u32;
        }
    }
    r
}
fn compare_case(
    address: u32,
    iteration: u32,
    seed: &mut u64,
    ram: &mut memory::Memory,
    reference: &mut [u8],
    output: &mut Option<File>,
) {
    let mut r = initial_state(seed);
    if address == 0x13d10 {
        r.eax = 0x10100;
        r.edx = 0x11100;
    }
    if address == 0x13e50 {
        r.eax = 0x10100;
    }
    if address == 0x244df0 {
        let edges = [
            0u64,
            0x8000000000000000,
            0x3ff0000000000000,
            0xbff0000000000000,
            0x3fe0000000000000,
            0xbfe0000000000000,
            0x3ff8000000000000,
            0xbff8000000000000,
            0x4004000000000000,
            0xc004000000000000,
            0x43dfffffffffffff,
            0x43e0000000000000,
            0xc3e0000000000000,
            0xc3e0000000000001,
            1,
            0x8000000000000001,
            0x7ff0000000000000,
            0xfff0000000000000,
            0x7ff8000000000001,
            0x7ff0000000000001,
        ];
        let bits = if iteration % 2 == 0 {
            edges[(iteration as usize / 2) % edges.len()]
        } else {
            next_random(seed)
        };
        r.fp_stack[r.fp_top as usize] = f64::from_bits(bits);
        r.fp_control_word = 0x037f | ((iteration / 40 % 4) as u16) << 10;
    }
    if address==0x15570 {r.ecx=0x11000;}
    if address==0x243450 {r.eax=0x10000+(iteration/16%6)*4;}
    let bounds = if address==0x243450 {if iteration%2==0 {(0,0)}else{(0x10000,0x20000)}} else if address == 0x1fbae0 || address==0x15570 {
        match iteration / 4 % 4 {
            0 => (0, 0),
            1 => (0x10000, 0x20000),
            2 => (0x16000, 0x20000),
            _ => (1, 0),
        }
    } else {
        (0, 0)
    };
    ram.call_bounds(bounds.0, bounds.1).unwrap();
    unsafe {
        oracle::call_bounds(bounds.0, bounds.1);
    }
    let input_state = r.encode();
    let mut initial = Vec::new();
    for &(base, count) in &SPANS {
        let mut bytes = vec![0; count];
        for b in &mut bytes {
            *b = next_random(seed) as u8;
        }
        if base == 0x4a1c40 {
            for n in 0..6 {
                bytes[0x34 + n] = u8::from((iteration >> n) & 1 != 0);
            }
        }
        if base == 0x10000 && !matches!(address, 0x13d10 | 0x13e50) {
            for n in 0..4 {
                bytes[(r.eax - base) as usize + n] = u8::from((iteration >> (6 + n)) & 1 != 0);
            }
        }
        if base == 0x11000 {
            let at = (r.ecx - base) as usize;
            bytes[at + 0x40..at + 0x44].copy_from_slice(
                &[0u32, 1, 2, 3, 4, u32::MAX, 0x80000000][iteration as usize % 7].to_le_bytes(),
            );
            bytes[at + (iteration as usize % 9) * 2..at + (iteration as usize % 9) * 2 + 2].fill(0);
        }
        if base == 0x3b1680 {
            bytes[12..16].copy_from_slice(
                &[
                    0u32, 0x80000000, 1, 0x7f7fffff, 0x7f800000, 0x7fc12345, 0xff800000, 0x3f800000,
                ][iteration as usize % 8]
                    .to_le_bytes(),
            );
        }
        if base==0x1fe00 && address==0x243450 {
            let at=(r.esp-base) as usize;
            let target=[0x15590u32,0x155a0,0x155e0,0x15600][iteration as usize%4];
            for (offset,value) in [(4,0x10000u32),(8,4),(12,target)] {bytes[at+offset..at+offset+4].copy_from_slice(&value.to_le_bytes());}
        }
        if base == 0x1fe00 && matches!(address, 0x13bd0 | 0x13ca0) {
            let at = (r.esp - base) as usize;
            bytes[at + 4..at + 8].copy_from_slice(&0x11080u32.to_le_bytes());
            bytes[at + 8..at + 12].copy_from_slice(&0x110c0u32.to_le_bytes());
        }
        if base == 0x463b00 {
            bytes[20..24].copy_from_slice(
                &[0u32, 1, 2, 3, u32::MAX, 0x80000000, 0x7fffffff][iteration as usize % 7]
                    .to_le_bytes(),
            );
        }
        if matches!(address, 0x13d10 | 0x13e50) {
            if base == 0x10000 || base == 0x11000 {
                // Finite, bounded matrix/quaternion inputs; NaN helper policy is
                // qualified separately, not hidden by relaxed RAM comparison.
                for at in (0..count).step_by(4) {
                    let value = ((next_random(seed) % 2049) as i32 - 1024) as f32 / 1024.0;
                    bytes[at..at + 4].copy_from_slice(&value.to_bits().to_le_bytes());
                }
            }
            if base == 0x1fe00 && address == 0x13e50 {
                let at = (r.esp - base) as usize;
                bytes[at + 4..at + 8].copy_from_slice(&0x11200u32.to_le_bytes());
            }
        }
        if base == 0x3a34a0 {
            bytes[24..28].copy_from_slice(&1.0f32.to_bits().to_le_bytes());
        }
        if address == 0x1fbae0 {
            if base == 0x10000 {
                bytes[..4].copy_from_slice(&0x10100u32.to_le_bytes());
                bytes[4..8].copy_from_slice(&0x11000u32.to_le_bytes());
            }
            if base == 0x11000 {
                bytes[4..8].copy_from_slice(&0x11100u32.to_le_bytes());
                let target = match iteration % 4 {
                    0 | 1 => 0x155a0u32,
                    2 => 0x8000 + (iteration % 24) * 4,
                    _ => 0,
                };
                bytes[256..260].copy_from_slice(&target.to_le_bytes());
            }
            if base == 0x1fe00 {
                let at = (r.esp - base) as usize;
                bytes[at + 4..at + 8].copy_from_slice(&0x10000u32.to_le_bytes());
                bytes[at + 8..at + 12].fill(0);
            }
        }
        if address==0x15570 {
            if base==0x10000 {
                bytes[..4].copy_from_slice(&0x10100u32.to_le_bytes());
                let target=match iteration%8 {0|1=>0x155a0u32,2=>0x15590,3=>0x155e0,4=>0x15600,5|6=>0x8000+(iteration%24)*4,_=>0};
                bytes[256..260].copy_from_slice(&target.to_le_bytes());
            }
            if base==0x11000 {bytes[4..8].copy_from_slice(&(if iteration%9==0 {0u32}else{0x10000u32}).to_le_bytes());}
        }
        ram.write_bytes(base, &bytes).unwrap();
        reference[base as usize..base as usize + count].copy_from_slice(&bytes);
        initial.extend(bytes);
    }
    unsafe {
        oracle::setup(&input_state, reference.as_mut_ptr());
        oracle::invoke(address);
    }
    let expected = unsafe { oracle::capture() };
    dispatch::invoke(&mut r, ram, address).unwrap();
    let expected_calls = unsafe { oracle::call_snapshot() };
    assert_eq!(
        ram.call_snapshot().unwrap(),
        expected_calls,
        "call trace mismatch at {address:08x}, vector {iteration}"
    );
    compare_call_events();
    assert_eq!(
        r.encode(),
        expected,
        "register mismatch at {address:08x}, vector {iteration}"
    );
    // Compare the whole committed 8 MiB, including bytes outside the case spans.
    for (part, bytes) in reference.chunks(1048576).enumerate() {
        let mut actual = vec![0; bytes.len()];
        ram.read_bytes((part * 1048576) as u32, &mut actual)
            .unwrap();
        assert!(
            actual == bytes,
            "RAM mismatch at {address:08x}, vector {iteration}, region {part}"
        );
    }
    if let Some(file) = output {
        file.write_all(&address.to_le_bytes()).unwrap();
        file.write_all(&input_state).unwrap();
        file.write_all(&bounds.0.to_le_bytes()).unwrap();
        file.write_all(&bounds.1.to_le_bytes()).unwrap();
        file.write_all(&initial).unwrap();
        file.write_all(&expected).unwrap();
        for &(base, count) in &SPANS {
            file.write_all(&reference[base as usize..base as usize + count])
                .unwrap();
        }
        for value in expected_calls.0 {
            file.write_all(&value.to_le_bytes()).unwrap();
        }
        file.write_all(&expected_calls.1.to_le_bytes()).unwrap();
        file.write_all(&expected_calls.2.to_le_bytes()).unwrap();
    }
}
fn memory_boundaries(ram: &mut memory::Memory, window: u64) {
    assert_eq!(ram.write_bytes(0, &vec![0; 1048561]), Err(-1));
    assert_eq!(ram.read_bytes(0, &mut vec![0; 1048577]), Err(-1));
    let mut mirror = ram.call_view();
    mirror
        .attach(0x80000000, 0x4000000, window, 0, true)
        .unwrap();
    assert_eq!(mirror.read_u32(0x80010000), ram.read_u32(0x10000));
    mirror.write_u32(0x80010001, 0x12345678).unwrap();
    assert_eq!(ram.read_u32(0x10001), Ok(0x12345678));
    let mut ro = ram.call_view();
    ro.attach(0, 0x1000, window, 0, false).unwrap();
    assert_eq!(ro.write_u8(0, 0), Err(-4));
    assert_eq!(ram.attach(0, 1, window, 0, true), Err(-1));
    assert_eq!(mirror.attach(0xfffffffe, 4, window, 0, true), Err(-1));
    assert_eq!(
        mirror.attach(0x4000000, 1, window, 0x4000000, true),
        Err(-1)
    );
    assert_eq!(
        mirror.attach(0x4000000, 1, window + 10000, 0, true),
        Err(-4)
    );
    assert_eq!(ram.read_u32(0xfffffffe), Err(-1));
    assert_eq!(ram.write_u32(0x4000000, 0), Err(-4));
    assert_eq!(ram.read_u32(0x1000000), Err(-4)); // Within window, uncommitted.
    let mut request = [
        window.to_le_bytes(),
        0u64.to_le_bytes(),
        4096u64.to_le_bytes(),
    ]
    .concat();
    request.extend(1u32.to_le_bytes());
    local::call(3, &request, 0).unwrap();
    assert_eq!(ram.write_u32(0, 0), Err(-4));
    assert_eq!(ram.read_u32(0), Ok(0));
    let mut context = registers::Registers::new();
    assert_eq!(dispatch::invoke(&mut context, ram, u32::MAX), Err(-5));
    context.fp_top = -1;
    assert_eq!(dispatch::invoke(&mut context, ram, 0x155a0), Err(-1));
    local::shutdown();
    assert_eq!(ram.read_u8(0), Err(-4));
}
fn compare_vector_helpers() {
    let edges = [
        0u32, 0x80000000, 1, 0x80000001, 0x007fffff, 0x00800000, 0x3f800000, 0xbf800000,
        0x7f7fffff, 0xff7fffff, 0x7f800000, 0xff800000, 0x7fc12345, 0xffc23456, 0x7f812345,
    ];
    let mut dual_nan_selections = 0usize;
    for left in 0..edges.len() {
        for right in 0..edges.len() {
            let first = std::array::from_fn(|lane| edges[(left + lane) % edges.len()]);
            let second = std::array::from_fn(|lane| edges[(right + lane * 3) % edges.len()]);
            for operation in 0..7 {
                let expected = unsafe { oracle::helper(operation, first, second, 0x39) };
                let actual = match operation {
                    0 => vector::add(first, second),
                    1 => vector::subtract(first, second),
                    2 => vector::multiply(first, second),
                    3 => vector::xor(first, second),
                    4 => vector::shuffle(first, second, 0x39),
                    5 => vector::scalar(f32::from_bits(first[0])),
                    6 => vector::zero(),
                    _ => unreachable!(),
                };
                for lane in 0..4 {
                    let first_nan = (first[lane] & 0x7fffffff) > 0x7f800000;
                    let second_nan = (second[lane] & 0x7fffffff) > 0x7f800000;
                    if operation <= 2 && (first_nan || second_nan) {
                        let prescribed = if first_nan {
                            first[lane] | 0x400000
                        } else {
                            second[lane] | 0x400000
                        };
                        assert_eq!(
                            actual[lane], prescribed,
                            "portable NaN rule {operation}, edges {left}/{right}, lane {lane}"
                        );
                    }
                    if actual[lane] != expected[lane] {
                        // This exception is exactly the documented, observed
                        // dual-NaN operand choice; finite results and all other
                        // bit operations remain exact baseline comparisons.
                        assert!(operation <= 2 && first_nan && second_nan);
                        assert!(
                            expected[lane] == (first[lane] | 0x400000)
                                || expected[lane] == (second[lane] | 0x400000)
                        );
                        dual_nan_selections += 1;
                    }
                }
            }
        }
    }
    for immediate in 0..256 {
        let first = [0x1, 0x2, 0x3, 0x4];
        let second = [0x10, 0x20, 0x30, 0x40];
        assert_eq!(vector::shuffle(first, second, immediate), unsafe {
            oracle::helper(4, first, second, immediate)
        });
    }
    let mut lanes = [0x7fc12345, 0x80000000, 1, 0x3f800000];
    assert_eq!(vector::float(lanes, 0).unwrap().to_bits(), 0x7fc12345);
    assert_eq!(vector::word(lanes, 4), Err(-1));
    assert_eq!(vector::set_word(&mut lanes, 4, 0), Err(-1));
    assert_eq!(
        vector::set_float(&mut lanes, 1, f32::from_bits(0xffc23456)),
        Ok(())
    );
    assert_eq!(lanes[1], 0xffc23456);
    println!("Observed {dual_nan_selections} baseline dual-NaN payload choices; portable result bits independently verified");
    println!("PASS: 1575 pinned-original vector helper edge comparisons, all 256 shuffle selectors and checked lane bounds/bit preservation");
}
fn compare_scalar_helpers(output: &mut Option<File>) {
    let fractions = [
        0u64,
        1,
        0x0fffffff,
        0x10000000,
        0x10000001,
        0x1fffffff,
        0x20000000,
        0x30000000,
        0x0007ffffffffffff,
        0x0008000000000000,
        0x000fffffe0000000,
        0x000fffffffffffff,
    ];
    let mut values = Vec::new();
    for exponent in 0u64..2048 {
        for fraction in fractions {
            for sign in [0, 0x8000000000000000] {
                values.push(sign | (exponent << 52) | fraction);
            }
        }
    }
    let mut random = 0xb93a14721c5d8e6fu64;
    for _ in 0..65536 {
        values.push(next_random(&mut random));
    }
    if let Some(file) = output.as_mut() {
        file.write_all(&(values.len() as u32).to_le_bytes())
            .unwrap();
    }
    for bits in &values {
        let input = f64::from_bits(*bits);
        let expected = unsafe { oracle::scalar_round24(input) }.to_bits();
        let actual = scalar::round24(input).to_bits();
        assert_eq!(actual, expected, "round24 input {bits:016x}");
        if let Some(file) = output.as_mut() {
            file.write_all(&bits.to_le_bytes()).unwrap();
            file.write_all(&expected.to_le_bytes()).unwrap();
        }
    }
    let parity_count = 65536u32;
    if let Some(file) = output.as_mut() {
        file.write_all(&parity_count.to_le_bytes()).unwrap();
    }
    for index in 0..parity_count {
        let value = index.wrapping_mul(0x9e3779b9);
        let expected = unsafe { oracle::scalar_parity8(value) };
        assert_eq!(expected, i32::from((value as u8).count_ones() % 2 == 0));
        assert_eq!(scalar::parity8(value), expected);
        if let Some(file) = output.as_mut() {
            file.write_all(&value.to_le_bytes()).unwrap();
            file.write_all(&expected.to_le_bytes()).unwrap();
        }
    }
    let rotate_count = (256 + 64 + 64) * 68 * 5 * 2u32;
    if let Some(file) = output.as_mut() {
        file.write_all(&rotate_count.to_le_bytes()).unwrap();
    }
    let mut rotations = 0;
    for width in [8, 16, 32] {
        let mut words = vec![
            0u32,
            1,
            0xffff,
            0x8000,
            0x7fff,
            u32::MAX,
            0x80000000,
            0x7fffffff,
        ];
        while words.len() < 64 {
            words.push(next_random(&mut random) as u32);
        }
        if width == 8 {
            words = (0..256u32).map(|i| i.wrapping_mul(0x01010101)).collect();
        }
        let mut counts = (0..64).collect::<Vec<u32>>();
        counts.extend([255, 256, 0x80000000, u32::MAX]);
        for value in words {
            for count in &counts {
                for carry in [-2, -1, 0, 1, 2] {
                    for left in [false, true] {
                        let mut expected_carry = carry;
                        let expected = unsafe {
                            oracle::scalar_rotate(value, *count, &mut expected_carry, width, left)
                        };
                        let mut actual_carry = carry;
                        assert_eq!(
                            scalar::rotate_carry(value, *count, &mut actual_carry, width, left),
                            Ok(expected)
                        );
                        assert_eq!(actual_carry, expected_carry);
                        if let Some(file) = output.as_mut() {
                            for word in [
                                value,
                                *count,
                                carry as u32,
                                width,
                                u32::from(left),
                                expected,
                                expected_carry as u32,
                            ] {
                                file.write_all(&word.to_le_bytes()).unwrap();
                            }
                        }
                        rotations += 1;
                    }
                }
            }
        }
    }
    assert_eq!(rotations, rotate_count);
    for invalid in [0, 1, 7, 9, 15, 17, 31, 33, 64, u32::MAX] {
        let mut carry = -1;
        assert_eq!(
            scalar::rotate_carry(u32::MAX, 1, &mut carry, invalid, true),
            Err(-1)
        );
        assert_eq!(carry, -1);
    }
    compare_integer_rounding(output);
    // Control words occupy previously reserved bytes, without widening the
    // private record. Every bit survives encode/decode independent of FP math.
    for control in 0..=u16::MAX {
        let mut state = registers::Registers::new();
        state.fp_control_word = control;
        let bytes = state.encode();
        let mut decoded = registers::Registers::new();
        decoded.decode(&bytes).unwrap();
        assert_eq!(decoded.fp_control_word, control);
    }
    println!("PASS: {} pinned round24 bit comparisons, {parity_count} parity inputs, {rotations} rotate/carry comparisons, invalid-width atomicity and 65536 control-word records; default floating environment only",values.len());
}
fn compare_integer_rounding(output: &mut Option<File>) {
    let mut values = Vec::new();
    // All binary64 exponent classes, signed subnormal/normal/nonfinite values,
    // tie neighbors, carry boundaries and signed-integer range boundaries.
    let fractions = [
        0u64,
        1,
        0x0003ffffffffffff,
        0x0004000000000000,
        0x0004000000000001,
        0x0007ffffffffffff,
        0x0008000000000000,
        0x0008000000000001,
        0x000fffffffffffff,
    ];
    for exponent in 0u64..2048 {
        for fraction in fractions {
            for sign in [0, 0x8000000000000000] {
                values.push(sign | exponent << 52 | fraction);
            }
        }
    }
    for width in [16u32, 32, 64] {
        let limit = (1023u64 + u64::from(width) - 1) << 52;
        for offset in -8i64..=8 {
            for sign in [0, 0x8000000000000000] {
                values.push(sign | limit.wrapping_add_signed(offset));
            }
        }
    }
    let mut random = 0x651bc3742f91a85du64;
    for _ in 0..16384 {
        values.push(next_random(&mut random));
    }
    let base_count = values.len() as u32 * 4;
    // Additional records exercise every control-word pattern independently of
    // its rounding bits; status/mask/precision fields must not change values.
    let count = base_count + 65536;
    if let Some(file) = output.as_mut() {
        file.write_all(&count.to_le_bytes()).unwrap();
    }
    for index in 0..count {
        let (bits, control) = if index < base_count {
            (
                values[(index / 4) as usize],
                0x037fu16 | ((index % 4) as u16) << 10,
            )
        } else {
            let control = (index - base_count) as u16;
            (
                [
                    0x3ff8000000000000,
                    0xbff8000000000000,
                    0x3fe0000000000000,
                    0xbfe0000000000000,
                    1,
                    0x8000000000000001,
                    0x7ff0000000000001,
                    0xfff8000000000001,
                ][control as usize % 8],
                control,
            )
        };
        let expected =
            unsafe { oracle::scalar_round_integer(f64::from_bits(bits), control) }.to_bits();
        assert_eq!(
            scalar::round_integer(f64::from_bits(bits), control).to_bits(),
            expected,
            "round integer input {bits:016x}, control {control:04x}"
        );
        if let Some(file) = output.as_mut() {
            file.write_all(&bits.to_le_bytes()).unwrap();
            file.write_all(&u32::from(control).to_le_bytes()).unwrap();
            file.write_all(&expected.to_le_bytes()).unwrap();
        }
    }
    let store_count = count * 3;
    if let Some(file) = output.as_mut() {
        file.write_all(&store_count.to_le_bytes()).unwrap();
    }
    for index in 0..count {
        let (bits, control) = if index < base_count {
            (
                values[(index / 4) as usize],
                0x037fu16 | ((index % 4) as u16) << 10,
            )
        } else {
            let control = (index - base_count) as u16;
            (
                [
                    0x3ff8000000000000,
                    0xbff8000000000000,
                    0x3fe0000000000000,
                    0xbfe0000000000000,
                    1,
                    0x8000000000000001,
                    0x7ff0000000000001,
                    0xfff8000000000001,
                ][control as usize % 8],
                control,
            )
        };
        for width in [16, 32, 64] {
            let expected =
                unsafe { oracle::scalar_integer_store(f64::from_bits(bits), control, width) };
            assert_eq!(
                scalar::integer_store(f64::from_bits(bits), control, width),
                Ok(expected),
                "integer store input {bits:016x}, control {control:04x}, width {width}"
            );
            if let Some(file) = output.as_mut() {
                file.write_all(&bits.to_le_bytes()).unwrap();
                file.write_all(&u32::from(control).to_le_bytes()).unwrap();
                file.write_all(&width.to_le_bytes()).unwrap();
                file.write_all(&(expected as u64).to_le_bytes()).unwrap();
            }
        }
    }
    for width in [0, 1, 8, 15, 17, 31, 33, 63, 65, u32::MAX] {
        assert_eq!(scalar::integer_store(f64::NAN, 0x37f, width), Err(-1));
    }
    println!("PASS: {count} pinned integer-rounding bit comparisons, {store_count} signed integer-store comparisons, all 65536 control words, 16/32/64-bit range/tie edges and invalid widths");
}
fn main() {
    compare_vector_helpers();
    let args: Vec<_> = std::env::args().collect();
    let owner = local::runtime::Runtime::new(
        &[local::runtime::Window {
            address: 0x20000000,
            bytes: 0x04000000,
        }],
        vec![],
        128 * 1024 * 1024,
        0,
        65536,
    )
    .unwrap();
    let window = owner.windows().unwrap()[0].0;
    owner.commit(window, 0, 8 * 1024 * 1024, 2).unwrap();
    local::install(owner).unwrap();
    let mut ram = memory::Memory::new().unwrap();
    ram.attach(0, 0x04000000, window, 0, true).unwrap();
    let mut output = if args.len() == 2 {
        Some(File::create(Path::new(&args[1])).unwrap())
    } else {
        None
    };
    let count: u32 = ADDRESSES
        .iter()
        .map(|address| reference_case_count(*address))
        .sum();
    if let Some(file) = &mut output {
        file.write_all(b"TKN7").unwrap();
        file.write_all(&count.to_le_bytes()).unwrap();
    }
    let mut reference = vec![0u8; 8 * 1024 * 1024];
    let mut seed = 0x1234_5678_9abc_def0;
    for &address in &ADDRESSES {
        let count = reference_case_count(address);
        for iteration in 0..count {
            compare_case(
                address,
                iteration,
                &mut seed,
                &mut ram,
                &mut reference,
                &mut output,
            );
        }
    }
    if args.len() == 2 {
        write_call_events(Path::new(&args[1]).with_file_name("diagnostics").as_path());
    }
    compare_runtime_guards();
    compare_scalar_helpers(&mut output);
    copy_checks::verify(&mut ram, window).unwrap();
    memory_boundaries(&mut ram, window);
    println!("PASS: {} original native functions / {count} differential register and whole-RAM vectors; aliases, unaligned LE accesses, overflow, read-only/protected/uncommitted memory and revocation; no instruction interpreter",ADDRESSES.len());
}
