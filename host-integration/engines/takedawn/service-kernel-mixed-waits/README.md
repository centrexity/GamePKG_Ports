# Checked mixed event/semaphore ownership

This is dependency-free port code over GamePKG's existing atomic mixed-wait SDK.
It owns neither platform access nor a new trusted host implementation. Actual NT
wait/close imports and full title admission remain blocked. No quota or SDK
identity changes; use SDK
`9801f83896bc4dc53f135f9fe169f27305323fe82f1b8fae5f578092792745d8`.

## API and ownership contract

`mixed.rs` re-exports the same checked `kernel_pool::MixedWait` type used by the
native import owners. Every fallible operation returns `Result<_,i32>`; SDK errors
remain SDK errors and are not prematurely converted into guest NTSTATUS values.

- `MixedWait::mixed_wait_new(&Memory,&Handles,&Events,&Semaphores) -> MixedWait`
  validates the same live root/KSV/table and both typed domains, captures the
  actual acquiring task, and allocates no opaque object or record.
- `mixed_wait_prepare(&mut self,&Memory,tokens:&[u32])` accepts 1..64 entries and
  one preparation attempt. It validates the **complete** set under the actual
  tagged-table guard before allocating any wait record: each tag must resolve to
  an owned live event (HTR2 kind 1) or semaphore (kind 3) of the correct actual SDK
  type. Other types return Unsupported. Wrong roots/tasks/domains, foreign or
  stale objects, and malformed tags never establish a lease. Repeated underlying
  native IDs, including different aliases of one object, return Invalid. This
  follows the [Win32 multiple-wait input contract](https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-waitformultipleobjectsex).
- Each accepted participant retains one existing WLT1 or SWL1 record before
  fallible publication. Its ACTIVE entry is published before the table guard is
  released; all participants are protected before preparation succeeds. Last
  alias removal and owner shutdown return Busy while ACTIVE. An error preserves
  every accepted private/published prefix in the transaction. Call close after
  every attempted preparation, including failure. Cancellation/revocation does
  not turn missing obligations into completed cleanup.
- `mixed_wait(&mut self,&Memory,all:bool,milliseconds:u32) -> LocalWaitResult`
  accepts 0..1000 ms and **one** actual host call. It holds no table guard across
  that call. WaitAll is the SDK's atomic acquisition, never serial acquisition
  with fabricated rollback. WaitAny returns the lowest ready caller index and
  consumes only that participant; WaitAll returns index zero. These two object
  types cannot report mutex abandonment.
- The actual native outcome is retained before any later liveness check or DONE
  publication. Known KSV revocation returns Denied even after an accepted native
  acquisition; acquired signals/permits are not rolled back. All participant
  outcomes are retained before any completion write can fail. Selected/all
  participants receive success, unselected WaitAny participants Busy, and a
  failed operation its error. A failed completion can be retried by close without
  executing the wait again. The remaining participant completions are attempted
  even when an earlier one fails.
- `mixed_wait_native_result()` observes the original SDK result independently of
  later liveness/completion errors. Before a native attempt it returns Invalid.
  A transport/protocol error may follow an acquisition and must never be replayed
  or treated as proof that nothing changed.
- `mixed_wait_close(&mut self,&Memory)` requires the actual root/task, including
  on completed repeats. It finishes/releases all retained participants and the
  outer guard, attempts remaining cleanup after the first error, and retains all
  incomplete prefixes. Only completed close makes `mixed_wait_closed()` true.
  Published record ownership stays with the domains for journaled retirement;
  the waiter never releases a published record directly. Private untransferred
  records are released by the waiter. A never-executed published wait completes
  with Denied. Root/session loss can still require outer process teardown.
- `mixed_wait_count()` is an infallible historical participant count, not native
  authority, a lease or evidence of completion. It is set after full validation;
  preparation can subsequently fail with only a prefix registered.

No new wire record is introduced. WLT1/SWL1 remain exact 80-byte LE records;
EVD2/SSD1 keep 32 wait slots each. Existing 64-record/64 KiB, 128-sync-object and
32-task quotas still apply independently. A 64-entry input is a format bound,
not a promise that current quotas can accommodate it. Exhaustion after a prefix
retains that prefix for close.

Private storage shares root/table/domain/task/creator bindings once. Per-entry
arrays retain the actual native ID, owned record ID and lifecycle flags. Flags
are presence=1, published=2, attempted=4, observed=8, finished=16, semaphore=32.
The effective group result and original index determine per-part outcomes; only
one typed lease view is materialized at a time, with accepted state copied back
before propagating errors. The temporary view carries an already-owned lease;
a copied record or ID cannot grant ownership. This avoids large Windows32 stack
frames without permitting the rejected compiler stack-helper import.

## Guard retirement correction

Both HTR2 `HandleGuard` and KSH1 `Guard` now track native depth release separately
from accounting retirement. Close first releases the actual native depth, then
atomically retires exactly its outstanding count. Failed unlock leaves the count
active; a stale count CAS after successful unlock retains the count obligation
and retries it without unlocking another guard. A pending retirement keeps owner
shutdown Busy. The count-only copied-record CAS can finish after native unlock,
including alongside another acquired guard; it never changes guarded pages.
Count increments/retirements retry only an uncommitted version-conflict Busy,
up to 64 attempts. Exhaustion returns Busy with the outstanding claim retained;
read errors and every other SDK error propagate immediately. Shadow root-page
linking likewise preserves current counts across this bounded contention while
the page remains privately retained. Actual page edits keep their strict CAS;
no native wait, signal, acquisition, failed read or unknown error is retried.
`*_guard_closed()` is true only after both steps. Root and actual-task checks
apply to repeated close as well. Ordinary guard use requires a held depth and an
unretired accounting claim. No generic host mutex semantics were relaxed.

## Required logical-wait integration

This transaction currently completes one bounded host wait. An actual NT
finite/infinite/time-warped wait needs a retained logical-wait/slice extension
that keeps every lease ACTIVE across uncommitted Busy slices and finalizes only
at the logical result. Recreating transactions between slices would introduce
last-alias lifetime gaps and is not sufficient. Native successes, protocol
failures and other errors must never be replayed. Combine that extension with
the original timeout/status/CPU rules and reviewed alert/APC and resource owners
before admitting NT waits. The current one-shot API is not that integration.

## Qualification and limits

Run `check.py --app APP --app-lib RLIB --wine --evidence NEW_DIRECTORY`.
The exact original `wait_many` body and source-kit member hash are pinned in
`reference_inputs.json`. Windows C execution uses real Win32 objects through
Wine. The Linux C provider models the documented Win32 acquisition contract,
not the upstream POSIX shim. This oracle covers 128 zero-timeout combinations of
manual/auto events, signals, counted permits, all/any and input order: 2560 exact
vector bytes. Time-warp, NT absolute/relative/infinite timeouts and alertable APC
semantics are **not** established by this component.

Native tests cover wrong roots/domains/kinds, aliases, complete-input rejection,
known revocation, record exhaustion after an accepted prefix, and 32 ACTIVE
leases held by one caller. The capacity control is not 32 physical waiting jobs.
Actual workers exercise signal, cancellation, revocation, last-alias lifetime,
concurrent table access and consuming joins. Twenty-four maintainer-only failure
cases per target exercise stale publication/completion/count retirement,
failed unlock/release, accepted-acquisition revocation and protocol failure,
and propagation of the five tested SDK errors (-1 through -5) without replay. Providers are rejected even if
packaged as unused source. The ordinary fixture also contains explicitly
synthetic damaged-count/restore controls for both guard types.

Eighteen altered native copies compile and must fail. Three broken signed copies
(serial WaitAll and false completed retirement for each guard type) must pass
ordinary source and all-object admission before failing in confined Linux.
All eight optimized game objects are audited before trusted runtime/transport
linking; LTO stays off. Receipts bind exact SDK, source kit, compiler/target,
shader list and raw/audited/final object identities. Helpers have no shaders;
this is not NV2A or visual qualification.

The authenticated KIC4 mode 82 creates, signals and releases objects through the
actual prepared 189/225/193/222 import plan, verifies complete CPU/caller/output
behavior, executes all/any mixed waits, and removes both owned aliases before
startup joins and teardown. Its exact eight-word witness is
`[-2 as u64,0,0,0,1,1,1,1]`. The private image remains read in place.

Linux32/64 execute actual signed confined children. Windows32/64 signed artifacts
are build/audit-only; native Windows components execute through Wine. None of
these checks establishes physical Windows confinement, controllers, audible
sound, original rendering or measured frame pacing. Remaining title work includes
typed mutex/file/thread owners, async overwrite/detachment, actual NT timeout and
status/CPU adapters, alert/APC integration, full generated handler/loop,
renderer/media and Linux/Windows build/launcher/gameplay qualification. The real
KIC4 main import gap remains 98; no wait/close ordinal or title is enabled here.
