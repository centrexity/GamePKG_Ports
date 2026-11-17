// Maintainer C-oracle/native SDK shell, forbidden even unused in game source.
mod gamepkg {
    pub use crate::client::*;
    pub fn diagnostic(_: u32, code: u32, values: &[u64]) -> Result<u64, i32> {
        eprintln!("event import control {code:x}: {values:?}");
        Ok(0)
    }
}
extern "C" {
    fn semaphore_import_reference(output: *mut u8) -> u32;
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
        vec![],
        256 * 1024 * 1024,
        4,
        1048576,
    )
    .unwrap();
    local::install(runtime).unwrap();
    let actual = setup::semaphore_import_run().unwrap();
    let mut expected = vec![0u8; 3100];
    assert_eq!(
        unsafe { semaphore_import_reference(expected.as_mut_ptr()) },
        3100
    );
    assert_eq!(actual, expected, "original C semaphore imports differ");
    if let Some(path) = std::env::args_os().nth(1) {
        std::fs::write(path, actual).unwrap();
    }
    local::shutdown();
    println!("PASS actual native SDK semaphore imports, original C vectors and protected-error controls; component only, no title gameplay");
}
