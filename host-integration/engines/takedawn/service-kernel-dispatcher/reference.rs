// Maintainer-only diagnostic collector; absent from admitted source and bindings.
mod gamepkg {
    pub use crate::client::*;
    pub static LOGS: std::sync::Mutex<Vec<(u32, u32, Vec<u64>)>> =
        std::sync::Mutex::new(Vec::new());
    pub static AFTER_WRITE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    pub static CAS_BUSY: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    pub static CAS_ERROR: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
    pub fn local_call(op: u32, request: &[u8], reply: &mut [u8]) -> Result<usize, i32> {
        let result = crate::client::local_call(op, request, reply)?;
        if op == 6 && request.len() >= 16 {
            let offset = u64::from_le_bytes(request[8..16].try_into().unwrap());
            if (0x11000..0x1102c).contains(&offset) {
                let step = AFTER_WRITE.fetch_update(
                    std::sync::atomic::Ordering::AcqRel,
                    std::sync::atomic::Ordering::Acquire,
                    |v| v.checked_sub(1),
                );
                if step == Ok(1) {
                    let mut protect = request[..8].to_vec();
                    protect.extend(0x11000u64.to_le_bytes());
                    protect.extend(4096u64.to_le_bytes());
                    protect.extend(1u32.to_le_bytes());
                    crate::client::local_call(3, &protect, &mut [])?;
                }
            }
        }
        Ok(result)
    }
    pub fn local_record_compare_exchange(id: u64, version: u64, bytes: &[u8]) -> Result<u64, i32> {
        if bytes.len() == 64 && bytes[..4] == 0x3356534bu32.to_le_bytes() {
            let (_, mut prior) = crate::client::local_record_snapshot(id, 64)?;
            if bytes[4..8] != prior[4..8] {
                let error = CAS_ERROR.load(std::sync::atomic::Ordering::Acquire);
                if error != 0 {
                    return Err(error);
                }
                if CAS_BUSY
                    .fetch_update(
                        std::sync::atomic::Ordering::AcqRel,
                        std::sync::atomic::Ordering::Acquire,
                        |v| v.checked_sub(1),
                    )
                    .is_ok()
                {
                    let skew = u64::from_le_bytes(prior[24..32].try_into().unwrap());
                    prior[24..32].copy_from_slice(&skew.checked_add(1).unwrap().to_le_bytes());
                    crate::client::local_record_compare_exchange(id, version, &prior)?;
                    return Err(-2);
                }
            }
        }
        crate::client::local_record_compare_exchange(id, version, bytes)
    }
    pub fn diagnostic(level: u32, code: u32, values: &[u64]) -> Result<u64, i32> {
        LOGS.lock().unwrap().push((level, code, values.to_vec()));
        Ok(1)
    }
}
unsafe extern "C" {
    fn dispatcher_reference(
        input: *const u8,
        output: *mut u8,
        cpu: *mut u8,
        ordinal: u32,
        diagnostics: *mut u32,
    );
    fn dispatcher_buffer_reference(va: u32, bytes: u32) -> i32;
}
fn worker(_: local::runtime::TaskContext, arg: u64) -> u64 {
    controls::dispatcher_worker(arg)
}
fn ticker(_: local::runtime::TaskContext, arg: u64) -> u64 {
    tick_service::tick_worker(arg)
}
fn main() {
    local::install(
        local::runtime::Runtime::new(
            &[local::runtime::Window {
                address: 0x20000000,
                bytes: 262144,
            }],
            vec![worker, ticker],
            4 * 1024 * 1024,
            4,
            65536,
        )
        .unwrap(),
    )
    .unwrap();
    let (mut ram, services, window) = probe::dispatcher_memory(true).unwrap();
    let mut vectors = Vec::new();
    for mode in 0..768u32 {
        let values = cases::dispatcher_case(mode);
        let mut state = probe::dispatcher_reset(&mut ram, window, mode).unwrap();
        let before = probe::dispatcher_snapshot(window).unwrap();
        let mut expected = vec![0; 262144];
        let mut cpu = state.encode();
        let mut logs = [0u32; 5];
        gamepkg::LOGS.lock().unwrap().clear();
        unsafe {
            dispatcher_reference(
                before.as_ptr(),
                expected.as_mut_ptr(),
                cpu.as_mut_ptr(),
                values[0],
                logs.as_mut_ptr(),
            );
        }
        kernel_pool::kernel_invoke(
            &mut state,
            &mut ram,
            probe::dispatcher_slot(values[0]).unwrap(),
        )
        .unwrap();
        assert_eq!(state.encode(), cpu, "CPU {mode}");
        let actual = probe::dispatcher_snapshot(window).unwrap();
        assert_eq!(actual, expected, "RAM {mode}");
        let (_, record) = client::local_record_snapshot(services.service_handle(), 64).unwrap();
        let flags = u32::from_le_bytes(record[4..8].try_into().unwrap());
        assert_eq!(flags, logs[4], "seen flags {mode}");
        let events = gamepkg::LOGS.lock().unwrap();
        if logs[0] == 0 {
            assert!(events.is_empty(), "diagnostics {mode}: {events:?}");
        } else {
            assert_eq!(logs[0], 1);
            assert_eq!(
                *events,
                vec![(
                    2,
                    0x544b1700,
                    vec![u64::from(logs[1]), u64::from(logs[2]), u64::from(logs[3])]
                )]
            );
        }
        let mut hash = sha256::Sha256::sha_new();
        hash.sha_update(&expected).unwrap();
        vectors.extend(cpu);
        vectors.extend(hash.sha_finish());
        vectors.extend(logs[4].to_le_bytes());
    }
    for va in [
        0u32,
        1,
        4095,
        4096,
        0x3ffffff,
        0x4000000,
        0x80000000,
        0x83ffffff,
        0x84000000,
        0xf0000000,
        u32::MAX,
    ] {
        for length in [0u32, 1, 32, 44, 4096, u32::MAX] {
            assert_eq!(
                kernel_pool::kernel_buffer_mapped(va, length),
                unsafe { dispatcher_buffer_reference(va, length) } != 0
            );
        }
    }
    probe::dispatcher_cleanup(&mut ram, &services).unwrap();
    controls::dispatcher_controls().unwrap();
    reference_prefix_controls();
    reference_cas_controls();
    local::shutdown();
    if let Some(path) = std::env::args_os().nth(1) {
        std::fs::write(path, &vectors).unwrap();
    }
    println!("PASS: 768 exact original HAL-vector/DPC/interrupt initialization calls, every CPU and 262144 RAM byte, aliases and argument/output overlap, range-boundary refusal, once-per-root diagnostics across map reinstalls, fixed-width buffer geometry; no actual IRQ/DPC invocation");
}

fn reference_protect(window: u64, mode: u32) {
    let request = [
        window.to_le_bytes().as_slice(),
        &0x11000u64.to_le_bytes(),
        &4096u64.to_le_bytes(),
        &mode.to_le_bytes(),
    ]
    .concat();
    client::local_call(3, &request, &mut []).unwrap();
}
fn reference_prefix_controls() {
    let (mut ram, services, window) = probe::dispatcher_memory(false).unwrap();
    for ordinal in [107u32, 109] {
        for step in 1..=4u32 {
            let mut state =
                probe::dispatcher_reset(&mut ram, window, if ordinal == 107 { 13 } else { 14 })
                    .unwrap();
            ram.write_u32(0x18004, 0x11000).unwrap();
            let cpu = state.encode();
            let mut expected = probe::dispatcher_snapshot(window).unwrap();
            let values = cases::dispatcher_case(if ordinal == 107 { 13 } else { 14 });
            let size = if ordinal == 107 { 32 } else { 44 };
            expected[0x11000..0x11000 + size].fill(0);
            if ordinal == 107 {
                if step >= 2 {
                    expected[0x11000..0x11002].copy_from_slice(&0x13u16.to_le_bytes());
                }
                if step >= 3 {
                    expected[0x1100c..0x11010].copy_from_slice(&values[2].to_le_bytes());
                }
                if step >= 4 {
                    expected[0x11010..0x11014].copy_from_slice(&values[3].to_le_bytes());
                }
            } else {
                if step >= 2 {
                    expected[0x11000..0x11004].copy_from_slice(&values[2].to_le_bytes());
                }
                if step >= 3 {
                    expected[0x11004..0x11008].copy_from_slice(&values[3].to_le_bytes());
                }
                if step >= 4 {
                    expected[0x11008..0x1100c].copy_from_slice(&values[4].to_le_bytes());
                }
            }
            gamepkg::AFTER_WRITE.store(step, std::sync::atomic::Ordering::Release);
            let result = kernel_pool::kernel_invoke(
                &mut state,
                &mut ram,
                probe::dispatcher_slot(ordinal).unwrap(),
            );
            if step < 4 {
                assert_eq!(result, Err(-4));
                assert_eq!(state.encode(), cpu);
            } else {
                assert_eq!(result, Ok(()));
            }
            assert_eq!(
                probe::dispatcher_snapshot(window).unwrap(),
                expected,
                "write prefix {ordinal}/{step}"
            );
            reference_protect(window, 2);
        }
    }
    probe::dispatcher_cleanup(&mut ram, &services).unwrap();
}
fn reference_cas_controls() {
    for attempts in [0u32, 63, 64] {
        let (mut ram, services, window) = probe::dispatcher_memory(false).unwrap();
        let mut state = probe::dispatcher_reset(&mut ram, window, 1).unwrap();
        let _ = kernel_pool::kernel_tsc_read(&ram).unwrap();
        let tsc = kernel_pool::kernel_tsc_state(&ram).unwrap();
        let cpu = state.encode();
        let bytes = probe::dispatcher_snapshot(window).unwrap();
        if attempts == 0 {
            for error in [-3i32, -4, -5] {
                let metadata =
                    client::local_record_snapshot(services.service_handle(), 64).unwrap();
                gamepkg::LOGS.lock().unwrap().clear();
                gamepkg::CAS_ERROR.store(error, std::sync::atomic::Ordering::Release);
                assert_eq!(
                    kernel_pool::kernel_invoke(&mut state, &mut ram, 1),
                    Err(error)
                );
                assert_eq!(state.encode(), cpu);
                assert_eq!(probe::dispatcher_snapshot(window).unwrap(), bytes);
                assert!(gamepkg::LOGS.lock().unwrap().is_empty());
                assert_eq!(
                    client::local_record_snapshot(services.service_handle(), 64).unwrap(),
                    metadata
                );
            }
            gamepkg::CAS_ERROR.store(0, std::sync::atomic::Ordering::Release);
        }
        gamepkg::CAS_BUSY.store(attempts, std::sync::atomic::Ordering::Release);
        gamepkg::LOGS.lock().unwrap().clear();
        let result = kernel_pool::kernel_invoke(&mut state, &mut ram, 1);
        let (_, record) = client::local_record_snapshot(services.service_handle(), 64).unwrap();
        assert_eq!(kernel_pool::kernel_tsc_state(&ram).unwrap(), tsc);
        assert_eq!(
            kernel_pool::kernel_service_values(&ram).unwrap()[2],
            u64::from(attempts)
        );
        if attempts == 64 {
            assert_eq!(result, Err(-2));
            assert_eq!(state.encode(), cpu);
            assert_eq!(&record[4..8], &0u32.to_le_bytes());
            assert!(gamepkg::LOGS.lock().unwrap().is_empty());
            assert_eq!(kernel_pool::kernel_invoke(&mut state, &mut ram, 1), Ok(()));
        } else {
            assert_eq!(result, Ok(()));
            assert_eq!(&record[4..8], &1u32.to_le_bytes());
        }
        assert_eq!(gamepkg::LOGS.lock().unwrap().len(), 1);
        probe::dispatcher_cleanup(&mut ram, &services).unwrap();
    }
    println!("PASS: maintainer-injected later-write denial preserves exact clear/type/routine/context prefixes and CPU errors; Failed/Denied/Unsupported metadata errors, 63/64 bounded CAS boundaries, retained competing skew and first-use TSC fields; no injection in admitted source");
}
