use crate::cases;
use crate::gamepkg;
use crate::kernel_pool;
use crate::memory;
use crate::probe;
pub fn clock_arithmetic_controls() -> Result<(), i32> {
    for (ordinal, base, frequency, skew, expected) in [
        (126u32, 0u64, 0u64, 0u64, -1),
        (126, 0, u64::MAX, 0, -1),
        (126, u64::MAX, 1, 0, -1),
        (126, 1, i64::MAX as u64, 1000000000, -1),
        (126, 0, i64::MAX as u64, 2, -1),
        (126, 0, 1000000000, u64::MAX, -1),
        (127, 0, 0, 0, -1),
        (127, u64::MAX, 0, 0, -1),
        (128, i64::MAX as u64, 0, 100, -1),
        (128, u64::MAX, 0, 0, -1),
        (129, u64::MAX, 0, 0, -5),
    ] {
        if kernel_pool::kernel_time_value(ordinal, base, frequency, skew) != Err(expected) {
            return Err(-3);
        }
    }
    if kernel_pool::kernel_time_value(126, 0, i64::MAX as u64, 1)? != 9223372036
        || kernel_pool::kernel_time_value(127, i64::MAX as u64, 0, u64::MAX)? != i64::MAX as u64
        || kernel_pool::kernel_time_value(125, u64::MAX, 0, 0)? != u64::MAX - 9999
    {
        return Err(-3);
    }
    Ok(())
}
pub fn clock_actual_calls(ram: &mut memory::Memory, skew: u64, tag: u32) -> Result<(), i32> {
    for turn in 0..16u32 {
        let ordinal = 125 + turn % 4;
        let pointer = 0x28001 + tag * 16;
        let mut state = cases::clock_initial(turn);
        state.esp += tag * 4096;
        ram.write_u32(state.esp, 0x12345678)?;
        ram.write_u32(state.esp + 4, pointer)?;
        let before_cpu = state.encode();
        // Fixture brackets an actual kernel call with host calendar samples.
        // Production clock routing itself does not do these extra reads.
        let before = gamepkg::clock_sample()?;
        kernel_pool::kernel_invoke(&mut state, ram, ordinal - 125)?;
        let after = gamepkg::clock_sample()?;
        let (observed, low, high) = if ordinal == 128 {
            (
                ram.read_u64(pointer)?,
                kernel_pool::kernel_time_value(128, before.utc_100ns, 0, skew)?,
                kernel_pool::kernel_time_value(128, after.utc_100ns, 0, skew)?,
            )
        } else {
            let low_base = if ordinal == 125 {
                before.uptime_ms
            } else if ordinal == 126 {
                before.counter
            } else {
                before.frequency
            };
            let high_base = if ordinal == 125 {
                after.uptime_ms
            } else if ordinal == 126 {
                after.counter
            } else {
                after.frequency
            };
            (
                u64::from(state.eax) | (u64::from(state.edx) << 32),
                kernel_pool::kernel_time_value(ordinal, low_base, before.frequency, skew)?,
                kernel_pool::kernel_time_value(ordinal, high_base, after.frequency, skew)?,
            )
        };
        if observed < low || observed > high || before.frequency != after.frequency {
            gamepkg::diagnostic(
                2,
                0x544b1404,
                &[u64::from(tag), u64::from(ordinal), observed, low, high],
            )?;
            return Err(-3);
        }
        if probe::clock_check_cpu(&before_cpu, &state, observed, ordinal).is_err() {
            gamepkg::diagnostic(
                2,
                0x544b1405,
                &[u64::from(tag), u64::from(ordinal), u64::from(state.esp)],
            )?;
            return Err(-3);
        }
        gamepkg::diagnostic(1, 0x544b1410 + tag, &[u64::from(ordinal), observed])?;
    }
    Ok(())
}
pub fn clock_worker(context: u64) -> u64 {
    let result = clock_worker_run(context);
    match result {
        Ok(()) => 16u64,
        Err(code) => code as i64 as u64,
    }
}
fn clock_worker_run(context: u64) -> Result<(), i32> {
    let (_, record) = gamepkg::local_record_snapshot(context, 40)?;
    if record.len() != 40 {
        return Err(-1);
    }
    let mut ids = [0u64; 5];
    for index in 0..5usize {
        let at = index * 8;
        ids[index] = u64::from_le_bytes([
            record[at],
            record[at + 1],
            record[at + 2],
            record[at + 3],
            record[at + 4],
            record[at + 5],
            record[at + 6],
            record[at + 7],
        ]);
    }
    if ids[4] == 0 || ids[4] > 4 {
        return Err(-1);
    }
    let mut ram = memory::Memory::with_context([ids[0], ids[1], ids[2], ids[3]])?;
    let mut windows = [0u8; 16];
    if gamepkg::local_call(1, &[], &mut windows)? != 16 {
        return Err(-3);
    }
    let window = u64::from_le_bytes([
        windows[0], windows[1], windows[2], windows[3], windows[4], windows[5], windows[6],
        windows[7],
    ]);
    ram.attach(0, 262144, window, 0, true)?;
    let values = kernel_pool::kernel_service_values(&ram)?;
    if values[2] != 12345678901 {
        gamepkg::diagnostic(2, 0x544b1406, &[values[2], ids[4]])?;
        return Err(-3);
    }
    clock_actual_calls(&mut ram, values[2], ids[4] as u32)
}
fn clock_join(job: u64) -> Result<(), i32> {
    for _ in 0..10000usize {
        let mut bytes = [0u8; 8];
        match gamepkg::local_call(11, &u64::to_le_bytes(job), &mut bytes) {
            Ok(8) => {
                let value = u64::from_le_bytes(bytes);
                if value != 16 {
                    gamepkg::diagnostic(2, 0x544b1403, &[job, value])?;
                }
                return if value == 16 { Ok(()) } else { Err(-3) };
            }
            Err(-2) => memory::Memory::call_retry_pause()?,
            Err(code) => return Err(code),
            _ => return Err(-3),
        }
    }
    Err(-2)
}
pub fn clock_live_controls(commit: bool) -> Result<(), i32> {
    gamepkg::diagnostic(1, 0x544b1402, &[0u64])?;
    clock_arithmetic_controls()?;
    gamepkg::diagnostic(1, 0x544b1402, &[1u64])?;
    let (mut ram, services) = probe::clock_memory(commit)?;
    services.service_advance(&ram, 12345678901)?;
    clock_actual_calls(&mut ram, 12345678901, 0)?;
    gamepkg::diagnostic(1, 0x544b1402, &[2u64])?;
    let context = ram.thread_context();
    let mut records = [0u64; 4];
    let mut jobs = [0u64; 4];
    for index in 0..4usize {
        let mut payload = std::vec::Vec::new();
        for id in context {
            payload.extend(u64::to_le_bytes(id));
        }
        payload.extend(u64::to_le_bytes(index as u64 + 1));
        records[index] = gamepkg::local_record_create(&payload)?;
        let mut request = std::vec::Vec::new();
        request.extend(u32::to_le_bytes(0));
        request.extend(u64::to_le_bytes(records[index]));
        request.extend(u32::to_le_bytes(0));
        let mut out = [0u8; 8];
        if gamepkg::local_call(10, &request, &mut out)? != 8 {
            return Err(-3);
        }
        jobs[index] = u64::from_le_bytes(out);
    }
    for index in 0..4usize {
        clock_join(jobs[index])?;
        gamepkg::local_record_release(records[index])?;
    }
    gamepkg::diagnostic(1, 0x544b1402, &[3u64])?;
    // Denied output is checked as one complete eight-byte store. Both CPU and
    // bytes remain unchanged, including a page-straddling store.
    for pointer in [0x11ffcu32, 0x12000] {
        probe::clock_arguments(&mut ram, pointer)?;
        let mut old = [0u8; 8];
        ram.read_bytes(pointer, &mut old)?;
        let backing = ram.memory_page_backing(0x12000)?;
        let mut request = std::vec::Vec::new();
        request.extend(u64::to_le_bytes(backing[0]));
        request.extend(u64::to_le_bytes(backing[1]));
        request.extend(u64::to_le_bytes(4096));
        request.extend(u32::to_le_bytes(1));
        gamepkg::local_call(3, &request, &mut [])?;
        let mut state = cases::clock_initial(1);
        let cpu = state.encode();
        if kernel_pool::kernel_invoke(&mut state, &mut ram, 3) != Err(-4) || state.encode() != cpu {
            return Err(-3);
        }
        let mut actual = [0u8; 8];
        ram.read_bytes(pointer, &mut actual)?;
        if actual != old {
            return Err(-3);
        }
        request[24..28].copy_from_slice(&u32::to_le_bytes(2));
        gamepkg::local_call(3, &request, &mut [])?;
    }
    for (pointer, expected) in [(0x3fffcu32, -4), (0x40000, -4), (u32::MAX, -1)] {
        probe::clock_arguments(&mut ram, pointer)?;
        let old = ram.read_u32(0x3fffc)?;
        let mut state = cases::clock_initial(5);
        let cpu = state.encode();
        if kernel_pool::kernel_invoke(&mut state, &mut ram, 3) != Err(expected)
            || state.encode() != cpu
            || ram.read_u32(0x3fffc)? != old
        {
            return Err(-3);
        }
    }
    clock_split_output(&ram)?;
    gamepkg::diagnostic(1, 0x544b1402, &[4u64])?;
    services.service_close(&ram)?;
    for slot in [0u32, 1, 3] {
        probe::clock_arguments(&mut ram, 0x11001)?;
        let mut state = cases::clock_initial(2);
        let cpu = state.encode();
        if kernel_pool::kernel_invoke(&mut state, &mut ram, slot) != Err(-5)
            || state.encode() != cpu
        {
            return Err(-3);
        }
    }
    gamepkg::diagnostic(1, 0x544b1402, &[5u64])?;
    // Frequency is independent of skew state. Null SystemTime must remain a
    // successful no-op even after services are removed.
    probe::clock_arguments(&mut ram, 0)?;
    let mut state = cases::clock_initial(3);
    let cpu = state.encode();
    kernel_pool::kernel_invoke(&mut state, &mut ram, 3)?;
    probe::clock_check_cpu(&cpu, &state, 0, 128)?;
    let mut state = cases::clock_initial(4);
    kernel_pool::kernel_invoke(&mut state, &mut ram, 2)?;
    if state.eax == 0 && state.edx == 0 {
        return Err(-3);
    }
    ram.kernel_clear()?;
    for ordinal in [125u32, 126, 128] {
        ram.write_u32(0x10000, 0x80000000 | ordinal)?;
        if kernel_pool::kernel_prepare(&mut ram, 0x10000, 1) != Err(-5)
            || ram.read_u32(0x10000)? != 0x80000000 | ordinal
            || ram.kernel_handle() != 0
        {
            return Err(-3);
        }
    }
    ram.write_u32(0x10000, 0x8000007f)?;
    kernel_pool::kernel_prepare(&mut ram, 0x10000, 1)?;
    let mut state = cases::clock_initial(6);
    kernel_pool::kernel_invoke(&mut state, &mut ram, 0)?;
    if state.eax == 0 && state.edx == 0 {
        return Err(-3);
    }
    gamepkg::diagnostic(1, 0x544b1402, &[6u64])?;
    probe::clock_cleanup(&mut ram, &services)?;
    gamepkg::diagnostic(1, 0x544b1401, &[80u64])?;
    Ok(())
}

// One logical eight-byte output over two physically separated backing ranges.
fn clock_split_output(ram: &memory::Memory) -> Result<(), i32> {
    let backing = ram.memory_page_backing(0)?;
    let mut split = memory::Memory::with_context(ram.thread_context())?;
    split.attach(0, 0x12000, backing[0], 0, true)?;
    split.attach(0x12000, 0x10000, backing[0], 0x20000, true)?;
    probe::clock_arguments(&mut split, 0x11ffc)?;
    let old_gap = ram.read_u64(0x12000)?;
    let before = gamepkg::clock_sample()?;
    let mut state = cases::clock_initial(8);
    let cpu = state.encode();
    kernel_pool::kernel_invoke(&mut state, &mut split, 3)?;
    let after = gamepkg::clock_sample()?;
    let value = split.read_u64(0x11ffc)?;
    let low = kernel_pool::kernel_time_value(128, before.utc_100ns, 0, 12345678901)?;
    let high = kernel_pool::kernel_time_value(128, after.utc_100ns, 0, 12345678901)?;
    if value < low || value > high || ram.read_u64(0x12000)? != old_gap {
        return Err(-3);
    }
    probe::clock_check_cpu(&cpu, &state, value, 128)?;
    let old = split.read_u64(0x11ffc)?;
    let mut request = std::vec::Vec::new();
    request.extend(u64::to_le_bytes(backing[0]));
    request.extend(u64::to_le_bytes(0x20000));
    request.extend(u64::to_le_bytes(4096));
    request.extend(u32::to_le_bytes(1));
    gamepkg::local_call(3, &request, &mut [])?;
    let mut state = cases::clock_initial(9);
    let cpu = state.encode();
    if kernel_pool::kernel_invoke(&mut state, &mut split, 3) != Err(-4)
        || state.encode() != cpu
        || split.read_u64(0x11ffc)? != old
        || ram.read_u64(0x12000)? != old_gap
    {
        return Err(-3);
    }
    request[24..28].copy_from_slice(&u32::to_le_bytes(2));
    gamepkg::local_call(3, &request, &mut [])?;
    Ok(())
}
