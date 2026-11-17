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

fn virtual_query_fixture(
    ram: &mut memory::Memory,
    stack: u32,
    query: u32,
    address: u32,
    expected: [u32; 7],
) -> Result<(), i32> {
    let output = stack + 108;
    ram.write_u32(output - 4, 0x0badcafe)?;
    ram.write_u32(output + 28, 0x0badcafe)?;
    sync_startup_need(
        1043,
        protection_fixture_invoke(ram, stack, query, &[address, output], 0)? == 0,
    )?;
    for (index, expected_value) in expected.iter().enumerate() {
        sync_startup_need(
            1044,
            ram.read_u32(output + (index as u32) * 4)? == *expected_value,
        )?;
    }
    sync_startup_need(
        1045,
        ram.read_u32(output - 4)? == 0x0badcafe && ram.read_u32(output + 28)? == 0x0badcafe,
    )
}

pub fn virtual_startup_fixture(ram: &mut memory::Memory, slots: [u32; 3]) -> Result<(), i32> {
    let [allocate, free, query] = slots;
    let stack = ram.read_u32(0x4008)?.checked_add(0x2200).ok_or(-1)?;
    let base_at = stack + 80;
    let size_at = stack + 84;
    let allocate_args = [base_at, 0, size_at, 0x2000, 4];
    let arena_start = 0x03800000;
    virtual_query_fixture(
        ram,
        stack,
        query,
        arena_start + 333,
        [arena_start, 0, 0, 8388608, 0x10000, 1, 0],
    )?;
    sync_startup_need(
        1046,
        protection_fixture_invoke(ram, stack, query, &[arena_start, 0], 0)? == 0xc000000d
            && kernel_pool::kernel_ordinal(&ram.call_view(), query) == Err(-5),
    )?;
    protection_fixture_invoke(ram, stack, query, &[0x10000, stack + 108], -5)?;
    ram.write_u32(base_at, 0)?;
    ram.write_u32(size_at, 1048576)?;
    sync_startup_need(
        1031,
        protection_fixture_invoke(ram, stack, allocate, &allocate_args, 0)? == 0,
    )?;
    let region = ram.read_u32(base_at)?;
    let alias = region + 0x04000000;
    virtual_query_fixture(
        ram,
        stack,
        query,
        alias + 8193,
        [alias + 8192, alias, 4, 1048576 - 8192, 0x2000, 0, 0x20000],
    )?;
    sync_startup_need(
        1032,
        region == 0x03800000
            && ram.read_u32(size_at)? == 1048576
            && ram.read_u32(region) == Err(-4)
            && ram.read_u32(alias) == Err(-4),
    )?;
    sync_startup_need(
        1033,
        protection_fixture_invoke(ram, stack, allocate, &allocate_args, 0)? == 0xc0000018
            && ram.read_u32(base_at)? == region
            && ram.read_u32(size_at)? == 1048576,
    )?;
    ram.write_u32(size_at, 4096)?;
    let commit_first = [base_at, 0, size_at, 0x1000, 4];
    sync_startup_need(
        1034,
        protection_fixture_invoke(ram, stack, allocate, &commit_first, 0)? == 0
            && ram.read_u32(region)? == 0
            && ram.read_u32(region + 4096) == Err(-4),
    )?;
    virtual_query_fixture(
        ram,
        stack,
        query,
        alias + 8,
        [alias, alias, 4, 4096, 0x1000, 4, 0x20000],
    )?;
    virtual_query_fixture(
        ram,
        stack,
        query,
        region + 4097,
        [region + 4096, region, 4, 1048576 - 4096, 0x2000, 0, 0x20000],
    )?;
    let boundary = (stack & !4095) + 4096;
    ram.write_u32(boundary - 4, 0x4abcde98)?;
    ram.memory_set_guest_protection(boundary, 1, 2)?;
    protection_fixture_invoke(ram, stack, query, &[region, boundary - 4], -4)?;
    sync_startup_need(1047, ram.read_u32(boundary - 4)? == 0x4abcde98)?;
    ram.memory_set_guest_protection(boundary, 1, 4)?;
    ram.write_u32(region, 0x76755452)?;
    ram.write_u32(region + 8, region)?;
    let free_from_region = [alias + 8, size_at, 0x4000];
    protection_fixture_invoke(ram, stack, free, &free_from_region, -5)?;
    sync_startup_need(1035, ram.read_u32(region)? == 0x76755452)?;

    ram.write_u32(size_at, 8192)?;
    let commit_two = [base_at, 0, size_at, 0x1000, 2];
    sync_startup_need(
        1036,
        protection_fixture_invoke(ram, stack, allocate, &commit_two, 0)? == 0
            && ram.read_u32(alias)? == 0x76755452
            && ram.read_u32(region + 4096)? == 0
            && ram.write_u32(region, 0) == Err(-4),
    )?;
    virtual_query_fixture(
        ram,
        stack,
        query,
        region,
        [region, region, 4, 8192, 0x1000, 2, 0x20000],
    )?;
    ram.memory_set_guest_protection(region + 4096, 1, 1)?;
    virtual_query_fixture(
        ram,
        stack,
        query,
        region,
        [region, region, 4, 4096, 0x1000, 2, 0x20000],
    )?;
    virtual_query_fixture(
        ram,
        stack,
        query,
        alias + 4096,
        [alias + 4096, alias, 4, 4096, 0x1000, 1, 0x20000],
    )?;
    let decommit = [base_at, size_at, 0x4000];
    sync_startup_need(
        1037,
        protection_fixture_invoke(ram, stack, free, &decommit, 0)? == 0
            && ram.read_u32(alias) == Err(-4)
            && protection_fixture_invoke(ram, stack, allocate, &commit_first, 0)? == 0
            && ram.read_u32(alias)? == 0,
    )?;
    ram.write_u32(size_at, 0)?;
    let release = [base_at, size_at, 0x8000];
    sync_startup_need(
        1038,
        protection_fixture_invoke(ram, stack, free, &release, 0)? == 0
            && ram.read_u32(size_at)? == 1048576
            && ram.read_u32(region) == Err(-4)
            && protection_fixture_invoke(ram, stack, allocate, &commit_first, 0)? == 0xc0000018,
    )?;
    ram.write_u32(base_at, 0)?;
    ram.write_u32(size_at, 1048576)?;
    let create_combined = [base_at, 0, size_at, 0x103000, 4];
    sync_startup_need(
        1039,
        protection_fixture_invoke(ram, stack, allocate, &create_combined, 0)? == 0,
    )?;
    let high = ram.read_u32(base_at)?;
    sync_startup_need(1040, high == 0x03f00000 && ram.read_u32(high)? == 0)?;
    // Release the second page only, then show that the remaining fragments
    // retain their bytes while a commit into the released gap is refused.
    ram.write_u32(high, 0xea123987)?;
    ram.write_u32(base_at, high + 4096)?;
    ram.write_u32(size_at, 4096)?;
    sync_startup_need(
        1041,
        protection_fixture_invoke(ram, stack, free, &release, 0)? == 0
            && ram.read_u32(high)? == 0xea123987
            && ram.read_u32(high + 4096) == Err(-4)
            && ram.read_u32(high + 8192)? == 0
            && protection_fixture_invoke(ram, stack, allocate, &commit_first, 0)? == 0xc0000018,
    )?;
    virtual_query_fixture(
        ram,
        stack,
        query,
        high + 4096,
        [high + 4096, 0, 0, 4096, 0x10000, 1, 0],
    )?;
    virtual_query_fixture(
        ram,
        stack,
        query,
        high + 8192,
        [
            high + 8192,
            high + 8192,
            4,
            1048576 - 8192,
            0x1000,
            4,
            0x20000,
        ],
    )?;
    for fragment in [high, high + 8192] {
        ram.write_u32(base_at, fragment)?;
        ram.write_u32(size_at, 0)?;
        sync_startup_need(
            1042,
            protection_fixture_invoke(ram, stack, free, &release, 0)? == 0,
        )?;
    }
    gamepkg::diagnostic(1, 0x544b4a00, &[184, 199, 1048576, 8192])?;
    gamepkg::diagnostic(1, 0x544b4b00, &[217, 28, 4096, 8388608])?;
    Ok(())
}
