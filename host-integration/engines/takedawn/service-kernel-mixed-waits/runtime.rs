// Maintainer-only original-C/actual-SDK provider; never admitted.
mod gamepkg {
    pub use crate::client::*;
}
fn mixed_worker(_: local::runtime::TaskContext, arg: u64) -> u64 {
    concurrency::mixed_worker(arg)
}
unsafe extern "C" {
    fn mixed_reference(output: *mut u8, length: usize) -> usize;
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
        vec![mixed_worker],
        256 * 1024 * 1024,
        4,
        1048576,
    )
    .unwrap();
    local::install(runtime).unwrap();
    controls::mixed_controls().unwrap();
    concurrency::mixed_concurrency().unwrap();
    let actual = probe::mixed_vectors().unwrap();
    let mut expected = vec![0u8; 2560];
    assert_eq!(
        unsafe { mixed_reference(expected.as_mut_ptr(), expected.len()) },
        2560
    );
    assert_eq!(actual, expected);
    if let Some(path) = std::env::args_os().nth(1) {
        std::fs::write(path, &actual).unwrap();
    }
    local::shutdown();
    println!("PASS 128 atomic mixed event/semaphore cases against pinned original wait_many; not NT/title integration");
}
