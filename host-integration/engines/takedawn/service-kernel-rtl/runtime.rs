// Maintainer C-oracle/native SDK shell, forbidden even unused in game source.
mod gamepkg {
    pub use crate::client::*;
    pub fn diagnostic(_: u32, code: u32, values: &[u64]) -> Result<u64, i32> {
        eprintln!("rtl control {code:x}: {values:?}");
        Ok(0)
    }
}
extern "C" {
    fn rtl_reference(output: *mut u8) -> u32;
}
fn main() {
    let runtime = local::runtime::Runtime::new(
        &[local::runtime::Window {
            address: 0x20000000,
            bytes: 67108864,
        }],
        vec![],
        256 * 1024 * 1024,
        4,
        1048576,
    )
    .unwrap();
    local::install(runtime).unwrap();
    let actual = setup::rtl_run().unwrap();
    let mut expected = vec![0u8; 2872];
    assert_eq!(unsafe { rtl_reference(expected.as_mut_ptr()) }, 2872);
    assert_eq!(actual, expected, "original C RTL imports differ");
    if let Some(path) = std::env::args_os().nth(1) {
        std::fs::write(path, actual).unwrap();
    }
    local::shutdown();
    println!("PASS actual native SDK RTL imports, original C vectors and protected-error controls; component only, no title gameplay");
}
