// Asset-free game-side checks; also run after normal source/object admission.
use crate::call_state;
pub fn call_verify() -> Result<(), i32> {
    let mut ledger = call_state::CallState::call_new(0x10000, 0x20000)?;
    for target in [
        0u32, 0xffff, 0x10000, 0x1ffff, 0x20000, 0xfdffffff, 0xfe000000, 0xffffffff,
    ] {
        let expected = target >= 0xfe000000 || (target >= 0x10000 && target < 0x20000);
        if ledger.call_record(target) != expected {
            return Err(-3);
        }
    }
    for number in 1..=10000u64 {
        ledger.call_record(0);
        let expected =
            if number == 1 || number == 10 || number == 100 || number == 1000 || number == 10000 {
                Some([0u64, number, number + 8])
            } else {
                None
            };
        if ledger.call_noncode_values(0) != expected {
            return Err(-3);
        }
    }
    for target in 1..16u32 {
        ledger.call_record(target);
        if ledger.call_noncode_values(target)
            != Some([u64::from(target), 1, 10008 + u64::from(target)])
        {
            return Err(-3);
        }
    }
    if ledger.call_noncode_values(16) != None || ledger.call_noncode_values(0) != None {
        return Err(-3);
    }
    let counters = ledger.call_counters();
    if counters != (10023, 10023) {
        return Err(-3);
    }
    let words = ledger.call_missing_values(0x12345678);
    if words[0] != 0x12345678 || words[1] != 10023 || words[2] != 10023 {
        return Err(-3);
    }
    let trace = ledger.call_trace();
    for index in 0..16 {
        if words[index + 3] != u64::from(trace[index]) {
            return Err(-3);
        }
    }
    let mut unbounded = call_state::CallState::call_new(0x10000, 0)?;
    if !unbounded.call_record(0) || !unbounded.call_record(0xffffffff) {
        return Err(-3);
    }
    if call_state::CallState::call_new(2, 1).is_ok() {
        return Err(-3);
    }
    // A successful diagnostic goes through GamePKG's private service, never
    // through game-owned stdout/stderr, files or a platform logger.
    ledger.call_emit_missing(0x12345678)?;
    let mut first = call_state::CallState::call_new(1, 2)?;
    first.call_record(0);
    if first.call_emit_noncode(0)?.is_none() || first.call_emit_noncode(0)?.is_some() {
        return Err(-3);
    }
    Ok(())
}

pub fn call_memory_verify() -> Result<(), i32> {
    let root = crate::memory::Memory::new()?;
    let view = root.call_view();
    root.call_bounds(1, 2)?;
    if !view.call_begin(1)? {
        return Err(-3);
    }
    let worker = crate::memory::Memory::with_call_handle(root.call_handle())?;
    if !worker.call_begin(0xfe000000)? {
        return Err(-3);
    }
    let (trace, index, total) = root.call_snapshot()?;
    if trace[0] != 1 || trace[1] != 0xfe000000 || index != 2 || total != 2 {
        return Err(-3);
    }
    crate::gamepkg::local_record_release(root.call_handle())?;
    if root.call_snapshot() != Err(-4)
        || view.call_snapshot() != Err(-4)
        || worker.call_snapshot() != Err(-4)
    {
        return Err(-3);
    }
    Ok(())
}
