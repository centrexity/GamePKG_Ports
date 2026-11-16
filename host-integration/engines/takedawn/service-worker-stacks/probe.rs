use crate::cases;
use crate::memory;
use crate::stacks;
fn stacks_need(ok: bool) -> Result<(), i32> {
    if ok {
        Ok(())
    } else {
        Err(-3)
    }
}
pub fn stacks_vector(ram: &memory::Memory, index: u32) -> Result<std::vec::Vec<u8>, i32> {
    let input = cases::stacks_case(index);
    let mut owner = stacks::Pool::stacks_new(ram, input[0], input[1])?;
    let mut held = [
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None,
    ];
    let mut output = std::vec::Vec::new();
    output.resize(4096, 0u8);
    let mut mask = 0u64;
    for step in 0..128u32 {
        let op = cases::stacks_operation(index, step);
        let mut result = 0u64;
        let mut top = 0u64;
        if op[0] == 0 {
            match owner.stacks_allocate(ram)? {
                Some(lease) => {
                    let slot = lease.stack_slot(ram)?;
                    stacks_need(slot < 16 && held[slot as usize].is_none())?;
                    result = u64::from(slot);
                    top = u64::from(lease.stack_guest_top(ram)?);
                    mask |= 1u64 << slot;
                    held[slot as usize] = Some(lease);
                }
                None => result = u64::MAX,
            }
        } else if let Some(mut lease) = held[op[1] as usize].take() {
            lease.stack_release(ram)?;
            stacks_need(lease.stack_released())?;
            mask &= !(1u64 << op[1]);
        }
        let usage = owner.stacks_usage(ram)?;
        stacks_need(usage[0] == mask.count_ones() && usage[1] == (input[1] / 262144).min(16))?;
        let values = [result, top, mask, u64::from(usage[0])];
        for lane in 0..4usize {
            let value = u64::to_le_bytes(values[lane]);
            for byte in 0..8usize {
                output[step as usize * 32 + lane * 8 + byte] = value[byte];
            }
        }
    }
    for slot in 0..16usize {
        if let Some(mut lease) = held[slot].take() {
            lease.stack_release(ram)?;
        }
    }
    stacks_need(owner.stacks_usage(ram)?[0] == 0)?;
    owner.stacks_close(ram)?;
    stacks_need(owner.stacks_closed())?;
    Ok(output)
}
