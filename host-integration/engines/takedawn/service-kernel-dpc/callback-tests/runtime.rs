// Maintainer C-reference shell. Never admitted game code.
mod gamepkg {
    pub use crate::client::*;
}
unsafe extern "C" {
    fn callback_reference(index: u32, initial: *const u8, params: *const u32, out: *mut u8);
}
fn worker(_: local::runtime::TaskContext, arg: u64) -> u64 {
    probe::callback_worker(arg)
}
fn main() {
    local::install(
        local::runtime::Runtime::new(
            &[local::runtime::Window {
                address: 0x20000000,
                bytes: 524288,
            }],
            vec![worker],
            8 * 1024 * 1024,
            4,
            65536,
        )
        .unwrap(),
    )
    .unwrap();
    let actual = probe::callback_probe().unwrap();
    assert_eq!(actual.len(), 256 * 336);
    let mut vectors = Vec::new();
    for index in 0..256u32 {
        let initial = cases::callback_initial(0x7bfff0).encode();
        let params = cases::callback_params(index);
        let mut expected = [0u8; 336];
        unsafe {
            callback_reference(
                index,
                initial.as_ptr(),
                params.as_ptr(),
                expected.as_mut_ptr(),
            );
        }
        assert_eq!(
            &actual[index as usize * 336..index as usize * 336 + 336],
            &expected,
            "callback vector {index}"
        );
        vectors.extend(expected);
    }
    if let Some(path) = std::env::args_os().nth(1) {
        std::fs::write(path, vectors).unwrap();
    }
    local::shutdown();
    println!("PASS 256 original DPC CPU/full-RAM/return/profile-order witnesses; actual retained worker stack and IRQL cleanup, explicit unsupported and bad-return/body/observer/readonly controls. Synthetic finite callbacks, not title gameplay.");
}
