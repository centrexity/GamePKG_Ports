// Maintainer native oracle shell; never admitted as game source.
mod gamepkg {
    pub use crate::client::*;
}
fn semaphore_worker(_: local::runtime::TaskContext, arg: u64) -> u64 {
    probe::semaphore_worker(arg)
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
        vec![semaphore_worker, wait_worker, creation_worker],
        256 * 1024 * 1024,
        4,
        1048576,
    )
    .unwrap();
    local::install(runtime).unwrap();
    let actual = probe::semaphore_probe().unwrap();
    assert_eq!(actual.len(), 1344);
    unsafe {
        let mut expected = vec![0u8; 1344];
        assert_eq!(semaphore_reference(expected.as_mut_ptr()), 1344);
        assert_eq!(actual, expected);
    }
    if let Some(path) = std::env::args_os().nth(1) {
        std::fs::write(path, actual).unwrap();
    }
    local::shutdown();
    println!("PASS native semaphore ownership, original-C counts and aliases; component only, no title admission");
}

extern "C" {
    fn semaphore_reference(output: *mut u8) -> u32;
}
fn creation_worker(_: local::runtime::TaskContext, arg: u64) -> u64 {
    probe::creation_worker(arg)
}
