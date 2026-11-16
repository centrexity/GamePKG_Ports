// Maintainer shell; private proof construction is tested by the app compiler gate.
mod gamepkg {
    pub use crate::client::*;
}
fn worker(_: local::runtime::TaskContext, value: u64) -> u64 {
    probe::join_worker(value)
}
fn main() {
    local::install(
        local::runtime::Runtime::new(&[], vec![worker], 8 * 1024 * 1024, 4, 65536).unwrap(),
    )
    .unwrap();
    let bytes = probe::join_probe().unwrap();
    assert_eq!(bytes.len(), 48);
    if let Some(path) = std::env::args_os().nth(1) {
        std::fs::write(path, bytes).unwrap();
    }
    local::shutdown();
    println!("PASS shared actual join from queue, schedule and SDK; false/wrong/borrowed/foreign claims rejected, full-width results and revoked cleanup. Not gameplay.");
}
