# Live periodic clock under authenticated startup ownership

`tick_startup.rs::startup_tick(&mut Startup,&mut Tick,&Memory,&Data,entry:u32)`
composes the actual checked periodic `KeTickCount` worker with the authenticated
import owner. It is ordinary dependency-free port code. It allocates the clock's
stop event and metadata, gets its opaque argument, uses `Startup::startup_spawn`
to create/register a suspended job, assigns that exact job to the tick metadata,
then resumes. The startup owner's consuming-join obligation therefore precedes
any worker write. The reviewed Plan entry must call `tick_service::tick_worker`.
No host callback, platform task, engine exemption or new SDK operation is added.

All successful partial steps remain in the caller's Startup and Tick values if
a later step fails. Stop/join and close the Tick before closing shared startup,
data, root or banks. Startup's normal join barrier still applies to failed
assignment and startup: a suspended job is an actual cleanup obligation.
This does not implement the separate timer/DPC/vblank worker or title entry.

## Tick lifecycle APIs

The original `tick_start(ram,data,entry)` remains a sole-owner composition of the
same steps and keeps its cleanup/error contract. The split form exposes:

- `tick_allocate(&mut self,&Memory,&Data)` validates the exact writable word,
  live root/KSV and actual uncancelled creator, then allocates only a manual stop
  event and the original 64-byte TCK1 record with unpublished task ID zero.
  A failure retains partial ownership. Duplicate allocation rejects.
- `tick_argument(&self,&Memory) -> Result<u64,i32>` is creator/root-only. It
  requires fully allocated, unassigned live state and exact original metadata.
  The returned ID is copied data, not a lease or a native pointer.
- `tick_assign(&mut self,&Memory,task:u64)` requires the caller's actual suspended
  SDK spawn. It retains the supplied nonzero/non-creator task before subsequent
  fallible checks, so failed binding still requires cancel/join. It checks actual
  job liveness, pinned KSV and exact unpublished metadata, then CAS-publishes the
  actual task ID. Zero, already assigned or inappropriate owner state rejects.
  Passing an unrelated job is a caller error, not a task-discovery mechanism.
- `tick_resume(&mut self,&Memory)` revalidates creator/root/KSV and **all** cached
  metadata before resuming that job. Unassigned, already resumed, cancelled,
  replaced or corrupt state cannot launch. The worker separately revalidates its
  identity, immutable metadata and KSV before writes. Checks do not make arbitrary
  later revocation atomic; a previously accepted write can complete until join.
- `tick_task(&self,&Memory) -> Result<u64,i32>` exposes the current unjoined opaque
  job ID for the actual creator, including a failed assignment. Zero means this
  owner has no remaining job, not that its record/event are already released.
- `tick_join_task(&mut self,&Memory) -> Result<LocalTaskJoin,i32>` requests stop,
  cancels a not-yet-resumed job and performs one actual nonblocking SDK join.
  Busy retains state. Success records the existing terminal result and clears
  this owner's job obligation, returning the SDK's private typed join evidence.
- `tick_accept_join(&mut self,&Memory,&LocalTaskJoin)` accepts a join already
  consumed by Startup or another owner only for the exact current task. Unrelated
  and repeated evidence rejects before changing result or ownership. Creator/root
  checks still apply during cancellation/revocation cleanup. No evidence can be
  constructed by checked game code, and a finished snapshot is not evidence.

For a shared clock, stop then poll `tick_join_task` off the window owner thread;
pass its success to `Startup::startup_accept_join`, then call `tick_close` and
close shared owners in order. Alternatively, stop/wait and consume the join via
`Startup::startup_join_task`; pass that same proof to `Tick::tick_accept_join`
before `tick_close`. If an intermediate cleanup step fails, retain the actual
proof and owner values; do not join a second time or treat a missing job as proof.
Always inspect `LocalTaskJoin::result()`; a join is a lifetime barrier, not a
claim that the worker ran successfully.

`tick_close` remains the sole-owner convenience: it uses the same actual join
internally, then releases metadata/event. Shared coordinators must obtain/share
the proof explicitly instead of dropping it through this convenience. The
existing Tick terminal worker-error contract is unchanged: cleanup returns it
once after release; a subsequent empty close succeeds. A new successful allocation
resets the result. The startup owner's own stable terminal cleanup errors retain
their separate existing contract.

The creator and [window,offset,KSV] cache are private Rust owner fields, not a
metadata schema change. TCK1/64, root288, KSV3/64, CPU v5/316, worker four-ID
transfer, quotas, source-kit archive, SDK and blocked profile gates are unchanged.
Do not relabel prior native receipts; the checked Tick/coordinator source changes
require new fixture/source-kit identities.

## Qualification and remaining integration

The image suite's modes 24–31 use the real SDK worker and authenticated synthetic
image/bootstrap/heap/data/import setup. They cover both consuming-join directions,
wrong/repeated evidence, live raw clock and shared-skew updates without TSC
initialization, no writes beyond the data word, allocation without a worker,
failed Plan spawn, malformed assignment retained until cancellation/join,
foreign-root controls while gated, revoked KSV binding and actual task cancellation.
They do not execute original title code, raise a vblank or exercise a device.
The existing earlier startup/image rejection modes remain part of the suite.

Run `../service-kernel-imports/check_image.py` with its app/library/call-targets
arguments, fresh private `--evidence`, and `--mutations`. Optional `--iso` grants
the supplied image read-only in place after independent manifest authentication;
full-title imports still reject before publication. The new deliberately broken
copies test untracked spawn, missing assignment, unrelated join acceptance,
release without a witnessed join, losing a failed assignment and resuming corrupt
metadata. These are checked test copies, not production fallbacks or exemptions.
The original Tick `check.py --app APP --app-lib APP_RLIB --wine` also remains the
regression command for original arithmetic, lifecycle, quotas and stalled-broker
independence.

This shared-clock stage must still be adopted by the complete native session
factory. The timer/DPC/IRQ/pacing loop, real handlers, remaining kernel/generated/
manual/media/renderer closure, builders/launchers and actual gameplay remain
required. Windows cross-builds and Wine component checks are distinct from real
Windows confinement, graphics/controls/audio and measured gameplay pacing.

## Bound-configuration race handling

A repeated integration run exposed an old Tick race: configuration could be
unlinked after the worker's initial root check but before settings lookup. The
lookup then returned Unsupported for an apparently fresh unconfigured root,
instead of Denied for this already-bound worker. The production loop now retains
the settings result, rechecks its exact pinned KSV binding, and only then uses or
propagates the result. Replacement with another live KSV also rejects before the
word write. A write accepted after a successful check may still complete before
join; the explicit join barrier remains necessary.

`service-kernel-tick/revocation.rs` is a maintainer-only deterministic seam. It
unlinks or replaces KSV on the exact third worker root read, between preliminary
validation and the settings fetch. It verifies Denied, an unchanged tick word,
actual join and cleanup on all four targets (Windows via Wine). A separately
compiled old-guard copy must fail with Unsupported 5 rather than Denied 4. The
seam is never a game object; adding it unused to a package is required to fail
source admission. No production fault hook or allowlist exception is introduced.

## Qualified checkpoint — 2026-11-16

All four target builds pass complete source scanning before generated bindings
and both separately optimized game-object audits before trusted linking. Actual
confined Linux x64/i686 each pass 30 synthetic image cases plus two private
in-place ISO checks, 64 sessions total. Modes 24–31 exercise the shared live
clock; the earlier image/import/startup cases remain intact. Eleven deliberately
broken copies pass the same source/object/link checks and fail actual confined
Linux64 behavior, including all six clock integration mutations listed above.
An unused direct-filesystem helper and the unused maintainer race seam both
reject before compilation. Test buffers use bounded heap storage to avoid an
unapproved 32-bit Windows stack helper; no audit vocabulary was expanded.

The original Tick suite also passes: 2,048 pinned original arithmetic cases per
target, actual local lifecycle/quota/revocation checks including 132 repeated
cycles, four-target source/object/link qualification, and actual confined Linux
writer progress while a queued window request holds the broker client mutex for
at least 60 ms. Deterministic unlink/replacement controls pass on all four native
SDK targets (Windows execution through Wine), and the compiled old-guard copy
fails with its reproduced 5-versus-4 result. Its seam is not an admitted object.
These scopes are distinct; they do not establish real Windows confinement,
physical controls, audible audio, graphics, title gameplay or measured pacing.

The private image still rejects before publication with 107 unsupported main
functions when context-bound, 108 without context. No generated-source closure
count changes or playable title admission follow from the clock. SDK remains
`fe309935dd602e2cf098e50065f6035de33dac88b51d45a2bc1ba47cf026742f`; the installed
matching app requires no binary change for this checked port-only stage.
