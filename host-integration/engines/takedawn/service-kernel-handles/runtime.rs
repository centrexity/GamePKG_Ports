// Maintainer native oracle shell; never admitted as game source.
mod gamepkg {
    pub use crate::client::*;
}
extern "C" {
    fn handle_reference(index: u32, args: *const u64, out: *mut u8) -> u32;
}
fn handle_worker(_: local::runtime::TaskContext, arg: u64) -> u64 {
    controls::handle_worker(arg)
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
        vec![handle_worker],
        256 * 1024 * 1024,
        4,
        1048576,
    )
    .unwrap();
    local::install(runtime).unwrap();
    let actual = probe::handle_probe().unwrap();
    let mut expected = Vec::new();
    for index in 0..512u32 {
        let args = cases::handle_case(index);
        let mut out = vec![0u8; 131080];
        let count = unsafe { handle_reference(index, args.as_ptr(), out.as_mut_ptr()) };
        expected.extend(&out[..count as usize]);
    }
    assert_eq!(actual, expected, "original tagged handle table differs");
    assert_eq!(actual.len(), 139256);
    if let Some(path) = std::env::args_os().nth(1) {
        std::fs::write(path, expected).unwrap();
    }
    local::shutdown();
    println!("PASS all 16383 tagged slots, 512 original handle operations and full final-slot snapshot; metadata only, no native pointer/event authority");
}
