// Maintainer reference driver; never compiled into admitted game objects.
mod gamepkg {
    pub use crate::client::*;
}
unsafe extern "C" {
    fn shadow_event_reference(index: u32, args: *const u32, out: *mut u8);
}
fn worker(_: local::runtime::TaskContext, arg: u64) -> u64 {
    event_workers::event_worker(arg)
}
fn main() {
    local::install(
        local::runtime::Runtime::new(
            &[local::runtime::Window {
                address: 0x20000000,
                bytes: 65536,
            }],
            vec![worker],
            16 * 1024 * 1024,
            8,
            65536,
        )
        .unwrap(),
    )
    .unwrap();
    let actual = event_probe::event_probe().unwrap();
    assert_eq!(actual.len(), 256 * 1232);
    let mut vectors = Vec::new();
    for index in 0..256u32 {
        let args = cases::event_case(index);
        let mut expected = [0u8; 1232];
        unsafe {
            shadow_event_reference(index, args.as_ptr(), expected.as_mut_ptr());
        }
        assert_eq!(
            &actual[index as usize * 1232..index as usize * 1232 + 1232],
            &expected,
            "event case {index}"
        );
        vectors.extend(expected);
    }
    if let Some(path) = std::env::args_os().nth(1) {
        std::fs::write(path, vectors).unwrap();
    }
    local::shutdown();
    println!("PASS 256 pinned event/header/lazy/reset/signal witnesses, actual SDK concurrent first users/auto consumption, quota and fatal-prefix cleanup controls; not full guest waits or gameplay");
}
