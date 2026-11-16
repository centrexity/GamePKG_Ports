// Maintainer C-reference shell. Excluded from admitted fixture sources.
mod gamepkg {
    pub use crate::client::*;
}
unsafe extern "C" {
    fn notify_reference(index: u32, out: *mut u64);
}
fn worker(_: local::runtime::TaskContext, arg: u64) -> u64 {
    probe::wake_worker(arg)
}
fn main() {
    local::install(
        local::runtime::Runtime::new(
            &[local::runtime::Window {
                address: 0x20000000,
                bytes: 65536,
            }],
            vec![worker],
            4 * 1024 * 1024,
            4,
            65536,
        )
        .unwrap(),
    )
    .unwrap();
    let actual = probe::wake_probe().unwrap();
    assert_eq!(actual.len(), 512 * 24);
    let mut expected = Vec::new();
    for index in 0..512u32 {
        let mut values = [0u64; 3];
        unsafe {
            notify_reference(index, values.as_mut_ptr());
        }
        let bytes = values
            .into_iter()
            .flat_map(u64::to_le_bytes)
            .collect::<Vec<_>>();
        assert_eq!(
            &actual[index as usize * 24..index as usize * 24 + 24],
            &bytes,
            "DPC wake vector {index}"
        );
        expected.extend(bytes);
    }
    if let Some(path) = std::env::args_os().nth(1) {
        std::fs::write(path, expected).unwrap();
    }
    local::shutdown();
    println!("PASS: 512 original DpcQueued/release policy witnesses with actual task, recursive gate and wake generations; lifecycle/insert/no-lost-prewait-wake controls. No callbacks/gameplay.");
}
