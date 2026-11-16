// Port-owned native DPC calling convention. Static checked routes only.
use crate::dpc_observer;
use crate::dpc_routes;
use crate::gamepkg;
use crate::irql;
use crate::memory;
use crate::queue;
use crate::registers;
use crate::stacks;
pub struct Callback {
    root: u64,
    services: u64,
    queue: u64,
    controller: u64,
    pool: u64,
    worker: u64,
}
fn callback_word(bytes: &[u8], at: usize) -> Result<u64, i32> {
    let b = bytes.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}
impl Callback {
    pub fn callback_new(
        ram: &memory::Memory,
        pending: &queue::Queue,
        controller: &irql::Controller,
        pool: &stacks::Pool,
    ) -> Result<Self, i32> {
        let bytes = pending.queue_state(ram)?;
        controller.controller_counts(ram)?;
        pool.stacks_usage(ram)?;
        if callback_word(&bytes, 40)? != 1 {
            return Err(-5);
        }
        let worker = callback_word(&bytes, 32)?;
        if gamepkg::local_task_finished(worker)? {
            return Err(-4);
        }
        Ok(Self {
            root: ram.call_handle(),
            services: ram.services_handle()?,
            queue: pending.queue_id(),
            controller: controller.controller_id(),
            pool: pool.stacks_id(),
            worker,
        })
    }
    fn callback_check(
        &self,
        ram: &memory::Memory,
        task: &irql::Task,
        cpu: &registers::Registers,
        lease: &stacks::Lease,
    ) -> Result<(), i32> {
        if ram.call_handle() != self.root
            || ram.services_handle()? != self.services
            || task.task_key() != [self.controller, self.worker]
            || gamepkg::local_task_status()? != (self.worker, false)
        {
            return Err(-4);
        }
        task.task_values(ram)?;
        let pending = queue::Queue::queue_borrow(ram, self.queue)?;
        let bytes = pending.queue_state(ram)?;
        if callback_word(&bytes, 40)? != 1 || callback_word(&bytes, 32)? != self.worker {
            return Err(-4);
        }
        if lease.stack_identity(ram)?[0] != self.pool {
            return Err(-4);
        }
        let top = lease.stack_guest_top(ram)?;
        let base = top
            .checked_add(16)
            .ok_or(-1)?
            .checked_sub(262144)
            .ok_or(-1)?;
        if !cpu.valid() || cpu.esp > top || cpu.esp < base.checked_add(20).ok_or(-1)? {
            return Err(-1);
        }
        Ok(())
    }
    // A false result is the original null DPC or null routine only. An absent
    // finite native route is Unsupported before argument/stack mutation.
    pub fn callback_run(
        &self,
        ram: &mut memory::Memory,
        task: &mut irql::Task,
        cpu: &mut registers::Registers,
        lease: &stacks::Lease,
        dpc: u32,
        first: u32,
        second: u32,
        profile: i32,
    ) -> Result<bool, i32> {
        self.callback_check(ram, task, cpu, lease)?;
        if dpc == 0 {
            return Ok(false);
        }
        let routine = ram.read_u32(dpc.checked_add(12).ok_or(-1)?)?;
        let context = ram.read_u32(dpc.checked_add(16).ok_or(-1)?)?;
        if routine == 0 {
            return Ok(false);
        }
        if !dpc_routes::dpc_route_supported(routine) {
            return Err(-5);
        }
        // Validate fixed guest arithmetic before beginning a partial publication.
        let arg1 = dpc.checked_add(20).ok_or(-1)?;
        let arg2 = dpc.checked_add(24).ok_or(-1)?;
        let old = task.irql_current(ram)?;
        let result = self.callback_invoke(
            ram, task, cpu, dpc, routine, context, arg1, arg2, first, second, profile, old,
        );
        if let Err(code) = result {
            // Error/stop cleanup, not a successful callback acknowledgement.
            // The timer owner still must release its stack and actually join.
            if task.irql_abort(ram).is_err() {
                return Err(-3);
            }
            return Err(code);
        }
        Ok(true)
    }
    fn callback_invoke(
        &self,
        ram: &mut memory::Memory,
        task: &mut irql::Task,
        cpu: &mut registers::Registers,
        dpc: u32,
        routine: u32,
        context: u32,
        arg1: u32,
        arg2: u32,
        first: u32,
        second: u32,
        profile: i32,
        old: u8,
    ) -> Result<(), i32> {
        let returned_esp = cpu.esp;
        ram.write_u32(arg1, first)?;
        ram.write_u32(arg2, second)?;
        for value in [second, first, context, dpc, 0u32] {
            cpu.esp = cpu.esp.wrapping_sub(4);
            ram.write_u32(cpu.esp, value)?;
        }
        let ticket = dpc_observer::dpc_profile_begin(task, ram, cpu, profile, routine)?;
        let entered = task.irql_enter_interrupt(ram, cpu, 2);
        let called = match entered {
            Ok(_) => match dpc_observer::dpc_profile_start(task, ram, cpu, ticket) {
                Ok(()) => match dpc_routes::dpc_route_call(task, ram, cpu, routine) {
                    Ok(()) if cpu.esp == returned_esp => Ok(()),
                    Ok(()) => Err(-3), // The reviewed stdcall return consumes 20 bytes.
                    Err(code) => Err(code),
                },
                Err(code) => Err(code),
            },
            Err(code) => Err(code),
        };
        // Attempt both endings even on a callback/observer/IRQL error. End is
        // before restoration as in the original profiled kernel_run_dpc.
        let ended = dpc_observer::dpc_profile_end(task, ram, cpu, ticket, called.is_ok());
        let restored = task.irql_leave_interrupt(ram, cpu, i32::from(old));
        called?;
        ended?;
        restored
    }
}
