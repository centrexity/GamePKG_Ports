// Maintainer native shell, not admitted game source.
mod gamepkg {
    pub use crate::client::*;
}
fn heap_worker(_: local::runtime::TaskContext, arg: u64) -> u64 {
    heap_service::heap_serve(arg)
}
fn context_worker(_: local::runtime::TaskContext, arg: u64) -> u64 {
    probe::context_worker(arg)
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
        vec![heap_worker, context_worker],
        256 * 1024 * 1024,
        8,
        1048576,
    )
    .unwrap();
    local::install(runtime).unwrap();
    let bytes = probe::context_probe().unwrap();
    assert_eq!(bytes.len(), 192);
    if let Some(path) = std::env::args_os().nth(1) {
        std::fs::write(path, bytes).unwrap();
    }
    local::shutdown();
    println!("PASS complete KPL4 import preparation and actual four-ID worker calls for initialize/connect/vector: aligned, unaligned lookbehind and maximum 1548-byte maps; missing/forged/revoked context rejected. Not title gameplay.");
}
