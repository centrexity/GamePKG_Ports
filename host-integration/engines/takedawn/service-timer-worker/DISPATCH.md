# Retained worker DPC and timer dispatch

`dispatch.rs` composes the original pending-DPC drain followed by the timer firing
pass on `Worker`'s actual retained guest context. The timer's outer loop must
already have performed its vblank tick. This module does not perform that tick,
SpinWake, pause/warp clock advancement, deadline selection or the following wait.
It adds no host operation, game dependency or platform/device access.

## APIs

`Dispatch::dispatch_new(&mut Worker, pending_id:u64, control_id:u64, pool_id:u64,
table_id:u64, map_id:u64, guard_us:i64, lag_ms:i32, profile:i32) -> Result<Dispatch,i32>`
validates the borrowed queue, IRQL controller, stack pool, timer table and event
shadow map against the live worker root/KSV. The table must be assigned to this
queue and actual native job. The stack pool must match the worker's retained
lease. Queue/controller/pool/table/map validation occurs before IRQL task
registration. The callback adapter then validates the resulting actor and CPU;
a failed final validation attempts actor abort rather than publishing a usable
dispatcher. The caller still owns worker finish on construction failure.

`dispatch_actor(&mut self, &mut Worker) -> Result<(&mut Memory,&mut irql::Task),i32>`
provides owner-checked port borrows for the existing typed IRQ attachment API.
Before production hooks run, call `Interrupts::irq_attach_task(ram, task)` on the
actual bound interrupt queue. The IRQ layer checks controller association and
binds its copied record ID. No handler pointer or function value is introduced.
This access point does not attach an IRQ queue by itself; the coordinator must
supply it. It denies another retained worker context and failed/closed state.

`dispatch_pass(&mut self, &mut Worker, vblank_next:u64, warp:bool)
-> Result<[u64;4],i32>` returns [still queued, queued DPCs run, timers fired,
next timer due represented as signed i64 bits]. It checks the exact root,
actual actor and retained [pool,slot,generation], not just a shared pool ID.
Preflight failures do not mutate or poison a healthy dispatcher. After that:

1. Drain pending DPCs with the supplied next vblank edge. Warp suppresses the
   drain edge guard, preserving the original call's zero-edge argument. This
   boolean does not advance the guest clock or implement warp waits.
2. Sample the real host-owned performance counter through the production
   millisecond clock helper, including the original separately truncated skew.
3. Fire timers through the existing callback/event composition.

Clock sampling is after drain. A queued native routine can change timers while
it runs; an earlier sample can miss a newly armed immediate timer. Guard and lag
behavior remain in the existing retained `Drain` object. Caller-supplied values
must come from reviewed configuration; original defaults are guard 1200us and
lag 0ms. The profile value must be the production caller's reviewed category.
There is no callback-local loop execution cap or new source-policy exception.

An error after work starts permanently fails the dispatcher and attempts IRQL
abort. Earlier dequeues, timer updates and native side effects remain published.
The pass returns an error, not partial success, and cannot be replayed. Failed
abort reports Failed (-3); the caller must finish/retry cleanup or end the
confined session. No IRQ is acknowledged or successful callback fabricated.

`dispatch_finish(&mut self, &mut Worker)` validates the exact retained identity
through the cleanup lane, aborts this actor and its gates **before** worker stack
release, then calls `worker_finish`. It does not free a stack if actor abort
fails. `dispatch_closed()` means actor abort completed; it does not mean worker
finish succeeded or that the native job was joined. Inspect both finish results.
Repeated finish delegates to the worker's stable terminal result. The real owner
must still perform one SDK join and close shared owners in their required order.

`Worker::worker_cleanup() -> Result<(&mut Memory,[u64;3]),i32>` is an actual-owner-only
cleanup lane that remains available after cancellation, KSV revocation and local
finish. It returns the cached successful lease identity and checked memory view,
with no usable CPU or mutable lease. It grants no new host capability or lease
extension. The identity is captured inside fallible CPU setup so a failed final
liveness check still follows the normal startup cleanup path. `worker_parts`
continues to reject cancelled or finished workers.

## Qualification

Run `check_dispatch.py --app APP --app-lib APP_RLIB --wine --evidence DIRECTORY`.
Eight modes use actual SDK workers, the serialized heap actor, retained stack,
new worker TIB, production host/guest clocks, queue, IRQL gate, timer table and
native event shadows. Literal finite native routes and inert profiling/IRQ
hooks are **fixture providers**, not production title handlers. Cases cover:

- Drain then timer execution with the real guest stack and TIB/IRQL publication.
- Timer-body and queued-body errors, preserving their different published prefix.
- Cancellation and KSV revocation while a callback holds the real recursive
  dispatch mutex; the owner also refuses premature stack-pool close.
- Another stack/TIB on the same actual job rejected for pass, actor access and
  finish, without poisoning the original dispatcher.
- A timer table bound to another queue rejected before actor registration.
- Read-only guest signal-word failure after the native event was signaled.

The queued routine waits 3ms through a GamePKG mailbox and re-arms an immediate
timer before returning. This detects clock sampling before drain. A foreign
owner verifies the actual gate is Busy while the native callback is blocked,
and acquires it successfully after join. All published trace/FS/ESP, native and
guest signal states, remaining timer slots and zero gate/IRQL counts are checked.

All four Linux/Windows x64/i686 targets produce the same 576-byte witnesses;
Windows component execution uses Wine. Six required-failing copies change pass
order, clock order, retained-stack admission, retained-stack close, queue binding
or cancelled cleanup access. Complete source scanning precedes SDK injection;
every one of eight optimized game objects per target is audited before trusted
linking. Actual confined Linux x64/i686 children execute the same cases and an
unused maintainer shell in game source rejects. The ten-mode startup matrix is
also requalified for the cached identity and cleanup lane. SDK remains
`fe309935dd602e2cf098e50065f6035de33dac88b51d45a2bc1ba47cf026742f`.

This is real-clock native dispatch composition with synthetic guest memory and
finite test routines. It is not original-C whole-loop, actual IRQ/device/profiler
handler or title qualification. Production context/IRQ attachment, vblank,
spin/pause/warp, pacing/deadline/wait integration, full generated source and
builders/launchers remain required. It establishes no title gameplay, GPU
fidelity, physical input, audible audio or measured frame pacing. Both unmigrated
title profiles remain blocked.
