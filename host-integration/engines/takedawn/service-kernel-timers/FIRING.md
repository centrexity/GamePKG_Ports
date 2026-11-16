# Checked timer firing composition

`firing.rs` connects the checked timer table, finite native DPC adapter and
SDK-owned event shadows. It remains port game code, not trusted runtime/engine
code. No SDK operation, dependency, native handle, callback pointer, task,
record, mutex or quota is added by the composition itself.

`timer_fire_pass(table, map, invoker, &mut Memory, &mut irql::Task,
&mut Registers, &Lease, now_ms:i64, profile_kind:i32) -> Result<[u64;2],i32>`
returns `[timers_fired,next_due_ms_bits]` only after a complete successful pass.
The caller supplies the single guest-millisecond sample taken in the original
worker-loop order, and the reviewed original timer profile kind. The helper does
not replace the caller's pending-DPC drain, vblank/IRQ loop or clock conversion.

Before mutation it validates the native callback's actual worker/controller/
stack/CPU readiness, then the timer table's queue and actual worker. The new
read-only `Callback::callback_queue_id` and `Timers::timer_worker_binding`
expose only opaque port-record/job identities for that comparison. A wrong
queue cannot pass merely because it names the same worker. The shadow view is
validated with a typed zero-time mutex probe; it does **not** wait for the shadow
lock before executing a callback. Preflight errors publish nothing and do not
abort an otherwise healthy actor.

For every original slot in order:

1. The timer scan publishes periodic rearm or one-shot removal under its mutex,
   unlocks, and returns a captured `[timer_va,dpc_va]`.
2. A nonzero DPC runs through the existing `Callback` with arguments zero/zero
   and the supplied profile kind. Null routines still count as fired timers.
3. Only after native code returns, the event adapter looks up the **current**
   mapping, signals it and then writes guest SignalState=1. A callback may have
   replaced or removed the event. Missing mapping is not a skipped callback or
   failed timer, and does not cause a guest write.

Callbacks can rearm/cancel timers while the table is unlocked. The cursor keeps
its original increasing slot index: rearming an earlier slot does not make it
run again in the same pass, and its new due time is not retroactively added to
the original next-due accumulator. Later slots can be changed before they are
visited. Neither callback-returned false nor a missing shadow changes the timer
count. There is no new instruction meter, arbitrary callback cap or render cap.

Any error after scan start attempts cursor abort and permanently aborts the
IRQL actor. An abort-cleanup failure becomes Failed (-3); otherwise the original
error propagates. Already published slots/native side effects/events are not
rolled back or acknowledged as a completed pass. Subsequent calls with that
aborted task are denied at readiness. The worker must still release its retained
stack, return, actually join once and close its owners using the shared join
witness. Stop/report a fatal prefix; do not blindly retry the frame/pass.
This does not undo legitimate earlier callback/IRQ side effects or qualify the
production IRQ hooks, which remain a separate integration requirement.

## Executed scope

`python3 check_firing.py --app APP --app-lib CURRENT_APP_RLIB --wine --evidence DIR`
uses actual SDK workers, locks, queues, IRQL actors, retained stacks, native
callback adapter and event objects. Nine controlled scenarios cover null DPC,
null routine, event replacement and self/later rearm/cancel, unknown native
route, failing native body, read-only guest signal publication, missing mapping,
wrong queue preflight and event removal by a callback. The callback asks a
separate actual producer task to acquire the timer mutex before it returns;
that acquisition proves the callback is outside the lock. Output bytes match
across Linux/Windows x64/i686 component executions (Windows uses Wine).

Required-failing controls signal too early, omit queue binding, swallow signal
failure, skip fatal actor abort, or hold the timer lock across native code.
Source checking precedes generated bindings; all eight optimized game objects
are audited before trusted runtime linking on all four targets. The Linux
x64/i686 packages execute with actual GamePKG confinement. Original timer-table
and callback C differentials are separately requalified against the added
read-only getters.

These are **native composition assertions with controlled finite routines**, not
an independent original-C whole-loop differential or production title handlers.
The fixed test clock and fixture IRQ/profiler providers are not production
implementations. No gameplay, real Windows confinement, physical controls,
audible audio, GPU fidelity or measured frame pacing is established. Full timer/
vblank/spin/TIB/context loop, cancellation/guest waits, actual handlers/profiler
and complete source-kit/builder/launcher closure remain required. No native title
profile, main import, manifest, source archive or launch default is enabled here.
