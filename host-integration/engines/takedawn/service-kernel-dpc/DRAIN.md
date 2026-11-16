# Retained native deferred-work drain

`drain.rs` composes the checked queue, locked observation, native callback, IQL
gate and retained guest-stack lease. It translates original
`kernel_drain_dpcs_before` without an interpreter, direct OS/device calls, game
dependencies, callback pointers or trusted engine code. The production timer,
finite title routines, profiler/configuration and full bridge remain unfinished.
Neither native engine profile is enabled. No SDK operation, quota, root/KSV3/CPU
format or four-ID native context changes.

## Exact API

- `drain_guard_ns(microseconds:i64) -> u64` safely resolves the original guard:
  nonpositive is zero, 1..3999 multiply by 1000, >=4000 is 4,000,000 ns. The
  original default is 1200 microseconds. Extreme i64 inputs clamp without signed
  multiplication overflow; the C comparison covers its defined launch values.
- `Drain::drain_new(&Memory,&Queue,&Controller,&Pool,guard_us:i64,lag_ms:i32,
  profile:i32) -> Result<Drain,i32>` is restricted to the assigned actual timer.
  It creates a nonowning port value with a validated Callback binding, resolved
  configuration and initially zero retained host-lag timestamp. Construct once
  and retain across passes, not once per frame or DPC. No SDK object is allocated.
- `drain_before(&mut self,&mut Memory,&mut Task,&mut Registers,&Lease,due:u64)
  -> Result<[u64;2],i32>` returns `[still_queued,ran]`. The first lane is 0/1;
  ran is the original wrapping 32-bit count represented as u64 bits. Cancelled
  holes and null routines are consumed without incrementing ran. Unknown nonzero
  native routes are explicit Unsupported, not silently skipped.
- `drain_last_host_ns() -> u64` exposes retained lag bookkeeping for diagnostics,
  not a host clock, permission or resource identity.

`Callback::callback_ready(&Memory,&Task,&Registers,&Lease) -> Result<(),i32>`
is the explicit nonmutating readiness boundary also used by callback_run. It
checks exact root/KSV3/queue/controller/actual-worker/pool/lease identity, live
ownership, v5 state and available stack range. Readiness does not prove that the
original guest TIB was initialized; that remains bootstrap work.

All errors use SDK Invalid -1, Busy -2, Failed -3, Denied -4, Unsupported -5.
Preflight errors do not mutate or poison an unrelated actor. Errors after a
pass begins abort the assigned actor using existing bounded IQL cleanup and
are never reported as a successful pending result or acknowledgement.

## Original sequence

Positive lag first samples **host** time. If wrapping elapsed time is below
lag_ms * 1,000,000, return current head != tail without updating the retained
timestamp. Otherwise update it, including when the queue is empty or the gate
will be busy. Nonpositive lag does not sample host time. These are resolved
settings for this retained value; original configuration parsing, lazy-cache/
reload behavior and diagnostics must still be supplied by the production port
adapter. No game std::env access is allowed.

For each entry, a nonzero due enables the resolved guard. Check head != tail
on the timer, then sample **guest** time (host counter plus shared skew). Defer
when now < due and wrapping now + guard >= due, including the exact edge.
An expired due permits work; due=0 disables the guard. Do not replace unsigned
wrapping arithmetic with saturation or use queue_pending, which suppresses the
timer's own pending indication for a different purpose.

Attempt the original recursive dispatch gate with a **zero-time try**. Busy is
a successful defer: record its optional metric and return head != tail/ran.
Never wait behind a title holding the gate on the vblank/timer source.

While gated, acquire the queue observation. Empty releases its queue lock,
then perform original full IQL gate leave and return zero pending. Otherwise
record depth while the queue lock and old head/Inserted flag are intact, publish
head, clear Inserted and unlock. Cancelled holes are still consumed. A nonzero
DPC/timestamp invokes the original latency-statistics boundary before the
native callback, even when a null routine means no callback will run. The
callback performs the original argument/stack/priority sequence. Count only a
successful executed callback, then perform the full safe/release gate leave
and continue. Newly queued work can be consumed in the next iteration, including
a callback that requeues itself after Inserted was cleared.

The loop preserves the original work-drain semantics; it adds no instruction
meter, fixed render limit or arbitrary maximum callback count. Cancellation and
host session/memory/time policies remain unchanged. Blocking service waits stay
off the window owner; this value runs only on its assigned reviewed worker.

## Providers, lifetimes and failure

Stage `clock.rs` as checked `dpc_clock` for production:
`dpc_host_now(&mut Memory)` / `dpc_guest_now(&mut Memory) -> Result<u64,i32>`
delegate to the existing wait_service host-owned counter and guest-skew helpers.
The mutable port interface permits local accounting in tests; it exposes no
native clock or arbitrary wait. The fixture clock is a separate checked source
variant and must not become a production clock.

In addition to the Callback profiler interface, checked `dpc_observer` supplies:

- `dpc_drain_busy(&mut Task,&mut Memory,&mut Registers) -> Result<(),i32>`.
- `dpc_drain_depth(...,depth:u32) -> Result<(),i32>`, while still queue-locked.
- `dpc_drain_latency(...,timestamp_ns:u64) -> Result<(),i32>`, after queue unlock
  and before native execution, only for a nonzero DPC/timestamp.

The production provider must preserve optional profiler behavior, latency
histogram/reporting and original counter semantics. It must remain local/bounded
under the queue lock and must not execute title code there. These functions are
checked game code, not host-injected callbacks or an engine exemption. The
deterministic fixture observer records call order/values only; it does not
implement or qualify the real histogram/profiler.

On an observer, dequeue, callback, clock or gate-leave error, first attempt
observation cleanup, then abort the IQL actor. Do not run another safe point or
acknowledge work after a failed prefix. A callback may already have modified RAM,
and head may already have advanced. Do not retry blindly. If cleanup fails,
return Failed and stop the session or finish owned cleanup before allowing
other workers to continue. No cleanup bound is increased, foreign hold released
or task completion fabricated. The retained stack remains caller-owned: release
it on every worker exit, then actually join before closing queue/controller/
pool/root. The notifier/sleeper readiness handshake, timer/event/TIB/spin and
native-context integration are still required.

## Qualification

Run `python3 check_drain.py --app APP --app-lib LIB --wine --evidence
PRIVATE_NEW_DIRECTORY` as one command. Original archive/file and exact C
run/guard/drain bodies are pinned in `drain-tests/reference_inputs.json`.
The unmodified bodies use macro-renamed per-case functions so their original
static guard/lag caches persist independently, matching one retained Drain
per scenario. C providers for clocks, native lookup/routines, gate contention,
IRQL and profiler observation are controlled test code, not original production
devices or a complete timer. Python/C are maintainer-only.

96 retained scenarios with four passes each compare 384 ordered witnesses per
target: all 316 CPU bytes, complete 512 KiB guest-RAM and canonical ring hashes,
pending/ran, host/guest clock-read counts and priority. Coverage includes original
default/clamped/disabled guards, exact guard edges, unsigned wrap, expired/zero
due, positive/negative lag, persistent lag timestamp, empty/wrapped queues,
cancelled holes, null routines, full-width arguments, normal/manual finite routes,
FS/priority changes, optional profiling and self-requeue. Actual parent/consumer
tasks create real foreign-gate Busy states; the timer retains one guest lease
through all passes. Nine error-worker scenarios verify observer, clock,
unsupported/body/bad-return/start/end failures, partial publication, permanent
abort, own/foreign hold accounting, queue unlock, lease release and actual join.

Required failing controls change the inclusive guard edge, replace wrapping
with saturation, count holes, reset retained lag or move depth observation after
publication. None is admitted as game code. Current notification and Callback
fixtures are also requalified with the new queue/ready source hashes.

All fixture sources, including production clock wrappers, precede host binding
injection; every one of eight separately optimized objects is audited before
trusted linking on Linux32/64 and Windows32/64. Exact SDK/source/compiler/target/
object/executable identities remain in private receipts. Confined Linux runs
are distinct from Windows component execution through Wine and Windows artifact
compile/link checks. These are synthetic native routines and controlled clocks;
they do not prove real Windows confinement, title gameplay, hardware graphics,
physical controls, audible audio or measured 60 FPS/frame-work time.

Finish production route/profiler/configuration adaptation, timer/event/spin/TIB/
context, complete generated source closure and renderer/media/bootstrap/launcher
integration before native title admission or gameplay qualification.
