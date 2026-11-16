// Maintainer-only independent C oracle, never staged as game source.
unsafe extern "C" {
    fn gamepkg_heap_reset(ram: *mut u8, base: u32, top: u32);
    fn gamepkg_heap_stats(out: *mut u32);
    fn gamepkg_tib(template: u32, total: u32) -> u32;
}
fn heap_worker(_: local::runtime::TaskContext, arg: u64) -> u64 {
    heap_service::heap_serve(arg)
}
fn tib_worker(_: local::runtime::TaskContext, arg: u64) -> u64 {
    probe::tib_worker(arg)
}
fn main() {
    let runtime = local::runtime::Runtime::new(
        &[
            local::runtime::Window {
                address: 0x20000000,
                bytes: 67108864,
            },
            local::runtime::Window {
                address: 0x24000000,
                bytes: 67108864,
            },
            local::runtime::Window {
                address: 0x28000000,
                bytes: 1048576,
            },
        ],
        vec![heap_worker, tib_worker],
        256 * 1024 * 1024,
        8,
        1048576,
    )
    .unwrap();
    local::install(runtime).unwrap();
    let mut vectors = Vec::new();
    for mode in 0..7u32 {
        let case = probe::tib_case(mode).unwrap();
        let mut ram = vec![0xa5u8; 67108864];
        for at in 0..64usize {
            ram[0x4000 + at] = (at as u32 * 37 + mode * 11) as u8;
        }
        for at in 0..case[1] as usize {
            ram[case[0] as usize + at] = (at as u32 * 71 + mode * 13) as u8;
        }
        unsafe {
            gamepkg_heap_reset(ram.as_mut_ptr(), 0x1000000, case[2]);
        }
        let first = unsafe { gamepkg_tib(case[0], case[1]) };
        ram[0x4010..0x4014].copy_from_slice(&0x12345678u32.to_le_bytes());
        if case[1] != 0 {
            ram[(case[0] + case[1] - 1) as usize] = 0x6d;
        }
        let second = unsafe { gamepkg_tib(case[0], case[1]) };
        let mut stats = [0u32; 3];
        unsafe {
            gamepkg_heap_stats(stats.as_mut_ptr());
        }
        let mut expected = Vec::new();
        for value in [first, second, stats[0], stats[1], stats[2]] {
            expected.extend(value.to_le_bytes());
        }
        let mut hash = sha256::Sha256::sha_new();
        hash.sha_update(&ram).unwrap();
        expected.extend(hash.sha_finish());
        let actual = probe::tib_sample(mode).unwrap();
        assert_eq!(actual, expected, "original per-thread TIB mode {mode}");
        vectors.extend(expected);
    }
    local::shutdown();
    assert_eq!(vectors.len(), 364);
    if let Some(path) = std::env::args_os().nth(1) {
        std::fs::write(path, vectors).unwrap();
    }
    println!("PASS original thread TIB: seven modes, two allocations on actual SDK worker, separate heap actor, full 64MiB RAM hashes and allocation stats; not title execution");
}
