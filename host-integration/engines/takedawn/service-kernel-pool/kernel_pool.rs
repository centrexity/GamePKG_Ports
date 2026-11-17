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
        15 | 44 | 113 | 168 | 225 | 337 => Ok(8),
        189 => Ok(16),
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
// Context-dependent imports require a live typed binding, not classification alone.
pub fn kernel_requires_context(ordinal: u32) -> bool {
    ordinal == 98 || ordinal == 113 || ordinal == 189 || ordinal == 225
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
    kernel_context_ready(
        ram,
        if routing_start == 32 {
            pool_long(&record, 24)?
        } else {
            0
        },
        ordinal,
    )?;
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
    if ordinal == 189 || ordinal == 225 {
        let (_, record, _, routing_start) = pool_snapshot(ram)?;
        if routing_start != 32 {
            return Err(-5);
        }
        let context = KernelContext::kernel_context_borrow(ram, pool_long(&record, 24)?)?;
        let ids = context.kernel_context_objects(ram)?;
        let table = Handles::handles_borrow(ram, ids[0])?;
        let domain = Events::events_borrow(ram, &table, ids[1])?;
        let status = if ordinal == 189 {
            // ObjectAttributes is intentionally ignored, as in the original.
            pool_nt_create_event(
                ram,
                &table,
                &domain,
                first,
                pool_word(&args, 8)?,
                pool_word(&args, 12)?,
            )?
        } else {
            pool_nt_set_event(ram, &table, &domain, first, pool_word(&args, 4)?)?
        };
        state.eax = status;
        state.esp = end as u32;
        return Ok(());
    }
    if ordinal == 113 {
        let (_, record, _, routing_start) = pool_snapshot(ram)?;
        if routing_start != 32 {
            return Err(-5);
        }
        let context = KernelContext::kernel_context_borrow(ram, pool_long(&record, 24)?)?;
        let map = context.kernel_context_shadow(ram)?;
        timer_initialize(&map, ram, first, pool_word(&args, 4)?)?;
        state.eax = 0;
        state.esp = end as u32;
        return Ok(());
    }
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
        || ordinal == 113
        || ordinal == 189
        || ordinal == 225
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

// Explicit KIC1/KIC2 copied context for context-dependent import calls. No host ABI.
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
    pub fn kernel_context_new_with_shadow(
        ram: &memory::Memory,
        registry: &Connections,
        map: &Shadow,
    ) -> Result<Self, i32> {
        registry.connection_snapshot(ram)?;
        map.shadow_snapshot(ram)?;
        Shadow::shadow_borrow(ram, map.shadow_id())?;
        let services = ram.services_handle()?;
        let creator = gamepkg::local_task_status()?.0;
        let mut b = [0u8; 72];
        for (at, value) in [
            (0usize, 0x3243494bu64),
            (8, ram.call_handle()),
            (16, services),
            (24, registry.connections_id()),
            (32, map.shadow_id()),
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
    pub fn kernel_context_new_with_objects(
        ram: &memory::Memory,
        registry: &Connections,
        map: &Shadow,
        table: u64,
        event_domain: u64,
    ) -> Result<Self, i32> {
        registry.connection_snapshot(ram)?;
        map.shadow_snapshot(ram)?;
        let services = ram.services_handle()?;
        let creator = gamepkg::local_task_status()?.0;
        let mut b = [0u8; 80];
        for (at, value) in [
            (0usize, 0x3343494bu64),
            (8, ram.call_handle()),
            (16, services),
            (24, registry.connections_id()),
            (32, map.shadow_id()),
            (40, table),
            (48, event_domain),
        ] {
            let encoded = u64::to_le_bytes(value);
            for lane in 0..8usize {
                b[at + lane] = encoded[lane];
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
        let (_, b) = gamepkg::local_record_snapshot(self.record, 80)?;
        let version = if b.len() == 64 && pool_long(&b, 0)? == 0x3143494b {
            1
        } else if b.len() == 72 && pool_long(&b, 0)? == 0x3243494b {
            2
        } else if b.len() == 80 && pool_long(&b, 0)? == 0x3343494b {
            3
        } else {
            return Err(-1);
        };
        if pool_long(&b, 8)? != self.root || pool_long(&b, 16)? != self.services {
            return Err(-4);
        }
        let reserved = if version == 1 {
            32usize
        } else if version == 2 {
            40
        } else {
            56
        };
        for at in (reserved..b.len()).step_by(8) {
            if pool_long(&b, at)? != 0 {
                return Err(-1);
            }
        }
        let registry = pool_long(&b, 24)?;
        if registry == 0 {
            return Err(-1);
        }
        Connections::connections_borrow(ram, registry)?;
        if version >= 2 {
            let map = pool_long(&b, 32)?;
            if map == 0 {
                return Err(-1);
            }
            Shadow::shadow_borrow(ram, map)?;
        }
        if version == 3 {
            self.context_object_records(ram, &b)?;
        }
        connection_live(ram, self.services)?;
        Ok(registry)
    }
    // Validate copied structural bindings, not native authority or resource leases.
    // The typed consumer must still borrow the full game-owned table/domain and
    // validate actual SDK kind/liveness when it uses an event.
    fn context_object_records(&self, ram: &memory::Memory, b: &[u8]) -> Result<[u64; 2], i32> {
        let table = pool_long(b, 40)?;
        let domain = pool_long(b, 48)?;
        if table == 0 || domain == 0 || table == domain {
            return Err(-1);
        }
        let (_, t) = gamepkg::local_record_snapshot(table, 80)?;
        if t.len() != 80
            || pool_long(&t, 0)? != 0x32525448
            || pool_long(&t, 40)? != 2
            || pool_long(&t, 64)? != 16
            || pool_long(&t, 72)? != 0
        {
            return Err(-1);
        }
        if pool_long(&t, 8)? != self.root
            || pool_long(&t, 16)? != self.services
            || pool_long(&t, 24)? == 0
            || pool_long(&t, 48)? == 0
            || pool_long(&t, 56)? % 262144 != 0
            || pool_long(&t, 56)? > 786432
        {
            return Err(-4);
        }
        let (_, d) = gamepkg::local_record_snapshot(domain, 2624)?;
        if d.len() != 2624
            || pool_long(&d, 0)? != 0x32445645
            || pool_long(&d, 40)? != 0
            || pool_long(&d, 48)? != 0
            || pool_long(&d, 56)? != 0
        {
            return Err(-1);
        }
        if pool_long(&d, 8)? != self.root
            || pool_long(&d, 16)? != self.services
            || pool_long(&d, 24)? != table
            || pool_long(&d, 32)? == u64::MAX
        {
            return Err(-4);
        }
        connection_live(ram, self.services)?;
        Ok([table, domain])
    }
    pub fn kernel_context_registry(&self, ram: &memory::Memory) -> Result<Connections, i32> {
        Connections::connections_borrow(ram, self.context_snapshot(ram)?)
    }
    pub fn kernel_context_shadow(&self, ram: &memory::Memory) -> Result<Shadow, i32> {
        self.context_snapshot(ram)?;
        let (_, b) = gamepkg::local_record_snapshot(self.record, 80)?;
        if !((b.len() == 72 && pool_long(&b, 0)? == 0x3243494b)
            || (b.len() == 80 && pool_long(&b, 0)? == 0x3343494b))
        {
            return Err(-5);
        }
        Shadow::shadow_borrow(ram, pool_long(&b, 32)?)
    }
    pub fn kernel_context_objects(&self, ram: &memory::Memory) -> Result<[u64; 2], i32> {
        self.context_snapshot(ram)?;
        let (_, b) = gamepkg::local_record_snapshot(self.record, 80)?;
        if b.len() != 80 || pool_long(&b, 0)? != 0x3343494b {
            return Err(-5);
        }
        self.context_object_records(ram, &b)
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

// Port-owned 4096-entry guest-to-opaque-token map. Tokens confer no native authority.
pub struct Shadow {
    root: u64,
    services: u64,
    record: u64,
    lock: u64,
    creator: u64,
    owned: bool,
    cleaning: bool,
    pages: std::vec::Vec<u64>,
}
pub struct Guard {
    root: u64,
    services: u64,
    record: u64,
    lock: u64,
    task: u64,
    held: bool,
    cleanup: bool,
}
fn shadow_word(bytes: &[u8], at: usize) -> Result<u64, i32> {
    let b = bytes.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}
fn shadow_u32(bytes: &[u8], at: usize) -> Result<u32, i32> {
    let b = bytes.get(at..at + 4).ok_or(-1)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}
fn shadow_put(bytes: &mut [u8], at: usize, value: u64) -> Result<(), i32> {
    if at.checked_add(8).ok_or(-1)? > bytes.len() {
        return Err(-1);
    }
    let encoded = u64::to_le_bytes(value);
    for lane in 0..8usize {
        bytes[at + lane] = encoded[lane];
    }
    Ok(())
}
fn shadow_put32(bytes: &mut [u8], at: usize, value: u32) -> Result<(), i32> {
    if at.checked_add(4).ok_or(-1)? > bytes.len() {
        return Err(-1);
    }
    let encoded = u32::to_le_bytes(value);
    for lane in 0..4usize {
        bytes[at + lane] = encoded[lane];
    }
    Ok(())
}
fn shadow_owner(ram: &memory::Memory, services: u64) -> Result<(), i32> {
    if services == 0 || ram.services_handle()? != services {
        return Err(-4);
    }
    let values = kernel_service_values(ram);
    if ram.services_handle()? != services {
        return Err(-4);
    }
    values?;
    if gamepkg::local_task_status()?.1 {
        return Err(-4);
    }
    Ok(())
}
fn shadow_root(
    record: u64,
    root: u64,
    services: u64,
    lock: u64,
) -> Result<(u64, std::vec::Vec<u8>), i32> {
    let (version, bytes) = gamepkg::local_record_snapshot(record, 152)?;
    if bytes.len() != 152 || shadow_word(&bytes, 0)? != 0x3153484b || shadow_word(&bytes, 40)? > 1 {
        return Err(-1);
    }
    if shadow_word(&bytes, 8)? != root
        || shadow_word(&bytes, 16)? != services
        || shadow_word(&bytes, 24)? != lock
        || root == 0
        || services == 0
        || lock == 0
    {
        return Err(-4);
    }
    let mut absent = false;
    for index in 0..13usize {
        let id = shadow_word(&bytes, 48 + index * 8)?;
        if id == 0 {
            absent = true;
        } else {
            if absent {
                return Err(-1);
            }
            for previous in 0..index {
                if shadow_word(&bytes, 48 + previous * 8)? == id {
                    return Err(-1);
                }
            }
        }
    }
    Ok((version, bytes))
}
fn shadow_publish(record: u64, version: u64, bytes: &[u8]) -> Result<(), i32> {
    match gamepkg::local_record_compare_exchange(record, version, bytes) {
        Ok(_) => Ok(()),
        Err(-2) => Err(-3),
        Err(code) => Err(code),
    }
}
fn shadow_page(record: u64, page: u64, index: usize) -> Result<(u64, std::vec::Vec<u8>), i32> {
    if index >= 13 || page == 0 {
        return Err(-1);
    }
    let count = if index == 12 { 16usize } else { 340 };
    let (version, bytes) = gamepkg::local_record_snapshot(page, 4096)?;
    if bytes.len() != 16 + count * 12
        || shadow_u32(&bytes, 0)? != 0x3150534b
        || shadow_u32(&bytes, 4)? != index as u32
    {
        return Err(-1);
    }
    if shadow_word(&bytes, 8)? != record {
        return Err(-4);
    }
    for slot in 0..count {
        if (shadow_u32(&bytes, 16 + slot * 12)? == 0) != (shadow_word(&bytes, 20 + slot * 12)? == 0)
        {
            return Err(-1);
        }
    }
    Ok((version, bytes))
}
fn shadow_unlock(lock: u64) -> Result<(), i32> {
    if gamepkg::local_call(24, &u64::to_le_bytes(lock), &mut [])? != 0 {
        return Err(-3);
    }
    Ok(())
}
impl Shadow {
    pub fn shadow_new(ram: &memory::Memory) -> Result<Self, i32> {
        let services = ram.services_handle()?;
        shadow_owner(ram, services)?;
        let creator = gamepkg::local_task_status()?.0;
        let mut out = [0u8; 8];
        if gamepkg::local_call(21, &[], &mut out)? != 8 {
            return Err(-3);
        }
        let lock = u64::from_le_bytes(out);
        let mut bytes = std::vec::Vec::new();
        bytes.resize(152, 0);
        for (at, value) in [
            (0usize, 0x3153484bu64),
            (8, ram.call_handle()),
            (16, services),
            (24, lock),
        ] {
            shadow_put(&mut bytes, at, value)?;
        }
        let record = match gamepkg::local_record_create(&bytes) {
            Ok(value) => value,
            Err(code) => {
                let _ = gamepkg::local_call(25, &u64::to_le_bytes(lock), &mut []);
                return Err(code);
            }
        };
        Ok(Self {
            root: ram.call_handle(),
            services,
            record,
            lock,
            creator,
            owned: true,
            cleaning: false,
            pages: std::vec::Vec::new(),
        })
    }
    pub fn shadow_id(&self) -> u64 {
        self.record
    }
    pub fn shadow_closed(&self) -> bool {
        self.record == 0 && self.lock == 0
    }
    pub fn shadow_borrow(ram: &memory::Memory, record: u64) -> Result<Self, i32> {
        let (_, bytes) = gamepkg::local_record_snapshot(record, 152)?;
        if bytes.len() != 152 {
            return Err(-1);
        }
        let view = Self {
            root: ram.call_handle(),
            services: ram.services_handle()?,
            record,
            lock: shadow_word(&bytes, 24)?,
            creator: 0,
            owned: false,
            cleaning: false,
            pages: std::vec::Vec::new(),
        };
        view.shadow_snapshot(ram)?;
        match gamepkg::local_mutex_lock(view.lock, 0) {
            Ok(()) => shadow_unlock(view.lock)?,
            Err(-2) => {}
            Err(code) => return Err(code),
        }
        Ok(view)
    }
    fn shadow_snapshot(&self, ram: &memory::Memory) -> Result<(u64, std::vec::Vec<u8>), i32> {
        if self.record == 0 || self.root != ram.call_handle() {
            return Err(-4);
        }
        shadow_owner(ram, self.services)?;
        let result = shadow_root(self.record, self.root, self.services, self.lock)?;
        if shadow_word(&result.1, 40)? != 0 {
            return Err(-4);
        }
        Ok(result)
    }
    pub fn shadow_lock(&self, ram: &memory::Memory) -> Result<Guard, i32> {
        self.shadow_snapshot(ram)?;
        let task = gamepkg::local_task_status()?.0;
        gamepkg::local_mutex_lock(self.lock, 1000)?;
        let result = self.shadow_enter(ram);
        match result {
            Ok(()) => Ok(Guard {
                root: self.root,
                services: self.services,
                record: self.record,
                lock: self.lock,
                task,
                held: true,
                cleanup: false,
            }),
            Err(code) => {
                let _ = shadow_unlock(self.lock);
                Err(code)
            }
        }
    }
    // Creator-only deletion lane after service revocation. It cannot insert
    // tokens and still requires the actual mutex and root/schema identities.
    pub fn shadow_cleanup_lock(&self, ram: &memory::Memory) -> Result<Guard, i32> {
        let (task, cancelled) = gamepkg::local_task_status()?;
        if !self.owned || self.root != ram.call_handle() || self.creator != task || cancelled {
            return Err(-4);
        }
        gamepkg::local_mutex_lock(self.lock, 1000)?;
        let result = self.shadow_cleanup_enter();
        match result {
            Ok(()) => Ok(Guard {
                root: self.root,
                services: self.services,
                record: self.record,
                lock: self.lock,
                task,
                held: true,
                cleanup: true,
            }),
            Err(code) => {
                let _ = shadow_unlock(self.lock);
                Err(code)
            }
        }
    }
    fn shadow_cleanup_enter(&self) -> Result<(), i32> {
        let (version, mut bytes) = shadow_root(self.record, self.root, self.services, self.lock)?;
        if shadow_word(&bytes, 40)? != 0 {
            return Err(-4);
        }
        let active = shadow_word(&bytes, 32)?.checked_add(1).ok_or(-2)?;
        shadow_put(&mut bytes, 32, active)?;
        shadow_publish(self.record, version, &bytes)
    }
    fn shadow_enter(&self, ram: &memory::Memory) -> Result<(), i32> {
        let (version, mut bytes) = self.shadow_snapshot(ram)?;
        let active = shadow_word(&bytes, 32)?.checked_add(1).ok_or(-2)?;
        shadow_put(&mut bytes, 32, active)?;
        shadow_publish(self.record, version, &bytes)
    }
    pub fn shadow_close(&mut self, ram: &memory::Memory) -> Result<(), i32> {
        if self.shadow_closed() {
            return Ok(());
        }
        let (task, cancelled) = gamepkg::local_task_status()?;
        if !self.owned || self.root != ram.call_handle() || task != self.creator || cancelled {
            return Err(-4);
        }
        let validation = shadow_owner(ram, self.services);
        gamepkg::local_mutex_lock(self.lock, 0)?;
        let result = self.shadow_cleanup();
        let unlocked = shadow_unlock(self.lock);
        result?;
        unlocked?;
        gamepkg::local_call(25, &u64::to_le_bytes(self.lock), &mut [])?;
        self.lock = 0;
        self.owned = false;
        validation?;
        Ok(())
    }
    fn shadow_cleanup(&mut self) -> Result<(), i32> {
        if !self.cleaning {
            let (version, mut bytes) =
                shadow_root(self.record, self.root, self.services, self.lock)?;
            if shadow_word(&bytes, 32)? != 0 {
                return Err(-2);
            }
            if shadow_word(&bytes, 40)? != 0 {
                return Err(-4);
            }
            let mut pages = std::vec::Vec::new();
            for index in 0..13usize {
                let page = shadow_word(&bytes, 48 + index * 8)?;
                if page != 0 {
                    let (_, contents) = shadow_page(self.record, page, index)?;
                    for slot in 0..if index == 12 { 16usize } else { 340 } {
                        if shadow_u32(&contents, 16 + slot * 12)? != 0 {
                            return Err(-2);
                        }
                    }
                    pages.push(page);
                }
            }
            shadow_put(&mut bytes, 40, 1)?;
            shadow_publish(self.record, version, &bytes)?;
            self.pages = pages;
            self.cleaning = true;
        }
        for page in &mut self.pages {
            if *page != 0 {
                match gamepkg::local_record_release(*page) {
                    Ok(()) | Err(-4) => *page = 0,
                    Err(code) => return Err(code),
                }
            }
        }
        if self.record != 0 {
            match gamepkg::local_record_release(self.record) {
                Ok(()) | Err(-4) => self.record = 0,
                Err(code) => return Err(code),
            }
        }
        Ok(())
    }
}
impl Guard {
    fn shadow_guard_owner(&self, ram: &memory::Memory) -> Result<(), i32> {
        if !self.held
            || self.root != ram.call_handle()
            || gamepkg::local_task_status()?.0 != self.task
        {
            return Err(-4);
        }
        Ok(())
    }
    fn shadow_guard_snapshot(&self, ram: &memory::Memory) -> Result<(u64, std::vec::Vec<u8>), i32> {
        self.shadow_guard_owner(ram)?;
        if !self.cleanup {
            shadow_owner(ram, self.services)?;
        }
        let result = shadow_root(self.record, self.root, self.services, self.lock)?;
        if shadow_word(&result.1, 40)? != 0 || shadow_word(&result.1, 32)? == 0 {
            return Err(-4);
        }
        Ok(result)
    }
    // [page index, slot index, old token]. A zero old token is the first free slot.
    fn shadow_find(&self, ram: &memory::Memory, guest: u32) -> Result<[u64; 3], i32> {
        self.shadow_guard_snapshot(ram)?;
        if guest == 0 {
            return Err(-1);
        }
        let (_, root) = shadow_root(self.record, self.root, self.services, self.lock)?;
        let mut free = [13u64, 0, 0];
        for index in 0..13usize {
            let page = shadow_word(&root, 48 + index * 8)?;
            if page == 0 {
                if free[0] == 13 {
                    free = [index as u64, 0, 0];
                }
                continue;
            }
            let (_, bytes) = shadow_page(self.record, page, index)?;
            for slot in 0..if index == 12 { 16usize } else { 340 } {
                let va = shadow_u32(&bytes, 16 + slot * 12)?;
                if va == guest {
                    return Ok([
                        index as u64,
                        slot as u64,
                        shadow_word(&bytes, 20 + slot * 12)?,
                    ]);
                }
                if va == 0 && free[0] == 13 {
                    free = [index as u64, slot as u64, 0];
                }
            }
        }
        Ok(free)
    }
    pub fn shadow_lookup(&self, ram: &memory::Memory, guest: u32) -> Result<Option<u64>, i32> {
        self.shadow_guard_snapshot(ram)?;
        if guest == 0 {
            return Ok(None);
        }
        let found = self.shadow_find(ram, guest)?;
        if found[2] == 0 {
            Ok(None)
        } else {
            Ok(Some(found[2]))
        }
    }
    // This only publishes port metadata. The typed caller closes a displaced
    // resource before publication, while this guard remains held.
    pub fn shadow_insert(
        &self,
        ram: &memory::Memory,
        guest: u32,
        token: u64,
    ) -> Result<Option<u64>, i32> {
        if self.cleanup {
            return Err(-4);
        }
        if token == 0 {
            return Err(-1);
        }
        let found = self.shadow_find(ram, guest)?;
        if found[0] == 13 {
            return Err(-2);
        }
        let index = found[0] as usize;
        let slot = found[1] as usize;
        let (root_version, mut root) = self.shadow_guard_snapshot(ram)?;
        let mut page = shadow_word(&root, 48 + index * 8)?;
        if page == 0 {
            let mut bytes = std::vec::Vec::new();
            bytes.resize(if index == 12 { 208 } else { 4096 }, 0);
            shadow_put32(&mut bytes, 0, 0x3150534b)?;
            shadow_put32(&mut bytes, 4, index as u32)?;
            shadow_put(&mut bytes, 8, self.record)?;
            page = gamepkg::local_record_create(&bytes)?;
            shadow_put(&mut root, 48 + index * 8, page)?;
            if let Err(code) = shadow_publish(self.record, root_version, &root) {
                let _ = gamepkg::local_record_release(page);
                return Err(code);
            }
        }
        let (version, mut bytes) = shadow_page(self.record, page, index)?;
        if shadow_u32(&bytes, 16 + slot * 12)? != if found[2] == 0 { 0 } else { guest }
            || shadow_word(&bytes, 20 + slot * 12)? != found[2]
        {
            return Err(-3);
        }
        shadow_put32(&mut bytes, 16 + slot * 12, guest)?;
        shadow_put(&mut bytes, 20 + slot * 12, token)?;
        shadow_publish(page, version, &bytes)?;
        if found[2] == 0 {
            Ok(None)
        } else {
            Ok(Some(found[2]))
        }
    }
    pub fn shadow_remove(&self, ram: &memory::Memory, guest: u32) -> Result<Option<u64>, i32> {
        self.shadow_guard_snapshot(ram)?;
        if guest == 0 {
            return Ok(None);
        }
        let found = self.shadow_find(ram, guest)?;
        if found[2] == 0 {
            return Ok(None);
        }
        let (_, root) = self.shadow_guard_snapshot(ram)?;
        let index = found[0] as usize;
        let slot = found[1] as usize;
        let page = shadow_word(&root, 48 + index * 8)?;
        let (version, mut bytes) = shadow_page(self.record, page, index)?;
        if shadow_u32(&bytes, 16 + slot * 12)? != guest
            || shadow_word(&bytes, 20 + slot * 12)? != found[2]
        {
            return Err(-3);
        }
        shadow_put32(&mut bytes, 16 + slot * 12, 0)?;
        shadow_put(&mut bytes, 20 + slot * 12, 0)?;
        shadow_publish(page, version, &bytes)?;
        Ok(Some(found[2]))
    }
    pub fn shadow_entries(&self, ram: &memory::Memory) -> Result<std::vec::Vec<[u64; 2]>, i32> {
        let (_, root) = self.shadow_guard_snapshot(ram)?;
        let mut result = std::vec::Vec::new();
        for index in 0..13usize {
            let page = shadow_word(&root, 48 + index * 8)?;
            if page == 0 {
                continue;
            }
            let (_, bytes) = shadow_page(self.record, page, index)?;
            for slot in 0..if index == 12 { 16usize } else { 340 } {
                let guest = shadow_u32(&bytes, 16 + slot * 12)?;
                if guest != 0 {
                    result.push([u64::from(guest), shadow_word(&bytes, 20 + slot * 12)?]);
                }
            }
        }
        Ok(result)
    }
    // Copied slot order, including holes. Values are port metadata, not
    // permission to use an arbitrary SDK handle or native address.
    pub fn shadow_slots(&self, ram: &memory::Memory) -> Result<std::vec::Vec<[u64; 2]>, i32> {
        let (_, root) = self.shadow_guard_snapshot(ram)?;
        let mut result = std::vec::Vec::new();
        for index in 0..13usize {
            let count = if index == 12 { 16usize } else { 340 };
            let page = shadow_word(&root, 48 + index * 8)?;
            if page == 0 {
                for _ in 0..count {
                    result.push([0, 0]);
                }
            } else {
                let (_, bytes) = shadow_page(self.record, page, index)?;
                for slot in 0..count {
                    result.push([
                        u64::from(shadow_u32(&bytes, 16 + slot * 12)?),
                        shadow_word(&bytes, 20 + slot * 12)?,
                    ]);
                }
            }
        }
        Ok(result)
    }
    pub fn shadow_guard_close(&mut self, ram: &memory::Memory) -> Result<(), i32> {
        if !self.held {
            return Ok(());
        }
        self.shadow_guard_owner(ram)?;
        let result = self.shadow_leave();
        let unlocked = shadow_unlock(self.lock);
        if unlocked.is_ok() {
            self.held = false;
        }
        result?;
        unlocked?;
        Ok(())
    }
    fn shadow_leave(&self) -> Result<(), i32> {
        let (version, mut root) = shadow_root(self.record, self.root, self.services, self.lock)?;
        let active = shadow_word(&root, 32)?;
        if active == 0 {
            return Err(-3);
        }
        shadow_put(&mut root, 32, active - 1)?;
        shadow_publish(self.record, version, &root)
    }
    pub fn shadow_guard_closed(&self) -> bool {
        !self.held
    }
}

// Port-owned event/timer headers and shadow order. Native events remain SDK-owned.
fn event_signal(id: u64, signal: bool) -> Result<(), i32> {
    if gamepkg::local_call(
        22,
        &[
            u64::to_le_bytes(id).as_slice(),
            &u32::to_le_bytes(u32::from(signal)),
        ]
        .concat(),
        &mut [],
    )? != 0
    {
        return Err(-3);
    }
    Ok(())
}
fn event_release(id: u64) -> Result<(), i32> {
    if gamepkg::local_call(25, &u64::to_le_bytes(id), &mut [])? != 0 {
        return Err(-3);
    }
    Ok(())
}
fn event_native_new(manual: bool, signal: bool) -> Result<u64, i32> {
    let mut out = [0u8; 8];
    if gamepkg::local_call(
        20,
        &[
            u32::to_le_bytes(u32::from(manual)),
            u32::to_le_bytes(u32::from(signal)),
        ]
        .concat(),
        &mut out,
    )? != 8
    {
        return Err(-3);
    }
    let id = u64::from_le_bytes(out);
    Ok(id)
}
fn event_create_locked(
    guard: &Guard,
    ram: &memory::Memory,
    guest: u32,
    manual: bool,
    signal: bool,
) -> Result<u64, i32> {
    let id = event_native_new(manual, signal)?;
    let result = event_publish_locked(guard, ram, guest, id);
    if let Err(code) = result {
        let _ = event_release(id);
        return Err(code);
    }
    Ok(id)
}
fn event_publish_locked(
    guard: &Guard,
    ram: &memory::Memory,
    guest: u32,
    id: u64,
) -> Result<(), i32> {
    if let Some(old) = guard.shadow_lookup(ram, guest)? {
        // Never close an unrelated mutex/task token presented as an event.
        gamepkg::local_event_state(old)?;
        event_release(old)?;
    }
    // A failure here is an explicit fatal partial prefix: an old event may
    // already be revoked. The caller must stop/report, not blindly retry.
    guard.shadow_insert(ram, guest, id)?;
    Ok(())
}
pub fn event_replace(
    map: &Shadow,
    ram: &memory::Memory,
    guest: u32,
    manual: bool,
    signal: bool,
) -> Result<u64, i32> {
    if guest == 0 {
        return Err(-1);
    }
    // The original initializer creates before acquiring the shadow-table lock.
    let id = event_native_new(manual, signal)?;
    let mut guard = match map.shadow_lock(ram) {
        Ok(value) => value,
        Err(code) => {
            let _ = event_release(id);
            return Err(code);
        }
    };
    let result = event_publish_locked(&guard, ram, guest, id);
    let closed = guard.shadow_guard_close(ram);
    if let Err(code) = result {
        let _ = event_release(id);
        return Err(code);
    }
    closed?;
    Ok(id)
}

pub fn event_lookup(map: &Shadow, ram: &memory::Memory, guest: u32) -> Result<Option<u64>, i32> {
    let mut guard = map.shadow_lock(ram)?;
    let result = guard.shadow_lookup(ram, guest);
    let closed = guard.shadow_guard_close(ram);
    let value = result?;
    closed?;
    if let Some(id) = value {
        gamepkg::local_event_state(id)?;
    }
    Ok(value)
}
pub fn event_for(map: &Shadow, ram: &memory::Memory, guest: u32) -> Result<Option<u64>, i32> {
    if let Some(id) = event_lookup(map, ram, guest)? {
        return Ok(Some(id));
    }
    if guest == 0 {
        return Ok(None);
    }
    let mut header = [0u8; 16];
    ram.read_bytes(guest, &mut header)?;
    let kind = header[0];
    if kind > 1 {
        return Ok(None);
    }
    let mut guard = map.shadow_lock(ram)?;
    let result = event_lazy_locked(&guard, ram, guest, kind);
    let closed = guard.shadow_guard_close(ram);
    let id = result?;
    closed?;
    Ok(Some(id))
}
fn event_lazy_locked(
    guard: &Guard,
    ram: &memory::Memory,
    guest: u32,
    kind: u8,
) -> Result<u64, i32> {
    if let Some(id) = guard.shadow_lookup(ram, guest)? {
        gamepkg::local_event_state(id)?;
        return Ok(id);
    }
    // Original Type is sampled before taking the map lock; SignalState is read
    // only here. Recheck the mapping so concurrent first users share one event.
    let signal = ram.read_u32(guest.checked_add(4).ok_or(-1)?)? != 0;
    event_create_locked(guard, ram, guest, kind == 0, signal)
}
pub fn event_initialize(
    map: &Shadow,
    ram: &mut memory::Memory,
    guest: u32,
    kind: u32,
    initial: u32,
) -> Result<(), i32> {
    if guest == 0 || guest.checked_add(15).is_none() {
        return Err(-1);
    }
    event_replace(map, ram, guest, kind == 0, initial != 0)?;
    // The mapping is published before the original guest-header writes.
    ram.write_u8(guest, u8::from(kind != 0))?;
    ram.write_u8(guest + 1, 0)?;
    ram.write_u8(guest + 2, 16)?;
    ram.write_u8(guest + 3, 1)?;
    ram.write_u32(guest + 4, u32::from(initial != 0))?;
    ram.write_u32(guest + 8, 0)?;
    ram.write_u32(guest + 12, 0)?;
    Ok(())
}
pub fn timer_initialize(
    map: &Shadow,
    ram: &mut memory::Memory,
    guest: u32,
    kind: u32,
) -> Result<(), i32> {
    if guest == 0 {
        return Ok(());
    }
    if guest.checked_add(39).is_none() {
        return Err(-1);
    }
    // Original timer bridge writes its 40-byte guest object before allocating
    // an event. A failed allocation leaves that prefix and any old mapping.
    ram.fill(guest, 0, 40)?;
    ram.write_u8(guest, 8 + (kind & 1) as u8)?;
    ram.write_u8(guest + 2, 40)?;
    event_replace(map, ram, guest, kind == 0, false)?;
    Ok(())
}
pub fn event_set(map: &Shadow, ram: &memory::Memory, guest: u32) -> Result<bool, i32> {
    match event_for(map, ram, guest)? {
        Some(id) => {
            event_signal(id, true)?;
            Ok(true)
        }
        None => Ok(false),
    }
}
pub fn event_reset(map: &Shadow, ram: &memory::Memory, guest: u32) -> Result<bool, i32> {
    match event_for(map, ram, guest)? {
        Some(id) => {
            event_signal(id, false)?;
            Ok(true)
        }
        None => Ok(false),
    }
}
pub fn event_wait(
    map: &Shadow,
    ram: &memory::Memory,
    guest: u32,
    milliseconds: u32,
) -> Result<(), i32> {
    let id = event_for(map, ram, guest)?.ok_or(-5)?;
    gamepkg::local_event_wait(id, milliseconds)
}
pub fn timer_reset_after_set(map: &Shadow, ram: &memory::Memory, guest: u32) -> Result<bool, i32> {
    match event_lookup(map, ram, guest)? {
        Some(id) => {
            event_signal(id, false)?;
            Ok(true)
        }
        None => Ok(false),
    }
}
pub fn timer_signal_after_callback(
    map: &Shadow,
    ram: &mut memory::Memory,
    guest: u32,
) -> Result<bool, i32> {
    match event_lookup(map, ram, guest)? {
        Some(id) => {
            event_signal(id, true)?;
            ram.write_u32(guest.checked_add(4).ok_or(-1)?, 1)?;
            Ok(true)
        }
        None => Ok(false),
    }
}
// Ordinary removal is available to live port callers. Owner-only shutdown
// below retains a deletion lane after KSV3 revocation; neither bypasses kind checks.
pub fn event_remove(map: &Shadow, ram: &memory::Memory, guest: u32) -> Result<bool, i32> {
    let mut guard = map.shadow_lock(ram)?;
    let result = event_remove_locked(&guard, ram, guest);
    let closed = guard.shadow_guard_close(ram);
    let value = result?;
    closed?;
    Ok(value)
}
fn event_remove_locked(guard: &Guard, ram: &memory::Memory, guest: u32) -> Result<bool, i32> {
    let id = match guard.shadow_lookup(ram, guest)? {
        Some(id) => id,
        None => return Ok(false),
    };
    let missing = match gamepkg::local_event_state(id) {
        Ok(_) => false,
        Err(-4) => true,
        Err(code) => return Err(code),
    };
    if !missing {
        event_release(id)?;
    }
    if guard.shadow_remove(ram, guest)? != Some(id) {
        return Err(-3);
    }
    if missing {
        return Err(-4);
    }
    Ok(true)
}
pub fn event_close_all(map: &Shadow, ram: &memory::Memory) -> Result<(), i32> {
    let mut guard = map.shadow_cleanup_lock(ram)?;
    let result = event_close_locked(&guard, ram);
    let closed = guard.shadow_guard_close(ram);
    result?;
    closed?;
    Ok(())
}
fn event_close_locked(guard: &Guard, ram: &memory::Memory) -> Result<(), i32> {
    let entries = guard.shadow_entries(ram)?;
    let mut missing = false;
    for entry in entries {
        match event_remove_locked(guard, ram, entry[0] as u32) {
            Ok(true) => {}
            Err(-4) => missing = true,
            Ok(false) => return Err(-3),
            Err(code) => return Err(code),
        }
    }
    if missing {
        return Err(-4);
    }
    Ok(())
}

// Classification is deliberately separate from per-import binding readiness.
pub fn kernel_context_ready(ram: &memory::Memory, context: u64, ordinal: u32) -> Result<(), i32> {
    if !kernel_requires_context(ordinal) {
        return Ok(());
    }
    if context == 0 {
        return Err(-5);
    }
    let bound = KernelContext::kernel_context_borrow(ram, context)?;
    if ordinal == 113 {
        bound.kernel_context_shadow(ram)?;
    }
    if ordinal == 189 || ordinal == 225 {
        let ids = bound.kernel_context_objects(ram)?;
        let table = Handles::handles_borrow(ram, ids[0])?;
        Events::events_borrow(ram, &table, ids[1])?;
    }
    Ok(())
}

// Shared checked game-owned object types. Every object remains audited game code.
// Checked game metadata over bounded GamePKG scratch. No native handle authority.
pub struct Handles {
    root: u64,
    services: u64,
    record: u64,
    lock: u64,
    creator: u64,
    window: u64,
    offset: u64,
    initialized: u64,
    phase: u64,
    close_error: i32,
}
pub struct HandleGuard {
    root: u64,
    services: u64,
    record: u64,
    lock: u64,
    task: u64,
    window: u64,
    offset: u64,
    held: bool,
    cleanup: bool,
}
fn handles_word(b: &[u8], at: usize) -> Result<u64, i32> {
    let p = b.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        p[0], p[1], p[2], p[3], p[4], p[5], p[6], p[7],
    ]))
}
fn handles_u32(b: &[u8], at: usize) -> Result<u32, i32> {
    let p = b.get(at..at + 4).ok_or(-1)?;
    Ok(u32::from_le_bytes([p[0], p[1], p[2], p[3]]))
}
fn handles_put(b: &mut [u8], at: usize, value: u64) -> Result<(), i32> {
    let p = b.get_mut(at..at + 8).ok_or(-1)?;
    p.copy_from_slice(&u64::to_le_bytes(value));
    Ok(())
}
fn handles_put32(b: &mut [u8], at: usize, value: u32) -> Result<(), i32> {
    let p = b.get_mut(at..at + 4).ok_or(-1)?;
    p.copy_from_slice(&u32::to_le_bytes(value));
    Ok(())
}
fn handles_owner(ram: &memory::Memory, services: u64) -> Result<(), i32> {
    if services == 0 || ram.services_handle()? != services {
        return Err(-4);
    }
    let values = kernel_service_values(ram);
    if ram.services_handle()? != services {
        return Err(-4);
    }
    values?;
    if gamepkg::local_task_status()?.1 {
        return Err(-4);
    }
    Ok(())
}
fn handles_scratch() -> Result<u64, i32> {
    let mut b = [0u8; 64];
    if gamepkg::local_call(1, &[], &mut b)? != 64 {
        return Err(-5);
    }
    let mut ids = [0u64; 4];
    for index in 0..4usize {
        ids[index] = handles_word(&b, index * 16)?;
        if ids[index] == 0
            || handles_word(&b, index * 16 + 8)? != if index < 2 { 67108864 } else { 1048576 }
        {
            return Err(-1);
        }
        for before in 0..index {
            if ids[before] == ids[index] {
                return Err(-1);
            }
        }
    }
    Ok(ids[3])
}
fn handles_publish(record: u64, version: u64, b: &[u8]) -> Result<(), i32> {
    match gamepkg::local_record_compare_exchange(record, version, b) {
        Ok(_) => Ok(()),
        Err(-2) => Err(-3),
        Err(code) => Err(code),
    }
}
fn handles_root(
    record: u64,
    root: u64,
    services: u64,
    lock: u64,
) -> Result<(u64, std::vec::Vec<u8>), i32> {
    let (version, b) = gamepkg::local_record_snapshot(record, 80)?;
    if b.len() != 80
        || handles_word(&b, 0)? != 0x32525448
        || handles_word(&b, 40)? > 3
        || handles_word(&b, 64)? > 16
        || handles_word(&b, 72)? != 0
    {
        return Err(-1);
    }
    if root == 0
        || services == 0
        || lock == 0
        || handles_word(&b, 8)? != root
        || handles_word(&b, 16)? != services
        || handles_word(&b, 24)? != lock
    {
        return Err(-4);
    }
    let phase = handles_word(&b, 40)?;
    let window = handles_word(&b, 48)?;
    let offset = handles_word(&b, 56)?;
    let count = handles_word(&b, 64)?;
    if phase == 0 {
        if window != 0 || offset != 0 || count != 0 {
            return Err(-1);
        }
    } else {
        if window == 0 || offset % 262144 != 0 || offset > 786432 {
            return Err(-1);
        }
        if phase == 2 && count != 16 {
            return Err(-1);
        }
    }
    Ok((version, b))
}
fn handles_unlock(lock: u64) -> Result<(), i32> {
    if gamepkg::local_call(24, &u64::to_le_bytes(lock), &mut [])? != 0 {
        return Err(-3);
    }
    Ok(())
}
fn handles_read(window: u64, offset: u64) -> Result<std::vec::Vec<u8>, i32> {
    let mut output = std::vec::Vec::new();
    output.resize(16384, 0);
    let request = [
        u64::to_le_bytes(window).as_slice(),
        &u64::to_le_bytes(offset),
        &u32::to_le_bytes(16384),
    ]
    .concat();
    if gamepkg::local_call(5, &request, &mut output)? != output.len() {
        return Err(-3);
    }
    Ok(output)
}
fn handles_page(
    window: u64,
    offset: u64,
    record: u64,
    index: usize,
) -> Result<std::vec::Vec<u8>, i32> {
    if index >= 16 {
        return Err(-1);
    }
    let b = handles_read(window, offset + index as u64 * 16384)?;
    if handles_u32(&b, 0)? != 0x32505448
        || handles_u32(&b, 4)? != index as u32
        || handles_word(&b, 24)? != 0
    {
        return Err(-1);
    }
    if handles_word(&b, 8)? != record {
        return Err(-4);
    }
    let count = if index == 15 { 1023usize } else { 1024 };
    for slot in 0..count {
        if (handles_u32(&b, 32 + slot * 12)? == 0) != (handles_word(&b, 36 + slot * 12)? == 0) {
            return Err(-1);
        }
    }
    for value in &b[32 + count * 12..] {
        if *value != 0 {
            return Err(-1);
        }
    }
    Ok(b)
}
fn handles_replace(
    window: u64,
    offset: u64,
    index: usize,
    old: &[u8],
    replacement: &[u8],
) -> Result<(), i32> {
    if gamepkg::local_memory_compare_exchange(
        window,
        offset + index as u64 * 16384,
        old,
        replacement,
    )? {
        Ok(())
    } else {
        Err(-3)
    }
}
fn handles_decode(token: u32) -> Result<[usize; 2], i32> {
    let number = token & 0x00ffffff;
    if token & 0xff000000 != 0x48000000 || number == 0 || number >= 16384 {
        return Err(-1);
    }
    let offset = (number - 1) as usize;
    Ok([offset / 1024, offset % 1024])
}
impl Handles {
    // Allocated, unbound owner. Bind once; failures retain explicit close obligations.
    pub fn handles_new(ram: &memory::Memory) -> Result<Self, i32> {
        let services = ram.services_handle()?;
        handles_owner(ram, services)?;
        let creator = gamepkg::local_task_status()?.0;
        let mut out = [0u8; 8];
        if gamepkg::local_call(21, &[], &mut out)? != 8 {
            return Err(-3);
        }
        let lock = u64::from_le_bytes(out);
        let mut b = [0u8; 80];
        for (at, value) in [
            (0usize, 0x32525448u64),
            (8, ram.call_handle()),
            (16, services),
            (24, lock),
        ] {
            handles_put(&mut b, at, value)?;
        }
        let record = match gamepkg::local_record_create(&b) {
            Ok(id) => id,
            Err(code) => {
                let _ = gamepkg::local_call(25, &u64::to_le_bytes(lock), &mut []);
                return Err(code);
            }
        };
        Ok(Self {
            root: ram.call_handle(),
            services,
            record,
            lock,
            creator,
            window: 0,
            offset: 0,
            initialized: 0,
            phase: 0,
            close_error: 0,
        })
    }
    fn handles_creator(&self, ram: &memory::Memory) -> Result<(), i32> {
        if ram.call_handle() != self.root || gamepkg::local_task_status()?.0 != self.creator {
            return Err(-4);
        }
        Ok(())
    }
    pub fn handles_id(&self) -> u64 {
        self.record
    }
    pub fn handles_closed(&self) -> bool {
        self.record == 0 && self.lock == 0
    }
    pub fn handles_progress(&self) -> [u64; 2] {
        [self.phase, self.initialized]
    }
    pub fn handles_bind(
        &mut self,
        ram: &memory::Memory,
        window: u64,
        offset: u64,
    ) -> Result<(), i32> {
        self.handles_creator(ram)?;
        handles_owner(ram, self.services)?;
        if self.phase != 0
            || self.window != 0
            || window == 0
            || offset % 262144 != 0
            || offset > 786432
        {
            return Err(-1);
        }
        if window != handles_scratch()? {
            return Err(-4);
        }
        let states = gamepkg::local_memory_page_states(window, offset, 64)?;
        if states.len() != 64 {
            return Err(-1);
        }
        for state in states {
            if state != 3 {
                return Err(-4);
            }
        }
        let (version, mut b) = handles_root(self.record, self.root, self.services, self.lock)?;
        if handles_word(&b, 40)? != 0 || handles_word(&b, 32)? != 0 {
            return Err(-4);
        }
        handles_put(&mut b, 40, 1)?;
        handles_put(&mut b, 48, window)?;
        handles_put(&mut b, 56, offset)?;
        handles_publish(self.record, version, &b)?;
        self.window = window;
        self.offset = offset;
        self.phase = 1;
        let mut handles_zero = std::vec::Vec::new();
        handles_zero.resize(16384, 0u8);
        for index in 0..16usize {
            handles_owner(ram, self.services)?;
            let mut page = handles_zero.clone();
            handles_put32(&mut page, 0, 0x32505448)?;
            handles_put32(&mut page, 4, index as u32)?;
            handles_put(&mut page, 8, self.record)?;
            if !gamepkg::local_memory_compare_exchange(
                window,
                offset + index as u64 * 16384,
                &handles_zero,
                &page,
            )? {
                return Err(-2);
            }
            // Retain actual accepted RAM prefix before any later fallible publication.
            self.initialized = index as u64 + 1;
            let (version, mut b) = handles_root(self.record, self.root, self.services, self.lock)?;
            if handles_word(&b, 40)? != 1
                || handles_word(&b, 48)? != window
                || handles_word(&b, 56)? != offset
                || handles_word(&b, 64)? + 1 != self.initialized
            {
                return Err(-4);
            }
            handles_put(&mut b, 64, self.initialized)?;
            handles_publish(self.record, version, &b)?;
        }
        handles_owner(ram, self.services)?;
        let (version, mut b) = handles_root(self.record, self.root, self.services, self.lock)?;
        if handles_word(&b, 64)? != 16 || handles_word(&b, 40)? != 1 {
            return Err(-4);
        }
        handles_put(&mut b, 40, 2)?;
        handles_publish(self.record, version, &b)?;
        self.phase = 2;
        Ok(())
    }
    pub fn handles_borrow(ram: &memory::Memory, record: u64) -> Result<Self, i32> {
        let (_, b) = gamepkg::local_record_snapshot(record, 80)?;
        if b.len() != 80 {
            return Err(-1);
        }
        let view = Self {
            root: ram.call_handle(),
            services: ram.services_handle()?,
            record,
            lock: handles_word(&b, 24)?,
            creator: u64::MAX,
            window: handles_word(&b, 48)?,
            offset: handles_word(&b, 56)?,
            initialized: 16,
            phase: 2,
            close_error: 0,
        };
        if view.window != handles_scratch()? {
            return Err(-4);
        }
        view.handles_snapshot(ram)?;
        match gamepkg::local_mutex_lock(view.lock, 0) {
            Ok(()) => handles_unlock(view.lock)?,
            Err(-2) => {}
            Err(code) => return Err(code),
        }
        Ok(view)
    }
    fn handles_snapshot(&self, ram: &memory::Memory) -> Result<(u64, std::vec::Vec<u8>), i32> {
        if self.record == 0 || ram.call_handle() != self.root {
            return Err(-4);
        }
        handles_owner(ram, self.services)?;
        let result = handles_root(self.record, self.root, self.services, self.lock)?;
        if self.phase != 2
            || handles_word(&result.1, 40)? != 2
            || handles_word(&result.1, 48)? != self.window
            || handles_word(&result.1, 56)? != self.offset
        {
            return Err(-4);
        }
        Ok(result)
    }
    pub fn handles_lock(&self, ram: &memory::Memory) -> Result<HandleGuard, i32> {
        self.handles_snapshot(ram)?;
        self.handles_enter(ram, false)
    }
    pub fn handles_cleanup_lock(&self, ram: &memory::Memory) -> Result<HandleGuard, i32> {
        self.handles_creator(ram)?;
        self.handles_enter(ram, true)
    }
    fn handles_enter(&self, ram: &memory::Memory, cleanup: bool) -> Result<HandleGuard, i32> {
        let task = gamepkg::local_task_status()?.0;
        gamepkg::local_mutex_lock(self.lock, 1000)?;
        let outcome = self.handles_enter_locked(ram, cleanup);
        if let Err(code) = outcome {
            let _ = handles_unlock(self.lock);
            return Err(code);
        }
        Ok(HandleGuard {
            root: self.root,
            services: self.services,
            record: self.record,
            lock: self.lock,
            task,
            window: self.window,
            offset: self.offset,
            held: true,
            cleanup,
        })
    }
    fn handles_enter_locked(&self, ram: &memory::Memory, cleanup: bool) -> Result<(), i32> {
        let (version, mut b) = if cleanup {
            handles_root(self.record, self.root, self.services, self.lock)?
        } else {
            self.handles_snapshot(ram)?
        };
        if ram.call_handle() != self.root
            || self.phase != 2
            || handles_word(&b, 40)? != 2
            || handles_word(&b, 48)? != self.window
            || handles_word(&b, 56)? != self.offset
        {
            return Err(-4);
        }
        let count = handles_word(&b, 32)?.checked_add(1).ok_or(-2)?;
        handles_put(&mut b, 32, count)?;
        handles_publish(self.record, version, &b)
    }
    pub fn handles_close(&mut self, ram: &memory::Memory) -> Result<(), i32> {
        self.handles_creator(ram)?;
        if self.handles_closed() {
            return if self.close_error == 0 {
                Ok(())
            } else {
                Err(self.close_error)
            };
        }
        let validation = handles_owner(ram, self.services);
        gamepkg::local_mutex_lock(self.lock, 0)?;
        let outcome = self.handles_close_locked();
        let unlocked = handles_unlock(self.lock);
        outcome?;
        unlocked?;
        if gamepkg::local_call(25, &u64::to_le_bytes(self.lock), &mut [])? != 0 {
            return Err(-3);
        }
        self.lock = 0;
        if let Err(code) = validation {
            self.close_error = code;
        }
        if self.close_error == 0 {
            Ok(())
        } else {
            Err(self.close_error)
        }
    }
    fn handles_close_locked(&mut self) -> Result<(), i32> {
        if self.record == 0 {
            return Ok(());
        }
        if self.phase != 3 {
            let (version, mut b) = handles_root(self.record, self.root, self.services, self.lock)?;
            if handles_word(&b, 32)? != 0 {
                return Err(-2);
            }
            if handles_word(&b, 40)? != self.phase
                || handles_word(&b, 48)? != self.window
                || handles_word(&b, 56)? != self.offset
                || handles_word(&b, 64)? > self.initialized
            {
                return Err(-4);
            }
            for index in 0..self.initialized as usize {
                let page = handles_page(self.window, self.offset, self.record, index)?;
                for slot in 0..if index == 15 { 1023usize } else { 1024 } {
                    if handles_u32(&page, 32 + slot * 12)? != 0 {
                        return Err(-2);
                    }
                }
            }
            handles_put(&mut b, 40, 3)?;
            handles_publish(self.record, version, &b)?;
            self.phase = 3;
        }
        // Reverse order retains the first-page claim until all other pages clear.
        while self.initialized != 0 {
            let index = (self.initialized - 1) as usize;
            let page = handles_page(self.window, self.offset, self.record, index)?;
            for slot in 0..if index == 15 { 1023usize } else { 1024 } {
                if handles_u32(&page, 32 + slot * 12)? != 0 {
                    return Err(-2);
                }
            }
            let mut handles_zero = std::vec::Vec::new();
            handles_zero.resize(16384, 0u8);
            handles_replace(self.window, self.offset, index, &page, &handles_zero)?;
            self.initialized -= 1;
        }
        match gamepkg::local_record_release(self.record) {
            Ok(()) | Err(-4) => {}
            Err(code) => return Err(code),
        }
        self.record = 0;
        Ok(())
    }
}
impl HandleGuard {
    fn handles_guard_owner(&self, ram: &memory::Memory) -> Result<(), i32> {
        if !self.held
            || ram.call_handle() != self.root
            || gamepkg::local_task_status()?.0 != self.task
        {
            return Err(-4);
        }
        Ok(())
    }
    fn handles_guard_snapshot(
        &self,
        ram: &memory::Memory,
    ) -> Result<(u64, std::vec::Vec<u8>), i32> {
        self.handles_guard_owner(ram)?;
        if !self.cleanup {
            handles_owner(ram, self.services)?;
        }
        let result = handles_root(self.record, self.root, self.services, self.lock)?;
        if handles_word(&result.1, 40)? != 2
            || handles_word(&result.1, 32)? == 0
            || handles_word(&result.1, 48)? != self.window
            || handles_word(&result.1, 56)? != self.offset
        {
            return Err(-4);
        }
        Ok(result)
    }
    pub fn handle_guard_insert(
        &self,
        ram: &memory::Memory,
        object: u64,
        kind: u32,
        fresh: bool,
    ) -> Result<u32, i32> {
        self.handles_guard_snapshot(ram)?;
        if self.cleanup {
            return Err(-4);
        }
        if object == 0 || kind == 0 {
            return Err(-1);
        }
        let mut free = [16usize, 0];
        for index in 0..16usize {
            let page = handles_page(self.window, self.offset, self.record, index)?;
            for slot in 0..if index == 15 { 1023usize } else { 1024 } {
                let occupied = handles_u32(&page, 32 + slot * 12)?;
                let value = handles_word(&page, 36 + slot * 12)?;
                if !fresh && occupied == kind && value == object {
                    return Ok(0x48000001 + (index * 1024 + slot) as u32);
                }
                if occupied == 0 && free[0] == 16 {
                    free = [index, slot];
                }
            }
        }
        if free[0] == 16 {
            return Err(-2);
        }
        let index = free[0];
        let slot = free[1];
        self.handles_guard_snapshot(ram)?;
        let old = handles_page(self.window, self.offset, self.record, index)?;
        if handles_u32(&old, 32 + slot * 12)? != 0 {
            return Err(-3);
        }
        let mut next = old.clone();
        handles_put(
            &mut next,
            16,
            handles_word(&old, 16)?.checked_add(1).ok_or(-3)?,
        )?;
        handles_put32(&mut next, 32 + slot * 12, kind)?;
        handles_put(&mut next, 36 + slot * 12, object)?;
        handles_replace(self.window, self.offset, index, &old, &next)?;
        Ok(0x48000001 + (index * 1024 + slot) as u32)
    }
    pub fn handle_guard_lookup(
        &self,
        ram: &memory::Memory,
        token: u32,
    ) -> Result<Option<[u64; 2]>, i32> {
        self.handles_guard_snapshot(ram)?;
        let at = handles_decode(token)?;
        let b = handles_page(self.window, self.offset, self.record, at[0])?;
        let kind = handles_u32(&b, 32 + at[1] * 12)?;
        if kind == 0 {
            Ok(None)
        } else {
            Ok(Some([handles_word(&b, 36 + at[1] * 12)?, u64::from(kind)]))
        }
    }
    pub fn handle_guard_remove(
        &self,
        ram: &memory::Memory,
        token: u32,
    ) -> Result<Option<[u64; 2]>, i32> {
        self.handles_guard_snapshot(ram)?;
        let at = handles_decode(token)?;
        let old = handles_page(self.window, self.offset, self.record, at[0])?;
        let kind = handles_u32(&old, 32 + at[1] * 12)?;
        if kind == 0 {
            return Ok(None);
        }
        let value = handles_word(&old, 36 + at[1] * 12)?;
        let mut next = old.clone();
        handles_put(
            &mut next,
            16,
            handles_word(&old, 16)?.checked_add(1).ok_or(-3)?,
        )?;
        handles_put32(&mut next, 32 + at[1] * 12, 0)?;
        handles_put(&mut next, 36 + at[1] * 12, 0)?;
        handles_replace(self.window, self.offset, at[0], &old, &next)?;
        Ok(Some([value, u64::from(kind)]))
    }
    pub fn handle_guard_slots(&self, ram: &memory::Memory) -> Result<std::vec::Vec<[u64; 2]>, i32> {
        self.handles_guard_snapshot(ram)?;
        let mut result = std::vec::Vec::new();
        for index in 0..16usize {
            let b = handles_page(self.window, self.offset, self.record, index)?;
            for slot in 0..if index == 15 { 1023usize } else { 1024 } {
                result.push([
                    handles_word(&b, 36 + slot * 12)?,
                    u64::from(handles_u32(&b, 32 + slot * 12)?),
                ]);
            }
        }
        Ok(result)
    }
    pub fn handle_guard_references(
        &self,
        ram: &memory::Memory,
        object: u64,
        kind: u32,
    ) -> Result<u32, i32> {
        if object == 0 || kind == 0 {
            return Err(-1);
        }
        let mut count = 0u32;
        for item in self.handle_guard_slots(ram)? {
            if item == [object, u64::from(kind)] {
                count += 1;
            }
        }
        Ok(count)
    }
    pub fn handles_guard_close(&mut self, ram: &memory::Memory) -> Result<(), i32> {
        if !self.held {
            return Ok(());
        }
        self.handles_guard_owner(ram)?;
        let outcome = self.handles_leave();
        let unlocked = handles_unlock(self.lock);
        if unlocked.is_ok() {
            self.held = false;
        }
        outcome?;
        unlocked
    }
    fn handles_leave(&self) -> Result<(), i32> {
        let (version, mut b) = handles_root(self.record, self.root, self.services, self.lock)?;
        let count = handles_word(&b, 32)?;
        if count == 0 {
            return Err(-3);
        }
        handles_put(&mut b, 32, count - 1)?;
        handles_publish(self.record, version, &b)
    }
    pub fn handles_guard_closed(&self) -> bool {
        !self.held
    }
}

// Checked game adapter. All native events remain opaque GamePKG-owned objects.
pub struct Events {
    root: u64,
    services: u64,
    table: u64,
    record: u64,
    owner: u64,
    owning: bool,
    close_error: i32,
}
pub struct EventCreation {
    root: u64,
    services: u64,
    table: u64,
    record: u64,
    owner: u64,
    task: u64,
    row: usize,
    event: u64,
    token: u32,
    phase: u32,
    attempted_create: bool,
    attempted_publish: bool,
    released: bool,
    cleared: bool,
    closed: bool,
    guard: HandleGuard,
}
fn events_word(b: &[u8], at: usize) -> Result<u64, i32> {
    let p = b.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        p[0], p[1], p[2], p[3], p[4], p[5], p[6], p[7],
    ]))
}
fn events_put(b: &mut [u8], at: usize, value: u64) -> Result<(), i32> {
    b.get_mut(at..at + 8)
        .ok_or(-1)?
        .copy_from_slice(&u64::to_le_bytes(value));
    Ok(())
}
fn events_live(ram: &memory::Memory, root: u64, services: u64) -> Result<(), i32> {
    if root == 0 || ram.call_handle() != root || services == 0 || ram.services_handle()? != services
    {
        return Err(-4);
    }
    kernel_service_values(ram)?;
    if ram.services_handle()? != services || gamepkg::local_task_status()?.1 {
        return Err(-4);
    }
    Ok(())
}
fn events_record(
    record: u64,
    root: u64,
    services: u64,
    table: u64,
    owner: u64,
) -> Result<(u64, std::vec::Vec<u8>), i32> {
    let (version, b) = gamepkg::local_record_snapshot(record, 2624)?;
    if b.len() != 2624
        || events_word(&b, 0)? != 0x32445645
        || events_word(&b, 40)? > 1
        || events_word(&b, 48)? != 0
        || events_word(&b, 56)? != 0
    {
        return Err(-1);
    }
    if root == 0
        || services == 0
        || table == 0
        || owner == u64::MAX
        || events_word(&b, 8)? != root
        || events_word(&b, 16)? != services
        || events_word(&b, 24)? != table
        || events_word(&b, 32)? != owner
    {
        return Err(-4);
    }
    for row in 0..128usize {
        let id = events_word(&b, 64 + row * 16)?;
        let state = events_word(&b, 72 + row * 16)?;
        if state > 3 || ((state < 2) != (id == 0)) {
            return Err(-1);
        }
        if id != 0 {
            for before in 0..row {
                if events_word(&b, 64 + before * 16)? == id {
                    return Err(-1);
                }
            }
        }
    }
    for slot in 0..32usize {
        let lease = events_word(&b, 2112 + slot * 16)?;
        let state = events_word(&b, 2120 + slot * 16)?;
        if state > 2 || ((state == 0) != (lease == 0)) {
            return Err(-1);
        }
        if lease != 0 {
            for before in 0..slot {
                if events_word(&b, 2112 + before * 16)? == lease {
                    return Err(-1);
                }
            }
        }
    }
    Ok((version, b))
}
fn events_commit(record: u64, version: u64, b: &[u8]) -> Result<(), i32> {
    match gamepkg::local_record_compare_exchange(record, version, b) {
        Ok(_) => Ok(()),
        Err(-2) => Err(-3),
        Err(code) => Err(code),
    }
}
fn events_release(id: u64) -> Result<(), i32> {
    if gamepkg::local_call(25, &u64::to_le_bytes(id), &mut [])? != 0 {
        return Err(-3);
    }
    Ok(())
}
fn events_signal(id: u64, value: bool) -> Result<(), i32> {
    gamepkg::local_event_state(id)?;
    if gamepkg::local_call(
        22,
        &[
            u64::to_le_bytes(id).as_slice(),
            &u32::to_le_bytes(u32::from(value)),
        ]
        .concat(),
        &mut [],
    )? != 0
    {
        return Err(-3);
    }
    Ok(())
}
impl Events {
    pub fn events_new(ram: &memory::Memory, map: &Handles) -> Result<Self, i32> {
        let root = ram.call_handle();
        let services = ram.services_handle()?;
        events_live(ram, root, services)?;
        // Only the table creator can own its domain and post-revocation cleanup.
        let mut guard = map.handles_cleanup_lock(ram)?;
        guard.handles_guard_close(ram)?;
        events_live(ram, root, services)?;
        let owner = gamepkg::local_task_status()?.0;
        let mut b = std::vec::Vec::new();
        b.resize(2624, 0);
        for (at, value) in [
            (0usize, 0x32445645u64),
            (8, root),
            (16, services),
            (24, map.handles_id()),
            (32, owner),
        ] {
            events_put(&mut b, at, value)?;
        }
        let record = gamepkg::local_record_create(&b)?;
        Ok(Self {
            root,
            services,
            table: map.handles_id(),
            record,
            owner,
            owning: true,
            close_error: 0,
        })
    }
    pub fn events_id(&self) -> u64 {
        self.record
    }
    pub fn events_closed(&self) -> bool {
        self.record == 0
    }
    pub fn events_borrow(ram: &memory::Memory, map: &Handles, record: u64) -> Result<Self, i32> {
        let (_, b) = gamepkg::local_record_snapshot(record, 2624)?;
        let view = Self {
            root: ram.call_handle(),
            services: ram.services_handle()?,
            table: map.handles_id(),
            record,
            owner: events_word(&b, 32)?,
            owning: false,
            close_error: 0,
        };
        view.events_snapshot(ram, map)?;
        Ok(view)
    }
    fn events_snapshot(
        &self,
        ram: &memory::Memory,
        map: &Handles,
    ) -> Result<(u64, std::vec::Vec<u8>), i32> {
        events_live(ram, self.root, self.services)?;
        if map.handles_id() != self.table {
            return Err(-4);
        }
        let snapshot = events_record(
            self.record,
            self.root,
            self.services,
            self.table,
            self.owner,
        )?;
        if events_word(&snapshot.1, 40)? != 0 {
            return Err(-4);
        }
        Ok(snapshot)
    }
    pub fn events_begin(&self, ram: &memory::Memory, map: &Handles) -> Result<EventCreation, i32> {
        let task = gamepkg::local_task_status()?.0;
        let mut guard = map.handles_lock(ram)?;
        let result = self.events_reserve(ram, map);
        let row = match result {
            Ok(row) => row,
            Err(code) => {
                let _ = guard.handles_guard_close(ram);
                return Err(code);
            }
        };
        // No fallible operation follows the accepted reservation before its owner is returned.
        Ok(EventCreation {
            root: self.root,
            services: self.services,
            table: self.table,
            record: self.record,
            owner: self.owner,
            task,
            row,
            event: 0,
            token: 0,
            phase: 0,
            attempted_create: false,
            attempted_publish: false,
            released: false,
            cleared: false,
            closed: false,
            guard,
        })
    }
    fn events_reserve(&self, ram: &memory::Memory, map: &Handles) -> Result<usize, i32> {
        let (version, mut b) = self.events_snapshot(ram, map)?;
        for row in 0..128usize {
            if events_word(&b, 72 + row * 16)? == 0 {
                events_put(&mut b, 72 + row * 16, 1)?;
                events_commit(self.record, version, &b)?;
                return Ok(row);
            }
        }
        Err(-2)
    }
    fn events_lookup_locked(
        &self,
        ram: &memory::Memory,
        map: &Handles,
        guard: &HandleGuard,
        token: u32,
    ) -> Result<(usize, u64), i32> {
        let (_, b) = self.events_snapshot(ram, map)?;
        let value = guard.handle_guard_lookup(ram, token)?.ok_or(-4)?;
        if value[1] != 1 {
            return Err(-1);
        }
        for row in 0..128usize {
            if events_word(&b, 64 + row * 16)? == value[0] {
                if events_word(&b, 72 + row * 16)? != 2 {
                    return Err(-2);
                }
                gamepkg::local_event_state(value[0])?;
                return Ok((row, value[0]));
            }
        }
        Err(-4)
    }
    pub fn events_state(
        &self,
        ram: &memory::Memory,
        map: &Handles,
        token: u32,
    ) -> Result<[bool; 2], i32> {
        let mut guard = map.handles_lock(ram)?;
        let result = self.events_state_locked(ram, map, &guard, token);
        let closed = guard.handles_guard_close(ram);
        let value = result?;
        closed?;
        Ok(value)
    }
    fn events_state_locked(
        &self,
        ram: &memory::Memory,
        map: &Handles,
        guard: &HandleGuard,
        token: u32,
    ) -> Result<[bool; 2], i32> {
        let (_, id) = self.events_lookup_locked(ram, map, guard, token)?;
        let state = gamepkg::local_event_state(id)?;
        Ok([state.manual, state.signaled])
    }
    pub fn events_set(
        &self,
        ram: &memory::Memory,
        map: &Handles,
        token: u32,
        signal: bool,
    ) -> Result<(), i32> {
        let mut guard = map.handles_lock(ram)?;
        let result = self.events_signal_locked(ram, map, &guard, token, signal);
        let closed = guard.handles_guard_close(ram);
        result?;
        closed
    }
    fn events_signal_locked(
        &self,
        ram: &memory::Memory,
        map: &Handles,
        guard: &HandleGuard,
        token: u32,
        signal: bool,
    ) -> Result<(), i32> {
        let (_, id) = self.events_lookup_locked(ram, map, guard, token)?;
        events_signal(id, signal)
    }
    // False means an ordinary invalid game handle; service/backing failures
    // remain errors. In particular, a failed table read is never a missing tag.
    fn events_nt_signal(
        &self,
        ram: &memory::Memory,
        map: &Handles,
        token: u32,
    ) -> Result<bool, i32> {
        self.events_snapshot(ram, map)?;
        if handles_decode(token).is_err() {
            return Ok(false);
        }
        let mut guard = map.handles_lock(ram)?;
        let result = self.events_nt_signal_locked(ram, map, &guard, token);
        let closed = guard.handles_guard_close(ram);
        let signaled = result?;
        closed?;
        Ok(signaled)
    }
    fn events_nt_signal_locked(
        &self,
        ram: &memory::Memory,
        map: &Handles,
        guard: &HandleGuard,
        token: u32,
    ) -> Result<bool, i32> {
        let (_, b) = self.events_snapshot(ram, map)?;
        let value = match guard.handle_guard_lookup(ram, token)? {
            Some(value) => value,
            None => return Ok(false),
        };
        if value[1] != 1 {
            return Ok(false);
        }
        for row in 0..128usize {
            if events_word(&b, 64 + row * 16)? == value[0] {
                if events_word(&b, 72 + row * 16)? != 2 {
                    return Err(-2);
                }
                match events_signal(value[0], true) {
                    Ok(()) => return Ok(true),
                    Err(code) => {
                        if code != -1 && code != -4 {
                            return Err(code);
                        }
                        // Revalidate ownership after a native failure before
                        // classifying a stale/wrong-kind native event as invalid.
                        let (_, current) = self.events_snapshot(ram, map)?;
                        if guard.handle_guard_lookup(ram, token)? != Some(value)
                            || events_word(&current, 64 + row * 16)? != value[0]
                            || events_word(&current, 72 + row * 16)? != 2
                        {
                            return Err(-3);
                        }
                        return Ok(false);
                    }
                }
            }
        }
        Ok(false)
    }
    pub fn events_alias(
        &self,
        ram: &memory::Memory,
        map: &Handles,
        token: u32,
        fresh: bool,
    ) -> Result<u32, i32> {
        let mut guard = map.handles_lock(ram)?;
        let result = self.events_alias_locked(ram, map, &guard, token, fresh);
        let closed = guard.handles_guard_close(ram);
        let value = result?;
        closed?;
        Ok(value)
    }
    fn events_alias_locked(
        &self,
        ram: &memory::Memory,
        map: &Handles,
        guard: &HandleGuard,
        token: u32,
        fresh: bool,
    ) -> Result<u32, i32> {
        let (_, id) = self.events_lookup_locked(ram, map, guard, token)?;
        guard.handle_guard_insert(ram, id, 1, fresh)
    }
    pub fn events_remove(
        &self,
        ram: &memory::Memory,
        map: &Handles,
        token: u32,
    ) -> Result<(), i32> {
        let mut guard = map.handles_lock(ram)?;
        let result = self.events_remove_locked(ram, map, &guard, token);
        let closed = guard.handles_guard_close(ram);
        result?;
        closed
    }
    fn events_remove_locked(
        &self,
        ram: &memory::Memory,
        map: &Handles,
        guard: &HandleGuard,
        token: u32,
    ) -> Result<(), i32> {
        let (row, id) = self.events_lookup_locked(ram, map, guard, token)?;
        let references = guard.handle_guard_references(ram, id, 1)?;
        if references == 0 {
            return Err(-3);
        }
        if references > 1 {
            if guard.handle_guard_remove(ram, token)? != Some([id, 1]) {
                return Err(-3);
            }
            return Ok(());
        }
        self.events_wait_busy(id)?;
        self.events_wait_reap()?;
        let (version, mut b) = self.events_snapshot(ram, map)?;
        events_put(&mut b, 72 + row * 16, 3)?;
        events_commit(self.record, version, &b)?;
        self.events_drain_row(ram, guard, row, id)
    }
    fn events_drain_row(
        &self,
        ram: &memory::Memory,
        guard: &HandleGuard,
        row: usize,
        id: u64,
    ) -> Result<(), i32> {
        let (_, b) = events_record(
            self.record,
            self.root,
            self.services,
            self.table,
            self.owner,
        )?;
        if events_word(&b, 64 + row * 16)? != id || events_word(&b, 72 + row * 16)? != 3 {
            return Err(-4);
        }
        match gamepkg::local_event_state(id) {
            Ok(_) => events_release(id)?,
            Err(-4) => {}
            Err(code) => return Err(code),
        }
        for (index, item) in guard.handle_guard_slots(ram)?.iter().enumerate() {
            if *item == [id, 1]
                && guard.handle_guard_remove(ram, 0x48000001 + index as u32)? != Some([id, 1])
            {
                return Err(-3);
            }
        }
        let (version, mut b) = events_record(
            self.record,
            self.root,
            self.services,
            self.table,
            self.owner,
        )?;
        if events_word(&b, 64 + row * 16)? != id || events_word(&b, 72 + row * 16)? != 3 {
            return Err(-4);
        }
        events_put(&mut b, 64 + row * 16, 0)?;
        events_put(&mut b, 72 + row * 16, 0)?;
        events_commit(self.record, version, &b)
    }
    pub fn events_close(&mut self, ram: &memory::Memory, map: &Handles) -> Result<(), i32> {
        if !self.owning
            || ram.call_handle() != self.root
            || gamepkg::local_task_status()?.0 != self.owner
        {
            return Err(-4);
        }
        if self.record == 0 {
            return if self.close_error == 0 {
                Ok(())
            } else {
                Err(self.close_error)
            };
        }
        if map.handles_id() != self.table {
            return Err(-4);
        }
        let validation = events_live(ram, self.root, self.services);
        let mut guard = map.handles_cleanup_lock(ram)?;
        let result = self.events_close_locked(ram, &guard);
        let closed = guard.handles_guard_close(ram);
        result?;
        closed?;
        gamepkg::local_record_release(self.record)?;
        self.record = 0;
        if let Err(code) = validation {
            self.close_error = code;
        }
        if self.close_error == 0 {
            Ok(())
        } else {
            Err(self.close_error)
        }
    }
    fn events_close_locked(
        &mut self,
        ram: &memory::Memory,
        guard: &HandleGuard,
    ) -> Result<(), i32> {
        let (_, table) = gamepkg::local_record_snapshot(self.table, 80)?;
        if events_word(&table, 32)? != 1 {
            return Err(-2);
        }
        let (version, mut b) = events_record(
            self.record,
            self.root,
            self.services,
            self.table,
            self.owner,
        )?;
        for row in 0..128usize {
            if events_word(&b, 72 + row * 16)? == 1 {
                return Err(-2);
            }
        }
        self.events_wait_busy(0)?;
        if events_word(&b, 40)? == 0 {
            events_put(&mut b, 40, 1)?;
            events_commit(self.record, version, &b)?;
        }
        self.events_wait_reap()?;
        for row in 0..128usize {
            let (version, mut b) = events_record(
                self.record,
                self.root,
                self.services,
                self.table,
                self.owner,
            )?;
            let state = events_word(&b, 72 + row * 16)?;
            if state == 0 {
                continue;
            }
            let id = events_word(&b, 64 + row * 16)?;
            if state == 2 {
                if gamepkg::local_event_state(id).err() == Some(-4) {
                    self.close_error = -4;
                }
                events_put(&mut b, 72 + row * 16, 3)?;
                events_commit(self.record, version, &b)?;
            }
            self.events_drain_row(ram, guard, row, id)?;
        }
        Ok(())
    }
}
impl EventCreation {
    fn event_creation_owner(&self, ram: &memory::Memory) -> Result<(), i32> {
        if self.closed
            || ram.call_handle() != self.root
            || gamepkg::local_task_status()?.0 != self.task
        {
            return Err(-4);
        }
        Ok(())
    }
    pub fn event_creation_id(&self) -> u64 {
        self.event
    }
    pub fn event_creation_token(&self) -> u32 {
        self.token
    }
    pub fn event_creation_phase(&self) -> u32 {
        self.phase
    }
    pub fn event_creation_closed(&self) -> bool {
        self.closed
    }
    pub fn event_create(
        &mut self,
        ram: &memory::Memory,
        event_type: u32,
        initial: u32,
    ) -> Result<(), i32> {
        self.event_creation_owner(ram)?;
        events_live(ram, self.root, self.services)?;
        if self.attempted_create {
            return Err(-1);
        }
        self.attempted_create = true;
        let mut reply = [0u8; 8];
        if gamepkg::local_call(
            20,
            &[
                u32::to_le_bytes(u32::from(event_type == 0)).as_slice(),
                &u32::to_le_bytes(u32::from(initial & 255 != 0)),
            ]
            .concat(),
            &mut reply,
        )? != 8
        {
            return Err(-3);
        }
        self.event = u64::from_le_bytes(reply);
        self.phase = 1;
        Ok(())
    }
    pub fn event_publish(&mut self, ram: &mut memory::Memory, guest: u32) -> Result<u32, i32> {
        self.event_creation_owner(ram)?;
        events_live(ram, self.root, self.services)?;
        if self.phase != 1 || self.attempted_publish {
            return Err(-1);
        }
        self.attempted_publish = true;
        let (version, mut b) = events_record(
            self.record,
            self.root,
            self.services,
            self.table,
            self.owner,
        )?;
        if events_word(&b, 40)? != 0
            || events_word(&b, 64 + self.row * 16)? != 0
            || events_word(&b, 72 + self.row * 16)? != 1
        {
            return Err(-4);
        }
        gamepkg::local_event_state(self.event)?;
        events_put(&mut b, 64 + self.row * 16, self.event)?;
        events_put(&mut b, 72 + self.row * 16, 2)?;
        events_commit(self.record, version, &b)?;
        // Retain transfer immediately: later tag publication or guest write can fail.
        self.phase = 2;
        if guest != 0 {
            self.token = self.guard.handle_guard_insert(ram, self.event, 1, false)?;
            self.phase = 3;
            ram.write_u32(guest, self.token)?;
        }
        self.phase = 4;
        Ok(self.token)
    }
    pub fn event_creation_close(&mut self, ram: &memory::Memory) -> Result<(), i32> {
        if self.closed {
            return Ok(());
        }
        self.event_creation_owner(ram)?;
        if self.phase < 2 && !self.cleared {
            if self.event != 0 && !self.released {
                gamepkg::local_event_state(self.event)?;
                events_release(self.event)?;
                self.released = true;
            }
            let (version, mut b) = events_record(
                self.record,
                self.root,
                self.services,
                self.table,
                self.owner,
            )?;
            if events_word(&b, 64 + self.row * 16)? != 0
                || events_word(&b, 72 + self.row * 16)? != 1
            {
                return Err(-4);
            }
            events_put(&mut b, 72 + self.row * 16, 0)?;
            events_commit(self.record, version, &b)?;
            self.cleared = true;
        }
        self.guard.handles_guard_close(ram)?;
        self.closed = true;
        Ok(())
    }
}

pub struct EventWait {
    root: u64,
    services: u64,
    table: u64,
    domain: u64,
    owner: u64,
    task: u64,
    event: u64,
    record: u64,
    slot: usize,
    published: bool,
    attempted: bool,
    outcome: i32,
    observed: bool,
    finished: bool,
    closed: bool,
    guard: Option<HandleGuard>,
}
fn events_wait_record(
    record: u64,
    root: u64,
    services: u64,
    table: u64,
    domain: u64,
) -> Result<(u64, std::vec::Vec<u8>), i32> {
    let (version, b) = gamepkg::local_record_snapshot(record, 80)?;
    if b.len() != 80
        || events_word(&b, 0)? != 0x31544c57
        || events_word(&b, 56)? > 1
        || events_word(&b, 72)? != 0
    {
        return Err(-1);
    }
    if root == 0
        || services == 0
        || table == 0
        || domain == 0
        || events_word(&b, 8)? != root
        || events_word(&b, 16)? != services
        || events_word(&b, 24)? != table
        || events_word(&b, 32)? != domain
        || events_word(&b, 40)? == 0
        || events_word(&b, 48)? == u64::MAX
    {
        return Err(-4);
    }
    let result = events_word(&b, 64)?;
    if (events_word(&b, 56)? == 0 && result != 0) || result != (result as i32 as i64 as u64) {
        return Err(-1);
    }
    Ok((version, b))
}
impl Events {
    pub fn events_wait_begin(
        &self,
        ram: &memory::Memory,
        map: &Handles,
        token: u32,
    ) -> Result<EventWait, i32> {
        let task = gamepkg::local_task_status()?.0;
        let mut guard = map.handles_lock(ram)?;
        let result = self.events_wait_reserve(ram, map, &guard, token, task);
        let (slot, event, record) = match result {
            Ok(value) => value,
            Err(code) => {
                let _ = guard.handles_guard_close(ram);
                return Err(code);
            }
        };
        Ok(EventWait {
            root: self.root,
            services: self.services,
            table: self.table,
            domain: self.record,
            owner: self.owner,
            task,
            event,
            record,
            slot,
            published: false,
            attempted: false,
            outcome: -4,
            observed: false,
            finished: false,
            closed: false,
            guard: Some(guard),
        })
    }
    fn events_wait_reserve(
        &self,
        ram: &memory::Memory,
        map: &Handles,
        guard: &HandleGuard,
        token: u32,
        task: u64,
    ) -> Result<(usize, u64, u64), i32> {
        let (_, event) = self.events_lookup_locked(ram, map, guard, token)?;
        self.events_wait_reap()?;
        let (_, b) = self.events_snapshot(ram, map)?;
        let mut slot = 32usize;
        for index in 0..32usize {
            if events_word(&b, 2120 + index * 16)? == 0 {
                slot = index;
                break;
            }
        }
        if slot == 32 {
            return Err(-2);
        }
        let mut lease = [0u8; 80];
        for (at, value) in [
            (0usize, 0x31544c57u64),
            (8, self.root),
            (16, self.services),
            (24, self.table),
            (32, self.record),
            (40, event),
            (48, task),
        ] {
            events_put(&mut lease, at, value)?;
        }
        // Accepted record ownership is returned without further fallible calls.
        let record = gamepkg::local_record_create(&lease)?;
        Ok((slot, event, record))
    }
    fn events_wait_busy(&self, event: u64) -> Result<(), i32> {
        let (_, b) = events_record(
            self.record,
            self.root,
            self.services,
            self.table,
            self.owner,
        )?;
        for slot in 0..32usize {
            let state = events_word(&b, 2120 + slot * 16)?;
            if state == 1 {
                let (_, lease) = events_wait_record(
                    events_word(&b, 2112 + slot * 16)?,
                    self.root,
                    self.services,
                    self.table,
                    self.record,
                )?;
                if events_word(&lease, 56)? == 0
                    && (event == 0 || events_word(&lease, 40)? == event)
                {
                    return Err(-2);
                }
            }
        }
        Ok(())
    }
    fn events_wait_reap(&self) -> Result<(), i32> {
        for slot in 0..32usize {
            let (version, mut b) = events_record(
                self.record,
                self.root,
                self.services,
                self.table,
                self.owner,
            )?;
            let mut state = events_word(&b, 2120 + slot * 16)?;
            if state == 0 {
                continue;
            }
            let record = events_word(&b, 2112 + slot * 16)?;
            if state == 1 {
                let (_, lease) =
                    events_wait_record(record, self.root, self.services, self.table, self.record)?;
                if events_word(&lease, 56)? == 0 {
                    continue;
                }
                events_put(&mut b, 2120 + slot * 16, 2)?;
                events_commit(self.record, version, &b)?;
                state = 2;
            }
            if state != 2 {
                return Err(-1);
            }
            match gamepkg::local_record_release(record) {
                Ok(()) | Err(-4) => {}
                Err(code) => return Err(code),
            }
            let (version, mut b) = events_record(
                self.record,
                self.root,
                self.services,
                self.table,
                self.owner,
            )?;
            if events_word(&b, 2112 + slot * 16)? != record
                || events_word(&b, 2120 + slot * 16)? != 2
            {
                return Err(-4);
            }
            events_put(&mut b, 2112 + slot * 16, 0)?;
            events_put(&mut b, 2120 + slot * 16, 0)?;
            events_commit(self.record, version, &b)?;
        }
        Ok(())
    }
}
impl EventWait {
    pub fn event_wait_id(&self) -> u64 {
        self.record
    }
    pub fn event_wait_closed(&self) -> bool {
        self.closed
    }
    pub fn event_wait_published(&self) -> bool {
        self.published
    }
    fn event_wait_owner(&self, ram: &memory::Memory) -> Result<(), i32> {
        if self.closed
            || ram.call_handle() != self.root
            || gamepkg::local_task_status()?.0 != self.task
        {
            return Err(-4);
        }
        Ok(())
    }
    fn event_wait_unlock(&mut self, ram: &memory::Memory) -> Result<(), i32> {
        if let Some(guard) = self.guard.as_mut() {
            guard.handles_guard_close(ram)?;
        }
        self.guard = None;
        Ok(())
    }
    pub fn event_wait(&mut self, ram: &memory::Memory, milliseconds: u32) -> Result<(), i32> {
        self.event_wait_owner(ram)?;
        if milliseconds > 1000 || self.attempted {
            return Err(-1);
        }
        events_live(ram, self.root, self.services)?;
        self.attempted = true;
        let (version, mut b) = events_record(
            self.domain,
            self.root,
            self.services,
            self.table,
            self.owner,
        )?;
        if events_word(&b, 40)? != 0 || events_word(&b, 2120 + self.slot * 16)? != 0 {
            return Err(-4);
        }
        gamepkg::local_event_state(self.event)?;
        events_put(&mut b, 2112 + self.slot * 16, self.record)?;
        events_put(&mut b, 2120 + self.slot * 16, 1)?;
        events_commit(self.domain, version, &b)?;
        self.published = true;
        self.event_wait_unlock(ram)?;
        // Never retain the tagged-table guard across the actual host-owned wait.
        let result = gamepkg::local_event_wait(self.event, milliseconds);
        let result = match events_live(ram, self.root, self.services) {
            Ok(()) => result,
            Err(code) => Err(code),
        };
        self.outcome = match result {
            Ok(()) => 0,
            Err(code) => code,
        };
        self.observed = true;
        self.event_wait_finish()?;
        result
    }
    fn event_wait_finish(&mut self) -> Result<(), i32> {
        if self.finished {
            return Ok(());
        }
        let (version, mut b) = events_wait_record(
            self.record,
            self.root,
            self.services,
            self.table,
            self.domain,
        )?;
        if events_word(&b, 40)? != self.event
            || events_word(&b, 48)? != self.task
            || events_word(&b, 56)? != 0
        {
            return Err(-4);
        }
        events_put(&mut b, 56, 1)?;
        events_put(
            &mut b,
            64,
            if self.observed {
                self.outcome as i64 as u64
            } else {
                (-4i64) as u64
            },
        )?;
        events_commit(self.record, version, &b)?;
        self.finished = true;
        Ok(())
    }
    pub fn event_wait_close(&mut self, ram: &memory::Memory) -> Result<(), i32> {
        if self.closed {
            return Ok(());
        }
        self.event_wait_owner(ram)?;
        if self.published {
            // Actual-task completion publication needs no live KSV or mutex
            // reacquisition, so cancellation cannot strand a held table lock.
            self.event_wait_finish()?;
        } else if self.record != 0 {
            gamepkg::local_record_release(self.record)?;
            self.record = 0;
        }
        self.event_wait_unlock(ram)?;
        self.closed = true;
        Ok(())
    }
}

// Checked original event bridges. SDK/control-plane errors remain explicit;
// only actual creation capacity and invalid-handle conditions become NT status.
fn pool_nt_create_event(
    ram: &mut memory::Memory,
    table: &Handles,
    domain: &Events,
    output: u32,
    event_type: u32,
    initial: u32,
) -> Result<u32, i32> {
    let mut incoming = domain.events_begin(ram, table)?;
    let created = incoming.event_create(ram, event_type, initial);
    if let Err(code) = created {
        incoming.event_creation_close(ram)?;
        if code != -2 {
            return Err(code);
        }
        // The original bridge still writes its null local HANDLE on failure.
        if output != 0 {
            ram.write_u32(output, 0)?;
        }
        return Ok(0xc000009a);
    }
    let published = incoming.event_publish(ram, output);
    let closed = incoming.event_creation_close(ram);
    published?;
    closed?;
    Ok(0)
}
fn pool_nt_set_event(
    ram: &mut memory::Memory,
    table: &Handles,
    domain: &Events,
    token: u32,
    previous: u32,
) -> Result<u32, i32> {
    // Original PreviousState is always zero, and written before SetEvent.
    if previous != 0 {
        ram.write_u32(previous, 0)?;
    }
    if domain.events_nt_signal(ram, table, token)? {
        Ok(0)
    } else {
        Ok(0xc0000008)
    }
}
