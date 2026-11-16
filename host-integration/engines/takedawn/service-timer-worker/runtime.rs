// Maintainer component shell; never staged as game source.
mod gamepkg {
    pub use crate::client::*;
}
fn heap_worker(_: local::runtime::TaskContext, arg: u64) -> u64 {
    heap_service::heap_serve(arg)
}
fn startup_worker(_: local::runtime::TaskContext, arg: u64) -> u64 {
    probe::startup_worker(arg)
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
        vec![heap_worker, startup_worker],
        256 * 1024 * 1024,
        8,
        1048576,
    )
    .unwrap();
    local::install(runtime).unwrap();
    let vectors = probe::startup_probe().unwrap();
    local::shutdown();
    assert_eq!(vectors.len(), 960);
    if let Some(path) = std::env::args_os().nth(1) {
        std::fs::write(path, vectors).unwrap();
    }
    println!("PASS ten actual timer-worker startup modes: retained guest stack, heap-owned TIB and CPU, exhaustion/foreign/context/revocation/cancellation cleanup; not title gameplay");
}
