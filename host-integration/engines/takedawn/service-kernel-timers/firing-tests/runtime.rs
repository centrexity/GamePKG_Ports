// Maintainer native shell, never injected into admitted game sources.
mod gamepkg {
    pub use crate::client::*;
}
fn worker(_: local::runtime::TaskContext, arg: u64) -> u64 {
    probe::firing_worker(arg)
}
fn main() {
    local::install(
        local::runtime::Runtime::new(
            &[local::runtime::Window {
                address: 0x20000000,
                bytes: 524288,
            }],
            vec![worker],
            16 * 1024 * 1024,
            4,
            65536,
        )
        .unwrap(),
    )
    .unwrap();
    let bytes = probe::firing_probe().unwrap();
    assert_eq!(bytes.len(), 9 * 512);
    if let Some(path) = std::env::args_os().nth(1) {
        std::fs::write(path, bytes).unwrap();
    }
    local::shutdown();
    println!("PASS nine native timer/callback/event compositions, actual foreign-mutex acquisition inside callbacks, replacement/removal, partial failures and real shared join; synthetic routes, not title gameplay or whole-loop fidelity");
}
