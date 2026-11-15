// Maintainer-only cross-target tests. Only call_state.rs is game source.
mod gamepkg {
    pub static EVENTS: std::sync::Mutex<Vec<(u32, u32, Vec<u64>)>> =
        std::sync::Mutex::new(Vec::new());
    pub static FAIL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    pub fn diagnostic(level: u32, code: u32, values: &[u64]) -> Result<u64, i32> {
        if FAIL.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(-4);
        }
        let mut events = EVENTS.lock().unwrap();
        events.push((level, code, values.to_vec()));
        Ok(events.len() as u64)
    }
}
mod call_state {
    include!("call_state.rs");
    #[test]
    fn ring_bounds_zero_high_kernel_classification_and_wrapping_counters() {
        let mut value = CallState::call_new(10, 20).unwrap();
        for target in 0..1000 {
            assert_eq!(value.call_record(target), (10..20).contains(&target));
        }
        let mut expected = [0u32; 16];
        for target in 984..1000 {
            expected[(target & 15) as usize] = target;
        }
        assert_eq!(value.call_trace(), expected);
        assert_eq!(value.call_counters(), (1000, 1000));
        value.index = u32::MAX;
        value.total = u64::MAX;
        assert!(value.call_record(0xfe000000));
        assert_eq!(value.call_counters(), (0, 0));
        assert_eq!(value.trace[15], 0xfe000000);
        assert!(value.call_record(u32::MAX));
        assert_eq!(value.call_counters(), (1, 1));
        assert!(!value.call_record(0xfdffffff));
        for low in [0, 1, u32::MAX] {
            let mut v = CallState::call_new(low, 0).unwrap();
            assert!(v.call_record(0));
            assert!(v.call_record(u32::MAX));
        }
        assert!(CallState::call_new(2, 1).is_err());
    }
    #[test]
    fn first_sixteen_offenders_log_only_at_exact_powers_of_ten_and_wrap() {
        let mut value = CallState::call_new(1, 2).unwrap();
        for target in 0..16 {
            value.call_record(target);
            assert_eq!(
                value.call_noncode_values(target),
                Some([target as u64, 1, target as u64 + 1])
            );
        }
        assert_eq!(value.call_noncode_values(16), None);
        for count in [
            0u64,
            1,
            2,
            9,
            10,
            11,
            99,
            100,
            101,
            999,
            1000,
            1001,
            1000000000000000000,
            10000000000000000000,
            u64::MAX,
        ] {
            value.hits[0] = count.wrapping_sub(1);
            let expected = if [
                1u64,
                10,
                100,
                1000,
                1000000000000000000,
                10000000000000000000,
            ]
            .contains(&count)
            {
                Some([0, count, 16])
            } else {
                None
            };
            assert_eq!(value.call_noncode_values(0), expected);
        }
        value.hits[0] = u64::MAX;
        assert_eq!(value.call_noncode_values(0), None);
        assert_eq!(value.call_noncode_values(0), Some([0, 1, 16]));
    }
    #[test]
    fn host_diagnostics_preserve_numeric_records_and_propagate_revocation() {
        gamepkg::EVENTS.lock().unwrap().clear();
        gamepkg::FAIL.store(false, std::sync::atomic::Ordering::SeqCst);
        let mut value = CallState::call_new(1, 2).unwrap();
        assert!(!value.call_record(0));
        assert_eq!(value.call_emit_noncode(0), Ok(Some(1)));
        assert_eq!(value.call_emit_noncode(0), Ok(None));
        assert_eq!(value.call_emit_missing(12), Ok(2));
        let events = gamepkg::EVENTS.lock().unwrap();
        assert_eq!(events[0], (2, 0x544b0001, vec![0, 1, 1]));
        assert_eq!(
            events[1],
            (2, 0x544b0002, value.call_missing_values(12).to_vec())
        );
        drop(events);
        gamepkg::FAIL.store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(value.call_emit_missing(12), Err(-4));
        let mut another = CallState::call_new(1, 2).unwrap();
        another.call_record(0);
        assert_eq!(another.call_emit_noncode(0), Err(-4));
        gamepkg::FAIL.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}
