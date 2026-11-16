// Ordinary checked adversarial controls, not an import/admission exception.
use crate::gamepkg;
use crate::imports;
use crate::kernel_data;
use crate::kernel_pool;
use crate::memory;
use crate::probe;
fn imports_denied(result: Result<imports::Imports, i32>, expected: i32) -> Result<(), i32> {
    match result {
        Err(code) => {
            if code == expected {
                Ok(())
            } else {
                Err(-3)
            }
        }
        Ok(_) => Err(-3),
    }
}
fn imports_mode(window: u64, offset: u64, operation: u32, mode: u32) -> Result<(), i32> {
    let mut req = std::vec::Vec::new();
    req.extend(u64::to_le_bytes(window));
    req.extend(u64::to_le_bytes(offset));
    req.extend(u64::to_le_bytes(4096));
    if operation != 4 {
        req.extend(u32::to_le_bytes(mode));
    }
    gamepkg::local_call(operation, &req, &mut [])?;
    Ok(())
}
fn imports_seed(ram: &mut memory::Memory, table: u32, ordinal: u32) -> Result<(), i32> {
    ram.fill(table - 64, 0, 64 + 379 * 4)?;
    ram.write_u32(table, 0x80000000 | ordinal)
}
fn imports_fail(
    ram: &memory::Memory,
    data: &kernel_data::Data,
    table: u32,
    remap: &[u16],
    error: i32,
) -> Result<(), i32> {
    let mut before = std::vec::Vec::new();
    before.resize(64 + 379 * 4, 0u8);
    ram.read_bytes(table - 64, &mut before)?;
    imports_denied(
        imports::Imports::imports_scan(ram, table, data, remap),
        error,
    )?;
    let mut after = std::vec::Vec::new();
    after.resize(before.len(), 0u8);
    ram.read_bytes(table - 64, &mut after)?;
    if before != after || ram.kernel_handle() != 0 {
        return Err(-3);
    }
    Ok(())
}
fn imports_corrupt(ram: &memory::Memory) -> Result<(), i32> {
    let id = ram.kernel_handle();
    for (at, value) in [
        (0usize, 0u32),
        (0, 0x324c504b),
        (4, 0),
        (4, u32::MAX),
        (20, 0),
        (20, u32::MAX),
        (20, 2),
        (28, 9),
    ] {
        let (version, original) = gamepkg::local_record_snapshot(id, 1540)?;
        let mut changed = std::vec::Vec::new();
        changed.extend(&original);
        let encoded_word = u32::to_le_bytes(value);
        for index in 0..4usize {
            changed[at + index] = encoded_word[index];
        }
        let next = gamepkg::local_record_compare_exchange(id, version, &changed)?;
        if kernel_pool::kernel_ordinal(ram, 0) != Err(-1) {
            return Err(-3);
        }
        gamepkg::local_record_compare_exchange(id, next, &original)?;
    }
    if kernel_pool::kernel_ordinal(ram, 0)? != 14 {
        return Err(-3);
    }
    Ok(())
}
fn imports_gap_corrupt(ram: &memory::Memory) -> Result<(), i32> {
    let id = ram.kernel_handle();
    for (at, value) in [(28usize, 14u32), (32, 0), (32, 16), (20, 2)] {
        let (version, original) = gamepkg::local_record_snapshot(id, 1540)?;
        let mut changed = std::vec::Vec::new();
        changed.extend(&original);
        let encoded_word = u32::to_le_bytes(value);
        for index in 0..4usize {
            changed[at + index] = encoded_word[index];
        }
        let next = gamepkg::local_record_compare_exchange(id, version, &changed)?;
        if kernel_pool::kernel_ordinal(ram, 0) != Err(-1) {
            return Err(-3);
        }
        gamepkg::local_record_compare_exchange(id, next, &original)?;
    }
    Ok(())
}
pub fn imports_controls() -> Result<(), i32> {
    let (mut ram, data, job, main, physical) = probe::imports_environment(true)?;
    let table = 0x20000u32;
    imports_seed(&mut ram, table, 14)?;
    for bad in [0u32, 0xffff, u32::MAX] {
        imports_denied(imports::Imports::imports_scan(&ram, bad, &data, &[]), -1)?;
    }
    let mut remap = std::vec::Vec::new();
    remap.resize(65537, 0u16);
    imports_fail(&ram, &data, table, &remap, -1)?;
    for ordinal in [0u32, 181, 378, 0x7fffffff] {
        imports_seed(&mut ram, table, ordinal)?;
        imports_fail(&ram, &data, table, &[], -5)?;
    }
    imports_seed(&mut ram, table, 14)?;
    for index in 0..379u32 {
        ram.write_u32(table + index * 4, 0x8000000e)?;
    }
    imports_fail(&ram, &data, table, &[], -1)?;
    for (count, distance) in [(377u32, 1u32), (362, 16), (0, 1)] {
        imports_seed(&mut ram, table, 14)?;
        ram.fill(table, 0, 379 * 4)?;
        for index in 0..count {
            ram.write_u32(table + index * 4, 0x8000000e)?;
        }
        ram.write_u32(table - distance * 4, 0x8000000e)?;
        imports_fail(&ram, &data, table, &[], -1)?;
    }
    for ordinal in [0u32, 16, 181] {
        imports_seed(&mut ram, table, 14)?;
        ram.write_u32(table - 4, 0x80000000 | ordinal)?;
        let mut remap = std::vec::Vec::new();
        remap.resize(182, 0u16);
        remap[ordinal as usize] = 14;
        imports_fail(&ram, &data, table, &remap, -5)?;
    }
    imports_seed(&mut ram, table, 14)?;
    let prepared = imports::Imports::imports_scan(&ram, table, &data, &[])?;
    ram.write_u32(table - 16, 0x1234)?;
    if prepared.imports_publish(&mut ram) != Err(-2) || ram.read_u32(table)? != 0x8000000e {
        return Err(-3);
    }
    imports_seed(&mut ram, table, 14)?;
    let prepared = imports::Imports::imports_scan(&ram, table, &data, &[])?;
    imports_mode(main, 0x20000, 3, 1)?;
    if prepared.imports_publish(&mut ram) != Err(-2)
        || gamepkg::local_memory_page_states(main, 0x20000, 1)? != [2u8]
    {
        return Err(-3);
    }
    imports_mode(main, 0x20000, 3, 2)?;
    imports_seed(&mut ram, table, 14)?;
    let prepared = imports::Imports::imports_scan(&ram, table, &data, &[])?;
    let mut alternate = ram.call_view();
    alternate.attach(0, 67108864, physical, 0, true)?;
    if prepared.imports_publish(&mut alternate) != Err(-4) {
        return Err(-3);
    }
    let mut other = memory::Memory::new()?;
    if prepared.imports_publish(&mut other) != Err(-4) {
        return Err(-3);
    }
    gamepkg::local_record_release(other.call_handle())?;
    // A translation-level read-only view fails even after temporary host RW.
    imports_mode(main, 0x1f000, 3, 1)?;
    imports_mode(main, 0x20000, 3, 1)?;
    let prepared = imports::Imports::imports_scan(&ram, table, &data, &[])?;
    let mut readonly = ram.call_view();
    readonly.attach(0, 67108864, main, 0, false)?;
    if prepared.imports_publish(&mut readonly) != Err(-4) || ram.read_u32(table)? != 0x8000000e {
        return Err(-3);
    }
    if gamepkg::local_memory_page_states(main, 0x1f000, 2)? != [2u8, 2] {
        return Err(-3);
    }
    // Failed publication released its record. The old 64-record quota still fits.
    let mut held = std::vec::Vec::new();
    held.resize(63, 0u64);
    for index in 0..63usize {
        held[index] = gamepkg::local_record_create(&[1u8])?;
    }
    if prepared.imports_publish(&mut ram) != Err(-2)
        || ram.read_u32(table)? != 0x8000000e
        || gamepkg::local_memory_page_states(main, 0x1f000, 2)? != [2u8, 2]
    {
        return Err(-3);
    }
    for id in held {
        gamepkg::local_record_release(id)?;
    }
    prepared.imports_publish(&mut ram)?;
    imports_corrupt(&ram)?;
    ram.kernel_clear()?;
    imports_mode(main, 0x1f000, 3, 2)?;
    imports_mode(main, 0x20000, 3, 2)?;
    imports_seed(&mut ram, table, 14)?;
    ram.write_u32(table - 4, 0x8000000e)?;
    let prepared = imports::Imports::imports_scan(&ram, table, &data, &[])?;
    prepared.imports_publish(&mut ram)?;
    imports_gap_corrupt(&ram)?;
    ram.kernel_clear()?;
    let late = 0x21004;
    imports_seed(&mut ram, late, 14)?;
    ram.write_u32(late - 64, 0x12345678)?;
    let prepared = imports::Imports::imports_scan(&ram, late, &data, &[])?;
    imports_mode(main, 0x21000, 4, 0)?;
    if prepared.imports_publish(&mut ram) != Err(-2)
        || ram.read_u32(late - 64)? != 0x12345678
        || gamepkg::local_memory_page_states(main, 0x20000, 1)? != [3u8]
    {
        return Err(-3);
    }
    imports_mode(main, 0x21000, 2, 2)?;
    imports_seed(&mut ram, table, 14)?;
    let prepared = imports::Imports::imports_scan(&ram, table, &data, &[])?;
    // Root revocation must not make a prepared map publishable.
    ram.heap_close()?;
    gamepkg::local_record_release(ram.call_handle())?;
    if prepared.imports_publish(&mut ram) != Err(-4) {
        return Err(-3);
    }
    // The owner has already stopped; cleanup still consumes its completion.
    probe::imports_cleanup(&mut ram, job)?;
    let (mut ram, data, job, _, _) = probe::imports_environment(false)?;
    imports_seed(&mut ram, table, 165)?;
    let prepared = imports::Imports::imports_scan(&ram, table, &data, &[])?;
    if prepared.imports_publish(&mut ram) != Err(-5)
        || ram.read_u32(table)? != 0x800000a5
        || ram.kernel_handle() != 0
    {
        return Err(-3);
    }
    let mut client = ram.heap_take()?;
    let result = client.heap_status();
    ram.heap_put(client);
    result?;
    probe::imports_cleanup(&mut ram, job)?;
    Ok(())
}
