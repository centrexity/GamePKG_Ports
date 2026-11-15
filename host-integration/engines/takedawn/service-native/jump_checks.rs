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
