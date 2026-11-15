// Maintainer-only pinned C oracle; never copied into an admitted package.
unsafe extern "C" {
    fn gamepkg_contig_reset(ram: *mut u8);
    fn gamepkg_contig_operation(op: u32, a: u32, b: u32, c: u32, d: u32) -> u32;
    fn gamepkg_contig_stats(out: *mut u32);
    fn gamepkg_contig_record(page: u32, out: *mut u32);
}
pub const CASES: u32 = 5;
pub struct Fixture {
    pub bytes: Vec<u8>,
    pub ram: Vec<u8>,
    pub records: Vec<[u32; 2]>,
    pub stats: [u32; 6],
    pub operations: u32,
}
struct Builder {
    vectors: Vec<u8>,
    count: u32,
}
impl Builder {
    fn step(&mut self, op: u32, a: u32, b: u32, c: u32, d: u32) -> u32 {
        let result = unsafe { gamepkg_contig_operation(op, a, b, c, d) };
        let mut stats = [0u32; 6];
        unsafe {
            gamepkg_contig_stats(stats.as_mut_ptr());
        }
        for value in [op, a, b, c, d, result] {
            self.vectors.extend(value.to_le_bytes());
        }
        for value in stats {
            self.vectors.extend(value.to_le_bytes());
        }
        self.count += 1;
        result
    }
    fn inspect_free(&mut self, address: u32, bytes: u32) {
        if address == 0 {
            return;
        }
        let size = self.step(3, address, 0, 0, 0);
        assert!(size >= bytes.max(1));
        self.step(3, address + 1, 0, 0, 0);
        self.step(3, address + size - 1, 0, 0, 0);
        self.step(4, address - 0x80000000, bytes.max(1).min(8192), 0, 0);
        assert_eq!(self.step(2, address + 1, 0, 0, 0), 0);
        assert_eq!(self.step(2, address, 0, 0, 0), 1);
        assert_eq!(self.step(3, address, 0, 0, 0), 0);
    }
}
fn advance(seed: &mut u32) -> u32 {
    *seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
    *seed
}
pub fn make_case(index: u32) -> Fixture {
    assert!(index < CASES);
    let mut ram = vec![0u8; 67108864];
    unsafe {
        gamepkg_contig_reset(ram.as_mut_ptr());
    }
    let mut build = Builder {
        vectors: Vec::new(),
        count: 0,
    };
    match index {
        0 => {
            for size in [0u32, 1, 4095, 4096, 4097, 65536, 0xfffff000, u32::MAX] {
                for alignment in [
                    0u32,
                    1,
                    4095,
                    4096,
                    4097,
                    8193,
                    65535,
                    65536,
                    0x40000000,
                    0x80000000,
                    0x80000001,
                    u32::MAX,
                ] {
                    let address = build.step(0, size, alignment, 0, u32::MAX);
                    build.inspect_free(address, size);
                }
            }
            for low in [
                0u32,
                1,
                4095,
                4096,
                0xefffff,
                0xf00000,
                0x1000000,
                0x3ffffff,
                0x4000000,
                0xfffffffe,
                u32::MAX,
            ] {
                for high in [
                    0u32,
                    4095,
                    4096,
                    0xffffff,
                    0x1000000,
                    0x1000fff,
                    0x3fdffff,
                    0x3ffffff,
                    u32::MAX,
                ] {
                    for size in [1u32, 4096, 4097] {
                        let address = build.step(0, size, 4097, low, high);
                        build.inspect_free(address, size);
                    }
                }
            }
            for size in [
                4096u32,
                15 * 1024 * 1024 - 4096,
                15 * 1024 * 1024,
                16 * 1024 * 1024,
                64 * 1024 * 1024,
            ] {
                for high in [0xefffffu32, 0xffffff, 0x1000000, 0x1000fff, u32::MAX] {
                    let address = build.step(0, size, 4096, 0, high);
                    build.inspect_free(address, size);
                }
            }
            let address = build.step(5, 4097, 8193, 0, 0);
            build.inspect_free(address, 4097);
        }
        1 => {
            let mut addresses = Vec::new();
            loop {
                let address = build.step(0, 4096, 0, 0, u32::MAX);
                if address == 0 {
                    break;
                }
                addresses.push(address);
                assert!(addresses.len() <= 16384);
            }
            assert_eq!(addresses.len(), 16095);
            for _ in 0..256 {
                let address = build.step(0, 4096, 4096, 0, 0xffffff);
                assert_ne!(address, 0);
                addresses.push(address);
            }
            assert_eq!(build.step(0, 1, 4096, 0, 0xffffff), 0);
            build.step(1, 0x83fe0000, 128 * 1024, 0, 0);
            assert_eq!(build.step(3, 0x83ffffff, 0, 0, 0), 1);
            for address in addresses.iter().step_by(2) {
                assert_eq!(build.step(2, *address, 0, 0, 0), 1);
            }
            for address in &addresses {
                build.step(3, address + 4095, 0, 0, 0);
            }
            for alignment in [4096u32, 8192, 65536] {
                let address = build.step(0, 8192, alignment, 0, u32::MAX);
                build.inspect_free(address, 8192);
            }
            for address in addresses {
                build.step(2, address, 0, 0, 0);
            }
            assert_eq!(build.step(2, 0x83fe0000, 0, 0, 0), 1);
        }
        2 => {
            for (address, size) in [
                (0u32, 1u32),
                (0x7fffffff, 4096),
                (0x80000000, 1),
                (0x80000fff, 2),
                (0x80001003, 4096),
                (0x80001000, 1),
                (0x80002000, 4096),
                (0x83ffffff, 1),
                (0x84000000, 1),
                (u32::MAX, u32::MAX),
                (0x80010000, u32::MAX),
                (0x80010000, 0),
                (0x80004000, 4096),
            ] {
                build.step(1, address, size, 0, 0);
                for at in [address, address.saturating_add(1), address & !4095] {
                    build.step(3, at, 0, 0, 0);
                }
            }
            for address in [
                0x80001001u32,
                0x80002000,
                0x80001000,
                0x80001000,
                0x83fff000,
                0x80004000,
            ] {
                build.step(2, address, 0, 0, 0);
            }
            let address = build.step(0, 8192, 4096, 4096, u32::MAX);
            build.inspect_free(address, 8192);
            for page in (1..16384u32).step_by(3) {
                build.step(1, 0x80000000 + page * 4096 + 3, 32, 0, 0);
            }
            for page in 0..16384u32 {
                build.step(3, 0x80000000 + page * 4096 + 4095, 0, 0, 0);
            }
        }
        3 => {
            let mut seed = 0x514b37a9u32;
            let mut handles = [0u32; 256];
            for _ in 0..8192 {
                let choice = advance(&mut seed);
                let slot = (choice as usize >> 8) & 255;
                match choice & 7 {
                    0..=3 => {
                        if handles[slot] != 0 {
                            build.step(2, handles[slot], 0, 0, 0);
                        }
                        let size = advance(&mut seed) % 1048576;
                        let alignment =
                            (1u32 << (advance(&mut seed) % 26)).wrapping_add(choice & 3);
                        let low = if choice & 16 == 0 {
                            0
                        } else {
                            advance(&mut seed) % 67108864
                        };
                        let high = if choice & 32 == 0 {
                            u32::MAX
                        } else {
                            advance(&mut seed) % 67108864
                        };
                        let address = build.step(0, size, alignment, low, high);
                        handles[slot] = address;
                        if address != 0 {
                            build.step(4, address - 0x80000000, size.max(1).min(4096), 0, 0);
                        }
                    }
                    4 => {
                        build.step(2, handles[slot], 0, 0, 0);
                        handles[slot] = 0;
                    }
                    5 => {
                        build.step(3, handles[slot].wrapping_add(choice & 4095), 0, 0, 0);
                    }
                    _ => {
                        let address =
                            0x80000000 + (advance(&mut seed) % 16384) * 4096 + (choice & 31);
                        build.step(1, address, advance(&mut seed) % 8192, 0, 0);
                    }
                }
            }
            for address in handles {
                build.step(2, address, 0, 0, 0);
            }
        }
        4 => {}
        _ => unreachable!(),
    }
    let mut stats = [0u32; 6];
    unsafe {
        gamepkg_contig_stats(stats.as_mut_ptr());
    }
    let mut records = Vec::new();
    for page in 0..16384 {
        let mut record = [0u32; 2];
        unsafe {
            gamepkg_contig_record(page, record.as_mut_ptr());
        }
        records.push(record);
    }
    let mut bytes = b"CTG1".to_vec();
    bytes.extend(build.count.to_le_bytes());
    bytes.extend(build.vectors);
    Fixture {
        bytes,
        ram,
        records,
        stats,
        operations: build.count,
    }
}
