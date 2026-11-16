# Checked native IRQ queue and callback epilogue

Port-owned dependency-free Rust composes the existing actual dispatch gate,
IRQL bookkeeping, guest worker stacks, register preservation and GamePKG local
services. It does not provide real NV2A/APU/OHCI device handlers, execute a title,
expose host pointers or move an engine behind trusted host code. The legacy
profile remains blocked. See WORK_IN_PROGRESS.md for remaining integration.

## Modules and exact API

Stage `irq.rs` as `irq_delivery`, `hooks.rs` as `irq_hooks`, and `clock.rs` as
`irq_clock`. The production clock delegates to the existing host-owned
`wait_service::host_now_ns`; it reads no wall clock or direct OS timer. Existing
`irql`, `gate`, `stacks`, `memory`, `registers`, `kernel_pool` and host-injected
`gamepkg` are required. Supply literal checked `irq_routes` / `irq_observer`
modules with the signatures below. No function pointer, dynamic callback name,
trait object, arbitrary shader string or new host operation is accepted.

All fallible operations return `Result<...,i32>`:

- `Interrupts::irq_new(&Memory,&irql::Controller,&stacks::Pool,handlers:u32,
  safe:bool,force:bool,wait_ms:i32,prompt:bool) -> Interrupts` creates one IRQ1
  record bound to the live root/KSV3, IQL controller, raw gate and worker pool.
  Handler mask must be 0..7 and a subset of the literal supported route mask.
  Original defaults are handlers supplied by boot, safe=true, force=false,
  wait_ms=250, prompt=true. Signed wait conversion preserves original unsigned
  ns wrapping, including negative settings; it does not silently clamp them.
- `irq_id() -> u64`, `irq_borrow(&Memory,id) -> Interrupts` expose only copied
  opaque identity/nonowning views. Borrow validates exact format/bindings and
  live IQL/gate/pool. Bootstrap must create one queue per title and share its ID.
- `irq_attach_task(&Memory,&mut irql::Task)` verifies the IQL association and
  installs the queue as that Task object's immutable opaque hook context.
  **Every newly constructed Task view starts unbound.** This does not expand
  IQL1 serialization, KSV3/root or the four-ID native context automatically.
- `irq_pending(&Memory) -> u32` returns the three coalesced bits.
- `irq_post(&mut Memory,&mut Task,&mut Registers,line:u32) -> i32` accepts only
  lines 0=vblank, 1=APU, 2=OHCI. Result **0 Posted** means left for the holder or
  already handled; **1 Gated** means the caller owns one additional actual raw
  hold and must perform its real finite device delivery then GateLeave;
  **2 Ungated** requests original configured ungated delivery. Neither result
  1 nor 2 claims a handler ran or grants direct OS/device access. The caller
  still uses checked port callbacks and GamePKG-owned services.
- `irq_safe_point(...)` runs pending work only on the actual gate holder with
  IRQ masking clear, outside ISR and at IRQL <= 2. It atomically takes a batch
  and visits lines in ascending order; new posts form the next batch. No worker
  stack restores the current AND remaining bits, increments refusal and returns
  without spinning or acknowledging delivery. No registered handler consumes
  its bit as the original does, without an on-holder acknowledgement.
- `irq_release_point(...)` refuses masked/in-ISR/high-IRQL delivery; otherwise
  it tries the raw gate once, drains if acquired, releases raw ownership and
  calls the literal DPC-release boundary. It does not run DPCs inline.
- `irq_note_ungated(&Memory,line)` records original ungated diagnostics only
  while somebody holds the gate; actual bootstrap task 0 is valid ownership.
- `irq_stats(&Memory,reset:bool) -> [u32;20]` returns posted[3], on-holder[3],
  second-try[3], forced[3], ungated[3], maximum-wait-us[3], wait-logs, refusals.
  Counters wrap as original u32 bits. Copied reset is one coherent CAS snapshot.
- `irq_close(&mut self,&Memory,discard:bool) -> u32` is creator-only and
  nonblocking. Active delivery or a held raw gate is Busy, including for discard.
  Pending work is Busy unless **explicit shutdown discard** was requested; a
  successful return identifies cancelled bits, never callbacks acknowledged.
  Closing denies new use and supports owner retry after partial record release.
  Inspect `irq_closed()` after cleanup errors; completed close is idempotent.

The raw gate's first try, post/coalesce and second try close the original
post/release race. First-post timestamps remain unchanged on coalesced repeats.
Safe-mode-off ungating and forced threshold logic still obey global CLI masking.
The lower's release point runs only after the new IRQL/PCR byte is published.
IRQL hooks resolve their per-object queue context and validate association,
rather than assuming a copied record ID establishes type or lock ownership.

`irq_wait_maximum(now_ns,posted_ns,previous:u32) -> u32` explicitly uses original
signed32 Xbox/Windows LONG conversion, on every architecture. Previous maxima
must be nonnegative signed32. A wrapped negative diagnostic sample does not
raise the maximum. It does not stop a callback or limit gameplay duration.
Original Linux64 compares a wider `long` before narrowing to LONG and can store
a negative or smaller maximum for large waits. Its successful CAS exits the
loop. The portable code follows original Windows/i686 conversion instead of
introducing a >35-minute failure or reproducing that host-width discrepancy.

## Literal port callbacks

`irq_routes` supplies `supported_mask() -> u32` and `vblank`, `apu`, `ohci`, each
`(&Interrupts,&mut irql::Task,&mut memory::Memory,&mut registers::Registers)
-> Result<(),i32>`. These must become the real finite generated native handlers,
not engine/media code relabelled trusted or fixture functions shipped as a game.

`irq_observer` supplies checked port functions:

- `post_window(&Interrupts,&mut Task,&mut Memory,&mut Registers,line:u32)`
- `release_window(&Interrupts,&mut Task,&mut Memory,&mut Registers)`
- `spin_wake(&Interrupts,&mut Task,&mut Memory)`
- `warn(&Interrupts,&mut Task,&mut Memory,kind:u32,old:u8,new:u8)`
- `dpc_release(&Interrupts,&mut Task,&mut Memory)`
- `wait(&Interrupts,&Memory,pending:u32,oldest_ns:u64,now_ns:u64,refusals:u32)`

All return `Result<(),i32>`. The wait observer is diagnostic-only/best effort,
matching original ignored logging failure; its event count still records the
attempt. DPC/spin observers must connect actual original timer/consumer roles;
fixture entry counters are not an implementation of those roles. The prompt
word is retained in the exact record for the real DPC adapter. Do not wake the
single clock lane as an unexplained substitute for original spin notification.

A delivered queued line acquires a real guest stack lease, notes waiting time,
saves complete v5/316 CPU values, sets the original worker ESP/zero GPR prefix,
marks ISR, enters IRQL16 and calls one literal route. It restores interrupted
IRQL even if the handler returned at a different level, clears ISR, restores
arithmetic state while retaining current FS, releases the stack and active
count, then increments on-holder. Error cleanup attempts all applicable steps;
failed abort cleanup is Failed, not a fabricated successful restoration.
Current/remaining pending bits are restored on errors. Stop the failed actor/
session rather than retrying a partially executed callback as if it rolled back.

## State and resource lifetimes

IRQ1 is exactly **384 bytes**, LE u64 words. Header offsets: magic 0x31515249 @0,
root @8, KSV3 @16, IQL @24, raw gate @32, worker pool @40, handler mask @48,
pending @56, safe @64, force @72, wait ns @80, prompt @88, last-log ns @96,
refusals u32 @104, closing @112, wait-logs u32 @120, active 0..33 @128;
@136..184 are seven reserved-zero words. Three 64-byte line records @192 store
[first-post ns, posted, on-holder, second-try, forced, ungated, maximum-wait-us,
reserved zero]. Counter words contain u32 bits; wait maxima fit signed32.
Malformed flags/ranges/reserved values, foreign associations and stale records
reject before use. This grants no host address, page permission or external IO.

Errors retain SDK meanings: -1 Invalid, -2 Busy, -3 Failed, -4 Denied,
-5 Unsupported. Normal record retries and every actual lock/wait remain bounded.
Cleanup retries in IQL abort/cleanup mutations, stack release/close and IRQ
active/pending/close use bounded **existing-record-only** loops. They cannot rely
on allocating a new wait event when the sync quota is full. Ordinary generic
SDK events remain available after cancellation; the reproduced bug was quota
exhaustion combined with a stale CAS, not a blanket event-cancellation denial.
The 64-attempt bound and all host resource limits remain; exhausted cleanup
returns an explicit error and may require retry or session shutdown.

Stop/join actual users before unloading code or closing dependencies. Order:
IRQ close/cancel pending, stack pool close, IQL/raw gate close, KSV close, root
close. Revocation denies ordinary work, but owned cleanup can finish and still
return Denied; inspect closed/released state. No creator-supplied finished flag,
foreign task ID or copied queue token grants a join or another task's ownership.

## Validation boundary

`check_differential.py --app APP --app-lib LIB --wine --evidence NEW_PRIVATE_DIR`
compares 128 original-C witnesses per target: CPU316, stats80 and normalized
state/RAM hash56 = **452 bytes/case**. The archive and 40 original function/type
ranges are pinned. Default coalescing, ordered traces, IRQL, guest stacks and
exhaustion retry are covered. Signed32 wait-stat comparisons run 36 boundaries
on each C ABI with 32-bit long (Linux32, Windows32/64); Rust checks portable
boundaries on all targets, including delivery after long waits. Linux64's
native-width narrowing differences are observed separately as described above.

`check_smoke.py --app APP --app-lib LIB --wine --evidence NEW_PRIVATE_DIR`
runs 32 suites/target: 128 worker interleavings, 192 option cases, 64 callback/
new-post/cancellation cases, plus malformed/state/lifetime/quota controls.
Checks cover before-post and after-post release, another holder consuming the
bit, original release-window post, [0,2,0,1] arrival order during a callback,
active-close denial, actual cancellation, full sync quota plus a forced stale
abort CAS, CLI force suppression, threshold +/-1ns, unrelated pending lines,
wait-log rate limits, negative settings, FS retention, callback errors,
read-only PCR, KSV revocation, absence of handlers and wrapping/reset statistics.

Race fixtures explicitly transform a checked copy with an after-mark pause and
once-only stale-abort seam. Production sources contain neither seam. Production
and instrumented hashes/receipts are retained separately. The latter uses IRQ
last-log @96 as a test-only once flag during cancellation. Required-failing
copies omit the second try, publish release before PCR, drop the exhausted
current bit, or restore event-dependent abort; none is admitted as game code.
`--fault-controls-only` runs native negative controls and cannot issue artifact
receipts. Fixtures, Python, C/FFI and bootstrap are maintainer-only.

Complete fixture sources scan before binding injection; all eight separately
optimized game objects audit before trusted runtime link. Source/dependency/
import policy, no-post-audit-LTO, existing compiler/process/resource limits and
profile gates remain unchanged. Actual confined Linux32/64 children repeat
vectors/boundaries. Windows components run through Wine; their audited artifacts
are cross-target compile/link checks, not native Windows confinement. There is
no qualification of real handlers, graphics, physical controls, audible audio,
gameplay or measured frame pacing from these tests.
