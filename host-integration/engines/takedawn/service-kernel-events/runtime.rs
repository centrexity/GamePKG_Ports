// Maintainer native oracle shell; never admitted as game source.
mod gamepkg {
    pub use crate::client::*;
}
fn event_worker(_: local::runtime::TaskContext, arg: u64) -> u64 {
    probe::event_worker(arg)
}
fn wait_worker(_: local::runtime::TaskContext, arg: u64) -> u64 {
    wait_probe::wait_worker(arg)
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
        vec![event_worker, wait_worker],
        256 * 1024 * 1024,
        4,
        1048576,
    )
    .unwrap();
    local::install(runtime).unwrap();
    let actual = probe::events_probe().unwrap();
    assert_eq!(actual.len(), 48);
    if let Some(path) = std::env::args_os().nth(1) {
        std::fs::write(path, actual).unwrap();
    }
    local::shutdown();
    println!("PASS native event ownership and aliases; component only, no title admission");
}
