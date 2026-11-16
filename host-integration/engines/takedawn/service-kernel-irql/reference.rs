// Maintainer harness only: C oracle FFI is never admitted as game source.
mod gamepkg {
    pub use crate::client::*;
}
extern "C" {
    fn irql_reference(index: u32, out: *mut u8);
}
fn worker(_: local::runtime::TaskContext, id: u64) -> u64 {
    controls::irql_worker(id)
}
fn main() {
    let runtime = local::runtime::Runtime::new(
        &[local::runtime::Window {
            address: 0x20000000,
            bytes: 65536,
        }],
        vec![worker],
        4 * 1024 * 1024,
        4,
        65536,
    )
    .unwrap();
    let window = runtime.windows().unwrap()[0].0;
    runtime.commit(window, 0, 65536, 2).unwrap();
    local::install(runtime).unwrap();
    let mut ram = memory::Memory::new().unwrap();
    ram.attach(0x740000, 65536, window, 0, true).unwrap();
    let services = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2).unwrap();
    let mut vectors = Vec::new();
    for index in 0..2048u32 {
        let mut expected = [0u8; 128];
        unsafe { irql_reference(index, expected.as_mut_ptr()) };
        let actual = probe::probe_case(&mut ram, window, index).unwrap();
        assert_eq!(actual, expected, "case {index}");
        vectors.extend_from_slice(&expected);
    }
    if let Some(path) = std::env::args_os().nth(1) {
        std::fs::write(path, vectors).unwrap();
    }
    controls::controls(&mut ram, window).unwrap();
    services.service_close(&ram).unwrap();
    client::local_record_release(ram.call_handle()).unwrap();
    local::shutdown();
    println!("PASS: 2048 original-C IRQL ordered state/RAM/return witnesses, controlled callback and full CPU restore; not real IRQ delivery");
}
