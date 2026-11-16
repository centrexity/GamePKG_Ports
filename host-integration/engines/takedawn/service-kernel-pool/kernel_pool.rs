// Reviewed allocator and SHA bridges. No native handler pointers or OS calls.
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
        14 | 17 | 23 | 165 | 171 | 180 | 335 => Ok(4),
        15 | 168 | 337 => Ok(8),
        336 => Ok(12),
        166 => Ok(20),
        _ => Err(-5),
    }
}
// Shared classification for the checked import planner, never a handler pointer.
pub fn kernel_supported(ordinal: u32) -> Result<u32, i32> {
    pool_argument_bytes(ordinal)
}
// Only these five exports require the shared physical allocator owner.
pub fn kernel_physical(ordinal: u32) -> bool {
    ordinal == 165 || ordinal == 166 || ordinal == 168 || ordinal == 171 || ordinal == 180
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
    let mut record = [0u8; 1540];
    let mut physical = false;
    record[..4].copy_from_slice(&u32::to_le_bytes(0x324c504b));
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
        physical |= kernel_physical(ordinal);
        record[24 + slot * 4..28 + slot * 4].copy_from_slice(&u32::to_le_bytes(ordinal));
        words[slot * 4..slot * 4 + 4]
            .copy_from_slice(&u32::to_le_bytes(0xfe000000 + slot as u32 * 4));
    }
    if physical {
        let mut client = ram.heap_take()?;
        let ready = client.heap_contiguous_counter(0);
        ram.heap_put(client);
        ready?;
    }
    let id = gamepkg::local_record_create(&record[..28 + count as usize * 4])?;
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
fn pool_snapshot(ram: &memory::Memory) -> Result<(u64, std::vec::Vec<u8>, u32), i32> {
    ram.call_snapshot()?;
    let id = ram.kernel_handle();
    if id == 0 {
        return Err(-5);
    }
    let (version, record) = gamepkg::local_record_snapshot(id, 1540)?;
    if record.len() < 28 || pool_long(&record, 8)? != ram.call_handle() {
        return Err(-1);
    }
    let magic = pool_word(&record, 0)?;
    let count = pool_word(&record, 4)?;
    let table = pool_word(&record, 16)?;
    if count == 0 || count > 378 {
        return Err(-1);
    }
    let declared = pool_word(&record, 20)?;
    let main = if magic == 0x324c504b {
        if declared != 0 || table & 3 != 0 {
            return Err(-1);
        }
        count
    } else if magic == 0x334c504b {
        if declared == 0 || declared > count || count > declared + 17 || count == declared + 1 {
            return Err(-1);
        }
        declared
    } else {
        return Err(-1);
    };
    if count == 0
        || count > 378
        || record.len() != 28 + count as usize * 4
        || table < 65536
        || u64::from(table) + u64::from(main) * 4 > 67108864
        || pool_word(&record, 24 + count as usize * 4)? > 8
    {
        return Err(-1);
    }
    if magic == 0x334c504b && count > main {
        if pool_word(&record, 24 + main as usize * 4)? != 0
            || pool_word(&record, 24 + (count as usize - 1) * 4)? == 0
        {
            return Err(-1);
        }
        for slot in main + 1..count {
            let ordinal = pool_word(&record, 24 + slot as usize * 4)?;
            if ordinal != 0 && pool_argument_bytes(ordinal).is_err() {
                return Err(-1);
            }
        }
    }
    Ok((version, record, count))
}
pub fn kernel_ordinal(ram: &memory::Memory, slot: u32) -> Result<u32, i32> {
    let (_, record, count) = pool_snapshot(ram)?;
    if slot >= count {
        return Err(-5);
    }
    let ordinal = pool_word(&record, 24 + slot as usize * 4)?;
    pool_argument_bytes(ordinal)?;
    Ok(ordinal)
}
// At most eight successful updates can compete, unlike the unbounded call
// trace. The existing bounded CAS loop therefore needs no extra wait/queue.
fn pool_refusal_ticket(ram: &memory::Memory) -> Result<bool, i32> {
    for _ in 0..64 {
        let (version, mut record, count) = pool_snapshot(ram)?;
        let at = 24 + count as usize * 4;
        let observed = pool_word(&record, at)?;
        if observed == 8 {
            return Ok(false);
        }
        let next = u32::to_le_bytes(observed + 1);
        for byte in 0..4usize {
            record[at + byte] = next[byte];
        }
        match gamepkg::local_record_compare_exchange(ram.kernel_handle(), version, &record) {
            Ok(_) => return Ok(true),
            Err(-2) => {}
            Err(code) => return Err(code),
        }
    }
    Err(-2)
}
fn pool_contiguous_ex(
    client: &mut crate::heap_service::HeapClient,
    ram: &memory::Memory,
    stack: u64,
    args: &[u8],
) -> Result<u32, i32> {
    let size = pool_word(args, 0)?;
    let low = pool_word(args, 4)?;
    let high = pool_word(args, 8)?;
    let alignment = pool_word(args, 12)?;
    let protection_flags = pool_word(args, 16)?;
    if low != 0 && high >= low && high.wrapping_sub(low).wrapping_add(1) <= size.wrapping_add(4096)
    {
        let address = 0x80000000u32.wrapping_add(low);
        client.heap_contiguous_clear_claim(address, size)?;
        return Ok(address);
    }
    if high < low {
        return Ok(0);
    }
    let alignment = alignment.max(4096);
    let address = client.heap_contiguous_allocate(size, alignment, low, high)?;
    if address == 0 && pool_refusal_ticket(ram)? {
        let extra = stack + 24;
        if extra + 140 > 4294967296 {
            return Err(-1);
        }
        let mut words = [0u8; 140];
        ram.read_bytes(extra as u32, &mut words)?;
        // The original ignores console write failures. Diagnostics are bounded
        // best effort, but the required guest stack reads still must succeed.
        let _ = gamepkg::diagnostic(
            2,
            0x544b0a00,
            &[
                u64::from(ram.kernel_caller()),
                u64::from(size),
                u64::from(low),
                u64::from(high),
                u64::from(alignment),
                u64::from(protection_flags),
            ],
        );
        let _ = gamepkg::diagnostic_bytes(2, 0x544b0a01, &words);
    }
    Ok(address)
}
fn pool_gpu_instance(
    client: &mut crate::heap_service::HeapClient,
    ram: &mut memory::Memory,
    args: &[u8],
) -> Result<u32, i32> {
    client.heap_contiguous_counter(0)?;
    let bytes = pool_word(args, 0)?;
    let padding = pool_word(args, 4)?;
    if padding != 0 {
        ram.write_u32(padding, 0)?;
    }
    Ok(0x84000000u32.wrapping_sub(if bytes == u32::MAX { 131072 } else { bytes }))
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
    let mut args = [0u8; 20];
    ram.read_bytes((stack + 4) as u32, &mut args[..arg_bytes as usize])?;
    let first = pool_word(&args, 0)?;
    if ordinal == 15 {
        let _ = pool_word(&args, 4)?;
    }
    ram.kernel_set_caller(caller);
    if ordinal >= 335 && ordinal <= 337 {
        pool_sha(ram, ordinal, &args)?;
        state.eax = 0;
        state.esp = end as u32;
        return Ok(());
    }
    let mut client = ram.heap_take()?;
    let outcome = match ordinal {
        14 | 15 => client.heap_alloc(first, 16),
        17 => match client.heap_release_block(first) {
            Ok(()) => Ok(0),
            Err(code) => Err(code),
        },
        23 => client.heap_query(first),
        165 => client.heap_contiguous_allocate(first, 4096, 0, u32::MAX),
        166 => pool_contiguous_ex(&mut client, ram, stack, &args),
        168 => pool_gpu_instance(&mut client, ram, &args),
        171 => match client.heap_allocation_release(first) {
            Ok(()) => Ok(0),
            Err(code) => Err(code),
        },
        180 => client.heap_allocation_query(first),
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

// Original 92-byte SHA context: five LE state words, two LE bit-count words,
// then 64 retained buffer bytes. Accesses remain capability-checked even for
// aliases; no digest/context pointer enters the host or a native crypto API.
fn pool_sha_extent(address: u32, bytes: u32) -> Result<(), i32> {
    if u64::from(address) + u64::from(bytes) > 4294967296 {
        return Err(-1);
    }
    Ok(())
}
fn pool_sha_transform(ram: &mut memory::Memory, context: u32, block: &[u8; 64]) -> Result<(), i32> {
    let mut words = [0u32; 80];
    for index in 0..16usize {
        let at = index * 4;
        words[index] = u32::from_be_bytes([block[at], block[at + 1], block[at + 2], block[at + 3]]);
    }
    for index in 16..80usize {
        words[index] =
            (words[index - 3] ^ words[index - 8] ^ words[index - 14] ^ words[index - 16])
                .rotate_left(1);
    }
    let mut encoded = [0u8; 20];
    ram.read_bytes(context, &mut encoded)?;
    let mut prior = [0u32; 5];
    for index in 0..5usize {
        prior[index] = pool_word(&encoded, index * 4)?;
    }
    let mut a = prior[0];
    let mut b = prior[1];
    let mut c = prior[2];
    let mut d = prior[3];
    let mut e = prior[4];
    for index in 0..80usize {
        let (bits, constant) = if index < 20 {
            ((b & c) | (!b & d), 0x5a827999u32)
        } else if index < 40 {
            (b ^ c ^ d, 0x6ed9eba1)
        } else if index < 60 {
            ((b & c) | (b & d) | (c & d), 0x8f1bbcdcu32)
        } else {
            (b ^ c ^ d, 0xca62c1d6u32)
        };
        let next = a
            .rotate_left(5)
            .wrapping_add(bits)
            .wrapping_add(e)
            .wrapping_add(constant)
            .wrapping_add(words[index]);
        e = d;
        d = c;
        c = b.rotate_left(30);
        b = a;
        a = next;
    }
    for (index, value) in [a, b, c, d, e].iter().enumerate() {
        let next = u32::to_le_bytes(prior[index].wrapping_add(*value));
        for byte in 0..4usize {
            encoded[index * 4 + byte] = next[byte];
        }
    }
    ram.write_bytes(context, &encoded)
}
fn pool_sha_count(ram: &mut memory::Memory, context: u32, bytes: u32) -> Result<u32, i32> {
    let low = ram.read_u32(context + 20)?;
    let high = ram.read_u32(context + 24)?;
    let added = bytes.wrapping_shl(3);
    let next = low.wrapping_add(added);
    let upper = high
        .wrapping_add(u32::from(next < added))
        .wrapping_add(bytes >> 29);
    let mut count = [0u8; 8];
    count[..4].copy_from_slice(&u32::to_le_bytes(next));
    count[4..].copy_from_slice(&u32::to_le_bytes(upper));
    ram.write_bytes(context + 20, &count)?;
    Ok((low >> 3) & 63)
}
fn pool_sha_update(
    ram: &mut memory::Memory,
    context: u32,
    input: u32,
    bytes: u32,
) -> Result<(), i32> {
    if context == 0 || input == 0 || bytes == 0 {
        return Ok(());
    }
    pool_sha_extent(context, 92)?;
    pool_sha_extent(input, bytes)?;
    let mut index = pool_sha_count(ram, context, bytes)?;
    let part = 64 - index;
    let mut block = [0u8; 64];
    let mut consumed = 0u32;
    if bytes >= part {
        ram.read_bytes(input, &mut block[..part as usize])?;
        ram.write_bytes(context + 28 + index, &block[..part as usize])?;
        ram.read_bytes(context + 28, &mut block)?;
        pool_sha_transform(ram, context, &block)?;
        consumed = part;
        while u64::from(consumed) + 63 < u64::from(bytes) {
            ram.read_bytes(input + consumed, &mut block)?;
            pool_sha_transform(ram, context, &block)?;
            consumed += 64;
        }
        index = 0;
    }
    let tail = (bytes - consumed) as usize;
    if tail != 0 {
        ram.read_bytes(input + consumed, &mut block[..tail])?;
        ram.write_bytes(context + 28 + index, &block[..tail])?;
    }
    Ok(())
}
// Final's one-byte padding inputs are host-independent copied values, not
// guest addresses. Keep the original update/transform and retained-buffer order.
fn pool_sha_byte(ram: &mut memory::Memory, context: u32, value: u8) -> Result<(), i32> {
    let index = pool_sha_count(ram, context, 1)?;
    ram.write_u8(context + 28 + index, value)?;
    if index == 63 {
        let mut block = [0u8; 64];
        ram.read_bytes(context + 28, &mut block)?;
        pool_sha_transform(ram, context, &block)?;
    }
    Ok(())
}
fn pool_sha(ram: &mut memory::Memory, ordinal: u32, args: &[u8]) -> Result<(), i32> {
    let context = pool_word(args, 0)?;
    if context == 0 {
        return Ok(());
    }
    if ordinal == 336 {
        return pool_sha_update(ram, context, pool_word(args, 4)?, pool_word(args, 8)?);
    }
    if ordinal == 337 && pool_word(args, 4)? == 0 {
        return Ok(());
    }
    pool_sha_extent(context, 92)?;
    if ordinal == 335 {
        let mut encoded = [0u8; 92];
        for (index, value) in [
            0x67452301u32,
            0xefcdab89,
            0x98badcfe,
            0x10325476,
            0xc3d2e1f0,
        ]
        .iter()
        .enumerate()
        {
            let bytes = u32::to_le_bytes(*value);
            for byte in 0..4usize {
                encoded[index * 4 + byte] = bytes[byte];
            }
        }
        return ram.write_bytes(context, &encoded);
    }
    if ordinal != 337 {
        return Err(-5);
    }
    let digest = pool_word(args, 4)?;
    pool_sha_extent(digest, 20)?;
    let high = u32::to_be_bytes(ram.read_u32(context + 24)?);
    let low = u32::to_be_bytes(ram.read_u32(context + 20)?);
    pool_sha_byte(ram, context, 0x80)?;
    // At most 63 zero bytes: modulo count uses the original low-word bits.
    for _ in 0..64usize {
        if (ram.read_u32(context + 20)? >> 3) & 63 == 56 {
            break;
        }
        pool_sha_byte(ram, context, 0)?;
    }
    for value in high {
        pool_sha_byte(ram, context, value)?;
    }
    for value in low {
        pool_sha_byte(ram, context, value)?;
    }
    // Read each word afresh, matching original byte-store alias behavior even
    // when Digest overlaps State. Callers must serialize updates to a context.
    for index in 0..5u32 {
        for byte in 0..4u32 {
            let value = ram.read_u32(context + index * 4)?;
            ram.write_u8(digest + index * 4 + byte, (value >> ((3 - byte) * 8)) as u8)?;
        }
    }
    Ok(())
}
