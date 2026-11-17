// Checked KIC4 controls and real retained workers; never a native title entry.
use crate::gamepkg;
use crate::kernel_pool;
use crate::kernel_startup;
use crate::memory;
use crate::registers;
fn sync_startup_need(stage: i32, value: bool) -> Result<(), i32> {
    if value {
        Ok(())
    } else {
        Err(-stage)
    }
}
fn sync_startup_word(bytes: &[u8], at: usize) -> Result<u64, i32> {
    let b = bytes.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}
fn sync_startup_put(bytes: &mut [u8], at: usize, value: u64) -> Result<(), i32> {
    let target = bytes.get_mut(at..at + 8).ok_or(-1)?;
    let encoded = u64::to_le_bytes(value);
    for lane in 0..8usize {
        target[lane] = encoded[lane];
    }
    Ok(())
}
pub fn sync_startup_schema(
    ram: &memory::Memory,
    owner: &kernel_startup::Startup,
    mode: u8,
) -> Result<(), i32> {
    let ids = owner.startup_ids(ram)?;
    let context = kernel_pool::KernelContext::kernel_context_borrow(ram, ids[2])?;
    let triple = context.kernel_context_sync_objects(ram)?;
    let (table, bound_events, semaphores) = owner.startup_sync_objects(ram)?;
    sync_startup_need(
        701,
        triple
            == [
                table.handles_id(),
                bound_events.events_id(),
                semaphores.semaphores_id(),
            ]
            && context.kernel_context_objects(ram)? == [triple[0], triple[1]],
    )?;
    let (_, original) = gamepkg::local_record_snapshot(ids[2], 88)?;
    sync_startup_need(
        702,
        original.len() == 88
            && sync_startup_word(&original, 0)? == 0x3443494b
            && sync_startup_word(&original, 56)? == triple[2],
    )?;
    if mode == 71 {
        // A copied structural ledger is not an owning capability. Startup must
        // reject its substituted identity even when typed borrowing is valid.
        let (_, domain_bytes) = gamepkg::local_record_snapshot(triple[2], 2624)?;
        let alternate = gamepkg::local_record_create(&domain_bytes)?;
        let (version, mut changed) = gamepkg::local_record_snapshot(ids[2], 88)?;
        sync_startup_put(&mut changed, 56, alternate)?;
        gamepkg::local_record_compare_exchange(ids[2], version, &changed)?;
        sync_startup_need(
            703,
            kernel_pool::KernelContext::kernel_context_borrow(ram, ids[2]).is_ok()
                && kernel_pool::kernel_context_ready(ram, ids[2], 193).is_ok()
                && owner.startup_ids(ram) == Err(-4)
                && owner.startup_sync_objects(ram).is_err(),
        )?;
        let (version, _) = gamepkg::local_record_snapshot(ids[2], 88)?;
        gamepkg::local_record_compare_exchange(ids[2], version, &original)?;
        gamepkg::local_record_release(alternate)?;
    }
    if mode == 72 {
        for at in [0usize, 8, 16, 24, 32, 40, 48, 56, 64, 72, 80] {
            let (version, mut changed) = gamepkg::local_record_snapshot(ids[2], 88)?;
            changed[at] ^= 1;
            gamepkg::local_record_compare_exchange(ids[2], version, &changed)?;
            sync_startup_need(
                704,
                kernel_pool::KernelContext::kernel_context_borrow(ram, ids[2]).is_err()
                    && owner.startup_ids(ram).is_err(),
            )?;
            let (version, _) = gamepkg::local_record_snapshot(ids[2], 88)?;
            gamepkg::local_record_compare_exchange(ids[2], version, &original)?;
        }
        for length in [64usize, 72, 80, 87, 89, 96] {
            let mut changed = original.clone();
            changed.resize(length, 0);
            let record = gamepkg::local_record_create(&changed)?;
            sync_startup_need(
                705,
                kernel_pool::KernelContext::kernel_context_borrow(ram, record).is_err(),
            )?;
            gamepkg::local_record_release(record)?;
        }
    }
    if mode == 73 {
        let registry = context.kernel_context_registry(ram)?;
        let bound_shadow = context.kernel_context_shadow(ram)?;
        let mut first = kernel_pool::KernelContext::kernel_context_new(ram, &registry)?;
        let mut second = kernel_pool::KernelContext::kernel_context_new_with_shadow(
            ram,
            &registry,
            &bound_shadow,
        )?;
        let mut third = kernel_pool::KernelContext::kernel_context_new_with_objects(
            ram,
            &registry,
            &bound_shadow,
            triple[0],
            triple[1],
        )?;
        for legacy in [
            first.kernel_context_id(),
            second.kernel_context_id(),
            third.kernel_context_id(),
        ] {
            let binding = kernel_pool::KernelContext::kernel_context_borrow(ram, legacy)?;
            sync_startup_need(
                706,
                binding.kernel_context_sync_objects(ram) == Err(-5)
                    && kernel_pool::kernel_context_ready(ram, legacy, 193) == Err(-5)
                    && kernel_pool::kernel_context_ready(ram, legacy, 222) == Err(-5),
            )?;
        }
        third.kernel_context_close(ram)?;
        second.kernel_context_close(ram)?;
        first.kernel_context_close(ram)?;
    }
    if mode == 80 {
        let (version, pristine) = gamepkg::local_record_snapshot(triple[2], 2624)?;
        let mut malformed = pristine.clone();
        malformed[72] = 255;
        gamepkg::local_record_compare_exchange(triple[2], version, &malformed)?;
        sync_startup_need(
            707,
            kernel_pool::KernelContext::kernel_context_borrow(ram, ids[2]).is_ok()
                && owner.startup_ids(ram).is_err()
                && kernel_pool::kernel_context_ready(ram, ids[2], 193).is_err(),
        )?;
        let (version, _) = gamepkg::local_record_snapshot(triple[2], 2624)?;
        gamepkg::local_record_compare_exchange(triple[2], version, &pristine)?;
    }
    Ok(())
}
pub fn sync_startup_jobs(
    ram: &memory::Memory,
    owner: &mut kernel_startup::Startup,
) -> Result<u64, i32> {
    let baseline_jobs = owner.startup_jobs();
    let ids = owner.startup_ids(ram)?;
    let arg = gamepkg::local_record_create(
        &[
            u64::to_le_bytes(ram.call_handle()),
            u64::to_le_bytes(ids[2]),
        ]
        .concat(),
    )?;
    let task = owner.startup_spawn(ram, 6, arg)?;
    sync_startup_need(
        708,
        owner.startup_close(&mut ram.call_view()) == Err(-2) && owner.startup_ids(ram)? == ids,
    )?;
    let mut previous = [0u8; 4];
    sync_startup_need(
        709,
        gamepkg::local_call(13, &u64::to_le_bytes(task), &mut previous)? == 4
            && u32::from_le_bytes(previous) == 1,
    )?;
    let mut finished = false;
    for _ in 0..4096u32 {
        if gamepkg::local_task_finished(task)? {
            finished = true;
            break;
        }
        memory::Memory::call_retry_pause()?;
    }
    sync_startup_need(
        710,
        finished
            && owner.startup_close(&mut ram.call_view()) == Err(-2)
            && owner.startup_jobs() == baseline_jobs + 1,
    )?;
    let proof = owner.startup_join_task(ram, task)?;
    let id = proof.result();
    sync_startup_need(
        711,
        id != 0
            && gamepkg::local_semaphore_state(id)?.count == 1
            && owner.startup_jobs() == baseline_jobs
            && owner.startup_accept_join(ram, &proof) == Err(-4),
    )?;
    gamepkg::local_record_release(arg)?;
    Ok(id)
}
fn sync_startup_actor(argument: u64) -> Result<u64, i32> {
    let (_, bytes) = gamepkg::local_record_snapshot(argument, 16)?;
    let mut ram = memory::Memory::with_call_handle(sync_startup_word(&bytes, 0)?)?;
    let context =
        kernel_pool::KernelContext::kernel_context_borrow(&ram, sync_startup_word(&bytes, 8)?)?;
    let ids = context.kernel_context_sync_objects(&ram)?;
    let table = kernel_pool::Handles::handles_borrow(&ram, ids[0])?;
    let domain = kernel_pool::Semaphores::semaphores_borrow(&ram, &table, ids[2])?;
    let mut incoming = domain.semaphores_begin(&ram, &table)?;
    let outcome = sync_startup_transfer(&mut ram, &mut incoming);
    let closed = incoming.semaphore_creation_close(&ram);
    let id = outcome?;
    closed?;
    Ok(id)
}
fn sync_startup_transfer(
    ram: &mut memory::Memory,
    incoming: &mut kernel_pool::SemaphoreCreation,
) -> Result<u64, i32> {
    incoming.semaphore_create(ram, 1, 4)?;
    let id = incoming.semaphore_creation_id();
    incoming.semaphore_publish(ram, 0)?;
    Ok(id)
}
pub fn sync_startup_worker(argument: u64) -> u64 {
    match sync_startup_actor(argument) {
        Ok(id) => id,
        Err(code) => code as i64 as u64,
    }
}
pub fn sync_startup_before_close(
    ram: &mut memory::Memory,
    owner: &mut kernel_startup::Startup,
    mode: u8,
) -> Result<(), i32> {
    if mode == 74 {
        let (table, _, domain) = owner.startup_sync_objects(ram)?;
        let mut incoming = domain.semaphores_begin(ram, &table)?;
        incoming.semaphore_create(ram, 0, 1)?;
        let id = incoming.semaphore_creation_id();
        sync_startup_need(
            712,
            owner.startup_close(ram) == Err(-2)
                && owner.startup_phase() == 4
                && !owner.startup_closed()
                && gamepkg::local_semaphore_state(id).is_ok(),
        )?;
        incoming.semaphore_creation_close(ram)?;
        sync_startup_need(713, gamepkg::local_semaphore_state(id).err() == Some(-4))?;
    }
    if mode == 75 {
        let services = ram.services_handle()?;
        ram.services_link(services, 0)?;
    }
    Ok(())
}

fn protection_fixture_invoke(
    ram: &mut memory::Memory,
    stack: u32,
    slot: u32,
    args: &[u32],
    failure: i32,
) -> Result<u32, i32> {
    let mut cpu = registers::Registers::new();
    cpu.esp = stack;
    cpu.eax = 0x12345678;
    cpu.edx = 0xfacade;
    ram.write_u32(stack, 0x87654321)?;
    for (index, arg) in args.iter().enumerate() {
        ram.write_u32(stack + 4 + index as u32 * 4, *arg)?;
    }
    ram.kernel_set_caller(0xabcd);
    let invocation = kernel_pool::kernel_invoke(&mut cpu, ram, slot);
    if failure != 0 {
        sync_startup_need(
            1023,
            invocation == Err(failure)
                && cpu.esp == stack
                && cpu.eax == 0x12345678
                && cpu.edx == 0xfacade
                && ram.kernel_caller() == 0xabcd,
        )?;
    } else {
        invocation?;
        sync_startup_need(
            1024,
            cpu.esp == stack + 4 + args.len() as u32 * 4
                && cpu.edx == 0xfacade
                && ram.kernel_caller() == 0x87654321,
        )?;
    }
    Ok(cpu.eax)
}

pub fn protection_fixture(ram: &mut memory::Memory, slots: [u32; 2]) -> Result<(), i32> {
    let [query, set] = slots;
    sync_startup_need(
        1025,
        kernel_pool::kernel_ordinal(ram, query)? == 179
            && kernel_pool::kernel_ordinal(ram, set)? == 182,
    )?;
    let stack = ram.read_u32(0x4008)?.checked_add(0x2000).ok_or(-1)?;
    let first = 0x01800000;
    let alias = 0x05800000;
    let next = first + 4096;
    let edge = 0x03ffffff;
    let hole = 0xfd000000;
    let reserved = 0xff010000;
    sync_startup_need(
        1026,
        protection_fixture_invoke(ram, stack, query, &[first], 0)? == 4
            && protection_fixture_invoke(ram, stack, query, &[reserved], 0)? == 0
            && protection_fixture_invoke(ram, stack, query, &[hole], 0)? == 1,
    )?;
    let original = ram.read_u8(next)?;
    protection_fixture_invoke(ram, stack, set, &[alias + 4095, 2, 2], 0)?;
    sync_startup_need(
        1027,
        protection_fixture_invoke(ram, stack, query, &[first + 1], 0)? == 2
            && protection_fixture_invoke(ram, stack, query, &[next], 0)? == 2
            && ram.write_u8(next, 0x51) == Err(-4)
            && ram.read_u8(next)? == original,
    )?;
    protection_fixture_invoke(ram, stack, set, &[next + 1, 1, 1], 0)?;
    sync_startup_need(
        1028,
        protection_fixture_invoke(ram, stack, query, &[alias + 4096], 0)? == 1
            && ram.read_u8(next) == Err(-4),
    )?;
    for rejected in [
        (first, 1, 0x40, -5),
        (first, 1, 0x104, -5),
        (first, 0, 4, -1),
        (u32::MAX, 2, 4, -1),
        (edge, 2, 4, -4),
        (hole, 1, 4, -4),
        (reserved, 1, 4, -4),
    ] {
        protection_fixture_invoke(
            ram,
            stack,
            set,
            &[rejected.0, rejected.1, rejected.2],
            rejected.3,
        )?;
    }
    sync_startup_need(
        1029,
        protection_fixture_invoke(ram, stack, query, &[first], 0)? == 2
            && protection_fixture_invoke(ram, stack, query, &[reserved], 0)? == 0,
    )?;
    protection_fixture_invoke(ram, stack, set, &[first, 8192, 4], 0)?;
    sync_startup_need(
        1030,
        protection_fixture_invoke(ram, stack, query, &[alias + 4096], 0)? == 4
            && ram.read_u8(next)? == original,
    )?;
    gamepkg::diagnostic(1, 0x544b4900, &[179, 182, 8192, 7])?;
    Ok(())
}
