// Maintainer original-C oracle, never part of admitted game source.
unsafe extern "C" {
    fn isr_reference_init(ram: *mut u8);
    fn isr_reference_case(values: *const u32, out: *mut u8);
}
mod gamepkg {
    pub use crate::client::*;
}
fn heap_worker(_: local::runtime::TaskContext, arg: u64) -> u64 {
    heap_service::heap_serve(arg)
}
fn isr_worker(_: local::runtime::TaskContext, arg: u64) -> u64 {
    probe::isr_worker(arg)
}
fn fault_worker(_: local::runtime::TaskContext, arg: u64) -> u64 {
    faults::fault_worker(arg)
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
        vec![heap_worker, isr_worker, fault_worker],
        256 * 1024 * 1024,
        8,
        1048576,
    )
    .unwrap();
    local::install(runtime).unwrap();
    let actual = probe::isr_probe().unwrap();
    let mut ram = vec![0u8; 67108864];
    let mut expected = Vec::new();
    unsafe {
        isr_reference_init(ram.as_mut_ptr());
    }
    for index in 0..128u32 {
        let input = cases::isr_case(index);
        let mut bytes = [0u8; 276];
        unsafe {
            isr_reference_case(input.as_ptr(), bytes.as_mut_ptr());
        }
        assert_eq!(
            &actual[index as usize * 276..(index as usize + 1) * 276],
            bytes,
            "original ISR case {index}"
        );
        expected.extend(bytes);
    }
    let fault_vectors = faults::fault_probe().unwrap();
    assert_eq!(fault_vectors.len(), 640);
    expected.extend(fault_vectors);
    local::shutdown();
    if let Some(path) = std::env::args_os().nth(1) {
        std::fs::write(path, expected).unwrap();
    }
    println!("PASS 128 original-C registration/ISR conventions on an actual retained worker: vector table, 44-byte objects, missing/null/manual routes, stdcall, return bit, IRQL/PCR, profiling order and CPU preservation, plus ten checked fatal-prefix/lifetime modes; not whole device or title qualification");
}
