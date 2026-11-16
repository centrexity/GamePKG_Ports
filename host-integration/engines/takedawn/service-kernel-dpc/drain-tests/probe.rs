use crate::cases;
use crate::draining;
use crate::gamepkg;
use crate::irql;
use crate::kernel_pool;
use crate::memory;
use crate::queue;
use crate::registers;
use crate::stacks;
fn drain_need(value: bool) -> Result<(), i32> {
    if value {
        Ok(())
    } else {
        Err(-3)
    }
}
fn drain_u64(bytes: &[u8], at: usize) -> Result<u64, i32> {
    let b = bytes.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}
fn drain_ready(commands: u64, replies: u64, group: u32, step: u32, busy: bool) -> Result<(), i32> {
    let mut packet = [0u8; 352];
    packet[0..8].copy_from_slice(&u64::to_le_bytes(u64::from(group)));
    packet[8..16].copy_from_slice(&u64::to_le_bytes(u64::from(step)));
    gamepkg::local_mailbox_send(replies, &packet)?;
    let mut cmd = [0u8; 8];
    drain_need(
        gamepkg::local_mailbox_receive(commands, 1000, &mut cmd)? == 8
            && u64::from_le_bytes(cmd) == u64::from(busy),
    )
}
fn drain_done(commands: u64) -> Result<(), i32> {
    let mut cmd = [0u8; 8];
    drain_need(
        gamepkg::local_mailbox_receive(commands, 1000, &mut cmd)? == 8
            && u64::from_le_bytes(cmd) == 2,
    )
}
fn drain_witness(
    ram: &memory::Memory,
    task: &irql::Task,
    cpu: &registers::Registers,
    pending: &queue::Queue,
    result: [u64; 2],
) -> Result<std::vec::Vec<u8>, i32> {
    let mut witness = cpu.encode();
    let mut bytes = std::vec::Vec::new();
    bytes.resize(524288, 0);
    ram.read_bytes(0x740000, &mut bytes)?;
    witness.extend(u64::to_le_bytes(cases::drain_hash(&bytes)));
    let b = pending.queue_state(ram)?;
    let mut ring = std::vec::Vec::new();
    ring.extend(&b[56..72]);
    ring.extend(&b[80..]);
    witness.extend(u64::to_le_bytes(cases::drain_hash(&ring)));
    for value in [
        result[0] as u32,
        result[1] as u32,
        ram.read_u32(0x741070)?,
        ram.read_u32(0x741088)?,
        u32::from(task.irql_current(ram)?),
    ] {
        witness.extend(u32::to_le_bytes(value));
    }
    drain_need(witness.len() == 352)?;
    Ok(witness)
}
fn drain_cases(
    ram: &mut memory::Memory,
    task: &mut irql::Task,
    pending: &queue::Queue,
    controller: &irql::Controller,
    pool: &stacks::Pool,
    lease: &stacks::Lease,
    commands: u64,
    replies: u64,
    scenario: u64,
) -> Result<(), i32> {
    let top = lease.stack_guest_top(ram)?;
    task.irql_hook_bind(ram, pending.queue_id())?;
    if scenario != 0 {
        let mut cpu = cases::drain_seed(24, ram, pending, top)?;
        ram.write_u32(0x741004, 1)?;
        match scenario {
            1 => ram.write_u32(0x7410a0, 1)?,
            2 => ram.write_u32(0x7410a0, 2)?,
            3 => ram.write_u32(0x74010c, 0x3300)?,
            4 => ram.write_u32(0x741030, 1)?,
            5 => ram.write_u32(0x741030, 2)?,
            6 => ram.write_u32(0x7410a0, 3)?,
            7 => ram.write_u32(0x7410a0, 4)?,
            8 => ram.write_u32(0x741030, 3)?,
            9 => ram.write_u32(0x741030, 6)?,
            _ => return Err(-1),
        }
        let mut runner = draining::Drain::drain_new(ram, pending, controller, pool, 1200, 0, 77)?;
        let due = cases::drain_step_seed(24, 1, ram)?;
        drain_ready(commands, replies, 0, 0, scenario == 7)?;
        let result = runner.drain_before(
            ram,
            task,
            &mut cpu,
            lease,
            if scenario == 6 { due } else { 0 },
        );
        let expected = match scenario {
            1 | 3 | 4 | 7 => -5,
            2 | 8 => -2,
            5 => -3,
            6 => -4,
            _ => -1,
        };
        drain_need(result == Err(expected) && task.irql_current(ram) == Err(-4))?;
        drain_need(controller.controller_counts(ram)?[0..2] == [0, 0])?;
        let raw = crate::gate::Gate::gate_borrow(ram, controller.controller_gate())?;
        drain_need(raw.gate_state(ram)?[1] == u64::from(scenario == 7))?;
        let not_popped = scenario == 1 || scenario == 6 || scenario == 7;
        drain_need(
            pending.queue_nonempty(ram)? == not_popped
                && ram.read_u8(0x740102)? == u8::from(not_popped),
        )?;
        gamepkg::local_mailbox_send(replies, &[0u8; 352])?;
        drain_done(commands)?;
        return Ok(());
    }
    for group in 0..96u32 {
        let mut cpu = cases::drain_seed(group, ram, pending, top)?;
        let mut runner = draining::Drain::drain_new(
            ram,
            pending,
            controller,
            pool,
            cases::drain_guard(group),
            cases::drain_lag(group),
            77,
        )?;
        for step in 0..4u32 {
            let due = cases::drain_step_seed(group, step, ram)?;
            drain_ready(
                commands,
                replies,
                group,
                step,
                cases::drain_busy_case(group, step),
            )?;
            let result = runner.drain_before(ram, task, &mut cpu, lease, due)?;
            let witness = drain_witness(ram, task, &cpu, pending, result)?;
            drain_need(controller.controller_counts(ram)?[0..2] == [0, 0])?;
            let raw = crate::gate::Gate::gate_borrow(ram, controller.controller_gate())?;
            drain_need(raw.gate_state(ram)?[1] == u64::from(cases::drain_busy_case(group, step)))?;
            gamepkg::local_mailbox_send(replies, &witness)?;
            drain_done(commands)?;
        }
        drain_need(!pending.queue_nonempty(ram)?)?;
    }
    Ok(())
}
fn drain_worker_run(context: u64) -> Result<u64, i32> {
    let (_, b) = gamepkg::local_record_snapshot(context, 64)?;
    drain_need(b.len() == 64)?;
    let mut ram = memory::Memory::with_call_handle(drain_u64(&b, 0)?)?;
    ram.attach(0x740000, 524288, drain_u64(&b, 8)?, 0, true)?;
    let pending = queue::Queue::queue_borrow(&ram, drain_u64(&b, 16)?)?;
    let controller = irql::Controller::controller_borrow(&ram, drain_u64(&b, 24)?)?;
    let pool = stacks::Pool::stacks_borrow(&ram, drain_u64(&b, 32)?)?;
    let mut task = controller.controller_task(&ram)?;
    let mut lease = pool.stacks_allocate(&ram)?.ok_or(-2)?;
    let result = drain_cases(
        &mut ram,
        &mut task,
        &pending,
        &controller,
        &pool,
        &lease,
        drain_u64(&b, 40)?,
        drain_u64(&b, 48)?,
        drain_u64(&b, 56)?,
    );
    let aborted = if result.is_err() {
        task.irql_abort(&ram)
    } else {
        Ok(())
    };
    let freed = lease.stack_release(&ram);
    result?;
    aborted?;
    freed?;
    drain_need(lease.stack_released())?;
    Ok(0)
}
pub fn drain_worker(context: u64) -> u64 {
    match drain_worker_run(context) {
        Ok(value) => value,
        Err(code) => {
            if let Ok((_, b)) = gamepkg::local_record_snapshot(context, 64) {
                if let Ok(replies) = drain_u64(&b, 48) {
                    let mut packet = [0u8; 352];
                    packet[0..8].copy_from_slice(&u64::to_le_bytes(0x445241494e455252));
                    packet[8..16].copy_from_slice(&u64::to_le_bytes(code as i64 as u64));
                    let _ = gamepkg::local_mailbox_send(replies, &packet);
                }
            }
            code as i64 as u64
        }
    }
}
fn drain_receive(replies: u64) -> Result<[u8; 352], i32> {
    let mut packet = [0u8; 352];
    drain_need(gamepkg::local_mailbox_receive(replies, 1000, &mut packet)? == 352)?;
    if drain_u64(&packet, 0)? == 0x445241494e455252 {
        return Err(drain_u64(&packet, 8)? as i32);
    }
    Ok(packet)
}
pub fn drain_probe() -> Result<std::vec::Vec<u8>, i32> {
    let mut b = [0u8; 16];
    drain_need(gamepkg::local_call(1, &[], &mut b)? == 16)?;
    let window = drain_u64(&b, 0)?;
    gamepkg::local_call(
        2,
        &[
            u64::to_le_bytes(window).as_slice(),
            &u64::to_le_bytes(0),
            &u64::to_le_bytes(524288),
            &u32::to_le_bytes(2),
        ]
        .concat(),
        &mut [],
    )?;
    let mut output = std::vec::Vec::new();
    for scenario in 0..10u64 {
        let mut ram = memory::Memory::new()?;
        ram.attach(0x740000, 524288, window, 0, true)?;
        let services = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2)?;
        let mut pending = queue::Queue::queue_new(&ram, false)?;
        let mut controller = irql::Controller::controller_new(&ram)?;
        let mut pool = stacks::Pool::stacks_new(&ram, 0x780000, 262144)?;
        let mut main_task = controller.controller_task(&ram)?;
        let mut main_cpu = registers::Registers::new();
        let commands = gamepkg::local_mailbox_create(8, 1)?;
        let replies = gamepkg::local_mailbox_create(352, 1)?;
        let context = gamepkg::local_record_create(
            &[
                u64::to_le_bytes(ram.call_handle()),
                u64::to_le_bytes(window),
                u64::to_le_bytes(pending.queue_id()),
                u64::to_le_bytes(controller.controller_id()),
                u64::to_le_bytes(pool.stacks_id()),
                u64::to_le_bytes(commands),
                u64::to_le_bytes(replies),
                u64::to_le_bytes(scenario),
            ]
            .concat(),
        )?;
        let mut id = [0u8; 8];
        gamepkg::local_call(
            10,
            &[
                u32::to_le_bytes(0).as_slice(),
                &u64::to_le_bytes(context),
                &u32::to_le_bytes(1),
            ]
            .concat(),
            &mut id,
        )?;
        let id = u64::from_le_bytes(id);
        pending.queue_assign(&ram, id)?;
        drain_need(
            draining::Drain::drain_new(&ram, &pending, &controller, &pool, 1200, 0, 77).err()
                == Some(-4),
        )?;
        let mut resumed = [0u8; 4];
        gamepkg::local_call(13, &u64::to_le_bytes(id), &mut resumed)?;
        for index in 0..if scenario == 0 { 384u32 } else { 1 } {
            let ready = drain_receive(replies)?;
            let group = if scenario == 0 { index / 4 } else { 0 };
            let step = if scenario == 0 { index % 4 } else { 0 };
            drain_need(
                drain_u64(&ready, 0)? == u64::from(group)
                    && drain_u64(&ready, 8)? == u64::from(step),
            )?;
            let busy = if scenario == 0 {
                cases::drain_busy_case(group, step)
            } else {
                scenario == 7
            };
            if busy {
                main_task.irql_gate_enter(&ram, 0)?;
            }
            gamepkg::local_mailbox_send(commands, &u64::to_le_bytes(u64::from(busy)))?;
            let witness = drain_receive(replies)?;
            if busy {
                main_task.irql_gate_leave(&mut ram, &mut main_cpu)?;
            }
            gamepkg::local_mailbox_send(commands, &u64::to_le_bytes(2))?;
            if scenario == 0 {
                output.extend(witness);
            } else {
                drain_need(witness == [0u8; 352])?;
            }
        }
        for _ in 0..1000u32 {
            if gamepkg::local_task_finished(id)? {
                break;
            }
            memory::Memory::call_retry_pause()?;
        }
        drain_need(
            gamepkg::local_task_finished(id)?
                && pending.queue_join(&ram)? == 0
                && pool.stacks_usage(&ram)? == [0, 1],
        )?;
        let state = pending.queue_state(&ram)?;
        let mutex = drain_u64(&state, 24)?;
        gamepkg::local_mutex_lock(mutex, 0)?;
        gamepkg::local_call(24, &u64::to_le_bytes(mutex), &mut [])?;
        pending.queue_close(&ram)?;
        controller.controller_close(&ram)?;
        pool.stacks_close(&ram)?;
        gamepkg::local_mailbox_release(commands)?;
        gamepkg::local_mailbox_release(replies)?;
        gamepkg::local_record_release(context)?;
        services.service_close(&ram)?;
        gamepkg::local_record_release(ram.call_handle())?;
    }
    gamepkg::local_call(
        4,
        &[
            u64::to_le_bytes(window),
            u64::to_le_bytes(0),
            u64::to_le_bytes(524288),
        ]
        .concat(),
        &mut [],
    )?;
    Ok(output)
}
