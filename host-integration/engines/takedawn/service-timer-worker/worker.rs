// Port-owned timer-worker startup. No host stack, TLS, thread or device access.
use crate::gamepkg;
use crate::layout;
use crate::memory;
use crate::registers;
use crate::stacks;
use crate::thread_tib;
pub struct Worker {
    ram: memory::Memory,
    cpu: registers::Registers,
    lease: stacks::Lease,
    actor: u64,
    stack_key: [u64; 3],
    finished: bool,
    finish_code: i32,
}
fn worker_cpu(
    ram: &mut memory::Memory,
    lease: &stacks::Lease,
    template: u32,
    total: u32,
) -> Result<(registers::Registers, [u64; 3]), i32> {
    let top = lease.stack_guest_top(ram)?;
    let tib = thread_tib::thread_tib_new(ram, template, total)?;
    if tib == 0 {
        return Err(-3);
    }
    // Publish usable CPU state only after the entire original TIB prefix succeeds.
    let mut cpu = registers::Registers::new();
    cpu.esp = top;
    cpu.fs_base = tib;
    let identity = lease.stack_identity(ram)?;
    Ok((cpu, identity))
}
impl Worker {
    pub fn worker_start(
        context: [u64; 4],
        pool_id: u64,
        template: u32,
        total: u32,
    ) -> Result<Self, i32> {
        let (actor, cancelled) = gamepkg::local_task_status()?;
        if actor == 0 || cancelled {
            return Err(-4);
        }
        let source = memory::Memory::with_context(context)?;
        let mapping = layout::Layout::layout_current(&source)?;
        let mut ram = mapping.layout_view(&source)?;
        let pool = stacks::Pool::stacks_borrow(&ram, pool_id)?;
        let mut lease = pool.stacks_allocate(&ram)?.ok_or(-2)?;
        let configured = worker_cpu(&mut ram, &lease, template, total);
        match configured {
            Ok((cpu, stack_key)) => Ok(Self {
                stack_key,
                ram,
                cpu,
                lease,
                actor,
                finished: false,
                finish_code: 0,
            }),
            Err(code) => {
                // TIB memory/heap writes are not rolled back. Release only this
                // actual actor's retained stack, and unregister its heap client.
                let released = lease.stack_release(&ram);
                let closed = ram.heap_close();
                if !lease.stack_released()
                    || released.is_err() && released != Err(-4)
                    || closed.is_err() && closed != Err(-4)
                {
                    return Err(-3);
                }
                // Revocation during cleanup supersedes a retryable setup error.
                if released == Err(-4) || closed == Err(-4) {
                    return Err(-4);
                }
                Err(code)
            }
        }
    }
    pub fn worker_parts(
        &mut self,
    ) -> Result<
        (
            &mut memory::Memory,
            &mut registers::Registers,
            &stacks::Lease,
        ),
        i32,
    > {
        if self.finished || gamepkg::local_task_status()? != (self.actor, false) {
            return Err(-4);
        }
        self.lease.stack_identity(&self.ram)?;
        Ok((&mut self.ram, &mut self.cpu, &self.lease))
    }
    // Owner-only cleanup lane: no usable CPU or mutable lease escapes. This
    // remains available after cancellation/revocation and local finish so the
    // dispatch owner can abort gates and verify idempotent teardown identity.
    pub fn worker_cleanup(&mut self) -> Result<(&mut memory::Memory, [u64; 3]), i32> {
        if gamepkg::local_task_status()?.0 != self.actor {
            return Err(-4);
        }
        Ok((&mut self.ram, self.stack_key))
    }
    pub fn worker_finish(&mut self) -> Result<(), i32> {
        if gamepkg::local_task_status()?.0 != self.actor {
            return Err(-4);
        }
        if self.finished {
            return if self.finish_code == 0 {
                Ok(())
            } else {
                Err(self.finish_code)
            };
        }
        let released = self.lease.stack_release(&self.ram);
        // Attempt both cleanup paths even if revocation made one report Denied.
        let closed = self.ram.heap_close();
        let result = match released {
            Ok(()) => closed,
            Err(code) => Err(code),
        };
        if self.lease.stack_released() {
            self.finished = true;
            self.finish_code = match result {
                Ok(()) => 0,
                Err(code) => code,
            };
        }
        result
    }
    pub fn worker_finished(&self) -> bool {
        self.finished
    }
}
