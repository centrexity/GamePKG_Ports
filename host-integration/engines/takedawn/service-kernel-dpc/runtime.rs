// Maintainer-only deterministic clock and original-C oracle. Never admitted.
mod gamepkg {
    pub use crate::client::*;
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    pub fn reset() {
        COUNTER.store(0, std::sync::atomic::Ordering::SeqCst);
    }
    pub fn local_performance_sample() -> Result<PerformanceSample, i32> {
        crate::client::local_task_status()?;
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(PerformanceSample {
            counter: 1000000000000 + n * 12345,
            frequency: 1000000000,
            uptime_ms: 1000000,
        })
    }
}
unsafe extern "C" {
    fn dpc_reference_init(enabled: i32);
    fn dpc_reference(op: u32, dpc: u32, first: u32, second: u32, timestamp: u64, out: *mut u8);
}
fn worker(_: local::runtime::TaskContext, arg: u64) -> u64 {
    probe::dpc_worker(arg)
}
fn main() {
    let stats = std::env::args().nth(1).unwrap().parse::<u32>().unwrap();
    assert!(stats <= 1);
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
    probe::dpc_controls().unwrap();
    gamepkg::reset();
    let actual = probe::dpc_probe(stats == 1).unwrap();
    assert_eq!(actual.len(), 1024 * 72);
    unsafe { dpc_reference_init(stats as i32) };
    let mut expected = Vec::new();
    let mut samples = 0;
    for index in 0..1024u32 {
        let c = cases::queue_case(index);
        let mut bytes = [0u8; 72];
        unsafe {
            dpc_reference(
                c[0],
                c[1],
                c[2],
                c[3],
                1000000000000 + samples * 12345,
                bytes.as_mut_ptr(),
            );
        }
        samples += u64::from_le_bytes(bytes[64..72].try_into().unwrap());
        assert_eq!(
            &actual[index as usize * 72..(index as usize + 1) * 72],
            &bytes,
            "vector {index}, stats {stats}"
        );
        expected.extend(bytes);
    }
    if let Some(path) = std::env::args_os().nth(2) {
        std::fs::write(path, expected).unwrap();
    }
    local::shutdown();
    println!("PASS: 1024 original queue/RAM/result/pending/clock-use vectors stats={stats}; actual reviewed consumer task, forged-event rejection, roles, join and cleanup. No DPC callbacks or IRQL execution.");
}
