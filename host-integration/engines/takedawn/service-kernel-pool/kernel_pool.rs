// Four reviewed pool bridges over one shared heap. No native handler pointers.
use crate::gamepkg;
use crate::memory;
use crate::registers;
fn pool_word(bytes: &[u8], at: usize) -> Result<u32, i32> {
    let b = bytes.get(at..at + 4).ok_or(-1)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}
fn pool_long(bytes: &[u8], at: usize) -> Result<u64, i32> {
    let b = bytes.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}
fn pool_argument_bytes(ordinal: u32) -> Result<u32, i32> {
    match ordinal {
        14 | 17 | 23 => Ok(4),
        15 => Ok(8),
        _ => Err(-5),
    }
}
// Until other kernel/data handlers are migrated, preparation accepts only a
// wholly supported pool import table. Unsupported imports fail before writes;
// no unresolved marker may silently fall into ICALL's non-code fallback.
pub fn kernel_prepare(ram: &mut memory::Memory, table: u32, count: u32) -> Result<(), i32> {
    ram.call_snapshot()?;
    if ram.kernel_handle() != 0
        || count == 0
        || count > 378
        || table < 65536
        || table & 3 != 0
        || u64::from(table) + u64::from(count) * 4 > 67108864
    {
        return Err(-1);
    }
    let mut words = [0u8; 1512];
    ram.read_bytes(table, &mut words[..count as usize * 4])?;
    let mut record = [0u8; 1536];
    record[..4].copy_from_slice(&u32::to_le_bytes(0x314c504b));
    record[4..8].copy_from_slice(&u32::to_le_bytes(count));
    record[8..16].copy_from_slice(&u64::to_le_bytes(ram.call_handle()));
    record[16..20].copy_from_slice(&u32::to_le_bytes(table));
    for slot in 0..count as usize {
        let thunk_word = pool_word(&words, slot * 4)?;
        if thunk_word == 0 {
            continue;
        }
        if thunk_word & 0x80000000 == 0 {
            return Err(-1);
        }
        let ordinal = thunk_word & 0x7fffffff;
        pool_argument_bytes(ordinal)?;
        record[24 + slot * 4..28 + slot * 4].copy_from_slice(&u32::to_le_bytes(ordinal));
        words[slot * 4..slot * 4 + 4]
            .copy_from_slice(&u32::to_le_bytes(0xfe000000 + slot as u32 * 4));
    }
    let id = gamepkg::local_record_create(&record[..24 + count as usize * 4])?;
    if let Err(code) = ram.write_bytes(table, &words[..count as usize * 4]) {
        let _ = gamepkg::local_record_release(id);
        return Err(code);
    }
    if let Err(code) = ram.kernel_install(id) {
        let _ = gamepkg::local_record_release(id);
        return Err(code);
    }
    Ok(())
}
pub fn kernel_ordinal(ram: &memory::Memory, slot: u32) -> Result<u32, i32> {
    ram.call_snapshot()?;
    let id = ram.kernel_handle();
    if id == 0 {
        return Err(-5);
    }
    let (_, record) = gamepkg::local_record_snapshot(id, 1536)?;
    if record.len() < 24
        || pool_word(&record, 0)? != 0x314c504b
        || pool_long(&record, 8)? != ram.call_handle()
        || pool_word(&record, 20)? != 0
    {
        return Err(-1);
    }
    let count = pool_word(&record, 4)?;
    let table = pool_word(&record, 16)?;
    if count == 0
        || count > 378
        || record.len() != 24 + count as usize * 4
        || table < 65536
        || table & 3 != 0
        || u64::from(table) + u64::from(count) * 4 > 67108864
    {
        return Err(-1);
    }
    if slot >= count {
        return Err(-5);
    }
    let ordinal = pool_word(&record, 24 + slot as usize * 4)?;
    pool_argument_bytes(ordinal)?;
    Ok(ordinal)
}
pub fn kernel_invoke(
    state: &mut registers::Registers,
    ram: &mut memory::Memory,
    slot: u32,
) -> Result<(), i32> {
    if !state.valid() {
        return Err(-1);
    }
    let ordinal = kernel_ordinal(ram, slot)?;
    let arg_bytes = pool_argument_bytes(ordinal)?;
    let stack = u64::from(state.esp);
    let end = stack + 4 + u64::from(arg_bytes);
    if end > 4294967296 {
        return Err(-1);
    }
    // The original dispatcher observes a nonzero dummy return before popping
    // it. All required reads occur before any heap request or register change.
    let caller = if state.esp != 0 {
        ram.read_u32(state.esp)?
    } else {
        0
    };
    let mut args = [0u8; 8];
    ram.read_bytes((stack + 4) as u32, &mut args[..arg_bytes as usize])?;
    let first = pool_word(&args, 0)?;
    if ordinal == 15 {
        let _ = pool_word(&args, 4)?;
    }
    ram.kernel_set_caller(caller);
    let mut client = ram.heap_take()?;
    let outcome = match ordinal {
        14 | 15 => client.heap_alloc(first, 16),
        17 => match client.heap_release_block(first) {
            Ok(()) => Ok(0),
            Err(code) => Err(code),
        },
        23 => client.heap_query(first),
        _ => Err(-5),
    };
    // Restore exclusive client state even on a known error or poisoned reply.
    // Failure aborts the caller; an accepted mutation must never be replayed.
    ram.heap_put(client);
    let result = outcome?;
    state.eax = result;
    state.esp = end as u32;
    Ok(())
}
