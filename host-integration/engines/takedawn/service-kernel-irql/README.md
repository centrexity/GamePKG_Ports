# Checked native interrupt-priority component

This is port-owned dependency-free Rust over the GamePKG local services. It is
not an interrupt decoder, native CPU emulator, production IRQ dispatcher or a
playable Burnout integration. The legacy profile remains blocked. No direct
platform calls, game dependencies or object-import exceptions are introduced.

## Exact contracts

`state.rs` is staged as `irql.rs`. It requires the existing `memory`, `registers`,
`kernel_pool`, `gate` and host-generated `gamepkg` modules. It also requires a
literal port-owned `irq_hooks` module implementing the signatures below. There
is no selectable function pointer, trait-object callback or default production
hook. Production source must supply and qualify the real native callback paths.

- `Controller::controller_new(&Memory) -> Result<Controller,i32>` binds a new
  2176-byte IQL1 copied record and existing raw recursive gate to the live root,
  KSV3 record and actual creator task. The gate owns its own 64-byte GAT1 record
  and actual GamePKG recursive mutex. All use existing quotas.
- `controller_id()` / `controller_gate()` return opaque copied-record IDs.
  `controller_borrow(&Memory,id)` creates a nonowning validated view.
  `controller_counts(&Memory) -> [u64;3]` returns raised, masked and transition
  counts; global interrupt blocking is raised != 0 || masked != 0.
- `controller_task(&Memory) -> Task` registers/reuses the actual current task,
  including bootstrap task 0, in one of 33 bounded slots. A copied ID cannot
  impersonate a worker. Aborted tasks cannot register again. Full table is Busy.
- `controller_close(&Memory)` is creator-only, nonblocking and idempotent after
  complete closure. It refuses nonzero counts, task hold/mask/ISR/release flags
  or raw recursive depth. The caller must join all users before closing; this
  is not a task-join operation. Closing rejects future use. A partial close
  retains enough owner state to retry gate cleanup. Revoked KSV cleanup may
  finish while still returning Denied; inspect `controller_closed()`.
- `Task::task_key() -> [u64;2]` is [controller record, actual task].
  `task_values(&Memory) -> [u64;6]` reports current IRQL, counted flag,
  raised-level gate-held flag, CLI mask, inside-ISR flag and deferred release.
- `irql_current` returns u8. `irql_blocks_dpcs` is counted || raised-held ||
  current >= 2, exactly the original per-thread predicate, not global masking.
  `irql_set_isr(&Memory,bool)` brackets a native ISR; it does not deliver one.
- `irql_raise(&mut Memory,&mut Registers,u8) -> u8` returns old priority.
  `irql_lower(...,u8) -> ()`, `irql_to_dpc(...) -> u8`,
  `irql_enter_interrupt(...,i32) -> i32` and
  `irql_leave_interrupt(...,i32) -> ()` preserve original boundary bookkeeping.
  Interrupt-entry gating uses the original signed level <= 2 before u8 casting.
  A mistaken lower-to-higher sets/publishes the level without counting a raise.
- `irql_gate_enter(&Memory,ms)` accepts 0..1000 ms using typed SDK26; Busy means
  no hold acquired. `irql_gate_leave(&mut Memory,&mut Registers)` runs a safe
  point before raw release and a release point only after depth reaches zero.
- `irql_disable(...)` acquires a separate recursive hold and global masking
  count only on the first CLI, clearing IF on every call.
  `irql_enable_after_shadow(...)` sets IF before releasing the mask and hold.
  The reviewed native call site MUST invoke it after the instruction following
  STI; this component does not decode or enforce the instruction shadow itself.
  `irql_interrupts_enabled(&Memory)` reports absence of the task CLI mask.
- `irql_abort(&Memory)` is explicit error/stop cleanup, not an interrupt ack or
  successful callback. It atomically subtracts this task's applied global counts,
  permanently marks its slot aborted, clears hold/mask/ISR/release flags, and
  releases at most 64 raw recursive holds actually owned by this task. More
  depth returns Busy with incomplete cleanup; retry is supported. It also works
  after cancellation/KSV revocation. It does not publish PCR, run IRQ/DPC hooks
  or change the historical transition count. Complete cleanup before code unload.

All results above are `Result<...,i32>` unless described otherwise. Errors retain
SDK meanings: -1 Invalid, -2 Busy, -3 Failed, -4 Denied, -5 Unsupported. CAS and
lock waits are bounded; this is not an unbounded native critical section. On an
error after a successful prefix (including a hook error, cancellation, failed
PCR write or counter publication), stop that actor and call `irql_abort`; do not
blindly retry the original operation. Counts and applied bits share one CAS so
cleanup does not mistake a TLS flag written before a counter for an applied
increment. A failed PCR write may leave the original prefix's new priority/count
visible. Error cleanup is explicit rather than pretending the operation rolled
back or that an interrupt ran. Unrecoverable corrupted/released record cleanup
errors require session shutdown, which revokes the host-owned resources.

## IQL1 representation

All words are little-endian u64. Header [magic 0x314c5149, root, KSV3, raw gate,
raised count, mask count, u32-wrapping transition count, closing flag]. Each of
33 64-byte task slots is [actual task, status, priority, counted, raised-held,
masked, in-ISR, release-due]. Status bits: 1 used, 2 raised increment applied,
4 mask increment applied, 8 aborted. Empty slots are entirely zero. Validation
rejects invalid flags/ranges, duplicate actual tasks, foreign bindings and global
counts inconsistent with the applied-bit sum. Slots are never recycled within
this controller; create a new controller after joined teardown if needed.
Root288, KSV3/64, CPU v5/316 and the source-kit archive remain unchanged.

## Required literal hooks

Each returns `Result<(),i32>` and runs in checked port code:

- `safe_point(&mut irql::Task,&mut memory::Memory,&mut registers::Registers)`
- `release_window` and `release_point` with the same three arguments
- `spin_wake(&mut irql::Task,&mut memory::Memory)`
- `warn(&mut irql::Task,&mut memory::Memory,kind:u32,old:u8,new:u8)`

Warning kind 0 is mistaken raise-to-lower; 1 is mistaken lower-to-higher.
Crossing down runs safe_point at the old level while holding the gate, drops
count and raised-held, calls release_window before raw unlock, then defers
release_point until the new level and fs:[0x24] byte have been written. Interrupt
leave always calls spin_wake. `fixture_hooks.rs` provides ONLY a controlled
pending callback and normalized trace in checked RAM; `probe_hooks.rs` is an
earlier no-pending smoke fixture. Neither is a production device/diagnostic hook.
Do not stage these fixtures as a runnable game or describe them as real IRQs.

## Qualification and remaining integration

Run `python3 check.py --app /path/to/app --app-lib /path/to/libgamepkg.rlib --wine`.
Python and C are maintainer-only differential tooling. `reference_inputs.json`
pins the asset-free source kit, original kernel source and 25 unmodified function
ranges. The scaffold models one reference thread/gate, disables profiling and
holder diagnostics, and supplies controlled pending IRQ and callback hooks.
2048 cases compare the entire 64 KiB RAM hash (including ordered hook traces and
PCR), return-value hash, all six task fields, shared counters, recursive depth,
flags, block/mask predicates and trace count. These include counter wrap,
negative signed interrupt levels, repeated raises/CLI and callback returning at a
different priority. Rust independently checks the complete v5 register restore.
A deliberately early release before PCR publication must fail this comparison.

Checked controls cover malformed records, stale/foreign handles, borrowed close,
PCR overflow/read-only denial and partial-prefix abort, synthetic full/closing
tables, actual KSV revocation cleanup, bounded abort/retry, real worker ownership,
contention, wrong-owner unlock, active-close denial, normal joined teardown and
cancellation cleanup. All admitted fixture sources are scanned before bindings;
every separately optimized game object is audited before trusted runtime linking.
The unused unsafe/FFI oracle is rejected even when unreachable. No enforcement
thresholds, imports or engine exceptions are relaxed.

Windows component execution is Wine only; Windows audited artifacts are compile/
link checks. Linux confined fixture execution is not gameplay, physical controls,
audible audio, real GPU rendering or measured frame pacing. Production pending
IRQ/worker-stack allocation, post/release races, ordered native handler routes,
DPC/timer delivery and original diagnostics still require integration. The full
candidate has not imported this controller or replaced its unresolved kernel
imports. Renderer/media/bootstrap/launcher migration and real title qualification
remain required before admission.
