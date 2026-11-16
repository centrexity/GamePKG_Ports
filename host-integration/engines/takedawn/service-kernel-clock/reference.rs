// Maintainer-only host samples. Actual admitted code below uses the real SDK.
mod gamepkg {
    pub use crate::client::*;
    pub static INPUT: std::sync::Mutex<[u64; 7]> = std::sync::Mutex::new([0, 0, 1, 0, 0, 0, 0]);
    pub static READS: std::sync::Mutex<[u32; 2]> = std::sync::Mutex::new([0, 0]);
    pub static CAS_BUSY: std::sync::Mutex<u32> = std::sync::Mutex::new(0);
    pub static STAGE: std::sync::Mutex<u64> = std::sync::Mutex::new(99);
    pub static ERROR: std::sync::Mutex<i32> = std::sync::Mutex::new(0);
    pub fn local_record_compare_exchange(id: u64, version: u64, bytes: &[u8]) -> Result<u64, i32> {
        let mut remaining = CAS_BUSY.lock().unwrap();
        if *remaining != 0 && bytes.len() == 64 {
            *remaining -= 1;
            let (_, mut original) = crate::client::local_record_snapshot(id, 64)?;
            let skew = u64::from_le_bytes(original[24..32].try_into().unwrap());
            original[24..32].copy_from_slice(&skew.checked_add(1).ok_or(-1)?.to_le_bytes());
            crate::client::local_record_compare_exchange(id, version, &original)?;
            return Err(-2);
        }
        crate::client::local_record_compare_exchange(id, version, bytes)
    }
    pub fn local_performance_sample() -> Result<PerformanceSample, i32> {
        READS.lock().unwrap()[0] += 1;
        let e = *ERROR.lock().unwrap();
        if e != 0 {
            return Err(e);
        }
        let v = *INPUT.lock().unwrap();
        Ok(PerformanceSample {
            counter: v[1],
            frequency: v[2],
            uptime_ms: v[3],
        })
    }
    pub fn clock_sample() -> Result<ClockSample, i32> {
        READS.lock().unwrap()[1] += 1;
        let e = *ERROR.lock().unwrap();
        if e != 0 {
            return Err(e);
        }
        let v = *INPUT.lock().unwrap();
        Ok(ClockSample {
            counter: v[1],
            frequency: v[2],
            uptime_ms: v[3],
            utc_100ns: v[4],
        })
    }
    pub fn diagnostic(_: u32, code: u32, values: &[u64]) -> Result<u64, i32> {
        if (0x544b1403..=0x544b1406).contains(&code) {
            eprintln!("worker result {values:?}");
        }
        if code == 0x544b1402 {
            *STAGE.lock().unwrap() = values[0];
        }
        Ok(1)
    }
}
unsafe extern "C" {
    fn clock_reference_reset(ram: *mut u8, input: *const u64);
    fn clock_reference_call(ordinal: u32, cpu: *mut u32);
}
fn worker(_: local::runtime::TaskContext, arg: u64) -> u64 {
    controls::clock_worker(arg)
}
fn main() {
    let runtime = local::runtime::Runtime::new(
        &[local::runtime::Window {
            address: 0x20000000,
            bytes: 262144,
        }],
        vec![worker],
        4 * 1024 * 1024,
        4,
        65536,
    )
    .unwrap();
    local::install(runtime).unwrap();
    let mut vectors = Vec::new();
    for mode in 0..256u32 {
        let input = cases::clock_case(mode);
        let ordinal = input[0] as u32;
        let (mut ram, services) = probe::clock_memory(mode == 0).unwrap();
        services.service_advance(&ram, input[5]).unwrap();
        probe::clock_arguments(&mut ram, input[6] as u32).unwrap();
        let mut expected = vec![0u8; 262144];
        ram.read_bytes(0, &mut expected).unwrap();
        *gamepkg::INPUT.lock().unwrap() = input;
        *gamepkg::READS.lock().unwrap() = [0, 0];
        unsafe {
            clock_reference_reset(expected.as_mut_ptr(), input.as_ptr());
        }
        let mut state = cases::clock_initial(mode);
        let mut cpu = state.encode();
        let mut registers = [state.eax, state.edx, state.esp];
        unsafe {
            clock_reference_call(ordinal, registers.as_mut_ptr());
        }
        kernel_pool::kernel_invoke(&mut state, &mut ram, ordinal - 125).unwrap();
        cpu[..4].copy_from_slice(&registers[0].to_le_bytes());
        cpu[12..16].copy_from_slice(&registers[1].to_le_bytes());
        cpu[28..32].copy_from_slice(&registers[2].to_le_bytes());
        assert_eq!(state.encode(), cpu, "CPU mode {mode}");
        let mut actual = vec![0u8; 262144];
        ram.read_bytes(0, &mut actual).unwrap();
        assert_eq!(actual, expected, "RAM mode {mode}");
        let expected_reads = if ordinal == 128 {
            if input[6] == 0 {
                [0, 0]
            } else {
                [0, 1]
            }
        } else {
            [1, 0]
        };
        assert_eq!(
            *gamepkg::READS.lock().unwrap(),
            expected_reads,
            "clock selection mode {mode}"
        );
        let result = if ordinal == 128 {
            if input[6] == 0 {
                0
            } else {
                u64::from_le_bytes(
                    expected[input[6] as usize..input[6] as usize + 8]
                        .try_into()
                        .unwrap(),
                )
            }
        } else {
            u64::from(registers[0]) | (u64::from(registers[1]) << 32)
        };
        let base = if ordinal == 125 {
            input[3]
        } else if ordinal == 126 {
            input[1]
        } else if ordinal == 127 {
            input[2]
        } else {
            input[4]
        };
        if ordinal != 128 || input[6] != 0 {
            assert_eq!(
                kernel_pool::kernel_time_value(ordinal, base, input[2], input[5]),
                Ok(result)
            );
        }
        vectors.extend(result.to_le_bytes());
        probe::clock_cleanup(&mut ram, &services).unwrap();
    }
    controls::clock_live_controls(false).unwrap_or_else(|code| {
        panic!(
            "live controls {code}, stage {}",
            *gamepkg::STAGE.lock().unwrap()
        )
    });
    // Transport errors propagate with no register/output changes; null SystemTime
    // does not touch even a failing clock backend.
    let (mut ram, services) = probe::clock_memory(false).unwrap();
    for error in [-1, -2, -3, -4, -5] {
        *gamepkg::ERROR.lock().unwrap() = error;
        for slot in 0..4u32 {
            probe::clock_arguments(&mut ram, 0x11001).unwrap();
            let mut old = [0; 8];
            ram.read_bytes(0x11001, &mut old).unwrap();
            let mut state = cases::clock_initial(slot);
            let before = state.encode();
            assert_eq!(
                kernel_pool::kernel_invoke(&mut state, &mut ram, slot),
                Err(error)
            );
            assert_eq!(state.encode(), before);
            let mut new = [0; 8];
            ram.read_bytes(0x11001, &mut new).unwrap();
            assert_eq!(old, new);
        }
        probe::clock_arguments(&mut ram, 0).unwrap();
        *gamepkg::READS.lock().unwrap() = [0, 0];
        let mut state = cases::clock_initial(0);
        kernel_pool::kernel_invoke(&mut state, &mut ram, 3).unwrap();
        assert_eq!(*gamepkg::READS.lock().unwrap(), [0, 0]);
    }
    *gamepkg::ERROR.lock().unwrap() = 0;
    probe::clock_cleanup(&mut ram, &services).unwrap();
    tsc_read_controls();
    if let Some(path) = std::env::args_os().nth(1) {
        std::fs::write(path, vectors).unwrap();
    }
    println!("PASS: 256 original clock bridges, every 262144 RAM byte, all CPU bytes, exact raw/calendar/no-read selection, propagated clock errors and shared-root/permission controls; no title entry");
}

// Maintainer-only failure/concurrency controls; admitted source uses real calls.
fn tsc_read_controls() {
    let (mut ram, services) = probe::clock_memory(false).unwrap();
    *gamepkg::INPUT.lock().unwrap() = [0, 100, 3, 0, 0, 0, 0];
    let id = services.service_handle();
    let initial = client::local_record_snapshot(id, 64).unwrap();
    for code in [-1, -2, -3, -4, -5] {
        *gamepkg::ERROR.lock().unwrap() = code;
        assert_eq!(kernel_pool::kernel_tsc_read(&ram), Err(code));
        assert_eq!(client::local_record_snapshot(id, 64).unwrap(), initial);
    }
    *gamepkg::ERROR.lock().unwrap() = 0;
    *gamepkg::CAS_BUSY.lock().unwrap() = 64;
    assert_eq!(kernel_pool::kernel_tsc_read(&ram), Err(-2));
    assert_eq!(kernel_pool::kernel_tsc_state(&ram), Ok([0, 0, 0]));
    assert_eq!(*gamepkg::CAS_BUSY.lock().unwrap(), 0);
    *gamepkg::CAS_BUSY.lock().unwrap() = 1;
    assert_eq!(kernel_pool::kernel_tsc_read(&ram), Ok(47));
    assert_eq!(kernel_pool::kernel_service_values(&ram).unwrap()[2], 65);
    assert_eq!(kernel_pool::kernel_tsc_state(&ram), Ok([100, 3, 1]));
    let active = client::local_record_snapshot(id, 64).unwrap();
    for code in [-1, -2, -3, -4, -5] {
        *gamepkg::ERROR.lock().unwrap() = code;
        assert_eq!(kernel_pool::kernel_tsc_read(&ram), Err(code));
        assert_eq!(client::local_record_snapshot(id, 64).unwrap(), active);
    }
    *gamepkg::ERROR.lock().unwrap() = 0;
    gamepkg::INPUT.lock().unwrap()[1] = 99;
    assert_eq!(kernel_pool::kernel_tsc_read(&ram), Err(-3));
    gamepkg::INPUT.lock().unwrap()[1] = 101;
    gamepkg::INPUT.lock().unwrap()[2] = 4;
    assert_eq!(kernel_pool::kernel_tsc_read(&ram), Err(-3));
    assert_eq!(client::local_record_snapshot(id, 64).unwrap(), active);
    gamepkg::INPUT.lock().unwrap()[2] = 3;
    assert_eq!(kernel_pool::kernel_tsc_read(&ram), Ok(244444491));
    services.service_advance(&ram, 1000000000).unwrap();
    assert_eq!(kernel_pool::kernel_tsc_read(&ram), Ok(977777824));
    assert_eq!(kernel_pool::kernel_tsc_state(&ram), Ok([100, 3, 1]));
    probe::clock_cleanup(&mut ram, &services).unwrap();
    // Publishing on the final bounded iteration retains a complete origin,
    // even though that invocation has no budget left for its current sample.
    let (mut ram, services) = probe::clock_memory(false).unwrap();
    *gamepkg::CAS_BUSY.lock().unwrap() = 63;
    assert_eq!(kernel_pool::kernel_tsc_read(&ram), Err(-2));
    assert_eq!(kernel_pool::kernel_tsc_state(&ram), Ok([101, 3, 1]));
    assert_eq!(kernel_pool::kernel_service_values(&ram).unwrap()[2], 63);
    assert_eq!(kernel_pool::kernel_tsc_read(&ram), Ok(46));
    probe::clock_cleanup(&mut ram, &services).unwrap();
}
