// Maintainer C comparison; never injected into checked game sources.
mod gamepkg {
    pub use crate::client::*;
}
unsafe extern "C" {
    fn shadow_map_reference(index: u32, args: *const u64, out: *mut u8) -> u32;
}
fn main() {
    local::install(local::runtime::Runtime::new(&[], vec![], 8 * 1024 * 1024, 0, 65536).unwrap())
        .unwrap();
    let actual = map_probe::map_probe().unwrap();
    assert_eq!(actual.len(), 409600);
    let mut expected = Vec::new();
    for index in 0..512u32 {
        let args = cases::shadow_case(index);
        let mut bytes = [0u8; 49184];
        let count = unsafe { shadow_map_reference(index, args.as_ptr(), bytes.as_mut_ptr()) };
        expected.extend(&bytes[..count as usize]);
    }
    assert_eq!(actual, expected, "original map slots/close order differ");
    if let Some(path) = std::env::args_os().nth(1) {
        std::fs::write(path, expected).unwrap();
    }
    local::shutdown();
    println!("PASS all 4096 logical slots, 512 original map/close-order operations, eight exact full-slot snapshots and revoked-owner cleanup; synthetic tokens, not native event admission");
}
