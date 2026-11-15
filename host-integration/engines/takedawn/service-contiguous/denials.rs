use crate::contiguous;
use crate::gamepkg;
use crate::memory;
fn contiguous_page_mode(window: u64, op: u32, mode: u32) -> Result<(), i32> {
    let mut request = std::vec::Vec::new();
    request.extend(u64::to_le_bytes(window));
    request.extend(u64::to_le_bytes(8192));
    request.extend(u64::to_le_bytes(4096));
    if op != 4 {
        request.extend(u32::to_le_bytes(mode));
    }
    gamepkg::local_call(op, &request, &mut [])?;
    Ok(())
}
pub fn contiguous_denials(ram: &memory::Memory, window: u64) -> Result<(), i32> {
    for wrong in [
        0u64,
        window.wrapping_sub(1),
        window.wrapping_add(1),
        0x24000000,
    ] {
        match contiguous::Contiguous::contiguous_new(ram, wrong) {
            Err(-4) => {}
            _ => return Err(-3),
        }
    }
    let mut state = contiguous::Contiguous::contiguous_new(ram, window)?;
    let original = state.contiguous_stats();
    let other = memory::Memory::new()?;
    if state.contiguous_allocate(&other, 8192, 4096, 0, u32::MAX) != Err(-4)
        || state.contiguous_claim(&other, 0x80001000, 8192) != Err(-4)
        || state.contiguous_free(&other, 0x80001000) != Err(-4)
        || state.contiguous_size(&other, 0x80001000) != Err(-4)
        || state.contiguous_stats() != original
    {
        return Err(-3);
    }
    gamepkg::local_record_release(other.call_handle())?;
    gamepkg::local_memory_fill(window, 4096, 8192, 0x7b)?;
    for op in [3u32, 4] {
        contiguous_page_mode(window, op, 1)?;
        if state.contiguous_allocate(ram, 8192, 4096, 0, u32::MAX) != Err(-4)
            || state.contiguous_stats() != original
            || state.contiguous_record(1)? != [0, 0]
            || state.contiguous_record(2)? != [0, 0]
        {
            return Err(-3);
        }
        let mut unchanged = [0u8; 16];
        gamepkg::local_memory_read_segments(&[[window, 4096, 16]], &mut unchanged)?;
        if unchanged != [0x7b; 16] {
            return Err(-3);
        }
        contiguous_page_mode(window, if op == 4 { 2 } else { 3 }, 2)?;
    }
    let allocated = state.contiguous_allocate(ram, 8192, 4096, 0, u32::MAX)?;
    if allocated != 0x80001000 || !state.contiguous_free(ram, allocated)? {
        return Err(-3);
    }
    let reused = state.contiguous_stats();
    gamepkg::local_memory_fill(window, 4096, 8192, 0x7b)?;
    contiguous_page_mode(window, 3, 1)?;
    if state.contiguous_allocate(ram, 8192, 4096, 0, u32::MAX) != Err(-4)
        || state.contiguous_stats() != reused
        || state.contiguous_record(1)? != [0, 0]
        || state.contiguous_record(2)? != [0, 0]
    {
        return Err(-3);
    }
    let mut first_page = [0u8; 16];
    gamepkg::local_memory_read_segments(&[[window, 4096, 16]], &mut first_page)?;
    if first_page != [0x7b; 16] {
        return Err(-3);
    }
    contiguous_page_mode(window, 3, 2)?;
    let mut unchanged = [0x5au8; 16];
    if state.contiguous_record(16384) != Err(-1)
        || state.contiguous_pages(16380, &mut unchanged) != Err(-1)
        || unchanged != [0x5a; 16]
        || state.contiguous_pages(u32::MAX, &mut unchanged) != Err(-1)
        || unchanged != [0x5a; 16]
    {
        return Err(-3);
    }
    state.contiguous_pages(16384, &mut [])?;
    let mut oversized = std::vec::Vec::new();
    oversized.resize(4097, 0x5au8);
    if state.contiguous_pages(0, &mut oversized) != Err(-1) {
        return Err(-3);
    }
    for byte in &oversized {
        if *byte != 0x5a {
            return Err(-3);
        }
    }
    let revoked = memory::Memory::new()?;
    let mut stale = contiguous::Contiguous::contiguous_new(&revoked, window)?;
    gamepkg::local_record_release(revoked.call_handle())?;
    if stale.contiguous_allocate(&revoked, 4096, 4096, 0, u32::MAX) != Err(-4)
        || stale.contiguous_claim(&revoked, 0x80001000, 4096) != Err(-4)
        || stale.contiguous_free(&revoked, 0x80001000) != Err(-4)
        || stale.contiguous_size(&revoked, 0x80001000) != Err(-4)
        || stale.contiguous_stats() != original
    {
        return Err(-3);
    }
    match contiguous::Contiguous::contiguous_new(&revoked, window) {
        Err(-4) => {}
        _ => return Err(-3),
    }
    Ok(())
}
