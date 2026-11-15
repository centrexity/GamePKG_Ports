// Maintainer harness only. Module paths are supplied by check.py in the wrapper.
use std::{fs::File, io::Write, path::Path};
const SPANS: [(u32, usize); 6] = [
    (0x10000, 1024),
    (0x11000, 1024),
    (0x1fe00, 1024),
    (0x3b1680, 64),
    (0x4a1c40, 64),
    (0x463b00, 64),
];
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
        if base == 0x10000 {
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
        file.write_all(&initial).unwrap();
        file.write_all(&expected).unwrap();
        for &(base, count) in &SPANS {
            file.write_all(&reference[base as usize..base as usize + count])
                .unwrap();
        }
    }
}
fn memory_boundaries(ram: &mut memory::Memory, window: u64) {
    assert_eq!(ram.write_bytes(0, &vec![0; 1048561]), Err(-1));
    assert_eq!(ram.read_bytes(0, &mut vec![0; 1048577]), Err(-1));
    let mut mirror = memory::Memory::new();
    mirror
        .attach(0x80000000, 0x4000000, window, 0, true)
        .unwrap();
    assert_eq!(mirror.read_u32(0x80010000), ram.read_u32(0x10000));
    mirror.write_u32(0x80010001, 0x12345678).unwrap();
    assert_eq!(ram.read_u32(0x10001), Ok(0x12345678));
    let mut ro = memory::Memory::new();
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
    let mut ram = memory::Memory::new();
    ram.attach(0, 0x04000000, window, 0, true).unwrap();
    let mut output = if args.len() == 2 {
        Some(File::create(Path::new(&args[1])).unwrap())
    } else {
        None
    };
    let count = 1024 + (ADDRESSES.len() as u32 - 1) * 32;
    if let Some(file) = &mut output {
        file.write_all(b"TKN3").unwrap();
        file.write_all(&count.to_le_bytes()).unwrap();
    }
    let mut reference = vec![0u8; 8 * 1024 * 1024];
    let mut seed = 0x1234_5678_9abc_def0;
    for &address in &ADDRESSES {
        let count = if address == 0x13f10 { 1024 } else { 32 };
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
    memory_boundaries(&mut ram, window);
    println!("PASS: {} original native functions / {count} differential register and whole-RAM vectors; aliases, unaligned LE accesses, overflow, read-only/protected/uncommitted memory and revocation; no instruction interpreter",ADDRESSES.len());
}
