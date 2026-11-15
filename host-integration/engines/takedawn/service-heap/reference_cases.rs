// Maintainer-only original oracle inputs. Never included in a game package.
unsafe extern "C" {
    fn gamepkg_heap_reset(ram: *mut u8, base: u32, top: u32);
    fn gamepkg_heap_operation(op: u32, first: u32, second: u32) -> u32;
    fn gamepkg_heap_stats(out: *mut u32);
    fn gamepkg_heap_record(index: u32, out: *mut u32) -> i32;
}
pub const CASES: usize = 8;
pub struct Fixture {
    pub ram: Vec<u8>,
    pub bytes: Vec<u8>,
    pub stats: [u32; 3],
    pub records: Vec<[u32; 3]>,
    pub operations: u32,
}
impl Fixture {
    fn new(base: u32, top: u32) -> Self {
        assert!((65536..=67108864).contains(&base) && base <= top && top <= 67108864);
        let mut ram = vec![0xa5; 67108864];
        unsafe { gamepkg_heap_reset(ram.as_mut_ptr(), base, top) };
        let mut bytes = b"GPH1".to_vec();
        for word in [base, top, 0] {
            bytes.extend(word.to_le_bytes());
        }
        Self {
            ram,
            bytes,
            stats: [base, 0, 0],
            records: Vec::new(),
            operations: 0,
        }
    }
    fn capture(&mut self, op: u32, a: u32, b: u32, result: u32, error: i32) {
        unsafe { gamepkg_heap_stats(self.stats.as_mut_ptr()) };
        for word in [
            op,
            a,
            b,
            result,
            error as u32,
            self.stats[0],
            self.stats[1],
            self.stats[2],
        ] {
            self.bytes.extend(word.to_le_bytes());
        }
        self.operations += 1;
    }
    fn operation(&mut self, op: u32, a: u32, b: u32) -> u32 {
        assert!(op <= 3);
        if op == 0 {
            // Unsafe original arithmetic must not wrap. Invalid cases below
            // never enter C; the checked port must reject them without writes.
            let size = u64::from(a.max(16));
            let mask = u64::from(b.max(4) - 1);
            let sum = u64::from(self.stats[0]) + mask;
            assert!(sum <= u32::MAX as u64);
            assert!((sum & !mask) + size <= u32::MAX as u64);
        }
        let result = if op == 3 {
            assert!(b <= 65536 && u64::from(a) + u64::from(b) <= 67108864);
            self.ram[a as usize..a as usize + b as usize].fill(0x7b);
            0
        } else {
            unsafe { gamepkg_heap_operation(op, a, b) }
        };
        self.capture(op, a, b, result, 0);
        result
    }
    fn invalid_allocate(&mut self, size: u32, alignment: u32) {
        let mask = u64::from(alignment.max(4) - 1);
        let sum = u64::from(self.stats[0]) + mask;
        assert!(sum > u32::MAX as u64 || (sum & !mask) + u64::from(size.max(16)) > u32::MAX as u64);
        self.capture(0, size, alignment, 0, -1);
    }
    fn finish(mut self) -> Self {
        self.bytes[12..16].copy_from_slice(&self.operations.to_le_bytes());
        for number in 0..self.stats[1] {
            let mut record = [0; 3];
            assert_eq!(
                unsafe { gamepkg_heap_record(number, record.as_mut_ptr()) },
                1
            );
            self.records.push(record);
        }
        let mut extra = [0; 3];
        assert_eq!(
            unsafe { gamepkg_heap_record(self.stats[1], extra.as_mut_ptr()) },
            0
        );
        self
    }
}
pub fn make_case(index: usize) -> Fixture {
    let mut f = match index {
        3 => Fixture::new(0x3fffff0, 0x4000000),
        4 => Fixture::new(0x4000000, 0x4000000),
        5 => Fixture::new(0x1000000, 0x2000000),
        _ => Fixture::new(0x1000000, 0x4000000),
    };
    match index {
        0 => {
            for size in [0, 1, 15, 16, 17, 31, 32, 63, 64, 65] {
                let address = f.operation(0, size, 4);
                assert_ne!(address, 0);
                f.operation(2, address, 0);
                f.operation(2, address + size.max(16) - 1, 0);
                f.operation(2, address + size.max(16), 0);
                f.operation(3, address, size.max(16));
                f.operation(1, address + 1, 0);
                f.operation(2, address, 0);
                f.operation(1, address, 0);
                f.operation(1, address, 0);
                f.operation(2, address, 0);
            }
            f.operation(1, 0, 0);
            f.operation(2, 0, 0);
            f.operation(1, 0xffffffff, 0);
            f.operation(2, 0xffffffff, 0);
        }
        1 => {
            let a = f.operation(0, 64, 4);
            let b = f.operation(0, 128, 4);
            let c = f.operation(0, 64, 4);
            for (addr, bytes) in [(a, 64), (b, 128), (c, 64)] {
                f.operation(3, addr, bytes);
            }
            f.operation(1, b, 0);
            f.operation(1, c, 0);
            f.operation(1, a, 0);
            assert_eq!(f.operation(0, 32, 4), a);
            assert_eq!(f.operation(2, a, 0), 256);
            assert_eq!(f.operation(2, a + 31, 0), 225);
            assert_eq!(f.operation(2, a + 256, 0), 0);
            f.operation(1, b, 0);
            f.operation(2, a, 0);
            f.operation(1, a, 0);
            assert_eq!(f.operation(0, 240, 4), a);
            assert_eq!(f.operation(2, a, 0), 256);
        }
        2 => {
            let mut addresses = Vec::new();
            for alignment in [
                0, 1, 2, 3, 4, 5, 6, 7, 8, 15, 16, 17, 31, 32, 63, 64, 255, 256, 4095, 4096, 65535,
                65536, 1048576,
            ] {
                let a = f.operation(0, 17, alignment);
                assert_ne!(a, 0);
                addresses.push(a);
                f.operation(3, a, 17);
            }
            for a in addresses {
                f.operation(1, a, 0);
            }
            for alignment in [1048576, 6, 32, 16, 5, 4] {
                let a = f.operation(0, 16, alignment);
                if a != 0 {
                    f.operation(2, a, 0);
                }
            }
        }
        3 => {
            let a = f.operation(0, 16, 4);
            assert_eq!(a, 0x3fffff0);
            assert_eq!(f.operation(0, 0, 4), 0);
            f.operation(1, a, 0);
            assert_eq!(f.operation(0, 1, 4), a);
            assert_eq!(f.operation(0, 17, 4), 0);
        }
        4 => {
            assert_eq!(f.operation(0, 0, 4), 0);
            f.operation(2, 0x4000000, 0);
            f.operation(1, 0x4000000, 0);
        }
        5 => {
            let mut seed = 0x12345678u32;
            let mut addresses = Vec::new();
            for _ in 0..12000 {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                match seed % 5 {
                    0 if !addresses.is_empty() => {
                        let a = addresses[seed as usize % addresses.len()];
                        f.operation(1, a, 0);
                    }
                    1 if !addresses.is_empty() => {
                        let a = addresses[seed as usize % addresses.len()];
                        f.operation(2, a + (seed & 15), 0);
                    }
                    _ => {
                        let a = f.operation(
                            0,
                            (seed >> 8) & 8191,
                            [4, 8, 16, 32, 64, 5, 6, 7][seed as usize & 7],
                        );
                        if a != 0 {
                            addresses.push(a);
                        }
                    }
                }
            }
        }
        6 => {
            f.invalid_allocate(u32::MAX, 4);
            f.invalid_allocate(16, u32::MAX);
            f.invalid_allocate(0, 0xfffffff0);
            let mut last = 0;
            for _ in 0..65540 {
                last = f.operation(0, 16, 4);
                assert_ne!(last, 0);
            }
            assert_eq!(f.stats[1], 65536);
            assert_eq!(f.stats[2], 65540);
            assert_eq!(f.operation(2, last, 0), 0);
            f.operation(1, last, 0);
            assert_eq!(f.operation(2, last, 0), 0);
            f.operation(1, 0x1000000, 0);
            assert_eq!(f.operation(0, 0, 4), 0x1000000);
        }
        7 => {}
        _ => panic!("unknown fixture"),
    }
    f.finish()
}
