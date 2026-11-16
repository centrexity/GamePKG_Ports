// Maintainer reference shell, never injected into admitted game code.
mod gamepkg {
    pub use crate::client::*;
}
unsafe extern "C" {
    fn timer_reference(index: u32, args: *const u64, out: *mut u8);
    fn timer_clock_reference(index: u32, args: *const u64) -> i64;
}
fn worker(_: local::runtime::TaskContext, arg: u64) -> u64 {
    probe::timer_worker(arg)
}
fn main() {
    local::install(
        local::runtime::Runtime::new(
            &[local::runtime::Window {
                address: 0x20000000,
                bytes: 65536,
            }],
            vec![worker],
            8 * 1024 * 1024,
            4,
            65536,
        )
        .unwrap(),
    )
    .unwrap();
    let actual = probe::timer_probe().unwrap();
    assert_eq!(actual.len(), 512 * 800 + 64 * 8);
    let mut vectors = Vec::new();
    for index in 0..512u32 {
        let args = cases::timer_case(index);
        let mut expected = [0u8; 800];
        unsafe {
            timer_reference(index, args.as_ptr(), expected.as_mut_ptr());
        }
        assert_eq!(
            &actual[index as usize * 800..index as usize * 800 + 800],
            &expected,
            "timer operation {index}"
        );
        vectors.extend(expected);
    }
    for index in 0..64u32 {
        let args = cases::timer_clock_case(index);
        let expected = unsafe { timer_clock_reference(index, args.as_ptr()) };
        assert_eq!(
            &actual[512 * 800 + index as usize * 8..512 * 800 + index as usize * 8 + 8],
            &expected.to_le_bytes(),
            "clock case {index}"
        );
        vectors.extend(expected.to_le_bytes());
    }
    if let Some(path) = std::env::args_os().nth(1) {
        std::fs::write(path, vectors).unwrap();
    }
    local::shutdown();
    println!("PASS 512 original timer slot/state/clock/ordered callback witnesses; synthetic callbacks, no shadow events or gameplay");
}
