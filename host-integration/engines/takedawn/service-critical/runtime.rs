// Maintainer-only actual SDK provider. Never game source.
mod gamepkg {
    pub use crate::client::*;
}
fn actor(_: local::runtime::TaskContext, arg: u64) -> u64 {
    concurrency::critical_worker(arg)
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
            local::runtime::Window {
                address: 0x29000000,
                bytes: 1048576,
            },
        ],
        vec![actor],
        256 * 1024 * 1024,
        4,
        1048576,
    )
    .unwrap();
    local::install(runtime).unwrap();
    probe::critical_checks().unwrap();
    probe::critical_controls().unwrap();
    concurrency::critical_concurrency().unwrap();
    local::shutdown();
    println!(
        "PASS critical-section recursion, cancellation, revocation, actual task join and cleanup"
    );
}
