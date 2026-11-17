# Retained timer job under authenticated startup ownership

`timer_startup.rs` is dependency-free checked port Rust. `Launch` coordinates one
actual suspended SDK job with the authenticated Startup, DPC queue, timer table
and pacing schedule. It does not implement a timer thread body, execute a title
entry, raise a vblank, install a device/profiler handler, or move an engine into
trusted host code. It owns no native thread/window/device handle or shared
argument record. All system operations remain GamePKG-owned services.

## Prepare and resume

`Launch::launch_new()` creates an idle, record-free value. Before an attempt
binds its root, only pure progress queries are meaningful; an unused idle value
can simply be discarded. One value represents one attempt, not a reusable task
slot. Once closed, create a new value for another attempt.

`launch_prepare(&mut self,&mut Startup,&Memory,&mut Queue,&mut Timers,
&mut Schedule,entry:u32,argument:u64) -> Result<(),i32>` first validates:

- Ready authenticated Startup and its current KSV/KPL4 lifetime identity.
- Actual creator-owned, live, unassigned queue/table/schedule, including the
  timer table's association with this exact queue.
- A live opaque argument record. Its schema and ownership remain the caller's
  responsibility; retain it and every referenced owner until the actual join.

The new `Queue::queue_start_ready`, `Timers::timers_start_ready` and
`Schedule::schedule_start_ready` APIs perform those strict preflights without
reserving a slot, creating a job or freezing subsequent state. Borrowed owners
cannot start another worker. Failed preflight leaves Launch idle and creates no
job. It can be retried after correcting the inputs.

After preflight, Launch caches its actual creator/root, exact [KSV,KPL4] pair
and [queue,table,schedule] IDs. It calls `Startup::startup_spawn` so the job is
suspended and already tracked before any subsystem assignment. It retains that
job ID immediately, then assigns queue, starts table and assigns schedule in
that order. It does **not** resume. A spawn failure retains a bound zero-job
attempt which can close without a join. Any later failure retains the exact
suspended job and all successfully published prefixes; do not retry preparation
or unassign/replay those prefixes. Cancel/join first, then release owners.

The timer table retains its private worker obligation before its fallible lock
and publication. Launch records that attempt separately from successful table
binding. A timeout may therefore require a real queue join witness to close the
table even though its public record still looks unstarted. No rollback or false
completed-start claim is made.

`launch_task(&Memory)` returns the bound opaque job ID for the actual creator.
`launch_progress()` returns nine booleans in this exact order: queue assigned,
table attempted, table bound, schedule assigned, resumed, actually joined,
queue witnessed, schedule witnessed, Startup witnessed. These are observations,
not native objects or leases. Snapshot status never substitutes for a join.

`launch_resume(&mut self,&Startup,&Memory,&Queue,&Timers,&Schedule)` requires the
same cached owners, ready live Startup, complete successful assignment and no
prior resume/join/close. Queue and table must name this exact job/queue, and
`Schedule::schedule_worker` must name the same **full u64** job. Only then does
it resume through the existing SDK. Duplicate resume rejects. A substituted
Startup with the same root/creator but another lifetime is not equivalent.

The reviewed Plan entry is an integer index, never a function value. Its actual
body must establish the retained Worker stack/TIB, Dispatch, timer sleeper and
required IRQ/device/profiler interfaces, then signal its own readiness before
asynchronous producers are enabled. Resume success is not that readiness signal.
This launcher does not consume/skip a pacing step or replace original handlers
with generic host callbacks. Original loop, pause/warp/spin/wait and device work
remain required.

## One real join, shared with all owners

Stop the actual resumed worker through its own reviewed stop/wakeup protocol.
`launch_join_task(&mut self,&Memory) -> Result<LocalTaskJoin,i32>` is nonblocking.
For a never-resumed job it first cancels the startup gate; for a resumed job the
caller must already have requested stop. Busy retains the job. Success marks
actual joined state and returns the SDK's private consuming-join evidence.
Inspect its raw `result()`; nonzero worker results are not transformed into
successful work. A lifetime barrier is not gameplay or callback success.

A queue, schedule or Startup may instead consume the same job's join. Pass its
actual evidence once to `launch_accept_join(&mut self,&Memory,&LocalTaskJoin)`.
Wrong or repeated evidence rejects. A finished snapshot and a missing task ID
cannot mark joined. The job ID remains cached for forwarding, not for rejoining.

`launch_share_join(&mut self,&mut Startup,&Memory,&mut Queue,&Timers,
&mut Schedule,&LocalTaskJoin)` requires actual joined state, the exact proof,
original owner IDs and original Startup lifetime **before any forwarding**.
It witnesses the assigned queue, assigned schedule and Startup in that order,
remembering successful prefixes individually. Already consumed queue/schedule
joins are verified using their actual private joined-state observations rather
than joined again. Startup's exact current tracked-task observation distinguishes
its own previously consumed join. The copied cleanup observations
`Startup::startup_lifetime`, `startup_has_task` and
`Schedule::schedule_joined_task` retain creator checks without requiring live KSV,
so configuration revocation does not discard an actual join proof.

Keep the proof and owner values after a forwarding error. Retrying this method
skips completed prefixes and never repeats the SDK join. Share before closing
Startup or replacing any owner. A never-assigned schedule needs no witness;
an assigned queue does, including after partial table startup. Joined table
cleanup continues to use Queue's existing private `Joined` witness.

`launch_close(&mut self,&Memory)` marks the attempt closed only when its actual
job is joined and every applicable owner has witnessed it. Otherwise Busy is
returned. Bound zero-job attempts can close. Repeated completed close succeeds.
It releases no caller-owned argument, queue/table/schedule, worker stack pool,
IRQL controller, events, periodic Tick, heap, root or backing. Close those in
their existing documented order after sharing the proof. Runtime/session teardown
remains the last cleanup path if transport/ownership has irrecoverably failed.

## Formats, enforcement and validation scope

No SDK operation, quota, TCK/KIC/KPL/root/KSV/CPU codec, four-ID transfer or public
source archive changes. All new game units are scanned before generated bindings,
optimized separately and audited before trusted linking. Copied IDs do not waive
permissions, dependency checks, object imports or native-profile gates. Every
ordinary game source still has no Cargo dependencies, aliases, macros, unsafe
platform calls or function values. No engine/media exemption is introduced.

The combined image suite adds modes 32–41. They use authenticated synthetic
image/bootstrap/data/heap setup, a simultaneous actual periodic Tick, the retained
Worker/TIB/stack and real ordered DPC/timer dispatch with controlled finite native
routes and inert IRQ/profiler providers. They cover all four join consumers,
foreign Startup identity, full-width schedule substitution, partial table-lock
startup, borrowed-owner and failed Plan-spawn preflight, callback error,
cancellation, missing TLS/TIB denial and revoked KSV cleanup. Explicit fixture
TLS supplies the original bootstrap template; Worker still rejects absent TIB.
Revoked normal stack queries remain Denied while cleanup and copied diagnostic
records confirm release. No production guard is relaxed to make a fixture pass.

Run `../service-kernel-imports/check_image.py` with app/library/call-targets,
fresh private `--evidence`, optional read-only `--iso`, and `--mutations`. Existing
DPC, timer, pacing and dispatch checkers remain the relevant regressions. Linux
execution uses actual confined children. Windows cross-builds and Wine component
checks do not establish actual Windows confinement, physical controls, audible
audio, graphics, measured pacing or playable Burnout. Full native factory/source/
import/handler/loop/renderer/media/build/launch and gameplay closure remain required.

## Qualified checkpoint — 2026-11-16

The current combined fixture passes four-target complete source admission before
SDK injection and both independently optimized game-object audits before trusted
linking. Actual confined Linux x64/i686 each pass 40 synthetic image cases plus
two private in-place ISO checks: 84 sessions total. Private images are independently
authenticated against all 716 manifest files before the same read-only image is
granted; full-title imports still reject before publication. No title entry is run.

All 17 deliberately broken startup/clock/timer-launch copies pass build checks
and fail actual confined Linux64 behavior. Six timer-launch mutations cover a
wrong schedule job, losing a partially created job, closing before witnesses,
unrelated join acceptance, substituted Startup owner during join forwarding,
and failure to cancel an unresumed job. The substituted-owner control exposed a
missing fixture assertion; the final test explicitly verifies refusal before
any forwarding flag changes. Implementation checks were not weakened.

Current DPC queue, timer table, pacing (32 repeats) and retained-dispatch suites
also pass their original-C/native witnesses, negative controls, four-target
source/object/link checks and actual confined Linux execution. Their Windows
component execution uses Wine. Combined image Windows scope is source/object/link
only; it does not qualify Windows confinement or gameplay. Current exact sources,
all four-target receipts and witness hashes are preserved privately; older
receipts are not retagged for these added preflight/observation APIs.

The shared SDK remains
`fe309935dd602e2cf098e50065f6035de33dac88b51d45a2bc1ba47cf026742f`. No installed
host binary change is needed for these port-only APIs. The private context-bound
main gap remains 107 imports; no complete generated-candidate count or title
admission is requalified. Original handlers, whole timer/pacing loop, remaining
kernel/generated/manual/media/renderer work, complete builders/launchers and
actual gameplay/graphics/input/audio/measured frame pacing remain required.
