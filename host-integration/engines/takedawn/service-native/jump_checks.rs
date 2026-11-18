// Source-checked component fixture, not a game entrypoint or native exemption.
use crate::calls_runtime;
use crate::gamepkg;
use crate::memory;
use crate::registers;
pub fn jump_verify() -> Result<(), i32> {
    let mut ram = memory::Memory::new()?;
    ram.call_bounds(0x20000, 0x30000)?;
    let mut state = registers::Registers::new();
    let initial_trace = ram.call_snapshot()?;
    // This real generated function is outside the trace path's code bounds.
    // Untraced lookup must still invoke it, without changing trace or totals.
    for stack in [0u32, 1, 0xfffffffc, 0xfffffffe, 0xffffffff] {
        state.eax = 0x12345678;
        state.esp = stack;
        calls_runtime::invoke_jump(&mut state, &mut ram, 0x15590)?;
        if state.eax != 0xffffffff
            || state.esp != stack.wrapping_add(4)
            || ram.call_snapshot()? != initial_trace
        {
            return Err(-3);
        }
    }
    state.eax = 0xabcdef01;
    state.esp = 0xfffffffe;
    calls_runtime::invoke_jump(&mut state, &mut ram, 0x8000)?;
    if state.eax != 0 || state.esp != 2 || ram.call_snapshot()? != initial_trace {
        return Err(-3);
    }
    let before = state.encode();
    gamepkg::local_record_release(ram.call_handle())?;
    for target in [0x15590u32, 0x8000, 0xfe000000] {
        if calls_runtime::invoke_jump(&mut state, &mut ram, target) != Err(-4)
            || state.encode() != before
        {
            return Err(-3);
        }
    }
    Ok(())
}

pub fn jump_sha_verify(window: u64) -> Result<(), i32> {
    let mut ram = memory::Memory::new()?;
    ram.attach(0, 67108864, window, 0, true)?;
    let table = 0x36b7c0;
    for (slot, ordinal) in [(0u32, 336u32), (1, 335), (2, 337)] {
        ram.write_u32(table + slot * 4, 0x80000000 | ordinal)?;
    }
    crate::kernel_pool::kernel_prepare(&mut ram, table, 3)?;
    let trace = ram.call_snapshot()?;
    let stack = 0x20000;
    let sha_state = 0x20101;
    let payload = 0x20201;
    let digest_at = 0x20301;
    ram.write_bytes(payload, b"abc")?;
    let mut state = registers::Registers::new();
    for (routine, argument_bytes, args) in [
        (0x1d7018u32, 4u32, [sha_state, 0, 0]),
        (0x1d7012, 12, [sha_state, payload, 3]),
        (0x1d701e, 8, [sha_state, digest_at, 0]),
    ] {
        ram.write_u32(stack, 0x12345678)?;
        for slot in 0..3usize {
            ram.write_u32(stack + 4 + slot as u32 * 4, args[slot])?;
        }
        state.esp = stack;
        state.eax = 0xdeadbeef;
        crate::dispatch::invoke(&mut state, &mut ram, routine)?;
        if state.eax != 0
            || state.esp != stack + 4 + argument_bytes
            || ram.call_snapshot()? != trace
        {
            return Err(-3);
        }
    }
    let mut digest = [0u8; 20];
    ram.read_bytes(digest_at, &mut digest)?;
    if digest
        != [
            0xa9, 0x99, 0x3e, 0x36, 0x47, 0x06, 0x81, 0x6a, 0xba, 0x3e, 0x25, 0x71, 0x78, 0x50,
            0xc2, 0x6c, 0x9c, 0xd0, 0xd8, 0x9d,
        ]
    {
        return Err(-3);
    }
    ram.kernel_clear()?;
    gamepkg::local_record_release(ram.call_handle())?;
    Ok(())
}
