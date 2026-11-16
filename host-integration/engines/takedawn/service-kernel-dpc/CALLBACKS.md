# Checked native deferred-callback contract

`callback.rs` translates original `kernel_run_dpc` using checked game source.
It is not a CPU interpreter, system-service implementation or trusted engine
bridge. It supplies the calling convention and lifecycle checks; production
finite title routes, profiler, timer and queue drain still need integration.
Neither native engine profile is enabled. No dependency, raw function pointer,
host address, SDK operation, record format, quota or import exemption is added.

## API and ownership

`callback_ready(&Memory,&Task,&Registers,&Lease) -> Result<(),i32>` exposes the
same nonmutating preconditions used by callback_run so the retained drain can
validate its actual consumer/stack even on empty or deferred passes. This does
not initialize or qualify a production TIB. See [DRAIN.md](DRAIN.md).

`Callback::callback_new(&Memory,&Queue,&Controller,&Pool)` returns a nonowning
Rust value bound to the root/KSV3, assigned actual queue consumer, IQL controller
and guest worker-stack pool. An unassigned queue is Unsupported (-5); a finished
consumer is Denied (-4). No resource is allocated or lifetime extended. Use the
one title-owned controller/pool/queue, not a new instance per invocation.

`callback_run(&self,&mut Memory,&mut Task,&mut Registers,&Lease,dpc:u32,
arg1:u32,arg2:u32,profile:i32) -> Result<bool,i32>` checks:

- Same live root/KSV3 and IQL association and the actual assigned worker task,
  not a copied owner number or another Task view.
- Same queue assignment and the actual current worker's live stack lease,
  including its pool/generation checks.
- Valid v5 register state and guest ESP within that lease's original 256 KiB
  slice, with room for all five pushed words. The upper bound is the original
  worker top minus 16, as returned by `stack_guest_top`.

These preconditions apply even to null requests. On a valid worker, null DPC
or null routine returns false without argument/stack/profile mutation. Unknown
nonzero routes return **Unsupported before mutation**, rather than reproducing
the original warning-and-skip behavior as silent success. The actor must report
that required migration gap. Other errors retain SDK Invalid -1, Busy -2,
Failed -3 and Denied -4. No guessed implementation or direct native fallback runs.

The caller retains the worker's stack lease and TIB for the timer's whole life.
This function neither allocates/releases the lease nor resets the guest stack
on each DPC. Initialize ESP and FS once using reviewed bootstrap/TIB code.
The queued-work drain must first acquire the dispatch gate with a **zero-time
try**, as in the original, because this worker is also the vblank source. This
callback helper alone does not implement that drain. Expired-timer callbacks
retain the original separate entry behavior: the IRQL transition itself acquires
the bounded raised hold when needed. Do not block the app's window owner.

## Native convention and ordering

The routine and context are read from KDPC +12/+16. Before dispatch, +20/+24
receive arg1/arg2. The helper pushes arg2, arg1, context, DPC, then a zero return
word. Thus ascending memory at entry ESP is `[0, dpc, context, arg1, arg2]`.
Guest arithmetic is fixed u32 with checked object offsets and checked RAM writes.
Writes are ordered and can fail partially; they are not a transaction.

Profiling begins before entering priority 2; its start sample occurs after that
entry. The finite checked native routine then runs. Profiling ends before
restoring the saved priority, including when the routine changed priority itself.
The callee's reviewed stdcall return must consume the dummy return and four
arguments (20 bytes), restoring the incoming ESP. A bad return is Failed and
never acknowledged as a completed DPC. This is a convention check, not a new
instruction, frame-work or gameplay-duration limit.

On success, preserve the native routine's register/RAM/FS changes. This is **not**
the IRQ interrupt-register rollback contract: no full CPU snapshot is restored
after a DPC. The IRQL epilogue publishes the restored level into the current
guest FS/PCR when the original transition requires it.

After mutation begins, any write, observer, priority-entry, callback, bad-return,
profile-end or restoration error triggers error/stop `irql_abort`. End and
priority restoration are both attempted after entry/start/callback failures.
Abort must release counted/masked ownership and all owned raw holds using the
existing bounded cleanup, without allocating a new retry event. Failed cleanup
returns Failed, never a callback acknowledgement. The worker is permanently
aborted and must not resume title code. The caller must still release its stack,
stop producers and actually join it; the helper does not fabricate completion.
If abort cannot finish within its existing bound, stop the session or finish
owned abort cleanup before allowing other workers to continue; do not interpret
Failed as proof that every raw hold was released. No cleanup bound is raised.
Preflight errors have not acquired holds or mutated arguments and leave cleanup
of a caller's existing outer drain hold to that caller.

Stop/join all users before closing the queue, controller, pool or root. Opaque
IDs are not lifetime leases. Concurrent revocation can produce an error after
a partial prefix; stop/report it, do not blindly retry a callback whose side
effects may already have run.

## Required literal providers

Both modules are checked **port** sources, scanned with the rest of the title
before host bindings are injected and included in the source/object receipts.
They must not be injected as trusted host code or replaced with fixture stubs.

`dpc_routes` supplies:

- `dpc_route_supported(routine:u32) -> bool`: finite reviewed guest targets.
- `dpc_route_call(&mut Task,&mut Memory,&mut Registers,routine:u32)
  -> Result<(),i32>`: static literal calls to generated native Rust functions.

The two tables must agree. Unsupported and partial route failures are explicit.
No dynamic resolver, arbitrary callback value, unsafe/extern, native address,
SDL/system function or unapproved crate is admitted.

`dpc_observer` supplies:

- `dpc_profile_begin(&mut Task,&mut Memory,&mut Registers,profile:i32,
  routine:u32) -> Result<u64,i32>`: begin before raise; return a port-owned
  bookkeeping ticket, zero when disabled. This is not a host pointer or grant.
- `dpc_profile_start(&mut Task,&mut Memory,&mut Registers,ticket:u64)
  -> Result<(),i32>`: sample after priority entry, before the routine.
- `dpc_profile_end(&mut Task,&mut Memory,&mut Registers,ticket:u64,
  completed:bool) -> Result<(),i32>`: finish before restoration and release
  all ticket bookkeeping, including after failed entry (no start sample), failed
  start, callback failure or bad return. `completed` means the callback returned
  successfully with the required convention, not that no partial effects exist.

A provider that returns an error from begin must clean any unpublished ticket
itself. End must accept zero and unstarted tickets. Preserve original optional
profiling configuration, intervals and reporting in real port code; the fixture
observer records deterministic ordering only and measures no performance.

## Qualification and limits

Run `python3 check_callbacks.py --app /path/to/app --app-lib
/path/to/libgamepkg.rlib --wine --evidence /private/new-directory` as one command.
Python/C and the reference shell are maintainer-only, outside admitted sources.

`callback-tests/reference_inputs.json` pins the source archive, kernel_bridge.c
and exact original `kernel_run_dpc` body. The C shell supplies controlled
lookup/manual-lookup native functions, profiling and IRQL providers. It does
not qualify original gate races, diagnostics, hook publication or a real title
routine. The separate original IRQL/queue/stack libraries retain their own
qualification; do not conflate their witness scopes with this shell.

256 cases per target compare all 316 CPU bytes, a hash of the complete 512 KiB
guest RAM, return value, restored priority and ordered profile records. They
cover null DPC/routine, normal/manual target lookup, profiling on/off, old
priorities 0/1/2/4, callback priority changes, FS changes, full-width arguments
and both native routes. Actual checked worker execution additionally verifies
count/hold restoration and retains one guest-stack lease across all callbacks.

Boundary tests cover unassigned/wrong-worker use, unknown finite target without
mutation, out-of-slice ESP, overflowing object arithmetic, callback error, bad
stdcall return, profile-start/end errors, read-only argument view and partial
IRQL entry through a read-only PCR view. They verify permanent actor abort,
zero raised/masked counts, release of an existing outer recursive hold, stack
release and actual task join. A worker error is reported explicitly through the
fixture mailbox rather than inferred from a timeout.

Wrong argument order, restoration before profiling end and omitted return
validation must each fail; these mutated maintainer executables are never
admitted. Complete source scans precede host injection; eight independently
optimized game objects are collectively audited before trusted linking on
Linux32/64 and Windows32/64. Exact SDK/source/compiler/target/object/executable
identities remain in private receipts. Original no-alias admission remains
unchanged; provider names are explicit and distinct from local bindings.

Confined Linux fixtures and Windows component execution through Wine are
different from real Windows confinement or hardware gameplay. No production
native DPC handler, queue drain, timer/event shadow, spin/TIB/context wiring,
renderer/media/launcher, physical control, audible audio or measured 60 FPS
claim follows from these synthetic callbacks. Finish those integrations before
enabling a native title.


The maintainer witness transport retains its single 336-byte mailbox slot.
Busy is retryable backpressure: `callback_send` retries using existing local
clock samples and bounded retry pauses for at most one counter-frequency second,
then returns Busy. Other errors propagate. This applies to witnesses and worker
error reports so an already-full channel cannot silently lose a report and turn
it into a receiver timeout. It does not alter callback scheduling, guest state,
quota, service timeout, title admission or the original C oracle.
The separate maintainer shell tests a deliberately full queue with no consumer
and a delayed consumer, and rejects controls that drop the witness or return
Busy immediately. The game fixture uses only the generated shared bindings.
