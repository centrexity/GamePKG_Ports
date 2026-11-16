# Checked deferred-work queue component

This is port-owned Rust, not trusted GamePKG runtime code. It translates the
original Takedawn pending-DPC ring using GamePKG records, typed recursive mutexes,
checked guest RAM and reviewed tasks. This page describes queue primitives;
the checked notification, callback and retained-drain helpers are linked below.
Production title routines, profiling and timer/interrupt/bridge integration are
still required before enabling any kernel import. Burnout remains blocked.

Requires the GamePKG SDK with local operation 26 (`local_mutex_lock`). The
shared ABI contract is the app's `docs/PORT_SERVICES.md`. No Cargo dependency,
unsafe/extern, native thread/lock/device/file/clock call or callback pointer is
present in the admitted component. Maintainer reference/driver files are kept
outside admitted sources; Python and C are not player requirements.

## API and lifetime

All functions return the existing SDK error codes: Invalid -1, Busy -2,
Failed -3, Denied -4, Unsupported -5. Errors are propagated; no platform fallback
or guessed clock is permitted. The caller supplies the existing root-bound
`memory::Memory` and KSV3 configuration. Guest addresses stay fixed-width u32;
opaque IDs are u64. Read/write accesses use the checked memory adapter.

- `Queue::queue_new(&Memory, stats: bool) -> Result<Queue, i32>` owns one recursive
  mutex and one fixed 1,616-byte record. Record allocation failure releases the
  just-created mutex. Existing 64-record/128-sync quotas remain unchanged.
- `queue_id() -> u64` supplies only the record ID; `queue_borrow(&Memory, id)`
  creates a nonowning view bound to the same root and current KSV3 record. It
  validates the complete metadata and proves the lock ID names a mutex using
  typed op 26. A signaled event rejects without consuming its signal.
- `queue_assign(&mut self, &Memory, task)` is creator-only and one-time. Publish
  an actual reviewed **suspended** consumer task before resuming it. The caller
  controls worker entry, guest stack/TIB, gate/IRQL checks and callback execution.
- `queue_insert(&self, &mut Memory, dpc, arg1, arg2) -> Result<[u64; 2], i32>`
  returns `[inserted, full]`. Null/duplicate addresses return `[0,0]`; a full
  ring returns `[0,1]`. No overwrite or inline callback occurs. The caller must
  preserve the original `DpcQueued`/gate-release notification policy and emit the
  original full-queue diagnostic after unlocking. Optional timestamps sample
  the independent host clock only on successful new insertion.
- `queue_remove(&self, &mut Memory, dpc) -> Result<bool, i32>` clears all matching
  pending slot addresses and then the guest Inserted byte. Cancelled slots keep
  their arguments/timestamps and occupy space until popped. A nonzero Inserted
  flag returns true even if no matching entry exists, as in the original.
- `queue_pop(&self, &mut Memory) -> Result<Option<[u64; 4]>, i32>` is restricted
  to the assigned reviewed task and returns `[dpc,arg1,arg2,timestamp_ns]`.
  Empty is None. A cancelled hole is Some with dpc=0 and advances head; callers
  must skip executing it. The caller must first acquire the original dispatch
  gate and establish that IRQL permits work. This helper does not grant that
  permission or execute anything.
- `queue_pending(&Memory) -> Result<bool, i32>` reports head!=tail for other
  tasks, including holes, and false for the assigned timer/consumer task.
  `queue_state(&Memory)` returns a copied complete record for diagnostics/tests.
- `queue_nonempty` also reports entries to the timer. `queue_take_begin` returns
  its assigned consumer's still-locked `Take` observation, or None after empty
  unlock. `take_depth`/`take_entry` observe before publication; `take_publish`
  advances/clears/unlocks and `take_abort` unlocks without consuming. See
  [TAKE.md](TAKE.md) for exact ownership, revocation and partial-error semantics.
- Creator-only `queue_join(&mut self, &Memory)` consumes the actual nonblocking
  reviewed-task join and returns its result. Until it completes, close is Busy.
  `queue_close` revokes/releases record and mutex, never fabricates task
  completion. Stop producers before close; borrowed views cannot assign/join/
  close. Resource cleanup still runs when the KSV3 binding is revoked, then
  returns Denied. `queue_closed()` distinguishes that completed cleanup.
  A partial cleanup retains any still-owned mutex ID for retry.

The ring has 64 slots and **63 usable entries**, exactly as the original sentinel
scheme. All legitimate writers acquire the same actual recursive mutex with a
bounded 1,000 ms wait. Copying a record ID grants no native pointer or lock
ownership. Conflicting CAS despite the mutex is Failed, not ordinary contention.
Cancellation/closure while waiting is Denied. Unlock remains explicit; stopping
a task while it owns a lock still requires coordinated owner cleanup.

Publication preserves observable original ordering. Insert writes guest
Inserted=1 before publishing tail; remove publishes holes before clearing the
flag; pop publishes head before clearing the popped nonzero flag. A failure in
these final stages is an explicit **partial operation**, never success: callers
must abort/report it rather than blindly retry, execute or acknowledge a DPC.
These operations are not all-or-nothing transactions across RAM and the record.
Root/KSV3 revocation before mutation rejects; a clock failure is propagated only
after rechecking that binding. In-flight cross-resource revocation can still
produce the documented partial prefix.

## DPC1 private port record

Every number is little endian. This format is private game state, not a new
host ABI, and changes no root/KSV3/CPU schema.

| Offset | Field |
| --- | --- |
| 0 | u64 magic 0x31435044 |
| 8 | root record ID |
| 16 | KSV3 record ID |
| 24 | actual recursive mutex ID |
| 32 | assigned reviewed task ID, zero before assignment |
| 40 | u64 assigned flag, 0 or 1, consistent with task ID |
| 48 | u64 statistics flag, 0 or 1 |
| 56 / 64 | u64 head / tail, each 0..63 |
| 72 | reserved zero u64 |
| 80 | 64 slots of 24 bytes each |

Each slot: u32 guest DPC address, u32 arg1, u32 arg2, reserved-zero u32,
u64 timestamp. Full metadata, shape, ranges and padding are checked on each
snapshot. Wrong-root/config/lock identity is Denied; malformed values are Invalid.

## Qualification

Run `python3 check.py --app /path/to/GamePKG/app --app-lib /path/to/libgamepkg.rlib
--wine` from this directory (join the command onto one line).

`reference_inputs.json` binds the asset-free source-kit archive, original source
file and exact byte ranges for the original insert/remove/pending functions and
the dequeue prefix. The maintainer C shell stubs lock/notification providers;
it does not qualify real gate ordering or callbacks. Original C functions are
otherwise copied unchanged. The harness compares 1,024 operations per statistics
mode, on Linux32/64 and Windows32/64 under Wine: complete 64 KiB guest RAM hash,
complete canonical ring hash, return values, pending state and clock-read count.
It exercises full capacity, duplicates, cancelled holes, wraparound and empty
pops. A deliberately broken cancellation copy must fail vector 65. Deterministic
clock injection exists only in this rejected maintainer harness.

Separate boundary controls cover malformed fields, wrong root/config/lock,
forged event signals, role/ownership denials, read-only guest memory, stale IDs,
record exhaustion without mutex leaks, unchanged sync quotas, live timestamp
bounds, actual task join and cleanup after configuration revocation. The actual
checked fixture scans **all** sources before binding injection, compiles and
optimizes game units separately, audits **every** object before trusted linking,
and checks receipt SDK/source identity on all four targets. It runs in actual
confined Linux32/64 children and matches the 1,024 statistics-off C witnesses.
Windows admission is compile/object/link qualification; Windows component
execution is Wine, not real Windows AppContainer or gameplay.

No GPU, physical input, audible audio, complete timer scheduling, native DPC
execution, frame pacing or playable-title claim follows from these checks.

## Remaining port integration

`notify.rs` now supplies the checked original queued/release **notification**
policy described in [NOTIFICATIONS.md](NOTIFICATIONS.md). It composes the real
IQL/raw gate, assigned queue consumer and existing host clock wake lane. It
does not drain the queue, execute a DPC or replace the production timer worker.
Its qualification is separate from the queue-only witnesses above.

`callback.rs` supplies the native callback convention on the assigned timer's
retained guest-stack lease, with explicit finite route/profiler interfaces,
priority restoration and abort cleanup. See [CALLBACKS.md](CALLBACKS.md).
Its original-C witnesses use synthetic native routines, not title callbacks.
The real provider tables, drain integration and production timer remain unfinished.

`drain.rs` now supplies the retained original work-drain algorithm, including
zero-time gate try, guard/lag, locked depth observation, latency/callback order,
holes, requeue and explicit abort cleanup. See [DRAIN.md](DRAIN.md). Its
controlled providers and synthetic callbacks do not finish the real timer,
profiler, handler tables, native context or actual title integration.

Connect the original gate/IRQL/CLI/STI and ISR safe points, deferred wake policy,
native callback dispatch and guest stack/TIB, timer/event shadow records,
statistics histogram/reporting, shutdown and reviewed production worker. Do not
make insertion always wake or invoke callbacks inline. Preserve original argument
writes and guest calling conventions in port code. Then close imports, regenerate
the checked source kit, qualify its full receipt/build and complete the renderer,
media, input and actual gameplay paths. No engine/media bridge exception is
permitted. Existing source/license provenance is in
`../service-native/UPSTREAM_LICENSES.txt` and the pinned source kit.


## Sharing the timer worker's actual join

`queue_worker_binding(&Memory)` returns `[queue_record_id,actual_worker_id]`
only after assignment, using the ordinary complete queue validation. The timer
component binds those identities while the worker is still suspended.
`queue_join_witness(&Memory)` is creator-only and returns Busy until the owning
queue's consuming `queue_join` has actually succeeded. Borrowed queues cannot
create this private-field `Joined` value. A second native join remains Invalid.

`Joined::joined_for(&Memory, queue_identity, worker)` validates its root, queue,
worker and current creator without another SDK call to join. Keep the value
before closing the queue; it remains usable for closing another owner bound to
that same worker afterward, including resource cleanup after KSV3 revocation.
It neither extends any native resource lifetime nor grants cross-root or
cross-worker authority. It is a checked Rust handoff, not a new host operation,
serialization format, completion flag, dependency or admission bypass.

The timer fixture in `../service-kernel-timers` tests missing/premature and wrong
scope witnesses, actual single join, close after queue release and revoked-root
cleanup. Its intentionally premature-join implementation must fail. The complete
queue source/object matrix and original queue operations are also requalified;
no existing ring/Inserted/notification/callback semantics change.

### SDK receipt adoption for a shared pacing worker

`queue_join_task` retains the typed SDK join receipt; `queue_accept_join` adopts
the same actual job's receipt from another owner. `queue_join_witness` is available
after either path. See [the shared join contract](../service-kernel-pacing/SHARED_JOIN.md)
for exact ownership, error and teardown rules. The older scalar `queue_join`
discards this SDK receipt and cannot coordinate a later pacing adoption.
