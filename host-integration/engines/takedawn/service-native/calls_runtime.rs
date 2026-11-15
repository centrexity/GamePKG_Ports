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
    // Only generated functions currently have a checked direct invoker. A
    // manual/kernel/entrypoint target remains an explicit required migration
    // fault, never an "unknown original address" or a successful no-op body.
    if tag >> 24 != 1 {
        gamepkg::diagnostic(3, 0x544b0003, &[u64::from(target), u64::from(tag)])?;
        return Err(-5);
    }
    crate::dispatch::invoke(state, ram, target)
}
