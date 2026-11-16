# Timer-worker startup and retained guest context

`worker.rs` composes the checked memory layout, worker-stack allocator, serialized
heap actor, per-worker TIB/TLS helper and native register values. It is ordinary
dependency-free port Rust. GamePKG owns every actual task, mailbox and RAM backing.
No host thread/TLS/stack pointer, OS/device call or permission exemption is added.

## Contract

`Worker::worker_start(context: [u64;4], pool_id:u64, template:u32, total:u32)
-> Result<Worker,i32>` must run on the actual reviewed worker, not the bootstrap
actor. `context` is the existing `Memory::thread_context()` value: root record,
optional kernel-import record and the two initialized heap endpoint IDs. It does
not copy an exclusive heap client or reply sequence. `pool_id` is the one shared
`Pool::stacks_id()` created from applied `Bootstrap::boot_layout()` indices 1/2.
TLS template and total come from indices 7/8 of that same applied bootstrap.
The outer coordinator still must publish and pass these values; this helper does
not discover an unpassed title context or add a registry to KSV3.

Startup rejects bootstrap task 0 or cancellation with Denied (-4), reconstructs
the checked memory view and validates the pool's root/KSV association. It then
allocates one slice owned by the actual worker. Exhaustion is Busy (-2), not a
fabricated stack. The worker lazily registers its own heap client and creates
its TIB using `thread_tib_new`. A zero TIB (no TLS or heap exhaustion) is Failed
(-3): the original timer worker also exits without entering its loop in this
case. Other checked errors propagate. Only after all setup succeeds are a fresh
native `Registers` value, ESP at the retained slice top and FS base at the new
guest TIB made available. These are original recompiled-function values, not
native CPU registers or an instruction decoder.

A failed setup attempts to release its own slice and unregister its heap client.
TIB allocations and published RAM prefixes are not rolled back. Incomplete or
unexpected cleanup is Failed; completed cleanup reporting revocation returns
Denied, superseding a retryable setup error. End a failed session if cleanup
cannot be established. No other actor's stack is reclaimed or joined here.

`worker_parts(&mut self) -> Result<(&mut Memory, &mut Registers, &Lease), i32>`
checks the same actual actor, cancellation, live root/KSV and retained lease.
It returns simultaneous safe borrows suitable for the existing drain/callback/
timer APIs. The slice stays retained across calls and timer iterations; callers
cannot acquire a mutable lease through this method. Keep these parts through the
actual timer loop rather than reconstructing a new stack or TIB for each pass.

`worker_finish(&mut self) -> Result<(), i32>` is actual-owner-only, including
cleanup after cancellation. Before calling it, the caller must complete or abort
its IRQL/callback actor and release any gates according to their contracts. The
helper releases its retained slice and attempts heap-client closure even if the
first action reports an error. If the slice remains held, the caller can retry;
otherwise the terminal result is retained and repeated finish calls return the
same result, including errors. `worker_finished()` means the local slice was
released and heap closure was attempted; it is neither a successful cleanup claim
nor native-task completion/join evidence. Always inspect the finish result.
`worker_parts` denies after finish. Guest TIB memory is not freed implicitly.

The outer owner must still stop producers, perform the one actual SDK join,
retain `LocalTaskJoin` for all shared controller owners, and close those owners
and the common heap in their required order. A cancelled worker's heap actor
registration may remain until the primary owner stops the actor; local finish
does not establish completion of that shared teardown. No record schema, quota,
wire operation or SDK identity changes.

## Qualification and limits

Run `check.py --app APP --app-lib APP_RLIB --wine --evidence DIRECTORY`.
Ten modes exercise actual SDK workers and a separate heap actor: successful
startup, no TLS, exhausted heap, absent heap endpoints, exhausted stack pool,
foreign pool root, malformed four-ID context, configuration revoked before
startup, revocation while live, and cancellation while blocked. The real owner
observes a retained slice and refuses premature pool close before releasing the
worker's wait. Native CPU defaults, guest ESP/FS, SEH, TLS slot zero, a selected template
byte and cleared thread data are checked. Repeated parts retain the same slice;
finish frees it and preserves errors on repeated calls. Bootstrap startup denies.

All four Linux/Windows x64/i686 targets produce identical 960-byte witnesses;
Windows component execution uses Wine. Six intentionally broken variants must
fail: bootstrap admission, wrong FS, wrong ESP, failed-start stack leak,
finish stack leak and erased finish error. The complete source/helper tree is
scanned before SDK injection; all eight separately optimized game objects per
target are audited before trusted linking. Actual confined Linux x64/i686 children
run the same fixture and reject an unused maintainer shell in game source.
SDK: `fe309935dd602e2cf098e50065f6035de33dac88b51d45a2bc1ba47cf026742f`.

This is startup/lifetime composition with synthetic title memory, not an
independent original-C whole-worker differential. The stack and TIB primitives
have their own original-C qualifications. Production configuration publication,
IRQL/IRQ/profiler/device handlers, spin/pause/wakeup and the retained timer loop
still need wiring, followed by full generated source/build/launcher closure.
This does not prove title startup/gameplay, physical controls, audible audio,
GPU fidelity, native Windows desktop behavior or measured frame pacing. Both
unmigrated title profiles remain blocked.
