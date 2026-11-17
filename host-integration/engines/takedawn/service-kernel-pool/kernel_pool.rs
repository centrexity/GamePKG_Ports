// Reviewed allocator, settings, clock and crypto bridges. No native handler pointers or OS calls.
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
        14 | 17 | 23 | 98 | 165 | 171 | 180 | 335 => Ok(4),
        15 | 44 | 168 | 337 => Ok(8),
        107 | 336 | 338 | 339 => Ok(12),
        109 | 340 => Ok(28),
        24 | 166 => Ok(20),
        125 | 126 | 127 => Ok(0),
        128 => Ok(4),
        _ => Err(-5),
    }
}
// Shared classification for the checked import planner, never a handler pointer.
pub fn kernel_supported(ordinal: u32) -> Result<u32, i32> {
    pool_argument_bytes(ordinal)
}
// Only these five exports require the shared physical allocator owner.
pub fn kernel_requires_context(ordinal: u32) -> bool {
    ordinal == 98
}
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
    let mut services = false;
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
        if kernel_requires_context(ordinal) {
            return Err(-5);
        }
        physical |= kernel_physical(ordinal);
        services |= kernel_requires_services(ordinal);
        record[24 + slot * 4..28 + slot * 4].copy_from_slice(&u32::to_le_bytes(ordinal));
        words[slot * 4..slot * 4 + 4]
            .copy_from_slice(&u32::to_le_bytes(0xfe000000 + slot as u32 * 4));
    }
    if services {
        kernel_service_values(ram)?;
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
fn pool_snapshot(ram: &memory::Memory) -> Result<(u64, std::vec::Vec<u8>, u32, usize), i32> {
    ram.call_snapshot()?;
    let id = ram.kernel_handle();
    if id == 0 {
        return Err(-5);
    }
    let (version, record) = gamepkg::local_record_snapshot(id, 1548)?;
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
    } else if magic == 0x334c504b || magic == 0x344c504b {
        if declared == 0 || declared > count || count > declared + 17 || count == declared + 1 {
            return Err(-1);
        }
        declared
    } else {
        return Err(-1);
    };
    let routing_start = if magic == 0x344c504b {
        32usize
    } else {
        24usize
    };
    if record.len() != routing_start + 4 + count as usize * 4
        || table < 65536
        || u64::from(table) + u64::from(main) * 4 > 67108864
        || pool_word(&record, routing_start + count as usize * 4)? > 8
    {
        return Err(-1);
    }
    if magic == 0x344c504b {
        let bound_context = pool_long(&record, 24)?;
        if bound_context == 0 {
            return Err(-1);
        }
        KernelContext::kernel_context_borrow(ram, bound_context)?;
    }
    if magic != 0x324c504b && count > main {
        if pool_word(&record, routing_start + main as usize * 4)? != 0
            || pool_word(&record, routing_start + (count as usize - 1) * 4)? == 0
        {
            return Err(-1);
        }
        for slot in main + 1..count {
            let ordinal = pool_word(&record, routing_start + slot as usize * 4)?;
            if ordinal != 0
                && (pool_argument_bytes(ordinal).is_err()
                    || kernel_requires_context(ordinal) && routing_start != 32)
            {
                return Err(-1);
            }
        }
    }
    Ok((version, record, count, routing_start))
}
pub fn kernel_ordinal(ram: &memory::Memory, slot: u32) -> Result<u32, i32> {
    let (_, record, count, routing_start) = pool_snapshot(ram)?;
    if slot >= count {
        return Err(-5);
    }
    let ordinal = pool_word(&record, routing_start + slot as usize * 4)?;
    pool_argument_bytes(ordinal)?;
    if kernel_requires_context(ordinal) && routing_start != 32 {
        return Err(-5);
    }
    Ok(ordinal)
}
// At most eight successful updates can compete, unlike the unbounded call
// trace. The existing bounded CAS loop therefore needs no extra wait/queue.
fn pool_refusal_ticket(ram: &memory::Memory) -> Result<bool, i32> {
    for _ in 0..64 {
        let (version, mut record, count, routing_start) = pool_snapshot(ram)?;
        let at = routing_start + count as usize * 4;
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
    let mut args = [0u8; 28];
    ram.read_bytes((stack + 4) as u32, &mut args[..arg_bytes as usize])?;
    let first = pool_word(&args, 0)?;
    if ordinal == 15 {
        let _ = pool_word(&args, 4)?;
    }
    ram.kernel_set_caller(caller);
    if ordinal == 98 {
        let (_, record, _, routing_start) = pool_snapshot(ram)?;
        if routing_start != 32 {
            return Err(-5);
        }
        let context = KernelContext::kernel_context_borrow(ram, pool_long(&record, 24)?)?;
        let registry = context.kernel_context_registry(ram)?;
        let connected = registry.connection_connect(ram, first)?;
        if connected[1] < 32 {
            // Original connection report is best effort after publication.
            let _ = gamepkg::diagnostic(
                1,
                0x544b3200,
                &[
                    u64::from(connected[1]),
                    u64::from(first),
                    u64::from(connected[2]),
                    u64::from(connected[3]),
                ],
            );
        }
        state.eax = connected[0];
        state.esp = end as u32;
        return Ok(());
    }
    if ordinal == 44 || ordinal == 107 || ordinal == 109 {
        let value = pool_dispatcher_initialize(ram, ordinal, &args)?;
        state.eax = value;
        state.esp = end as u32;
        return Ok(());
    }
    if ordinal >= 125 && ordinal <= 128 {
        if ordinal == 128 {
            if first != 0 {
                let sample = gamepkg::clock_sample()?;
                let values = kernel_service_values(ram)?;
                let value = kernel_time_value(ordinal, sample.utc_100ns, 0, values[2])?;
                ram.write_bytes(first, &u64::to_le_bytes(value))?;
            }
            state.eax = 0;
        } else {
            let value = if ordinal == 126 {
                // Original counter bridge observes shared skew before QPC.
                let values = kernel_service_values(ram)?;
                let sample = gamepkg::local_performance_sample()?;
                kernel_time_value(ordinal, sample.counter, sample.frequency, values[2])?
            } else {
                let sample = gamepkg::local_performance_sample()?;
                if ordinal == 125 {
                    let values = kernel_service_values(ram)?;
                    kernel_time_value(ordinal, sample.uptime_ms, 0, values[2])?
                } else {
                    kernel_time_value(ordinal, sample.frequency, 0, 0)?
                }
            };
            state.eax = value as u32;
            state.edx = (value >> 32) as u32;
        }
        state.esp = end as u32;
        return Ok(());
    }
    if ordinal == 24 {
        let result = pool_nv_setting(ram, &args)?;
        state.eax = result;
        state.esp = end as u32;
        return Ok(());
    }
    if ordinal >= 335 && ordinal <= 337 {
        pool_sha(ram, ordinal, &args)?;
        state.eax = 0;
        state.esp = end as u32;
        return Ok(());
    }
    if ordinal >= 338 && ordinal <= 340 {
        if ordinal == 340 {
            pool_hmac(ram, &args)?;
        } else {
            pool_rc4(ram, ordinal, &args)?;
        }
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
fn pool_sha_compress(prior: &mut [u32; 5], block: &[u8; 64]) {
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
        prior[index] = prior[index].wrapping_add(*value);
    }
}
fn pool_sha_transform(ram: &mut memory::Memory, context: u32, block: &[u8; 64]) -> Result<(), i32> {
    let mut encoded = [0u8; 20];
    ram.read_bytes(context, &mut encoded)?;
    let mut prior = [0u32; 5];
    for index in 0..5usize {
        prior[index] = pool_word(&encoded, index * 4)?;
    }
    pool_sha_compress(&mut prior, block);
    for index in 0..5usize {
        let bytes = u32::to_le_bytes(prior[index]);
        for byte in 0..4usize {
            encoded[index * 4 + byte] = bytes[byte];
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
    // At most 63 zero bytes: modulo count uses the original low-field_bytes bits.
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
    // Read each field_bytes afresh, matching original byte-store alias behavior even
    // when Digest overlaps State. Callers must serialize updates to a context.
    for index in 0..5u32 {
        for byte in 0..4u32 {
            let value = ram.read_u32(context + index * 4)?;
            ram.write_u8(digest + index * 4 + byte, (value >> ((3 - byte) * 8)) as u8)?;
        }
    }
    Ok(())
}

// RC4 keeps its original guest-visible 256-byte permutation and byte i/j.
// Reads deliberately follow prior writes when key/data alias that context.
fn pool_rc4(ram: &mut memory::Memory, ordinal: u32, args: &[u8]) -> Result<(), i32> {
    let context = pool_word(args, 0)?;
    let length = pool_word(args, 4)?;
    let input = pool_word(args, 8)?;
    if context == 0 || input == 0 || length == 0 {
        return Ok(());
    }
    pool_sha_extent(context, 258)?;
    if ordinal == 338 {
        // KSA reads at most 256 key bytes, even with a larger declared key.
        pool_sha_extent(input, length.min(256))?;
        for index in 0..256u32 {
            ram.write_u8(context + index, index as u8)?;
        }
        let mut j = 0u8;
        for index in 0..256u32 {
            let value = ram.read_u8(context + index)?;
            let key_byte = ram.read_u8(input + index % length)?;
            j = j.wrapping_add(value).wrapping_add(key_byte);
            let other = ram.read_u8(context + u32::from(j))?;
            ram.write_u8(context + index, other)?;
            ram.write_u8(context + u32::from(j), value)?;
        }
        ram.write_u8(context + 256, 0)?;
        ram.write_u8(context + 257, 0)?;
        return Ok(());
    }
    if ordinal != 339 {
        return Err(-5);
    }
    pool_sha_extent(input, length)?;
    for position in 0..length {
        let i = ram.read_u8(context + 256)?.wrapping_add(1);
        ram.write_u8(context + 256, i)?;
        let j = ram
            .read_u8(context + 257)?
            .wrapping_add(ram.read_u8(context + u32::from(i))?);
        ram.write_u8(context + 257, j)?;
        let value = ram.read_u8(context + u32::from(i))?;
        let other = ram.read_u8(context + u32::from(j))?;
        ram.write_u8(context + u32::from(i), other)?;
        ram.write_u8(context + u32::from(j), value)?;
        let selector = ram
            .read_u8(context + u32::from(i))?
            .wrapping_add(ram.read_u8(context + u32::from(j))?);
        let key_byte = ram.read_u8(context + u32::from(selector))?;
        let data_byte = ram.read_u8(input + position)?;
        ram.write_u8(input + position, data_byte ^ key_byte)?;
    }
    Ok(())
}
struct PoolHash {
    words: [u32; 5],
    bytes: u64,
    used: usize,
    buffer: [u8; 64],
}
impl PoolHash {
    fn hash_new() -> Self {
        Self {
            words: [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476, 0xc3d2e1f0],
            bytes: 0,
            used: 0,
            buffer: [0u8; 64],
        }
    }
    fn hash_update(&mut self, input: &[u8]) {
        self.bytes = self.bytes.wrapping_add(input.len() as u64);
        for value in input {
            self.buffer[self.used] = *value;
            self.used += 1;
            if self.used == 64 {
                pool_sha_compress(&mut self.words, &self.buffer);
                self.used = 0;
            }
        }
    }
    fn hash_final(&mut self) -> [u8; 20] {
        let bits = u64::to_be_bytes(self.bytes.wrapping_mul(8));
        self.hash_update(&[0x80u8]);
        for _ in 0..64usize {
            if self.used == 56 {
                break;
            }
            self.hash_update(&[0u8]);
        }
        self.hash_update(&bits);
        let mut digest = [0u8; 20];
        for index in 0..5usize {
            let bytes = u32::to_be_bytes(self.words[index]);
            for byte in 0..4usize {
                digest[index * 4 + byte] = bytes[byte];
            }
        }
        digest
    }
    fn hash_clear(&mut self) -> Result<(), i32> {
        gamepkg::zeroize(&mut self.buffer)?;
        self.words = [0u32; 5];
        self.bytes = 0;
        self.used = 0;
        Ok(())
    }
}
fn pool_hash_input(
    hash: &mut PoolHash,
    ram: &memory::Memory,
    input: u32,
    length: u32,
) -> Result<(), i32> {
    if input == 0 || length == 0 {
        return Ok(());
    }
    pool_sha_extent(input, length)?;
    let mut block = std::vec::Vec::new();
    block.resize(4096, 0u8);
    let mut offset = 0u32;
    while offset < length {
        let count = (length - offset).min(4096) as usize;
        if let Err(code) = ram.read_bytes(input + offset, &mut block[..count]) {
            let _ = gamepkg::zeroize(&mut block);
            return Err(code);
        }
        hash.hash_update(&block[..count]);
        offset += count as u32;
    }
    gamepkg::zeroize(&mut block)
}
struct PoolHmac {
    key: [u8; 64],
    inner: [u8; 64],
    outer: [u8; 64],
    digest: [u8; 20],
    hash: PoolHash,
}
impl PoolHmac {
    fn hmac_compute(&mut self, ram: &mut memory::Memory, args: &[u8]) -> Result<(), i32> {
        let key = pool_word(args, 0)?;
        let length = pool_word(args, 4)?;
        if length > 64 {
            pool_hash_input(&mut self.hash, ram, key, length)?;
            let mut digest = self.hash.hash_final();
            for index in 0..20usize {
                self.key[index] = digest[index];
            }
            gamepkg::zeroize(&mut digest)?;
        } else if length != 0 {
            ram.read_bytes(key, &mut self.key[..length as usize])?;
        }
        for index in 0..64usize {
            self.inner[index] = self.key[index] ^ 0x36;
            self.outer[index] = self.key[index] ^ 0x5c;
        }
        self.hash.hash_clear()?;
        self.hash = PoolHash::hash_new();
        self.hash.hash_update(&self.inner);
        pool_hash_input(
            &mut self.hash,
            ram,
            pool_word(args, 8)?,
            pool_word(args, 12)?,
        )?;
        pool_hash_input(
            &mut self.hash,
            ram,
            pool_word(args, 16)?,
            pool_word(args, 20)?,
        )?;
        self.digest = self.hash.hash_final();
        self.hash.hash_clear()?;
        self.hash = PoolHash::hash_new();
        self.hash.hash_update(&self.outer);
        self.hash.hash_update(&self.digest);
        let result = self.hash.hash_final();
        ram.write_bytes(pool_word(args, 24)?, &result)
    }
    fn hmac_clear(&mut self) -> Result<(), i32> {
        let key = gamepkg::zeroize(&mut self.key);
        let inner = gamepkg::zeroize(&mut self.inner);
        let outer = gamepkg::zeroize(&mut self.outer);
        let digest = gamepkg::zeroize(&mut self.digest);
        let hash = self.hash.hash_clear();
        key?;
        inner?;
        outer?;
        digest?;
        hash
    }
}
fn pool_hmac(ram: &mut memory::Memory, args: &[u8]) -> Result<(), i32> {
    let key = pool_word(args, 0)?;
    let output = pool_word(args, 24)?;
    if key == 0 || output == 0 {
        return Ok(());
    }
    pool_sha_extent(key, pool_word(args, 4)?)?;
    pool_sha_extent(output, 20)?;
    let mut hmac = PoolHmac {
        key: [0u8; 64],
        inner: [0u8; 64],
        outer: [0u8; 64],
        digest: [0u8; 20],
        hash: PoolHash::hash_new(),
    };
    let outcome = hmac.hmac_compute(ram, args);
    let cleaned = hmac.hmac_clear();
    outcome?;
    cleaned
}

// One root-bound private record, reached through the root's trailing ID. Its
// flags are immutable; guest skew changes by bounded CAS, never a copied view.
pub struct KernelServices {
    root: u64,
    id: u64,
}
fn pool_services_snapshot(ram: &memory::Memory) -> Result<(u64, u64, std::vec::Vec<u8>), i32> {
    let id = ram.services_handle()?;
    if id == 0 {
        return Err(-5);
    }
    let (version, record) = gamepkg::local_record_snapshot(id, 64)?;
    if record.len() != 64
        || pool_word(&record, 0)? != 0x3356534b
        || pool_word(&record, 4)? & !3u32 != 0
    {
        return Err(-1);
    }
    if pool_long(&record, 8)? != ram.call_handle() {
        return Err(-4);
    }
    if pool_word(&record, 16)? & !0x001f0000u32 != 0 {
        return Err(-1);
    }
    let audio = pool_word(&record, 20)?;
    if audio != 0 && audio != 0x00010002 {
        return Err(-1);
    }
    let origin = pool_long(&record, 32)?;
    let frequency = pool_long(&record, 40)?;
    let initialized = pool_long(&record, 48)?;
    if pool_long(&record, 56)? != 0
        || initialized > 1
        || (initialized == 0 && (origin != 0 || frequency != 0))
        || (initialized == 1
            && (origin > i64::MAX as u64 || frequency == 0 || frequency > i64::MAX as u64))
    {
        return Err(-1);
    }
    Ok((id, version, record))
}
pub fn kernel_service_values(ram: &memory::Memory) -> Result<[u64; 3], i32> {
    let (_, _, record) = pool_services_snapshot(ram)?;
    Ok([
        u64::from(pool_word(&record, 16)?),
        u64::from(pool_word(&record, 20)?),
        pool_long(&record, 24)?,
    ])
}
pub fn kernel_requires_services(ordinal: u32) -> bool {
    ordinal == 24
        || ordinal == 98
        || ordinal == 107
        || ordinal == 109
        || ordinal == 125
        || ordinal == 126
        || ordinal == 128
}
pub fn kernel_services_initialize(
    ram: &memory::Memory,
    video: u32,
    speakers: u32,
) -> Result<KernelServices, i32> {
    if ram.services_handle()? != 0 || ram.kernel_handle() != 0 || (speakers != 2 && speakers != 6) {
        return Err(-1);
    }
    let mut record = [0u8; 64];
    record[..4].copy_from_slice(&u32::to_le_bytes(0x3356534b));
    record[8..16].copy_from_slice(&u64::to_le_bytes(ram.call_handle()));
    record[16..20].copy_from_slice(&u32::to_le_bytes(video & 0x001f0000));
    record[20..24].copy_from_slice(&u32::to_le_bytes(if speakers == 6 {
        0x00010002u32
    } else {
        0
    }));
    let id = gamepkg::local_record_create(&record)?;
    if let Err(code) = ram.services_link(0, id) {
        let _ = gamepkg::local_record_release(id);
        return Err(code);
    }
    Ok(KernelServices {
        root: ram.call_handle(),
        id,
    })
}
impl KernelServices {
    pub fn service_handle(&self) -> u64 {
        self.id
    }
    pub fn service_advance(&self, ram: &memory::Memory, delta: u64) -> Result<u64, i32> {
        if self.root != ram.call_handle() {
            return Err(-4);
        }
        pool_advance_services(ram, self.id, delta)
    }
    pub fn service_close(&self, ram: &memory::Memory) -> Result<(), i32> {
        if self.root != ram.call_handle() {
            return Err(-4);
        }
        match ram.services_handle() {
            Ok(id) if id == self.id => ram.services_link(self.id, 0)?,
            Ok(0) => {}
            Err(-4) => {
                let _ = gamepkg::local_record_release(self.id);
                return Err(-4);
            }
            Ok(_) => return Err(-4),
            Err(code) => return Err(code),
        }
        match gamepkg::local_record_release(self.id) {
            Ok(()) | Err(-4) => Ok(()),
            Err(code) => Err(code),
        }
    }
}
pub fn kernel_advance_services(ram: &memory::Memory, delta: u64) -> Result<u64, i32> {
    let id = ram.services_handle()?;
    if id == 0 {
        return Err(-5);
    }
    pool_advance_services(ram, id, delta)
}
fn pool_advance_services(ram: &memory::Memory, id: u64, delta: u64) -> Result<u64, i32> {
    for _ in 0..64usize {
        let (observed, version, mut record) = pool_services_snapshot(ram)?;
        if observed != id {
            return Err(-4);
        }
        let skew = pool_long(&record, 24)?.checked_add(delta).ok_or(-1)?;
        record[24..32].copy_from_slice(&u64::to_le_bytes(skew));
        match gamepkg::local_record_compare_exchange(id, version, &record) {
            Ok(_) => return Ok(skew),
            Err(-2) => memory::Memory::call_retry_pause()?,
            Err(code) => return Err(code),
        }
    }
    Err(-2)
}
fn pool_nv_zero(ram: &mut memory::Memory, output: u32, length: u32) -> Result<(), i32> {
    if length == 0 {
        return Ok(());
    }
    if length > 67108864 || u64::from(output) + u64::from(length) > 4294967296 {
        return Err(-1);
    }
    let mut zeros = std::vec::Vec::new();
    zeros.resize(4096, 0u8);
    let mut offset = 0u32;
    while offset < length {
        let count = (length - offset).min(4096);
        ram.write_bytes(output + offset, &zeros[..count as usize])?;
        offset += count;
    }
    Ok(())
}
fn pool_nv_setting(ram: &mut memory::Memory, args: &[u8]) -> Result<u32, i32> {
    let values = kernel_service_values(ram)?;
    let index = pool_word(args, 0)?;
    let kind = pool_word(args, 4)?;
    let output = pool_word(args, 8)?;
    let length = pool_word(args, 12)?;
    let result = pool_word(args, 16)?;
    if output == 0 {
        return Ok(0xc000000d);
    }
    let _ = gamepkg::diagnostic(0, 0x544b1300, &[u64::from(index), u64::from(length)]);
    let nv_scalar = match index {
        0 => Some(0xfffffed4u32),
        7 | 18 => Some(1),
        8 => Some(values[0] as u32),
        9 => Some(values[1] as u32),
        10 | 12 | 17 => Some(0),
        _ => None,
    };
    if let Some(value) = nv_scalar {
        if length >= 4 {
            ram.write_u32(output, value)?;
            if kind != 0 {
                ram.write_u32(kind, 4)?;
            }
            if result != 0 {
                ram.write_u32(result, 4)?;
            }
        }
    } else {
        let _ = gamepkg::diagnostic(2, 0x544b1301, &[u64::from(index)]);
        pool_nv_zero(ram, output, length)?;
        if kind != 0 {
            ram.write_u32(kind, 4)?;
        }
        if result != 0 {
            ram.write_u32(result, length)?;
        }
    }
    Ok(0)
}

// Exact original unsigned interrupt-time wrap and split signed QPC scaling.
// Refuse inputs outside the original C's defined signed arithmetic domain;
// never turn signed-overflow undefined behavior into an invented guest value.
pub fn kernel_time_value(ordinal: u32, base: u64, frequency: u64, skew: u64) -> Result<u64, i32> {
    if ordinal < 125 || ordinal > 128 {
        return Err(-5);
    }
    if ordinal == 125 {
        return Ok(base.wrapping_mul(10000).wrapping_add(skew / 100));
    }
    if base > i64::MAX as u64 {
        return Err(-1);
    }
    if ordinal == 127 {
        return if base != 0 { Ok(base) } else { Err(-1) };
    }
    if ordinal == 128 {
        let value = base.checked_add(skew / 100).ok_or(-1)?;
        return if value <= i64::MAX as u64 {
            Ok(value)
        } else {
            Err(-1)
        };
    }
    if ordinal != 126 {
        return Err(-5);
    }
    if frequency == 0 || frequency > i64::MAX as u64 {
        return Err(-1);
    }
    let seconds = (skew / 1000000000).checked_mul(frequency).ok_or(-1)?;
    let fraction = (skew % 1000000000).checked_mul(frequency).ok_or(-1)?;
    if seconds > i64::MAX as u64 || fraction > i64::MAX as u64 {
        return Err(-1);
    }
    let value = base
        .checked_add(seconds)
        .ok_or(-1)?
        .checked_add(fraction / 1000000000)
        .ok_or(-1)?;
    if value > i64::MAX as u64 {
        Err(-1)
    } else {
        Ok(value)
    }
}

// Separate guest TSC units/origin. KSV3 keeps one immutable first-use pair in
// root-owned copied metadata. Calendar anchoring is never a side effect here.
pub fn kernel_tsc_state(ram: &memory::Memory) -> Result<[u64; 3], i32> {
    let (_, _, record) = pool_services_snapshot(ram)?;
    Ok([
        pool_long(&record, 32)?,
        pool_long(&record, 40)?,
        pool_long(&record, 48)?,
    ])
}
pub fn kernel_tsc_value(counter: u64, origin: u64, frequency: u64, skew: u64) -> Result<u64, i32> {
    if counter > i64::MAX as u64
        || origin > counter
        || frequency == 0
        || frequency > i64::MAX as u64
    {
        return Err(-1);
    }
    let ticks = counter - origin;
    let seconds = ticks / frequency;
    let remainder = ticks % frequency;
    Ok(seconds
        .wrapping_mul(733333333)
        .wrapping_add(remainder.wrapping_mul(733333333) / frequency)
        .wrapping_add((skew / 1000000000).wrapping_mul(733333333))
        .wrapping_add((skew % 1000000000).wrapping_mul(733333333) / 1000000000))
}
pub fn kernel_tsc_read(ram: &memory::Memory) -> Result<u64, i32> {
    let id = ram.services_handle()?;
    if id == 0 {
        return Err(-5);
    }
    for _ in 0..64usize {
        let (observed, version, mut record) = pool_services_snapshot(ram)?;
        if observed != id {
            return Err(-4);
        }
        if pool_long(&record, 48)? == 0 {
            let sample = gamepkg::local_performance_sample()?;
            if sample.counter > i64::MAX as u64
                || sample.frequency == 0
                || sample.frequency > i64::MAX as u64
            {
                return Err(-3);
            }
            record[32..40].copy_from_slice(&u64::to_le_bytes(sample.counter));
            record[40..48].copy_from_slice(&u64::to_le_bytes(sample.frequency));
            record[48..56].copy_from_slice(&u64::to_le_bytes(1));
            match gamepkg::local_record_compare_exchange(id, version, &record) {
                Ok(_) => {}
                Err(-2) => memory::Memory::call_retry_pause()?,
                Err(code) => return Err(code),
            }
        } else {
            let origin = pool_long(&record, 32)?;
            let frequency = pool_long(&record, 40)?;
            let sample = gamepkg::local_performance_sample()?;
            // Original GuestSkewNs is observed after the current counter read.
            // Revalidate publication; IDs/metadata snapshots are not leases.
            let (current, _, after) = pool_services_snapshot(ram)?;
            if current != id
                || pool_long(&after, 32)? != origin
                || pool_long(&after, 40)? != frequency
                || pool_long(&after, 48)? != 1
            {
                return Err(-4);
            }
            if sample.frequency != frequency
                || sample.counter < origin
                || sample.counter > i64::MAX as u64
            {
                return Err(-3);
            }
            return kernel_tsc_value(sample.counter, origin, frequency, pool_long(&after, 24)?);
        }
    }
    Err(-2)
}

// Default reviewed retail geometry, as bridge_va_mapped with map_size=0 and
// total_ram=64MiB. Mirrors/tiled views do not expand that original buffer rule.
pub fn kernel_buffer_mapped(address: u32, bytes: u32) -> bool {
    if bytes == 0 {
        return true;
    }
    let end = u64::from(address) + u64::from(bytes);
    address >= 4096 && (end <= 67108864 || (address >= 0x80000000 && end <= 0x84000000))
}
fn pool_buffer_ticket(ram: &memory::Memory, ordinal: u32) -> Result<bool, i32> {
    let bit = match ordinal {
        107 => 1u32,
        109 => 2u32,
        _ => return Err(-5),
    };
    let pinned = ram.services_handle()?;
    for _ in 0..64u32 {
        let (id, version, mut record) = pool_services_snapshot(ram)?;
        if id != pinned {
            return Err(-4);
        }
        let flags = pool_word(&record, 4)?;
        if flags & bit != 0 {
            return Ok(false);
        }
        record[4..8].copy_from_slice(&u32::to_le_bytes(flags | bit));
        match gamepkg::local_record_compare_exchange(id, version, &record) {
            Ok(_) => return Ok(true),
            Err(-2) => memory::Memory::call_retry_pause()?,
            Err(code) => return Err(code),
        }
    }
    Err(-2)
}
fn pool_dispatcher_initialize(
    ram: &mut memory::Memory,
    ordinal: u32,
    args: &[u8],
) -> Result<u32, i32> {
    let first = pool_word(args, 0)?;
    let second = pool_word(args, 4)?;
    if ordinal == 44 {
        if second != 0 {
            ram.write_u8(second, first as u8)?;
        }
        return Ok(first);
    }
    kernel_service_values(ram)?;
    let size = if ordinal == 107 {
        32u32
    } else if ordinal == 109 {
        44u32
    } else {
        return Err(-5);
    };
    if !kernel_buffer_mapped(first, size) {
        if pool_buffer_ticket(ram, ordinal)? {
            // Original diagnostics are best effort after its seen bit changes.
            let _ = gamepkg::diagnostic(
                2,
                0x544b1700,
                &[u64::from(ordinal), u64::from(first), u64::from(size)],
            );
        }
        return Ok(0);
    }
    if ordinal == 107 {
        ram.write_bytes(first, &[0u8; 32])?;
        ram.write_u16(first, 0x13)?;
        ram.write_u32(first + 12, second)?;
        ram.write_u32(first + 16, pool_word(args, 8)?)?;
    } else {
        ram.write_bytes(first, &[0u8; 44])?;
        ram.write_u32(first, second)?;
        ram.write_u32(first + 4, pool_word(args, 8)?)?;
        ram.write_u32(first + 8, pool_word(args, 12)?)?;
    }
    Ok(0)
}

// Shared port-owned interrupt registry; this remains checked game code.
pub struct Connections {
    root: u64,
    services: u64,
    record: u64,
    creator: u64,
    owned: bool,
}
fn connection_word(b: &[u8], at: usize) -> Result<u64, i32> {
    let s = b.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
    ]))
}
fn connection_live(ram: &memory::Memory, services: u64) -> Result<(), i32> {
    if services == 0 || ram.services_handle()? != services {
        return Err(-4);
    }
    let values = kernel_service_values(ram);
    if ram.services_handle()? != services || gamepkg::local_task_status()?.1 {
        return Err(-4);
    }
    values?;
    Ok(())
}
impl Connections {
    pub fn connections_new(ram: &memory::Memory) -> Result<Self, i32> {
        let services = ram.services_handle()?;
        connection_live(ram, services)?;
        let creator = gamepkg::local_task_status()?.0;
        let mut b = [0u8; 192];
        for (at, value) in [
            (0usize, 0x31525349u64),
            (8, ram.call_handle()),
            (16, services),
        ] {
            let field_bytes = u64::to_le_bytes(value);
            for lane in 0..8usize {
                b[at + lane] = field_bytes[lane];
            }
        }
        let record = gamepkg::local_record_create(&b)?;
        if let Err(code) = connection_live(ram, services) {
            if gamepkg::local_record_release(record).is_err() {
                return Err(-3);
            }
            return Err(code);
        }
        Ok(Self {
            root: ram.call_handle(),
            services,
            record,
            creator,
            owned: true,
        })
    }
    pub fn connections_id(&self) -> u64 {
        self.record
    }
    pub fn connections_closed(&self) -> bool {
        self.record == 0
    }
    pub fn connections_borrow(ram: &memory::Memory, record: u64) -> Result<Self, i32> {
        let view = Self {
            root: ram.call_handle(),
            services: ram.services_handle()?,
            record,
            creator: 0,
            owned: false,
        };
        view.connection_snapshot(ram)?;
        Ok(view)
    }
    fn connection_snapshot(&self, ram: &memory::Memory) -> Result<(u64, std::vec::Vec<u8>), i32> {
        if self.record == 0 || ram.call_handle() != self.root {
            return Err(-4);
        }
        connection_live(ram, self.services)?;
        let (version, b) = gamepkg::local_record_snapshot(self.record, 192)?;
        if b.len() != 192 || connection_word(&b, 0)? != 0x31525349 {
            return Err(-1);
        }
        if connection_word(&b, 8)? != self.root || connection_word(&b, 16)? != self.services {
            return Err(-4);
        }
        for at in [24usize, 32, 40, 48, 56] {
            if connection_word(&b, at)? != 0 {
                return Err(-1);
            }
        }
        connection_live(ram, self.services)?;
        Ok((version, b))
    }
    pub fn connection_get(&self, ram: &memory::Memory, vector_index: u32) -> Result<u32, i32> {
        let (_, b) = self.connection_snapshot(ram)?;
        if vector_index >= 32 {
            return Ok(0);
        }
        let at = 64 + vector_index as usize * 4;
        Ok(u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]]))
    }
    // The original reports connected even for null/out-of-range requests.
    // [return value, vector_index or u32::MAX, routine, context] retains its log data.
    pub fn connection_connect(&self, ram: &memory::Memory, guest: u32) -> Result<[u32; 4], i32> {
        self.connection_snapshot(ram)?;
        if guest == 0 {
            return Ok([1, u32::MAX, 0, 0]);
        }
        let vector_index = ram.read_u32(guest.checked_add(8).ok_or(-1)?)?;
        if vector_index >= 32 {
            return Ok([1, vector_index, 0, 0]);
        }
        let at = 64 + vector_index as usize * 4;
        let encoded = u32::to_le_bytes(guest);
        for _ in 0..64u32 {
            let (version, mut b) = self.connection_snapshot(ram)?;
            for lane in 0..4usize {
                b[at + lane] = encoded[lane];
            }
            match gamepkg::local_record_compare_exchange(self.record, version, &b) {
                Ok(_) => {
                    // Publication preceded these original diagnostic reads.
                    let routine = ram.read_u32(guest)?;
                    let context = ram.read_u32(guest.checked_add(4).ok_or(-1)?)?;
                    connection_live(ram, self.services)?;
                    return Ok([1, vector_index, routine, context]);
                }
                Err(-2) => memory::Memory::call_retry_pause()?,
                Err(code) => return Err(code),
            }
        }
        Err(-2)
    }
    pub fn connections_close(&mut self, ram: &memory::Memory) -> Result<(), i32> {
        if ram.call_handle() != self.root || gamepkg::local_task_status()?.0 != self.creator {
            return Err(-4);
        }
        if self.record == 0 {
            return Ok(());
        }
        if !self.owned {
            return Err(-4);
        }
        let validation = connection_live(ram, self.services);
        gamepkg::local_record_release(self.record)?;
        self.record = 0;
        self.owned = false;
        validation
    }
}

// Explicit KIC1 copied context for context-dependent import calls. No host ABI.
pub struct KernelContext {
    root: u64,
    services: u64,
    record: u64,
    creator: u64,
    owned: bool,
}
impl KernelContext {
    pub fn kernel_context_new(ram: &memory::Memory, registry: &Connections) -> Result<Self, i32> {
        registry.connection_snapshot(ram)?;
        let services = ram.services_handle()?;
        let creator = gamepkg::local_task_status()?.0;
        let mut b = [0u8; 64];
        for (at, value) in [
            (0usize, 0x3143494bu64),
            (8, ram.call_handle()),
            (16, services),
            (24, registry.connections_id()),
        ] {
            let field_bytes = u64::to_le_bytes(value);
            for lane in 0..8usize {
                b[at + lane] = field_bytes[lane];
            }
        }
        let record = gamepkg::local_record_create(&b)?;
        let result = Self {
            root: ram.call_handle(),
            services,
            record,
            creator,
            owned: true,
        };
        if let Err(code) = result.context_snapshot(ram) {
            if gamepkg::local_record_release(record).is_err() {
                return Err(-3);
            }
            return Err(code);
        }
        Ok(result)
    }
    pub fn kernel_context_id(&self) -> u64 {
        self.record
    }
    pub fn kernel_context_closed(&self) -> bool {
        self.record == 0
    }
    pub fn kernel_context_borrow(ram: &memory::Memory, record: u64) -> Result<Self, i32> {
        let view = Self {
            root: ram.call_handle(),
            services: ram.services_handle()?,
            record,
            creator: 0,
            owned: false,
        };
        view.context_snapshot(ram)?;
        Ok(view)
    }
    fn context_snapshot(&self, ram: &memory::Memory) -> Result<u64, i32> {
        if self.record == 0 || ram.call_handle() != self.root {
            return Err(-4);
        }
        connection_live(ram, self.services)?;
        let (_, b) = gamepkg::local_record_snapshot(self.record, 64)?;
        if b.len() != 64 || pool_long(&b, 0)? != 0x3143494b {
            return Err(-1);
        }
        if pool_long(&b, 8)? != self.root || pool_long(&b, 16)? != self.services {
            return Err(-4);
        }
        for at in [32usize, 40, 48, 56] {
            if pool_long(&b, at)? != 0 {
                return Err(-1);
            }
        }
        let registry = pool_long(&b, 24)?;
        if registry == 0 {
            return Err(-1);
        }
        Connections::connections_borrow(ram, registry)?;
        connection_live(ram, self.services)?;
        Ok(registry)
    }
    pub fn kernel_context_registry(&self, ram: &memory::Memory) -> Result<Connections, i32> {
        Connections::connections_borrow(ram, self.context_snapshot(ram)?)
    }
    pub fn kernel_context_close(&mut self, ram: &memory::Memory) -> Result<(), i32> {
        if ram.call_handle() != self.root || gamepkg::local_task_status()?.0 != self.creator {
            return Err(-4);
        }
        if self.record == 0 {
            return Ok(());
        }
        if !self.owned {
            return Err(-4);
        }
        let validation = connection_live(ram, self.services);
        gamepkg::local_record_release(self.record)?;
        self.record = 0;
        self.owned = false;
        validation
    }
}
