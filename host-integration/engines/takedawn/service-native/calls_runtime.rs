// Finite direct Rust calls for the exact original dispatch membership. There
// are no callable native pointers and no instruction fetch/decode loop.
use crate::gamepkg;
use crate::memory;
use crate::registers;
use crate::targets;
pub fn invoke_call(
    state: &mut registers::Registers,
    ram: &mut memory::Memory,
    target: u32,
    saved_stack: u32,
) -> Result<(), i32> {
    if !ram.call_begin(target)? {
        state.esp = saved_stack;
        state.eax = 0;
        return Ok(());
    }
    let tag = targets::original_target(target);
    if tag == 0 {
        ram.call_absent(target)?;
        state.esp = saved_stack;
        state.eax = 0;
        return Ok(());
    }
    invoke_registered(state, ram, target, tag)
}
// The reviewed computed-jump path has neither a trace increment nor a code-
// range guard. Original table absence logs and pops one guest stack word.
// The private ledger still has to be live before any callee/fallback runs.
pub fn invoke_jump(
    state: &mut registers::Registers,
    ram: &mut memory::Memory,
    target: u32,
) -> Result<(), i32> {
    ram.call_snapshot()?;
    let tag = targets::original_target(target);
    if tag == 0 {
        ram.call_absent(target)?;
        state.esp = state.esp.wrapping_add(4);
        state.eax = 0;
        return Ok(());
    }
    invoke_registered(state, ram, target, tag)
}
fn invoke_registered(
    state: &mut registers::Registers,
    ram: &mut memory::Memory,
    target: u32,
    tag: u32,
) -> Result<(), i32> {
    if tag >> 24 == 5 && ram.kernel_handle() != 0 {
        return crate::kernel_pool::kernel_invoke(state, ram, tag & 0x00ffffff);
    }
    // Generated functions and the explicitly configured pool kernel slots
    // have reviewed direct invokers. Other routes remain migration faults.
    // Other manual/kernel/entrypoint targets remain explicit required
    // migration faults, never successful absence/no-op bodies.
    if tag >> 24 != 1 {
        gamepkg::diagnostic(3, 0x544b0003, &[u64::from(target), u64::from(tag)])?;
        return Err(-5);
    }
    crate::dispatch::invoke(state, ram, target)
}
