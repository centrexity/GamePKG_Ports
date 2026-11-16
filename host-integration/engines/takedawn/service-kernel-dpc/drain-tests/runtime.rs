// Maintainer-only pinned C drain oracle. Not admitted game code.
mod gamepkg {
    pub use crate::client::*;
}
unsafe extern "C" {
    fn drain_reference(
        group: u32,
        step: u32,
        initial: *const u8,
        guard_us: i64,
        lag_ms: i32,
        values: *const u64,
        out: *mut u8,
    );
}
fn worker(_: local::runtime::TaskContext, arg: u64) -> u64 {
    probe::drain_worker(arg)
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
    let actual = probe::drain_probe().unwrap();
    assert_eq!(actual.len(), 384 * 352);
    let mut vectors = Vec::new();
    for group in 0..96u32 {
        for step in 0..4u32 {
            let initial = cases::drain_initial(0x7bfff0).encode();
            let values = cases::drain_step_values(group, step);
            let mut expected = [0u8; 352];
            unsafe {
                drain_reference(
                    group,
                    step,
                    initial.as_ptr(),
                    cases::drain_guard(group),
                    cases::drain_lag(group),
                    values.as_ptr(),
                    expected.as_mut_ptr(),
                );
            }
            let at = (group * 4 + step) as usize * 352;
            assert_eq!(
                &actual[at..at + 352],
                &expected,
                "drain group {group}, step {step}"
            );
            vectors.extend(expected);
        }
    }
    if let Some(path) = std::env::args_os().nth(1) {
        std::fs::write(path, vectors).unwrap();
    }
    local::shutdown();
    println!("PASS 384 original retained-drain/guard/lag/CPU/full-RAM/ring/clock/profile witnesses; actual foreign gate contention, retained guest stack, holes, null routines, requeue, partial guard and fatal cleanup. Controlled providers/native routes, not title gameplay or measured pacing.");
}
