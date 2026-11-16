# Checked DPC timer notification contract

`notify.rs` is dependency-free **port code**, composed from existing GamePKG
local services. It adds no trusted engine code, SDK operation, native callback,
OS import, pointer, event, queue, task, quota or admission exception. The app
SDK, DPC1/IQL1 records, root/KSV3 formats and four-ID native context are unchanged.

## Binding and calls

All errors use the SDK codes: Invalid -1, Busy -2, Failed -3, Denied -4,
Unsupported -5. A return of false means the original policy suppressed a kick,
not that work ran or that the queue is empty.

- `Notifier::notify_new(&Memory,&Queue,&Controller) -> Result<Notifier,i32>`
  captures the root/KSV3, queue record, assigned actual consumer, IQL controller
  and its actual raw gate. An unassigned queue returns Unsupported; a finished
  consumer rejects. This ordinary Rust value borrows identities and allocates
  no record or runtime object. It does not own or extend any resource lifetime.
- `notify_queued(&self,&Memory,&Task,prompt:bool) -> Result<bool,i32>` mirrors
  original `xbox_DpcQueued`: kick exactly when the **calling actual task's** gate
  depth is zero and prompt is on. It does not test pending, consumer role, ISR
  state or current IRQL. An ungated ISR needs the immediate kick because it
  has no subsequent dispatch-gate release. A nonzero depth defers notification.
- `notify_release(&self,&Memory,&Task,prompt:bool) -> Result<bool,i32>` requires
  zero caller gate depth, outside ISR, IRQL below 2, pending queue and prompt on.
  `queue_pending` includes cancelled holes but is false on the assigned consumer,
  so a timer's gate release never wakes itself. Another task's gate ownership is
  not the caller's original thread-local depth. The existing
  `irql_blocks_dpcs` counted/raised-level predicate is not a substitute for
  these distinct queued/release predicates.
- `notify_insert(&self,&mut Memory,&Task,prompt,dpc:u32,arg1:u32,arg2:u32)`
  returns `[inserted,full,kicked]` as u64 words. It invokes the queued hook only
  after a successful new queue publication and after unlocking. Null, duplicate
  and full insertions never kick. Emit the original full-queue diagnostic after
  this call; count a returned kick in the original optional statistics adapter.

Every call validates the exact queue, root/KSV3, assigned consumer and controller
association and the current actual Task view, and rejects finished consumers.
Queue metadata, mutex kind and gate kind/ownership are revalidated through the
existing checked helpers. A Task from another controller is Denied even with
the same root. Revoked/cancelled/stale resources never silently suppress a kick
as if successful. Malformed records propagate Invalid. Quotas and cooperative
cancellation are unchanged.

The actual kick is `wait_service::sleep_wake` through host-owned local clock
operation 41. The existing wake generation coalesces notifications without
losing a wake delivered before waiting. This is **only the timer's one-sleeper
clock lane**; original spin-wake notification remains a separate integration
requirement. No wait runs on the app window owner thread. Borrow validation
may encounter existing bounded locks on the game worker; the notifier does
not wait for a DPC to complete.

## Required production integration

Create one queue and IQL controller for the title; publish its actual suspended
timer with `queue_assign`, then construct the notifier. In that worker, register
its existing `Sleeper` and retain its guest stack lease/TIB before enabling
producers. A readiness handshake is required: the notifier constructor does not
prove that the sleeper, stack or TIB has been initialized.

The production IRQ observer must call `notify_release` at its existing DPC
boundary, preserving safe-point/unlock/new-PCR ordering. Pass the original
reviewed prompt setting consistently to queued and release paths. Keep original
statistics/reporting and configuration handling in checked port adapters. Do not
replace real observers with the no-op fixture hooks.

Stop producers, cancel/cooperatively finish and actually join the timer before
closing its queue/controller/root. The notifier owns nothing to release. A
stale copied value cannot recreate a closed queue. Insertion plus wake is not
an atomic transaction: an error after publication can leave work queued without
a successful kick. Report/stop the affected actor/session; do not blindly retry
as if insertion had failed before publication. Existing queue partial RAM-write
semantics remain unchanged.

Native callback dispatch, argument/stack/IRQL epilogue, drain guard/lag,
histogram/reporting, timer/event shadows, spin notification, device delivery,
full native context and production observer wiring remain required. Neither
Burnout nor Halo admission is enabled by this library.

## Qualification

Run `python3 check_notifications.py --app /path/to/app --app-lib
/path/to/libgamepkg.rlib --wine --evidence /private/new-directory` as one command.
Python/C are maintainer-only tools, excluded from admitted game sources.

Exact original `dpc_pending`, `dpc_release_point`, and `xbox_DpcQueued` byte ranges
and their archive/source hashes are pinned in `notification-tests/reference_inputs.json`.
The C shell supplies controlled already-published hooks and settings; it does
not test original native hook publication or real timer/device integration.
512 cases per target exercise prompt on/off, actual recursive depth 0..3,
IRQL 0/1/2/3/4/16/128/255, ISR on/off, empty/pending and actual timer/producer
roles. Results and actual clock-generation deltas must match original policy.
The timer registers a real Sleeper before producers, then checks retention and
coalesced consumption of pre-wait wakes. Cross-worker gate holds, malformed
queue, wrong root/controller, read-only RAM, duplicate/null/full insertion,
revoked root/KSV3 association, fully exhausted record/synchronization quotas,
finished/joined worker and stale notifier controls must also pass. Quota checks
use valid nonempty records and actual events; no quota is raised for the test.

Deliberately removing the ISR guard, adding a pending check to queued or using
global gate depth must fail. These mutants are maintainer controls and never
admitted objects. Complete dependency-free fixture sources are scanned before
binding injection; eight separately optimized game objects are collectively
audited before trusted linking on Linux32/64 and Windows32/64. Exact source,
SDK, compiler/target, object and final binary identities remain in private
receipts. Actual confined Linux runs are distinct from Windows component
execution through Wine and Windows artifact compile/link/audit checks.

These tests use controlled empty IRQ/profiling hooks and never execute a native
DPC. They qualify notification composition, not complete scheduling, GPU
visuals, physical input, audible audio, real Windows confinement, title gameplay
or measured 60 FPS.
