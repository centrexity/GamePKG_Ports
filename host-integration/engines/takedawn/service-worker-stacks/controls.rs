use crate::gamepkg;
use crate::kernel_pool;
use crate::memory;
use crate::stacks;
fn stack_probe_need(ok: bool) -> Result<(), i32> {
    if ok {
        Ok(())
    } else {
        Err(-3)
    }
}
fn stack_probe_word(b: &[u8], at: usize) -> Result<u64, i32> {
    let v = b.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7],
    ]))
}
fn stack_probe_put(b: &mut [u8], at: usize, value: u64) {
    let v = u64::to_le_bytes(value);
    for lane in 0..8usize {
        b[at + lane] = v[lane];
    }
}
fn stack_probe_reply(id: u64, value: u64) -> Result<[u64; 3], i32> {
    let mut b = [0u8; 24];
    stack_probe_need(
        gamepkg::local_mailbox_receive(id, 1000, &mut b)? == 24
            && stack_probe_word(&b, 0)? == value,
    )?;
    Ok([value, stack_probe_word(&b, 8)?, stack_probe_word(&b, 16)?])
}
fn stack_probe_send(id: u64, value: u64, slot: u64, generation: u64) -> Result<(), i32> {
    gamepkg::local_mailbox_send(
        id,
        &[
            u64::to_le_bytes(value),
            u64::to_le_bytes(slot),
            u64::to_le_bytes(generation),
        ]
        .concat(),
    )
}
fn stack_probe_worker(arg: u64) -> Result<(), i32> {
    let (_, b) = gamepkg::local_record_snapshot(arg, 56)?;
    stack_probe_need(b.len() == 56)?;
    let ram = memory::Memory::with_call_handle(stack_probe_word(&b, 0)?)?;
    let pool = stacks::Pool::stacks_borrow(&ram, stack_probe_word(&b, 8)?)?;
    if stack_probe_word(&b, 32)? == 1 {
        let command = stack_probe_word(&b, 40)?;
        let reply_id = stack_probe_word(&b, 48)?;
        stack_probe_send(reply_id, 1, 0, 0)?;
        let mut message = [0u8; 8];
        stack_probe_need(gamepkg::local_mailbox_receive(command, 1000, &mut message)? == 8)?;
        let mut held = [None, None, None, None];
        let mut mask = 0u64;
        for lane in 0..4usize {
            let lease = pool.stacks_allocate(&ram)?.ok_or(-3)?;
            let slot = lease.stack_slot(&ram)?;
            stack_probe_need(mask & (1u64 << slot) == 0)?;
            mask |= 1u64 << slot;
            held[lane] = Some(lease);
        }
        stack_probe_send(reply_id, 2, mask, 0)?;
        stack_probe_need(gamepkg::local_mailbox_receive(command, 1000, &mut message)? == 8)?;
        for lane in 0..4usize {
            if let Some(mut lease) = held[lane].take() {
                lease.stack_release(&ram)?;
            } else {
                return Err(-3);
            }
        }
        stack_probe_send(reply_id, 3, 0, 0)?;
        return Ok(());
    }
    stack_probe_need(
        stacks::Lease::stack_adopt(
            &ram,
            pool.stacks_id(),
            stack_probe_word(&b, 16)? as u32,
            stack_probe_word(&b, 24)?,
        )
        .err()
            == Some(-4)
            && pool.stacks_allocate(&ram)?.is_none(),
    )?;
    let command = stack_probe_word(&b, 40)?;
    let reply_id = stack_probe_word(&b, 48)?;
    stack_probe_send(reply_id, 1, 0, 0)?;
    let mut message = [0u8; 8];
    stack_probe_need(gamepkg::local_mailbox_receive(command, 1000, &mut message)? == 8)?;
    let mut lease = pool.stacks_allocate(&ram)?.ok_or(-3)?;
    let token = lease.stack_identity(&ram)?;
    stack_probe_need(token[1] == 0)?;
    stack_probe_send(reply_id, 2, token[1], token[2])?;
    let received = gamepkg::local_mailbox_receive(command, 1000, &mut message);
    let cancelled = gamepkg::local_task_status()?.1;
    if cancelled {
        stack_probe_need(
            received == Err(-4)
                && lease.stack_release(&ram) == Err(-4)
                && lease.stack_released()
                && pool.stacks_allocate(&ram).err() == Some(-4),
        )?;
    } else {
        stack_probe_need(received? == 8)?;
        lease.stack_release(&ram)?;
    }
    stack_probe_send(reply_id, 3, 0, 0)?;
    Ok(())
}
pub fn stacks_worker(arg: u64) -> u64 {
    match stack_probe_worker(arg) {
        Ok(()) => 0,
        Err(c) => c as i64 as u64,
    }
}
pub fn stacks_controls(ram: &mut memory::Memory, _window: u64) -> Result<(), i32> {
    for pair in [
        [0u32, 1048576],
        [0x780001, 1048576],
        [0x780000, 9 * 1048576],
        [0x3f00000, 2 * 1048576],
        [0x780000, 1],
    ] {
        stack_probe_need(stacks::Pool::stacks_new(ram, pair[0], pair[1]).err() == Some(-1))?;
    }
    let mut owner = stacks::Pool::stacks_new(ram, 0x780000, 1048576)?;
    let mut borrowed = stacks::Pool::stacks_borrow(ram, owner.stacks_id())?;
    stack_probe_need(borrowed.stacks_close(ram) == Err(-4))?;
    let foreign = memory::Memory::new()?;
    stack_probe_need(
        owner.stacks_usage(&foreign) == Err(-4)
            && stacks::Pool::stacks_borrow(&foreign, owner.stacks_id()).err() == Some(-4),
    )?;
    gamepkg::local_record_release(foreign.call_handle())?;
    let (_, original) = gamepkg::local_record_snapshot(owner.stacks_id(), 576)?;
    for pair in [
        [0u64, 0],
        [40, 17],
        [48, 2],
        [56, 1],
        [64, 2],
        [72, 1],
        [64, 1],
        [88, 1],
        [64 + 4 * 32, 1],
    ] {
        let mut bad = original.clone();
        stack_probe_put(&mut bad, pair[0] as usize, pair[1]);
        let (v, _) = gamepkg::local_record_snapshot(owner.stacks_id(), 576)?;
        gamepkg::local_record_compare_exchange(owner.stacks_id(), v, &bad)?;
        stack_probe_need(owner.stacks_usage(ram) == Err(-1))?;
        let (v, after) = gamepkg::local_record_snapshot(owner.stacks_id(), 576)?;
        stack_probe_need(after == bad)?;
        gamepkg::local_record_compare_exchange(owner.stacks_id(), v, &original)?;
    }
    let mut closing = original.clone();
    stack_probe_put(&mut closing, 48, 1);
    let (v, _) = gamepkg::local_record_snapshot(owner.stacks_id(), 576)?;
    gamepkg::local_record_compare_exchange(owner.stacks_id(), v, &closing)?;
    stack_probe_need(owner.stacks_allocate(ram).err() == Some(-4))?;
    owner.stacks_close(ram)?;
    stack_probe_need(owner.stacks_closed())?;
    let mut owner = stacks::Pool::stacks_new(ram, 0x780000, 1048576)?;
    ram.write_u32(0x7bfff0, 0x31415926)?;
    let mut lease = owner.stacks_allocate(ram)?.ok_or(-3)?;
    let first = lease.stack_identity(ram)?;
    stack_probe_need(
        lease.stack_guest_top(ram)? == 0x7bfff0
            && ram.read_u32(0x7bfff0)? == 0x31415926
            && owner.stacks_close(ram) == Err(-2),
    )?;
    let mut stale = stacks::Lease::stack_adopt(ram, first[0], first[1] as u32, first[2])?;
    lease.stack_release(ram)?;
    lease.stack_release(ram)?;
    let mut replacement = owner.stacks_allocate(ram)?.ok_or(-3)?;
    let fresh = replacement.stack_identity(ram)?;
    stack_probe_need(
        fresh[1] == first[1]
            && fresh[2] == first[2] + 1
            && stale.stack_release(ram) == Err(-4)
            && owner.stacks_usage(ram)? == [1, 4],
    )?;
    stack_probe_need(
        stacks::Lease::stack_adopt(ram, first[0], first[1] as u32, first[2]).err() == Some(-4)
            && stacks::Lease::stack_adopt(ram, first[0], 16, 1).err() == Some(-1)
            && stacks::Lease::stack_adopt(ram, first[0], 0, 0).err() == Some(-1),
    )?;
    replacement.stack_release(ram)?;
    let id = owner.stacks_id();
    owner.stacks_close(ram)?;
    owner.stacks_close(ram)?;
    stack_probe_need(
        stacks::Pool::stacks_borrow(ram, id).err() == Some(-4)
            && borrowed.stacks_usage(ram) == Err(-4),
    )?;
    // No generation wrap/ABA: exhaustion must be explicit and nonmutating.
    let mut owner = stacks::Pool::stacks_new(ram, 0x780000, 262144)?;
    let (v, mut b) = gamepkg::local_record_snapshot(owner.stacks_id(), 576)?;
    stack_probe_put(&mut b, 80, u64::MAX);
    gamepkg::local_record_compare_exchange(owner.stacks_id(), v, &b)?;
    stack_probe_need(owner.stacks_allocate(ram).err() == Some(-3))?;
    let (_, after) = gamepkg::local_record_snapshot(owner.stacks_id(), 576)?;
    stack_probe_need(after == b)?;
    owner.stacks_close(ram)?;
    for cancel in [false, true] {
        let mut owner = stacks::Pool::stacks_new(ram, 0x780000, 1048576)?;
        let mut zero = owner.stacks_allocate(ram)?.ok_or(-3)?;
        let token = zero.stack_identity(ram)?;
        let mut one = owner.stacks_allocate(ram)?.ok_or(-3)?;
        let mut two = owner.stacks_allocate(ram)?.ok_or(-3)?;
        let mut three = owner.stacks_allocate(ram)?.ok_or(-3)?;
        let command = gamepkg::local_mailbox_create(8, 1)?;
        let response = gamepkg::local_mailbox_create(24, 1)?;
        let context = gamepkg::local_record_create(
            &[
                u64::to_le_bytes(ram.call_handle()),
                u64::to_le_bytes(owner.stacks_id()),
                u64::to_le_bytes(token[1]),
                u64::to_le_bytes(token[2]),
                u64::to_le_bytes(0),
                u64::to_le_bytes(command),
                u64::to_le_bytes(response),
            ]
            .concat(),
        )?;
        let mut out = [0u8; 8];
        stack_probe_need(
            gamepkg::local_call(
                10,
                &[
                    u32::to_le_bytes(0).as_slice(),
                    &u64::to_le_bytes(context),
                    &u32::to_le_bytes(0),
                ]
                .concat(),
                &mut out,
            )? == 8,
        )?;
        let task = u64::from_le_bytes(out);
        stack_probe_reply(response, 1)?;
        zero.stack_release(ram)?;
        gamepkg::local_mailbox_send(command, &u64::to_le_bytes(1))?;
        let current = stack_probe_reply(response, 2)?;
        stack_probe_need(
            stacks::Lease::stack_adopt(ram, owner.stacks_id(), current[1] as u32, current[2]).err()
                == Some(-4)
                && owner.stacks_usage(ram)? == [4, 4]
                && owner.stacks_close(ram) == Err(-2),
        )?;
        if cancel {
            gamepkg::local_call(12, &u64::to_le_bytes(task), &mut [])?;
        } else {
            gamepkg::local_mailbox_send(command, &u64::to_le_bytes(2))?;
        }
        stack_probe_reply(response, 3)?;
        let mut joined = false;
        for _ in 0..4096u32 {
            match gamepkg::local_call(11, &u64::to_le_bytes(task), &mut out) {
                Ok(8) => {
                    stack_probe_need(u64::from_le_bytes(out) == 0)?;
                    joined = true;
                    break;
                }
                Err(-2) => memory::Memory::call_retry_pause()?,
                Err(c) => return Err(c),
                _ => return Err(-3),
            }
        }
        stack_probe_need(joined && owner.stacks_usage(ram)? == [3, 4])?;
        one.stack_release(ram)?;
        two.stack_release(ram)?;
        three.stack_release(ram)?;
        owner.stacks_close(ram)?;
        gamepkg::local_record_release(context)?;
        gamepkg::local_mailbox_release(command)?;
        gamepkg::local_mailbox_release(response)?;
    }
    // Four actual workers compete on the same CAS pool, each retaining four leases.
    let mut owner = stacks::Pool::stacks_new(ram, 0x780000, 8 * 1048576)?;
    let mut jobs = [[0u64; 4]; 4];
    for lane in 0..4usize {
        let command = gamepkg::local_mailbox_create(8, 1)?;
        let response = gamepkg::local_mailbox_create(24, 1)?;
        let context = gamepkg::local_record_create(
            &[
                u64::to_le_bytes(ram.call_handle()),
                u64::to_le_bytes(owner.stacks_id()),
                u64::to_le_bytes(0),
                u64::to_le_bytes(0),
                u64::to_le_bytes(1),
                u64::to_le_bytes(command),
                u64::to_le_bytes(response),
            ]
            .concat(),
        )?;
        let mut out = [0u8; 8];
        stack_probe_need(
            gamepkg::local_call(
                10,
                &[
                    u32::to_le_bytes(0).as_slice(),
                    &u64::to_le_bytes(context),
                    &u32::to_le_bytes(0),
                ]
                .concat(),
                &mut out,
            )? == 8,
        )?;
        jobs[lane] = [u64::from_le_bytes(out), context, command, response];
        stack_probe_reply(response, 1)?;
    }
    for lane in 0..4usize {
        gamepkg::local_mailbox_send(jobs[lane][2], &u64::to_le_bytes(1))?;
    }
    let mut union = 0u64;
    for lane in 0..4usize {
        let mask = stack_probe_reply(jobs[lane][3], 2)?[1];
        stack_probe_need(mask.count_ones() == 4 && union & mask == 0)?;
        union |= mask;
    }
    stack_probe_need(
        union == 65535
            && owner.stacks_usage(ram)? == [16, 16]
            && owner.stacks_allocate(ram)?.is_none()
            && owner.stacks_close(ram) == Err(-2),
    )?;
    for lane in 0..4usize {
        gamepkg::local_mailbox_send(jobs[lane][2], &u64::to_le_bytes(2))?;
    }
    for lane in 0..4usize {
        stack_probe_reply(jobs[lane][3], 3)?;
        let mut joined = false;
        let mut out = [0u8; 8];
        for _ in 0..4096u32 {
            match gamepkg::local_call(11, &u64::to_le_bytes(jobs[lane][0]), &mut out) {
                Ok(8) => {
                    stack_probe_need(u64::from_le_bytes(out) == 0)?;
                    joined = true;
                    break;
                }
                Err(-2) => memory::Memory::call_retry_pause()?,
                Err(c) => return Err(c),
                _ => return Err(-3),
            }
        }
        stack_probe_need(joined)?;
        gamepkg::local_record_release(jobs[lane][1])?;
        gamepkg::local_mailbox_release(jobs[lane][2])?;
        gamepkg::local_mailbox_release(jobs[lane][3])?;
    }
    stack_probe_need(owner.stacks_usage(ram)? == [0, 16])?;
    owner.stacks_close(ram)?;
    // Existing host record-count and byte quotas must remain effective.
    let mut ids = [0u64; 64];
    let mut count = 0usize;
    for _ in 0..64usize {
        match gamepkg::local_record_create(&[0u8; 1]) {
            Ok(id) => {
                ids[count] = id;
                count += 1;
            }
            Err(-2) => break,
            Err(c) => return Err(c),
        }
    }
    stack_probe_need(
        count > 0
            && count < 64
            && stacks::Pool::stacks_new(ram, 0x780000, 1048576).err() == Some(-2),
    )?;
    for lane in 0..count {
        gamepkg::local_record_release(ids[lane])?;
    }
    let mut ids = [0u64; 16];
    for lane in 0..15usize {
        ids[lane] = gamepkg::local_record_create(&[0u8; 4096])?;
    }
    ids[15] = gamepkg::local_record_create(&[0u8; 3500])?;
    stack_probe_need(stacks::Pool::stacks_new(ram, 0x780000, 1048576).err() == Some(-2))?;
    for lane in 0..16usize {
        gamepkg::local_record_release(ids[lane])?;
    }
    let mut recovered = stacks::Pool::stacks_new(ram, 0x780000, 1048576)?;
    recovered.stacks_close(ram)?;
    let revoked = memory::Memory::new()?;
    let services = kernel_pool::kernel_services_initialize(&revoked, 0x10000, 2)?;
    let mut owner = stacks::Pool::stacks_new(&revoked, 0x780000, 1048576)?;
    let mut lease = owner.stacks_allocate(&revoked)?.ok_or(-3)?;
    services.service_close(&revoked)?;
    stack_probe_need(
        lease.stack_guest_top(&revoked) == Err(-4)
            && owner.stacks_allocate(&revoked).err() == Some(-4),
    )?;
    stack_probe_need(lease.stack_release(&revoked) == Err(-4) && lease.stack_released())?;
    stack_probe_need(owner.stacks_close(&revoked) == Err(-4) && owner.stacks_closed())?;
    gamepkg::local_record_release(revoked.call_handle())?;
    Ok(())
}
