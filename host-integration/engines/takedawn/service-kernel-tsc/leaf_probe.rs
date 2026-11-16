use crate::calls_runtime;
use crate::gamepkg;
use crate::kernel_pool;
use crate::memory;
use crate::registers;
pub fn leaf_memory(commit: bool) -> Result<(memory::Memory, kernel_pool::KernelServices), i32> {
    let mut windows = [0u8; 16];
    if gamepkg::local_call(1, &[], &mut windows)? != 16 {
        return Err(-3);
    }
    let window = u64::from_le_bytes([
        windows[0], windows[1], windows[2], windows[3], windows[4], windows[5], windows[6],
        windows[7],
    ]);
    if commit {
        let mut request = std::vec::Vec::new();
        request.extend(u64::to_le_bytes(window));
        request.extend(u64::to_le_bytes(0));
        request.extend(u64::to_le_bytes(262144));
        request.extend(u32::to_le_bytes(2));
        gamepkg::local_call(2, &request, &mut [])?;
    }
    let mut ram = memory::Memory::new()?;
    ram.attach(0, 131072, window, 0, true)?;
    ram.attach(0x400000, 131072, window, 131072, true)?;
    ram.fill(0, 0xa5, 131072)?;
    ram.fill(0x400000, 0xa5, 131072)?;
    let services = kernel_pool::kernel_services_initialize(&ram, 0, 2)?;
    ram.call_bounds(0x10000, 0x400000)?;
    Ok((ram, services))
}
pub fn leaf_state(mode: u32) -> registers::Registers {
    let mut state = registers::Registers::new();
    state.eax = 0x12345678 ^ mode;
    state.ebx = 0x87654321 ^ mode;
    state.ecx = 0xabcdef01;
    state.edx = 0x23456789;
    state.esi = 0x778899aa;
    state.edi = 0x88776655;
    state.ebp = 0x13572468;
    state.esp = 0x18000;
    state.fs_base = 0x8000;
    state.fp_top = (mode & 7) as i32;
    state.fp_stack[1] = f64::from_bits(0x7ff8000012345678);
    state.xmm3 = [mode, 0x80000000, 0xffffffff, 0x7fc12345];
    state
}
pub fn leaf_invoke(
    kind: u32,
    state: &mut registers::Registers,
    ram: &mut memory::Memory,
) -> Result<(), i32> {
    let target = match kind {
        0 => 0x001d20acu32,
        1 => 0x001fc960,
        2 => 0x00356ac0,
        3 => 0x001fc930,
        4 => 0x001fc980,
        _ => return Err(-1),
    };
    let saved_stack = state.esp;
    calls_runtime::invoke_call(state, ram, target, saved_stack)
}
pub fn leaf_snapshot(ram: &memory::Memory) -> Result<std::vec::Vec<u8>, i32> {
    let mut bytes = std::vec::Vec::new();
    bytes.resize(262144, 0u8);
    ram.read_bytes(0, &mut bytes[..131072])?;
    ram.read_bytes(0x400000, &mut bytes[131072..])?;
    Ok(bytes)
}
pub fn leaf_live() -> Result<(), i32> {
    let (mut ram, services) = leaf_memory(true)?;
    services.service_advance(&ram, 12345678901)?;
    for mode in 0..80u32 {
        let kind = mode % 5;
        let pointer = [0x11001u32, 0x11ffc, 0x18004][(mode / 5) as usize % 3];
        ram.write_u32(0x18000, 0x12345678)?;
        ram.write_u32(0x18004, pointer)?;
        let mut state = leaf_state(mode);
        let cpu = state.encode();
        let mut expected = leaf_snapshot(&ram)?;
        let before = gamepkg::performance_sample()?;
        leaf_invoke(kind, &mut state, &mut ram)?;
        let after = gamepkg::performance_sample()?;
        let value = if kind >= 2 {
            u64::from(state.eax) | (u64::from(state.edx) << 32)
        } else {
            ram.read_u64(if kind == 0 { pointer } else { 0x41b960 })?
        };
        let tsc = kernel_pool::kernel_tsc_state(&ram)?;
        if tsc[2] != 1
            || tsc[1] != before.frequency
            || tsc[1] != after.frequency
            || tsc[0] > after.counter
        {
            return Err(-3);
        }
        let low =
            kernel_pool::kernel_tsc_value(before.counter.max(tsc[0]), tsc[0], tsc[1], 12345678901)?;
        let high = kernel_pool::kernel_tsc_value(after.counter, tsc[0], tsc[1], 12345678901)?;
        if value < low || value > high {
            return Err(-3);
        }
        let mut expected_cpu = std::vec::Vec::new();
        expected_cpu.extend(cpu);
        expected_cpu[..4].copy_from_slice(&u32::to_le_bytes(if kind >= 2 {
            value as u32
        } else {
            1
        }));
        expected_cpu[12..16].copy_from_slice(&u32::to_le_bytes((value >> 32) as u32));
        expected_cpu[28..32].copy_from_slice(&u32::to_le_bytes(if kind == 0 {
            0x18008
        } else {
            0x18004
        }));
        if kind == 0 {
            expected_cpu[8..12].copy_from_slice(&u32::to_le_bytes(pointer));
            for (index, byte) in u64::to_le_bytes(value).iter().enumerate() {
                expected[pointer as usize + index] = *byte;
            }
        } else if kind == 1 {
            for (index, byte) in u32::to_le_bytes(state.ebx).iter().enumerate() {
                expected[0x17ffc + index] = *byte;
            }
            for (index, byte) in u64::to_le_bytes(value).iter().enumerate() {
                expected[0x3b960 + index] = *byte;
            }
        }
        if kind >= 3 {
            for (index, byte) in u32::to_le_bytes(state.ebx).iter().enumerate() {
                expected[0x17ff4 + index] = *byte;
            }
            for (index, byte) in u64::to_le_bytes(value).iter().enumerate() {
                expected[0x17ff8 + index] = *byte;
            }
        }
        if state.encode() != expected_cpu || leaf_snapshot(&ram)? != expected {
            return Err(-3);
        }
        gamepkg::diagnostic(1, 0x544b1500, &[u64::from(kind), value, tsc[0], tsc[1]])?;
    }
    // These generated instructions use two 32-bit stores. A denied second
    // store preserves the first write and preceding register effects.
    ram.write_u32(0x18004, 0x11ffc)?;
    ram.write_u32(0x11ffc, 0xa5a5a5a5)?;
    let previous_high = ram.read_u32(0x12000)?;
    let backing = ram.memory_page_backing(0x12000)?;
    let mut request = std::vec::Vec::new();
    request.extend(u64::to_le_bytes(backing[0]));
    request.extend(u64::to_le_bytes(backing[1]));
    request.extend(u64::to_le_bytes(4096));
    request.extend(u32::to_le_bytes(1));
    gamepkg::local_call(3, &request, &mut [])?;
    let mut state = leaf_state(99);
    if leaf_invoke(0, &mut state, &mut ram) != Err(-4)
        || state.esp != 0x18000
        || state.ecx != 0x11ffc
        || ram.read_u32(0x11ffc)? != state.eax
        || ram.read_u32(0x12000)? != previous_high
    {
        return Err(-3);
    }
    request[24..28].copy_from_slice(&u32::to_le_bytes(2));
    gamepkg::local_call(3, &request, &mut [])?;
    services.service_close(&ram)?;
    for kind in 0..5u32 {
        ram.write_u32(0x18000, 0x12345678)?;
        ram.write_u32(0x18004, 0x11ffc)?;
        let mut state = leaf_state(100 + kind);
        let mut expected_cpu = state.encode();
        let mut expected = leaf_snapshot(&ram)?;
        if kind == 0 {
            expected_cpu[8..12].copy_from_slice(&u32::to_le_bytes(0x11ffc));
        }
        if kind == 1 || kind >= 3 {
            let stack = if kind == 1 { 0x17ffcu32 } else { 0x17ff4 };
            expected_cpu[28..32].copy_from_slice(&u32::to_le_bytes(stack));
            for (index, byte) in u32::to_le_bytes(state.ebx).iter().enumerate() {
                expected[stack as usize + index] = *byte;
            }
        }
        if leaf_invoke(kind, &mut state, &mut ram) != Err(-5)
            || state.encode() != expected_cpu
            || leaf_snapshot(&ram)? != expected
        {
            return Err(-3);
        }
    }
    gamepkg::local_record_release(ram.call_handle())?;
    Ok(())
}
