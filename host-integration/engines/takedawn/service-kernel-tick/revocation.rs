// Maintainer-only deterministic race seam. Never admitted or linked into game objects.
mod gamepkg {
    pub use crate::client::*;
    use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
    static ROOT: AtomicU64 = AtomicU64::new(0);
    static KSV: AtomicU64 = AtomicU64::new(0);
    static NEXT: AtomicU64 = AtomicU64::new(0);
    static READS: AtomicUsize = AtomicUsize::new(0);
    static FIRED: AtomicBool = AtomicBool::new(false);
    pub fn arm(root: u64, ksv: u64, next: u64) {
        NEXT.store(next, Ordering::SeqCst);
        ROOT.store(root, Ordering::SeqCst);
        KSV.store(ksv, Ordering::SeqCst);
        READS.store(0, Ordering::SeqCst);
        FIRED.store(false, Ordering::SeqCst);
    }
    pub fn fired() -> bool {
        FIRED.load(Ordering::SeqCst)
    }
    pub fn local_record_snapshot(record: u64, capacity: usize) -> Result<(u64, Vec<u8>), i32> {
        if record == ROOT.load(Ordering::SeqCst)
            && crate::client::local_task_status()?.0 != 0
            && READS.fetch_add(1, Ordering::SeqCst) == 2
        {
            let (version, bytes) = crate::client::local_record_snapshot(record, 288)?;
            let mut state = crate::call_state::CallState::call_decode(&bytes)?;
            state.call_set_services(KSV.load(Ordering::SeqCst), NEXT.load(Ordering::SeqCst))?;
            crate::client::local_record_compare_exchange(record, version, &state.call_encode())?;
            FIRED.store(true, Ordering::SeqCst);
        }
        crate::client::local_record_snapshot(record, capacity)
    }
}
fn worker(_: local::runtime::TaskContext, arg: u64) -> u64 {
    tick_service::tick_worker(arg)
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
    for replace in [false, true] {
        let mut bytes = [0u8; 16];
        assert_eq!(gamepkg::local_call(1, &[], &mut bytes).unwrap(), 16);
        let window = u64::from_le_bytes(bytes[..8].try_into().unwrap());
        let request = [
            window.to_le_bytes(),
            0u64.to_le_bytes(),
            65536u64.to_le_bytes(),
        ]
        .concat();
        if !replace {
            gamepkg::local_call(
                2,
                &[request.as_slice(), &2u32.to_le_bytes()].concat(),
                &mut [],
            )
            .unwrap();
        }
        let mut ram = memory::Memory::new().unwrap();
        ram.attach(0x740000, 65536, window, 0, true).unwrap();
        let services = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2).unwrap();
        let data = kernel_data::Data::data_initialize(&mut ram, 0x700000, 123, 0x10000, &[], None)
            .unwrap();
        let (_, source) = gamepkg::local_record_snapshot(services.service_handle(), 64).unwrap();
        let replacement = if replace {
            gamepkg::local_record_create(&source).unwrap()
        } else {
            0
        };
        gamepkg::arm(ram.call_handle(), services.service_handle(), replacement);
        let mut clock = tick_service::Tick::tick_new();
        clock.tick_start(&ram, &data, 0).unwrap();
        let end = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !clock.tick_finished(&ram).unwrap() {
            assert!(std::time::Instant::now() < end);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let joined = clock.tick_join_task(&ram).unwrap();
        assert!(gamepkg::fired());
        assert_eq!(
            joined.result(),
            4,
            "revoked bound KSV must be Denied, never newly-unconfigured Unsupported"
        );
        assert_eq!(
            ram.read_u32(0x740020).unwrap(),
            123,
            "revocation before settings fetch must not publish a tick"
        );
        assert_eq!(clock.tick_close(&ram), Err(-4));
        assert!(clock.tick_closed());
        clock.tick_close(&ram).unwrap();
        if replace {
            ram.services_link(replacement, 0).unwrap();
            gamepkg::local_record_release(replacement).unwrap();
        }
        services.service_close(&ram).unwrap();
        gamepkg::local_record_release(ram.call_handle()).unwrap();
    }
    local::shutdown();
    println!("PASS deterministic actual-worker KSV unlink/replacement between initial root read and settings fetch: Denied, no tick write, actual join and cleanup; maintainer seam only");
}
