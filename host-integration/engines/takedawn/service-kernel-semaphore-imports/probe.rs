use crate::gamepkg;
use crate::kernel_pool;
use crate::memory;
use crate::registers;
fn semaphore_import_need(stage: i32, ok: bool) -> Result<(), i32> {
    if ok {
        Ok(())
    } else {
        Err(-stage)
    }
}
fn semaphore_import_word(b: &[u8], at: usize) -> Result<u64, i32> {
    let p = b.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        p[0], p[1], p[2], p[3], p[4], p[5], p[6], p[7],
    ]))
}
fn semaphore_import_frame(
    ram: &mut memory::Memory,
    args: &[u32],
) -> Result<registers::Registers, i32> {
    let mut cpu = registers::Registers::new();
    cpu.esp = ram.read_u32(0x4008)?.checked_add(0x2000).ok_or(-1)?;
    cpu.eax = 0xdeadbeef;
    cpu.ebx = 0xabcdef01;
    cpu.ecx = 0x87654321;
    cpu.eflags = 0x202;
    cpu.fs_base = 0x4000;
    cpu.mm3 = [0x11223344, 0xfedcba98];
    ram.write_u32(cpu.esp, 0x12345678)?;
    for (index, value) in args.iter().enumerate() {
        ram.write_u32(cpu.esp + 4 + index as u32 * 4, *value)?;
    }
    Ok(cpu)
}
fn semaphore_import_call(
    ram: &mut memory::Memory,
    slot: u32,
    args: &[u32],
    status: u32,
) -> Result<(), i32> {
    let mut cpu = semaphore_import_frame(ram, args)?;
    let mut expected = registers::Registers::new();
    expected.decode(&cpu.encode())?;
    expected.eax = status;
    expected.esp += 4 + args.len() as u32 * 4;
    kernel_pool::kernel_invoke(&mut cpu, ram, slot)?;
    semaphore_import_need(
        401,
        cpu.encode() == expected.encode() && ram.kernel_caller() == 0x12345678,
    )
}
fn semaphore_import_error(
    ram: &mut memory::Memory,
    slot: u32,
    args: &[u32],
    code: i32,
) -> Result<(), i32> {
    let mut cpu = semaphore_import_frame(ram, args)?;
    let before = cpu.encode();
    let actual = kernel_pool::kernel_invoke(&mut cpu, ram, slot);
    let same = cpu.encode() == before;
    if actual != Err(code) || !same {
        let observed = match actual {
            Ok(()) => 0,
            Err(value) => value,
        };
        let _ = gamepkg::diagnostic(
            2,
            0x544b3d00,
            &[
                u64::from(slot),
                u64::from(args[0]),
                code as i64 as u64,
                observed as i64 as u64,
                u64::from(same),
            ],
        );
        return Err(-402);
    }
    Ok(())
}
fn semaphore_import_id(
    ram: &memory::Memory,
    table: &kernel_pool::Handles,
    token: u32,
) -> Result<u64, i32> {
    let mut guard = table.handles_lock(ram)?;
    let result = guard.handle_guard_lookup(ram, token);
    let closed = guard.handles_guard_close(ram);
    let value = result?.ok_or(-4)?;
    closed?;
    semaphore_import_need(403, value[1] == 3)?;
    Ok(value[0])
}
fn semaphore_import_put(out: &mut [u8], cursor: &mut usize, value: u32) -> Result<(), i32> {
    let encoded = u32::to_le_bytes(value);
    for lane in 0..4usize {
        *out.get_mut(*cursor + lane).ok_or(-1)? = encoded[lane];
    }
    *cursor += 4;
    Ok(())
}
pub fn semaphore_import_vectors(
    ram: &mut memory::Memory,
    table: &kernel_pool::Handles,
    domain: &kernel_pool::Semaphores,
    slots: [u32; 2],
) -> Result<std::vec::Vec<u8>, i32> {
    let base = ram.read_u32(0x4008)?;
    let output = base + 0x100;
    let previous = base + 0x108;
    let mut out = std::vec::Vec::new();
    out.resize(3100, 0);
    let mut cursor = 0usize;
    for maximum in [1u32, 2, 4, 0x7fffffff] {
        for initial in [0u32, 1, maximum / 2, maximum] {
            ram.write_u32(output, 0x33333333)?;
            ram.write_u32(output + 4, 0xabcdef01)?;
            semaphore_import_call(ram, slots[0], &[output, 0xffffffff, initial, maximum], 0)?;
            let token = ram.read_u32(output)?;
            let id = semaphore_import_id(ram, table, token)?;
            semaphore_import_need(
                601,
                token == 0x48000001 && ram.read_u32(output + 4)? == 0xabcdef01,
            )?;
            let state = gamepkg::local_semaphore_state(id)?;
            for value in [0u32, token, state.count, state.maximum] {
                semaphore_import_put(&mut out, &mut cursor, value)?;
            }
            for permits in [0u32, 1, 2, 0x7fffffff, 0x80000000, 0xffffffff] {
                let before = gamepkg::local_semaphore_state(id)?;
                let accepted = permits != 0
                    && permits <= 0x7fffffff
                    && u64::from(before.count) + u64::from(permits) <= u64::from(before.maximum);
                let expected = if accepted { 0 } else { 0xc0000008 };
                ram.write_u32(previous, 0x55555555)?;
                semaphore_import_call(ram, slots[1], &[token, permits, previous], expected)?;
                semaphore_import_need(
                    602,
                    ram.read_u32(previous)? == if accepted { before.count } else { 0x55555555 },
                )?;
                for value in [
                    expected,
                    ram.read_u32(previous)?,
                    gamepkg::local_semaphore_state(id)?.count,
                ] {
                    semaphore_import_put(&mut out, &mut cursor, value)?;
                }
            }
            domain.semaphores_remove(ram, table, token)?;
        }
    }
    for pair in [
        [0u32, 0],
        [1, 0],
        [2, 1],
        [0xffffffff, 1],
        [0, 0x80000000],
        [1, 0xffffffff],
        [0x80000000, 0x7fffffff],
        [0x7fffffff, 0],
    ] {
        ram.write_u32(output, 0x33333333)?;
        ram.write_u32(output + 4, 0xabcdef01)?;
        semaphore_import_call(
            ram,
            slots[0],
            &[output, 0xffffffff, pair[0], pair[1]],
            0xc000009a,
        )?;
        semaphore_import_need(
            603,
            ram.read_u32(output)? == 0x33333333 && ram.read_u32(output + 4)? == 0xabcdef01,
        )?;
        for value in [
            0xc000009au32,
            ram.read_u32(output)?,
            ram.read_u32(output + 4)?,
        ] {
            semaphore_import_put(&mut out, &mut cursor, value)?;
        }
    }
    for token in [
        0u32, 0xffffffff, 0xfffffffe, 0xdead0001, 0xbeef0010, 0x48000000, 0x48004000, 0x48003fff,
        0x49000001,
    ] {
        ram.write_u32(previous, 0x55555555)?;
        semaphore_import_call(ram, slots[1], &[token, 1, previous], 0xc0000008)?;
        semaphore_import_need(604, ram.read_u32(previous)? == 0x55555555)?;
        semaphore_import_put(&mut out, &mut cursor, 0xc0000008)?;
        semaphore_import_put(&mut out, &mut cursor, ram.read_u32(previous)?)?;
    }
    semaphore_import_call(ram, slots[0], &[0, 0xffffffff, 0, 4], 0)?;
    semaphore_import_put(&mut out, &mut cursor, 0)?;
    semaphore_import_call(ram, slots[0], &[output, 0xffffffff, 1, 4], 0)?;
    let token = ram.read_u32(output)?;
    semaphore_import_need(605, token == 0x48000001)?;
    semaphore_import_put(&mut out, &mut cursor, 0)?;
    semaphore_import_put(&mut out, &mut cursor, token)?;
    domain.semaphores_remove(ram, table, token)?;
    for index in 0..126u32 {
        ram.write_u32(output, 0x33333333)?;
        ram.write_u32(output + 4, 0xabcdef01)?;
        let expected = if index < 125 { 0 } else { 0xc000009a };
        semaphore_import_call(ram, slots[0], &[output, 0xffffffff, 0, 1], expected)?;
        semaphore_import_need(
            606,
            ram.read_u32(output)?
                == if index < 125 {
                    0x48000001 + index
                } else {
                    0x33333333
                },
        )?;
        for value in [expected, ram.read_u32(output)?, ram.read_u32(output + 4)?] {
            semaphore_import_put(&mut out, &mut cursor, value)?;
        }
    }
    for index in 0..125u32 {
        domain.semaphores_remove(ram, table, 0x48000001 + index)?;
    }
    semaphore_import_need(607, cursor == 3100)?;
    Ok(out)
}
pub fn semaphore_import_controls(
    ram: &mut memory::Memory,
    table: &kernel_pool::Handles,
    domain: &kernel_pool::Semaphores,
    slots: [u32; 2],
) -> Result<(), i32> {
    let base = ram.read_u32(0x4008)?;
    let output = base + 0x100;
    let previous = base + 0x108;
    semaphore_import_error(ram, slots[0], &[0x90000000, 0xffffffff, 0, 4], -4)?;
    let id = semaphore_import_id(ram, table, 0x48000001)?;
    semaphore_import_need(608, gamepkg::local_semaphore_state(id)?.count == 0)?;
    domain.semaphores_remove(ram, table, 0x48000001)?;
    semaphore_import_call(ram, slots[0], &[output, 0xffffffff, 0, 4], 0)?;
    let token = ram.read_u32(output)?;
    let id = semaphore_import_id(ram, table, token)?;
    // Bad PreviousCount cannot undo the accepted native count change.
    semaphore_import_error(ram, slots[1], &[token, 2, 0x90000000], -4)?;
    semaphore_import_need(609, gamepkg::local_semaphore_state(id)?.count == 2)?;
    let backing = ram.memory_writable_word_backing(output)?;
    semaphore_import_protect(backing, 1)?;
    semaphore_import_error(ram, slots[1], &[token, 1, previous], -4)?;
    semaphore_import_need(610, gamepkg::local_semaphore_state(id)?.count == 3)?;
    semaphore_import_protect(backing, 2)?;
    // Invalid count/handle does not touch an invalid PreviousCount pointer.
    semaphore_import_call(ram, slots[1], &[token, 0, 0x90000000], 0xc0000008)?;
    semaphore_import_call(ram, slots[1], &[0xffffffff, 1, 0x90000000], 0xc0000008)?;
    let scratch = crate::layout::Layout::layout_services(ram)?.layout_scratch()?;
    semaphore_import_protect([scratch, 0], 0)?;
    semaphore_import_error(ram, slots[1], &[token, 1, previous], -4)?;
    semaphore_import_need(611, gamepkg::local_semaphore_state(id)?.count == 3)?;
    semaphore_import_protect([scratch, 0], 2)?;
    let mut byte = [0u8; 1];
    gamepkg::local_call(
        5,
        &[
            u64::to_le_bytes(scratch).as_slice(),
            &u64::to_le_bytes(0),
            &u32::to_le_bytes(1),
        ]
        .concat(),
        &mut byte,
    )?;
    gamepkg::local_call(
        6,
        &[
            u64::to_le_bytes(scratch).as_slice(),
            &u64::to_le_bytes(0),
            &[0u8],
        ]
        .concat(),
        &mut [],
    )?;
    semaphore_import_error(ram, slots[1], &[token, 1, previous], -1)?;
    semaphore_import_need(612, gamepkg::local_semaphore_state(id)?.count == 3)?;
    gamepkg::local_call(
        6,
        &[
            u64::to_le_bytes(scratch).as_slice(),
            &u64::to_le_bytes(0),
            &byte,
        ]
        .concat(),
        &mut [],
    )?;
    let mut guard = table.handles_lock(ram)?;
    let wrong = guard.handle_guard_insert(ram, id, 1, true)?;
    guard.handles_guard_close(ram)?;
    ram.write_u32(previous, 0x55555555)?;
    semaphore_import_call(ram, slots[1], &[wrong, 1, previous], 0xc0000008)?;
    semaphore_import_need(
        613,
        ram.read_u32(previous)? == 0x55555555 && gamepkg::local_semaphore_state(id)?.count == 3,
    )?;
    let mut guard = table.handles_lock(ram)?;
    guard.handle_guard_remove(ram, wrong)?;
    guard.handles_guard_close(ram)?;
    let map_id = ram.kernel_handle();
    let (_, plan) = gamepkg::local_record_snapshot(map_id, 1548)?;
    let context =
        kernel_pool::KernelContext::kernel_context_borrow(ram, semaphore_import_word(&plan, 24)?)?;
    let ids = context.kernel_context_sync_objects(ram)?;
    semaphore_import_need(
        615,
        ids[0] == table.handles_id()
            && ids[2] == domain.semaphores_id()
            && kernel_pool::kernel_context_ready(ram, context.kernel_context_id(), 193).is_ok()
            && kernel_pool::kernel_context_ready(ram, context.kernel_context_id(), 222).is_ok(),
    )?;
    let registry = context.kernel_context_registry(ram)?;
    let bound_shadow = context.kernel_context_shadow(ram)?;
    let mut first = kernel_pool::KernelContext::kernel_context_new(ram, &registry)?;
    let mut second =
        kernel_pool::KernelContext::kernel_context_new_with_shadow(ram, &registry, &bound_shadow)?;
    let mut third = kernel_pool::KernelContext::kernel_context_new_with_objects(
        ram,
        &registry,
        &bound_shadow,
        ids[0],
        ids[1],
    )?;
    for legacy in [
        first.kernel_context_id(),
        second.kernel_context_id(),
        third.kernel_context_id(),
    ] {
        semaphore_import_need(
            616,
            kernel_pool::kernel_context_ready(ram, legacy, 193) == Err(-5)
                && kernel_pool::kernel_context_ready(ram, legacy, 222) == Err(-5),
        )?;
        let (version, mut changed) = gamepkg::local_record_snapshot(map_id, 1548)?;
        let encoded = u64::to_le_bytes(legacy);
        for lane in 0..8usize {
            changed[24 + lane] = encoded[lane];
        }
        gamepkg::local_record_compare_exchange(map_id, version, &changed)?;
        ram.write_u32(previous, 0x55555555)?;
        semaphore_import_error(ram, slots[1], &[token, 1, previous], -5)?;
        semaphore_import_need(
            617,
            ram.read_u32(previous)? == 0x55555555 && gamepkg::local_semaphore_state(id)?.count == 3,
        )?;
        let (version, _) = gamepkg::local_record_snapshot(map_id, 1548)?;
        gamepkg::local_record_compare_exchange(map_id, version, &plan)?;
    }
    third.kernel_context_close(ram)?;
    second.kernel_context_close(ram)?;
    first.kernel_context_close(ram)?;
    let context_id = context.kernel_context_id();
    let (_, original) = gamepkg::local_record_snapshot(context_id, 88)?;
    semaphore_import_need(
        618,
        original.len() == 88 && semaphore_import_word(&original, 0)? == 0x3443494b,
    )?;
    for at in [0usize, 8, 16, 24, 32, 40, 48, 56, 64, 72, 80] {
        let (version, mut changed) = gamepkg::local_record_snapshot(context_id, 88)?;
        changed[at] ^= 1;
        gamepkg::local_record_compare_exchange(context_id, version, &changed)?;
        semaphore_import_need(
            619,
            kernel_pool::KernelContext::kernel_context_borrow(ram, context_id).is_err(),
        )?;
        let (version, _) = gamepkg::local_record_snapshot(context_id, 88)?;
        gamepkg::local_record_compare_exchange(context_id, version, &original)?;
    }
    for length in [64usize, 72, 80, 87, 89, 96] {
        let mut changed = original.clone();
        changed.resize(length, 0);
        let malformed = gamepkg::local_record_create(&changed)?;
        semaphore_import_need(
            620,
            kernel_pool::KernelContext::kernel_context_borrow(ram, malformed).is_err(),
        )?;
        gamepkg::local_record_release(malformed)?;
    }
    let (version, original_domain) = gamepkg::local_record_snapshot(domain.semaphores_id(), 2624)?;
    let mut changed = original_domain.clone();
    changed[72] = 255;
    gamepkg::local_record_compare_exchange(domain.semaphores_id(), version, &changed)?;
    semaphore_import_need(
        621,
        kernel_pool::KernelContext::kernel_context_borrow(ram, context_id).is_ok()
            && kernel_pool::kernel_context_ready(ram, context_id, 222).is_err(),
    )?;
    semaphore_import_error(ram, slots[1], &[token, 1, previous], -1)?;
    let (version, _) = gamepkg::local_record_snapshot(domain.semaphores_id(), 2624)?;
    gamepkg::local_record_compare_exchange(domain.semaphores_id(), version, &original_domain)?;
    let foreign = gamepkg::local_semaphore_create(0, 4)?;
    let mut guard = table.handles_lock(ram)?;
    let foreign_token = guard.handle_guard_insert(ram, foreign, 3, false)?;
    guard.handles_guard_close(ram)?;
    ram.write_u32(previous, 0x55555555)?;
    semaphore_import_call(ram, slots[1], &[foreign_token, 1, previous], 0xc0000008)?;
    semaphore_import_need(
        622,
        gamepkg::local_semaphore_state(foreign)?.count == 0
            && ram.read_u32(previous)? == 0x55555555,
    )?;
    let mut guard = table.handles_lock(ram)?;
    guard.handle_guard_remove(ram, foreign_token)?;
    guard.handles_guard_close(ram)?;
    gamepkg::local_call(25, &u64::to_le_bytes(foreign), &mut [])?;
    let mut native = [0u8; 8];
    gamepkg::local_call(
        20,
        &[u32::to_le_bytes(0), u32::to_le_bytes(1)].concat(),
        &mut native,
    )?;
    let fake = u64::from_le_bytes(native);
    let (version, pristine) = gamepkg::local_record_snapshot(domain.semaphores_id(), 2624)?;
    semaphore_import_need(
        623,
        semaphore_import_word(&pristine, 80)? == 0 && semaphore_import_word(&pristine, 88)? == 0,
    )?;
    let mut forged = pristine.clone();
    for (at, value) in [(80usize, fake), (88, 2)] {
        let target = forged.get_mut(at..at + 8).ok_or(-1)?;
        let encoded = u64::to_le_bytes(value);
        for lane in 0..8usize {
            target[lane] = encoded[lane];
        }
    }
    gamepkg::local_record_compare_exchange(domain.semaphores_id(), version, &forged)?;
    let mut guard = table.handles_lock(ram)?;
    let fake_token = guard.handle_guard_insert(ram, fake, 3, false)?;
    guard.handles_guard_close(ram)?;
    semaphore_import_call(ram, slots[1], &[fake_token, 1, previous], 0xc0000008)?;
    semaphore_import_need(
        624,
        gamepkg::local_event_state(fake)?.signaled && ram.read_u32(previous)? == 0x55555555,
    )?;
    let mut guard = table.handles_lock(ram)?;
    guard.handle_guard_remove(ram, fake_token)?;
    guard.handles_guard_close(ram)?;
    let (version, current) = gamepkg::local_record_snapshot(domain.semaphores_id(), 2624)?;
    semaphore_import_need(625, current == forged)?;
    gamepkg::local_record_compare_exchange(domain.semaphores_id(), version, &pristine)?;
    gamepkg::local_call(25, &native, &mut [])?;
    let services = ram.services_handle()?;
    ram.services_link(services, 0)?;
    semaphore_import_error(ram, slots[1], &[token, 1, previous], -4)?;
    semaphore_import_need(614, gamepkg::local_semaphore_state(id)?.count == 3)?;
    ram.services_link(0, services)?;
    domain.semaphores_remove(ram, table, token)?;
    Ok(())
}
fn semaphore_import_protect(backing: [u64; 2], mode: u32) -> Result<(), i32> {
    gamepkg::local_call(
        3,
        &[
            u64::to_le_bytes(backing[0]).as_slice(),
            &u64::to_le_bytes(backing[1] & !4095),
            &u64::to_le_bytes(4096),
            &u32::to_le_bytes(mode),
        ]
        .concat(),
        &mut [],
    )?;
    Ok(())
}
