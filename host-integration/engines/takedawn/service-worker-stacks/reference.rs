// Maintainer-only FFI oracle; deliberately excluded from admitted sources.
mod gamepkg {
    pub use crate::client::*;
}
extern "C" {
    fn stacks_reference(index: u32, out: *mut u8);
}
fn worker(_: local::runtime::TaskContext, arg: u64) -> u64 {
    controls::stacks_worker(arg)
}
fn main() {
    let runtime = local::runtime::Runtime::new(
        &[local::runtime::Window {
            address: 0x20000000,
            bytes: 0x4000000,
        }],
        vec![worker],
        128 * 1048576,
        4,
        65536,
    )
    .unwrap();
    let window = runtime.windows().unwrap()[0].0;
    runtime.commit(window, 0x780000, 1048576, 2).unwrap();
    local::install(runtime).unwrap();
    let mut ram = memory::Memory::new().unwrap();
    ram.attach(0, 0x4000000, window, 0, true).unwrap();
    let services = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2).unwrap();
    let mut vectors = Vec::new();
    for index in 0..128u32 {
        let mut expected = [0u8; 4096];
        unsafe { stacks_reference(index, expected.as_mut_ptr()) };
        let actual = probe::stacks_vector(&ram, index).unwrap();
        assert_eq!(actual, expected, "worker stack case {index}");
        vectors.extend_from_slice(&expected);
    }
    controls::stacks_controls(&mut ram, window).unwrap();
    if let Some(path) = std::env::args_os().nth(1) {
        std::fs::write(path, vectors).unwrap();
    }
    services.service_close(&ram).unwrap();
    client::local_record_release(ram.call_handle()).unwrap();
    local::shutdown();
    println!("PASS: 16384 original-C worker allocation/free witnesses plus real task ownership/cancellation and revocation controls; not IRQ delivery");
}
