// Original firing composition. No native callback pointer or trusted engine code.
use crate::callback;
use crate::events;
use crate::gamepkg;
use crate::irql;
use crate::memory;
use crate::registers;
use crate::shadow;
use crate::stacks;
use crate::timers;
pub fn timer_fire_pass(
    table: &timers::Timers,
    map_view: &shadow::Shadow,
    invoker: &callback::Callback,
    ram: &mut memory::Memory,
    task: &mut irql::Task,
    cpu: &mut registers::Registers,
    lease: &stacks::Lease,
    now: i64,
    profile: i32,
) -> Result<[u64; 2], i32> {
    invoker.callback_ready(ram, task, cpu, lease)?;
    let binding = table.timer_worker_binding(ram)?;
    if binding[0] != invoker.callback_queue_id()
        || gamepkg::local_task_status()? != (binding[1], false)
    {
        return Err(-4);
    }
    // Validate the shadow view without waiting for its lock before a callback.
    // The original takes that lock only afterward when looking up the event.
    shadow::Shadow::shadow_borrow(ram, map_view.shadow_id())?;
    let mut scan = table.timer_fire_begin(ram, now)?;
    let result = timer_fire_loop(
        table, map_view, invoker, ram, task, cpu, lease, profile, &mut scan,
    );
    if let Err(code) = result {
        // Published slots/callback side effects are not rolled back or retried.
        // The scan has no held resource; the actor still owns stack/join cleanup.
        let _ = scan.timer_scan_abort(ram);
        if task.irql_abort(ram).is_err() {
            return Err(-3);
        }
        return Err(code);
    }
    result
}
fn timer_fire_loop(
    table: &timers::Timers,
    map_view: &shadow::Shadow,
    invoker: &callback::Callback,
    ram: &mut memory::Memory,
    task: &mut irql::Task,
    cpu: &mut registers::Registers,
    lease: &stacks::Lease,
    profile: i32,
    scan: &mut timers::Scan,
) -> Result<[u64; 2], i32> {
    loop {
        match table.timer_fire_next(ram, scan)? {
            None => return scan.timer_scan_result(ram),
            Some(entry) => {
                if entry[1] != 0 {
                    invoker.callback_run(ram, task, cpu, lease, entry[1], 0, 0, profile)?;
                }
                // Resolve the current mapping after native code: a callback may
                // have replaced or removed this timer's shadow event.
                events::timer_signal_after_callback(map_view, ram, entry[0])?;
            }
        }
    }
}
