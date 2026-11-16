// Maintainer-only original C comparison; never an admitted game source.
mod gamepkg {
    pub use crate::client::*;
}
extern "C" {
    fn irq_reference(before: *const u8, out: *mut u8);
    fn irq_wait_reference(elapsed_us: u64, previous: u32, out: *mut u32) -> i32;
}
fn main() {
    let runtime = local::runtime::Runtime::new(
        &[local::runtime::Window {
            address: 0x20000000,
            bytes: 0x4000000,
        }],
        vec![],
        128 * 1048576,
        4,
        65536,
    )
    .unwrap();
    let window = runtime.windows().unwrap()[0].0;
    runtime.commit(window, 0x740000, 65536, 2).unwrap();
    runtime.commit(window, 0x780000, 1048576, 2).unwrap();
    local::install(runtime).unwrap();
    let mut vectors = Vec::new();
    for index in 0..128u32 {
        let before = cases::irq_cpu(index).unwrap().encode();
        let mut expected = [0u8; 452];
        unsafe { irq_reference(before.as_ptr(), expected.as_mut_ptr()) };
        let mut ram = memory::Memory::new().unwrap();
        ram.attach(0, 0x4000000, window, 0, true).unwrap();
        let services = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2).unwrap();
        let actual = probe::irq_probe_case(&mut ram, window, index).unwrap();
        assert_eq!(actual, expected, "native IRQ case {index}");
        vectors.extend_from_slice(&expected);
        services.service_close(&ram).unwrap();
        client::local_record_release(ram.call_handle()).unwrap();
    }
    if let Some(path) = std::env::args_os().nth(1) {
        std::fs::write(path, vectors).unwrap();
    }
    let mut checked = 0u32;
    let mut wider = 0u32;
    for elapsed_us in [
        0u64, 1, 2147483646, 2147483647, 2147483648, 4294967295, 4294967296, 4294967297, 8589934591,
    ] {
        for previous in [0u32, 1, 2147483646, 2147483647] {
            let mut expected = 0u32;
            let available = unsafe { irq_wait_reference(elapsed_us, previous, &mut expected) };
            let actual = irq_delivery::irq_wait_maximum(
                1000000000 + elapsed_us * 1000,
                1000000000,
                previous,
            )
            .unwrap();
            if available == 32 || elapsed_us <= i32::MAX as u64 {
                assert_eq!(actual, expected, "signed32 wait {elapsed_us}/{previous}");
                checked += 1;
            } else {
                assert_eq!(available, 64);
                assert_eq!(
                    expected, elapsed_us as u32,
                    "native wider-long narrowing {elapsed_us}/{previous}"
                );
                wider += 1;
            }
        }
    }
    assert_eq!(irq_delivery::irq_wait_maximum(0, 0, u32::MAX), Err(-1));
    println!("PASS: signed32/common-range wait-stat reference cases {checked}; observed wider-long differences {wider}; no gameplay duration limit");
    local::shutdown();
    println!("PASS: 128 original-C native IRQ CPU/ordered RAM/stats/IRQL/stack witnesses; controlled handlers, not real devices");
}
