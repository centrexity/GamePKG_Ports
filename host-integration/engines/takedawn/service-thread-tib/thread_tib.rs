// Original per-thread guest TIB/TLS setup. All allocation and bytes use GamePKG.
use crate::memory;
pub fn thread_tib_bytes(template: u32, total: u32) -> Result<u32, i32> {
    // The reviewed image bootstrap rounds TLS payload to 16, then adds slot 0.
    if total > 65536 || total & 15 != 4 || template == 0 {
        return Err(-1);
    }
    template.checked_add(total).ok_or(-1)?;
    Ok(128 + total)
}

pub fn thread_tib_apply(
    ram: &mut memory::Memory,
    tib: u32,
    template: u32,
    total: u32,
) -> Result<(), i32> {
    let extent = thread_tib_bytes(template, total)?;
    tib.checked_add(extent).ok_or(-1)?;
    let block = tib + 64;
    let data = block + total;
    // Original order: inherit current main TIB, current TLS template, clear the
    // per-thread data, then replace own SEH, TLS slot 0 and inherited stack word.
    ram.copy_nonoverlapping(tib, 0x4000, 64)?;
    ram.copy_nonoverlapping(block, template, total)?;
    ram.fill(data, 0, 64)?;
    ram.write_u32(tib, u32::MAX)?;
    ram.write_u32(block, data)?;
    ram.write_u32(tib + 4, data)?;
    Ok(())
}

pub fn thread_tib_new(ram: &mut memory::Memory, template: u32, total: u32) -> Result<u32, i32> {
    ram.call_snapshot()?;
    if total == 0 {
        return Ok(0);
    }
    let size = thread_tib_bytes(template, total)?;
    let mut client = ram.heap_take()?;
    let allocated = client.heap_alloc(size, 16);
    ram.heap_put(client);
    let tib = allocated?;
    if tib != 0 {
        thread_tib_apply(ram, tib, template, total)?;
    }
    Ok(tib)
}
