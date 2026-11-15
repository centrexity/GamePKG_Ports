// These boundary checks are ordinary source-admitted game Rust, not host mocks.
use crate::gamepkg;
use crate::heap;
use crate::memory;
fn heap_page(window: u64, mode: u32) -> Result<(), i32> {
    let mut request = std::vec::Vec::new();
    request.extend(u64::to_le_bytes(window));
    request.extend(u64::to_le_bytes(0x21000));
    request.extend(u64::to_le_bytes(4096));
    request.extend(u32::to_le_bytes(mode));
    if gamepkg::local_call(3, &request, &mut [])? != 0 {
        return Err(-3);
    }
    Ok(())
}
pub fn heap_denials(ram: &mut memory::Memory, window: u64) -> Result<(), i32> {
    for (base, top) in [
        (0u32, 67108864u32),
        (65535, 67108864),
        (70000, 69999),
        (65536, 67108865),
    ] {
        match heap::Heap::heap_new(ram, base, top) {
            Err(-1) => {}
            _ => return Err(-3),
        }
    }
    let mut state = heap::Heap::heap_new(ram, 0x20000, 0x40000)?;
    let mut other = memory::Memory::new()?;
    if state.heap_allocate(&mut other, 16, 4) != Err(-4)
        || state.heap_free(&other, 0x20000) != Err(-4)
        || state.heap_size(&other, 0x20000) != Err(-4)
        || state.heap_stats() != [0x20000, 0, 0]
    {
        return Err(-3);
    }
    heap_page(window, 1)?;
    if state.heap_allocate(ram, 8192, 4) != Err(-4)
        || state.heap_stats() != [0x20000, 0, 0]
        || state.heap_record(0) != Err(-1)
    {
        return Err(-3);
    }
    let mut bytes = std::vec::Vec::new();
    bytes.resize(8192, 0u8);
    ram.read_bytes(0x20000, &mut bytes)?;
    for byte in &bytes {
        if *byte != 0xa5 {
            return Err(-3);
        }
    }
    heap_page(window, 2)?;
    if state.heap_allocate(ram, 8192, 4) != Ok(0x20000) {
        return Err(-3);
    }
    state.heap_free(ram, 0x20000)?;
    ram.fill(0x20000, 0xa5, 8192)?;
    heap_page(window, 1)?;
    if state.heap_allocate(ram, 8192, 4) != Err(-4)
        || state.heap_stats() != [0x22000, 1, 1]
        || state.heap_record(0) != Ok([0x20000, 8192, 1])
    {
        return Err(-3);
    }
    ram.read_bytes(0x20000, &mut bytes)?;
    for byte in &bytes {
        if *byte != 0xa5 {
            return Err(-3);
        }
    }
    heap_page(window, 2)?;
    let mut closed = memory::Memory::new()?;
    closed.attach(0, 67108864, window, 0, true)?;
    let mut stale = heap::Heap::heap_new(&closed, 0x20000, 0x40000)?;
    gamepkg::local_record_release(closed.call_handle())?;
    if stale.heap_allocate(&mut closed, 16, 4) != Err(-4)
        || stale.heap_free(&closed, 0x20000) != Err(-4)
        || stale.heap_size(&closed, 0x20000) != Err(-4)
        || stale.heap_stats() != [0x20000, 0, 0]
    {
        return Err(-3);
    }
    match heap::Heap::heap_new(&closed, 0x20000, 0x40000) {
        Err(-4) => {}
        _ => return Err(-3),
    }
    Ok(())
}
