// Maintainer-only runtime shell; no IRQ/device callback is provided.
mod gamepkg {
    pub use crate::client::*;
}
fn worker(_: local::runtime::TaskContext, arg: u64) -> u64 {
    probe::gate_worker(arg)
}
fn main() {
    local::install(
        local::runtime::Runtime::new(&[], vec![worker], 4 * 1024 * 1024, 4, 65536).unwrap(),
    )
    .unwrap();
    for _ in 0..32 {
        probe::gate_probe().unwrap();
    }
    local::shutdown();
}
