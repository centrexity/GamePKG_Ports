// Original per-thread guest TIB/TLS setup. All allocation and bytes use GamePKG.
use crate::memory;
pub fn thread_tib_new(ram: &mut memory::Memory, template: u32, total: u32) -> Result<u32, i32> {
    ram.call_snapshot()?;
    if total == 0 {
        return Ok(0);
    }
    // The reviewed image bootstrap rounds TLS payload to 16, then adds slot 0.
    if total > 65536 || total & 15 != 4 || template == 0 {
        return Err(-1);
    }
    let size = 64u32
        .checked_add(total)
        .ok_or(-1)?
        .checked_add(64)
        .ok_or(-1)?;
    template.checked_add(total).ok_or(-1)?;
    let mut client = ram.heap_take()?;
    let allocated = client.heap_alloc(size, 16);
    // Preserve the exclusive client even when allocation fails.
    ram.heap_put(client);
    let tib = allocated?;
    if tib == 0 {
        return Ok(0);
    }
    let block = tib.checked_add(64).ok_or(-1)?;
    let data = block.checked_add(total).ok_or(-1)?;
    // Original order: inherit current main TIB, current TLS template, clear the
    // per-thread data, then replace own SEH, TLS slot 0 and inherited stack word.
    // Failures retain their published prefix and allocation; caller must abort.
    ram.copy_nonoverlapping(tib, 0x4000, 64)?;
    ram.copy_nonoverlapping(block, template, total)?;
    ram.fill(data, 0, 64)?;
    ram.write_u32(tib, u32::MAX)?;
    ram.write_u32(block, data)?;
    ram.write_u32(tib + 4, data)?;
    Ok(tib)
}
