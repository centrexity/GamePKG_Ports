# Shared-service migration status — 2026-11-16

This is tested integration work, **not a playable Burnout release**. The legacy
profile remains blocked. No source pin, signature, cached executable or engine
exception may bypass that block. The source kit and catalog URLs are unchanged.

## Current checkpoint: IRQ queue/epilogue and bounded cleanup

`service-kernel-irq` composes coalesced pending lines, original post/release gate
races, checked worker slices, IRQL/PCR order and complete register restoration
through literal checked callback interfaces. Exact IRQ1/384 contracts and test
scopes are in its README. Default C witnesses cover 128 complete CPU/RAM/stats/
IRQL/stack cases per target; 36 signed32 wait boundaries run on each applicable
original C ABI. Long pauses do not impose a new gameplay timeout.

Actual-worker suites repeat32 per target: 128 controlled interleavings, 192
option cases, 64 callback/new-post/cancellation cases and lifecycle/quota
controls. Fault seams are explicitly checked test copies with separate receipts,
not production hooks. A reproduced stale-CAS/full-sync-quota failure stranded a
raised count with event-dependent abort; bounded record-only cleanup fixes it.
Required-failing controls retain that regression and ordering/bit/retry errors.
All eight optimized objects remain audited; confined Linux runs and Windows
components through Wine are distinguished. SDK, quotas, original source archive,
root/KSV/CPU/context formats and profile gates remain unchanged.

Real handlers, production diagnostics/reload/registration, DPC/timer/spin/TIB,
full bridge, renderer/media/bootstrap/launch and actual title qualification still
remain. Candidate/import counts below are unchanged; neither Burnout nor Halo
is enabled by these component results.

## Earlier checkpoint: guest worker-stack pool

`service-worker-stacks` adds checked lowest-free allocation of up to sixteen
256 KiB guest slices from the reviewed bootstrap layout, including shrunken
stacks. Root/KSV3-bound WKS1 state owns actual-task leases with stale-token
protection, bounded CAS, explicit exhaustion and cancellation/revocation cleanup.
It does not allocate native memory or bypass checked guest RAM access. Exact APIs,
576-byte representation, cleanup errors and one-pool integration rules are in its
README. Existing SDK, root/KSV/CPU layouts, helper hashes and quotas are unchanged.

16,384 original C allocation/free witnesses per target pass. Checked controls
cover stale/foreign/wrong-owner tokens, malformed/full/closing state, generation
overflow, real worker cancellation/revocation, unchanged record quotas and four
concurrent workers retaining sixteen unique slices until release/join. A reversed
allocation-order copy is required to fail. Full source scans and all eight
optimized object audits pass on all four targets; confined Linux32/64 children
repeat the vectors and controls. Windows component execution is Wine only.

Production IRQ delivery, real handlers, the long-lived timer/DPC worker, original
diagnostics/options and complete title paths remain unintegrated. Pending bits
must survive stack exhaustion. Full candidate/import counts and admission remain
unchanged; do not use these infrastructure tests to enable the legacy engine.

## Earlier checkpoint: checked IRQL controller

`service-kernel-irql` adds a root/KSV3-bound IQL1 controller with bounded real-task
registration, original IRQL/PCR publication order, separate raised/CLI gate
holds, interrupt bracketing and explicit bounded abort cleanup. The exact APIs,
2176-byte record, literal hook signatures and partial-error semantics are in its
README. No SDK, source-kit, root, CPU or KSV format/limit changes are introduced.

2048 ordered original-C state/RAM/return witnesses pass per target, including
controlled callback return at a different priority and full Rust CPU restoration.
Real workers exercise contention, wrong-owner release, active-close denial,
normal join and cancellation cleanup. Malformed/full/closing state, foreign/stale
handles, PCR overflow/read-only denial and actual KSV revocation are checked.
An early release before PCR publication is required to fail. Complete fixture
sources scan before bindings, and all eight optimized objects audit before
trusted linking on Linux32/64 and Windows32/64; actual confined Linux children
repeat the witnesses/controls. Windows component execution uses Wine, not a real
Windows gameplay or confinement check. Catalog validation passes all seven entries.

The production pending-device hooks and full timer/DPC/IRQ/native-handler
integration are still missing; the controller is not wired into the full
candidate. Main imports/counts below therefore remain unchanged. Renderer,
media, bootstrap, launch and real title qualification remain required. Burnout
and Halo stay blocked; synthetic callbacks are not gameplay or device evidence.

## Earlier checkpoint: IRQ state and raw gate

Ports `9a2ce80` / `64ae362` qualify private CPU **v5/316 bytes**, including flags,
FP comparison and all eight MMX values, plus the actual root-bound recursive
raw gate. Older CPU244/v4 references below are historical. Root288, KSV3/64,
four-ID context, underlying source-kit archive and production gates are unchanged.
The complete candidate closes **19,420** definitions, leaves **4,596**, and still
has **108 main imports** unresolved. Main entry/gameplay remains unadmitted.

Full source/object/link builds pass on Linux32/64 and Windows32/64; actual
confined Linux children pass 39 original functions / 5,888 complete CPU/RAM
vectors and prior scalar/vector/call/protection controls. Original IRQ state,
codec and raw-gate tests pass, including normal/cancelled worker cleanup.
Windows runtime here is Wine. Nothing qualifies real devices, graphics, audible
audio, gameplay or frame pacing.

Current shared SDK:
`885f6e7118f8a07e3ab9af872b5212eed7ef67e4d91375683cc8008cabe032a7`
(installed app native ID `c3660cab98f1b962`). Linux compiler scheduling uses a
private jobserver under the same 12 GiB limit. All eight optimized objects remain
audited; a fixture growth helper was removed rather than allowlisted. Native
Windows-host compiler scheduling is unchanged and still needs qualification.

Exact APIs: `service-native/IRQ_SERVICE_HANDOFF.md`, `service-kernel-gate/README.md`
and app PORT_SERVICES.md. The later IRQL checkpoint above implements the
bookkeeping/PCR and CLI/STI helper contracts; production safe/release points,
native IRQ/DPC callbacks and timer integration remain. Raw release is not final
GateLeave. Complete renderer/media/bootstrap/launch and gameplay qualification
remain required; do not enable the legacy engine or exempt media.

## Earlier checkpoint: shared display/vblank schedule

`service-kernel-pacing` adds checked display learning and nanosecond vblank
scheduling from ten unchanged pinned C functions, plus root/configuration-bound
copied state for the actual reviewed worker. It preserves forced/automatic
display selection, measured/stated periods, missed-frame neutral steps,
candidate learning, rate changes, irregular report recovery, exact grid/tolerance/
staleness behavior and restart after pauses. See its README for exact 104-byte
VPC1 /144-byte PCS1 contracts, APIs, ownership, errors and remaining wiring.

12,288 original C state/output witnesses per target pass in auto/off/on modes.
32 actual local worker/lifecycle suites per target exercise assignment before
resume, concurrent reports, real shared-skew pause restart, record exhaustion,
wrong/foreign/borrowed roles, malformed metadata, cancellation, nonblocking join
and cleanup. A deterministic mid-clock revocation control preserves the entire
record/version; a temporary old-error copy is required to fail. Full source scans
before SDK injection and every optimized object audit before trusted linking pass
on all four targets, plus actual confined Linux x64/i686 witnesses and >=60ms
queued-window stalls. Windows component execution is Wine only.

The app SDK/install, source kit archive, KSV3/root/CPU schemas, existing helper
hashes, quotas, source/dependency/import rules and profile gates are unchanged.
Main imports still have **108** unresolved functions and 4,709 generated
definitions are not closed. The schedule does not yet raise original IRQ/PCRTC
effects, execute the complete timer/DPC/IRQL worker or receive authentic display
reports. Time-warp/suspend/spin/profiling, remaining native/manual/device/renderer/
media and production build/launch still need integration. No title gameplay,
physical controls/audio/display, actual AppContainer or measured frame pacing
is qualified by these component tests.

## Earlier checkpoint: port-owned host clock and wakeable sleep

`service-kernel-wait` now supplies checked Rust host/guest nanosecond conversion
and the original one-sleeper pending-wake behavior using GamePKG local operations
38–41. It preserves coalesced notifications without replacing the observed
generation during a counter refresh, supports zero-duration consumption and
bounded relative sleeps, and binds sleepers to the root/configuration/reviewed
task that created them. A reproduced close-between-reads race now returns
revocation without acknowledging a pending wake; deterministic maintainer
injection also rejects a temporary copy with the old premature error return.
See [service-kernel-wait/README.md](service-kernel-wait/README.md) for exact APIs,
ownership, errors, pinned original C, commands and integration steps.

The installed app/SDK remains `c4edee3e44be562b` (full
`10c4f4ca7ea37783c08ea2cafd1b129ca9b3160cbe4d3cf30d6c1a5da8c51e88`).
No app implementation, source kit archive, helper hash, KSV3/root/CPU schema,
resource limit, dependency/import rule, admission gate or recipe changes.

4,096 unchanged original C clock vectors and 36,864 deadline properties per
target pass. Each Linux/Windows x64/i686 component runs 32 repeated lifecycle
fixtures (Windows under Wine), with 64 coalesced-wake cycles and 64 reviewed-task
handoffs per repeat. Full source scans before bindings and every optimized
game-object audit before trusted linking pass on all four targets. Confined
Linux x64/i686 reproduce the original C witnesses and continue local deadline/
wake consumption through a real queued-window/client-mutex stall of >=60ms.
The actual maintainer oracle is rejected if placed in unused game source.

The main import gap remains **108**, and 4,709 generated definitions are not
closed. Periodic KeTickCount and dispatcher initialization components already
exist; complete timer/shadow-event/DPC/IRQL/IRQ scheduling, vblank/display pacing,
time-warp/suspend, remaining kernel/device/generated/manual/media/renderer and
production entry/build/launch work remain. No gameplay, physical input, audible
audio, actual Windows AppContainer behavior or measured frame pacing is proved.
Older pins/counts below describe historical checkpoints, not the current SDK.

## Earlier checked component: shared guest timestamp counter

Ports `e206014` adds the original 733,333,333 Hz guest timestamp conversion,
one shared first-use origin, strict 64-byte KSV2 state and all five pinned
generated timestamp callers. See
[service-kernel-tsc/README.md](service-kernel-tsc/README.md) for exact APIs,
metadata, errors, partial effects, commands and validation boundaries.
KSV1 is rejected; root 288 / CPU 244 / four context IDs and all quotas remain.
The new ordinary game helper is not a host API, engine exemption or CPU loop.

Original C arithmetic (768 cases/target), original generated bodies (240
cases/target), four-target complete source/every-object audits and actual
confined Linux32/64 finite-router calls pass. Four first users share one origin;
clock faults, contention with intervening skew writes, malformed records and
denied second stores are covered. The full refreshed partial kit passes its
four-target source/object and Linux reference/confinement matrix: 19,307 literal
closed definitions, 2,439 emitted files, all five timestamp bodies identical to
the independently qualified component. A closed graph is not full title proof.

The main import gap remains **111**. Periodic tick/timer/scheduler/wakeup,
time-warp/suspend/IRQ/device, remaining kernel/generated/manual/media/renderer/
entry and production launcher requirements are unfinished. No title profile is
enabled; the SDK/installed app remains `9fef7a199380e4b4` with no binary change.

## Earlier checked component: kernel clock consumers

Ports `b70e07e` implements exports 125..128 through the actual GamePKG clock SDK
and existing shared root-bound skew, retaining original CPU/stack completion,
null behavior and arithmetic. See
[service-kernel-clock/README.md](service-kernel-clock/README.md) for exact
contracts, failure effects, original-source pins, command and validation scope.
All four targets pass source checks and every-object audits. Original C bridge
comparisons pass on Linux32/64 and Windows32/64 under Wine. Confined Linux32/64
tests each exercise 80 actual kernel calls across five views with independent
parent bounds; synthetic image mode 10 passes through the finite Call router.

The private in-place image still rejects **111 remaining main function exports**
before publication/title entry, down from 114. No game is enabled. Guest TSC,
periodic tick/timers/scheduling, other kernel/device/media/renderer/entry and
complete production builder/launcher work remain. The root/CPU/context formats,
SDK recipe `9fef7a199380e4b4`, installed app, quotas and admission rules are
unchanged. No app binary reinstall is needed for these port-only consumers.

## Earlier configuration and host prerequisite checkpoints

Ports `4f05bf1ed54937fce87ab7c7aefa830dc30c3b33` adds checked kernel export 24,
root-owned configuration and shared guest skew storage. See
[service-kernel-settings/README.md](service-kernel-settings/README.md) for the
exact 288-byte root / 64-byte KSV1 contracts, original behavior, error ordering,
ownership and validation matrix. The CPU record remains 244 bytes; context IDs
and runtime quotas are unchanged. All four targets pass source and every-object
audits; actual confined Linux32/64 settings and image-router witnesses pass.
Windows C/Rust comparisons use Wine, not real AppContainer gameplay.

The private image now rejects **114 remaining main function exports** before
publication or title entry. Kernel 125..128 remain unsupported. Their next host
prerequisite is a performance-only read that does not initialize the calendar
anchor: the existing combined sample must not move the original first-SystemTime
anchor to an earlier QPC query. No timing export, full native entry, renderer or
production launcher is enabled by this checkpoint.

The current installed app implementation is
`da4864db4e17a5d5f0c615a8383976d0a9e4f3cf`, SDK recipe `969defa5bb1d6421`.
This port-only change requires no app binary reinstall. The older component
counts and app pins below describe their recorded checkpoints; current shared
contracts live in the app's PORT_SERVICES.md and NATIVE_FUNCTION_MIGRATION.md.

The subsequent app `b617c3e` completes that performance-only prerequisite with
op 66, exact 24-byte copied counter/frequency/uptime replies and no calendar
anchor effect. Current SDK recipe is `9fef7a199380e4b4`; see
[the clock handoff](service-native/CLOCK_SERVICE_HANDOFF.md#performance-only-prerequisite-completed).
Settings and image/import checks pass against it. Timing consumers and all
production admission/gameplay requirements remain unfinished; the main gap
remains 114. The installed CLI/GUI have been rebuilt together for this SDK.

Incremental save migration: [service-save/README.md](service-save/README.md).
App `75d180311f5e29765ad4778f265908daaa0afb31` adds scoped metadata and duplicate
file handles. The save helper source passes all four target object audits and
three real-service helper tests at that stage; the later recovery/worker work is
described below.
`service-save/state.rs` now supplies bounded open/close and worker/pending
transitions, with seventeen transition tests and 4,096 synthetic events compared
against the unchanged pinned C pending policy. Combined production source passes
all four native target audits. `service-save/adapter.rs` now connects real disk
operations, with twelve actual file/service tests and explicit synthetic I/O
fault injection. App `e118ba7995cb8cb4e352040de90609ae0600d6da` adds presence,
typed directory helpers and reviewed local text validation. App
`5bfdf4dd8d4ef9759549bb376757f65906f6a699` adds empty-directory cleanup.
[Startup/export integration](service-save/STARTUP.md) and the
[reviewed save worker](service-save/WORKER.md) now pass 28 real-file/service tests,
18 transition tests and exact four-target source/object admission. Linux64 and
Linux32 each execute ten actual confined worker scenarios plus five process
terminations and five subsequent startup recoveries. The worker owns guard
state, recovery/history, correlated requests and bounded shutdown through
GamePKG-owned mailboxes; the actual generated engine still needs to call it.
Timestamp/retention/UI and original Windows rename retry timing remain pending.
This is not actual game or Windows worker-runtime qualification.

First generated-game migration: [service-native/README.md](service-native/README.md)
now contains 28 checked direct native functions from one pinned 50-function unit,
explicit register values, and safe offsets over GamePKG-owned RAM. The remaining
22 functions in that unit have explicit exclusions, not fallbacks. Each target
passes 1888 original-body register/whole-RAM comparisons (Linux32/64 native,
Windows32/64 under Wine). Exact source/object admission passes on all four;
actual confined Linux children execute the vectors. Vector helpers also pass
1575 edge cases and 256 shuffle controls, with an explicit portable dual-NaN
payload policy; see the component README for the narrow reference difference. This is an incomplete
component, not a complete kit, actual title entry or game-performance result.

The current application contract is GamePKG commit
`62f3a61f002cd89707952ef415d772feafca8dc8`:

- `docs/PORT_SERVICES.md`: exact wire/API, resource, permission and error contracts.
- `docs/FILE_RECOVERY_SERVICE_HANDOFF.md`: presence, directory and local text contracts.
- `docs/STARTUP_RECOVERY_HANDOFF.md`: startup/export contracts and native interruption evidence.
- `docs/TAKEDAWN_SERVICE_HANDOFF.md`: builder, launcher, rendering and remaining work.
- `docs/NATIVE_FUNCTION_MIGRATION.md`: checked native-function/RAM component and its limitations.
- `docs/NVI1.md`: verified program representation and reserved-value checks.
- `docs/TAKEDAWN_SERVICE_VALIDATION.md`: infrastructure test scope and limitations.

`SERVICE_MIGRATION.md` and `service-renderer/CONTRACT_REVIEW.md` preserve the
earlier source review. Their interface tables are historical: the app now has
shared depth/stencil attachments, pipeline/state variants, cube copies,
readback, visibility segments, viewport controls, and canonical border colors.
Windows atomic save replacement uses write-through; it is no longer rejected
solely because directory fsync is unavailable. Use the app's current contracts
instead of implementing against the historical tables. Arbitrary shader strings
and raw GPU/platform objects remain forbidden.

## Reproducible checks

Run from the repository root, with a current app library and pinned Slang tools:

```sh
sh host-integration/engines/takedawn/service-input/check.sh /path/to/app
python3 host-integration/engines/takedawn/service-input/check_source.py --app /path/to/app --app-lib /path/to/debug/deps/libgamepkg-HASH.rlib
sh host-integration/engines/takedawn/service-renderer/check_nvi1.sh /path/to/app
python3 host-integration/engines/takedawn/service-renderer/check_source.py --app /path/to/app --app-lib /path/to/debug/deps/libgamepkg-HASH.rlib --slang-root /path/to/slang
python3 host-integration/engines/takedawn/service-renderer/check_shaders.py --app /path/to/app --deps /path/to/debug/deps --slang-root /path/to/slang --hardware
python3 host-integration/engines/takedawn/service-save/check.py --app /path/to/app --app-lib /path/to/debug/deps/libgamepkg-HASH.rlib
python3 tools/check_catalog.py
python3 tools/check_current_app.py --cli /path/to/gamepkg
```

Previously recorded input/renderer/catalog results (not rerun for the save-worker change):

| Check | Result and scope |
| --- | --- |
| Controller translation | Four tests passed; exhaustive button masks/analog values, logical player errors, device/focus/recovery reports. Exact production source admitted, optimized, and object-audited for Linux64/Linux32/Windows64/Windows32; four source bypass cases rejected. |
| NVI1 producer | Seven tests passed, including differential comparisons with unchanged pinned C parsers, reserved encodings, paired operations, wire layouts and failed-update behavior. Actual app source/dependency/portability checks passed. |
| Signed shader preparation | All 32 WGSL and all 32 SPIR-V stages passed actual confined app preparation. The earlier SPIR-V `IsNan` failure is resolved with precise mode retained. |
| Shader structure | 64 compiled stages passed Naga validation, resource bounds and NVI1 uniform-layout checks. |
| Offscreen hardware vectors | RTX 3090: 13 MAC, eight ILU, paired issue, 256 swizzles, 12 ARL addresses, paired ARL, NaN clipping, 48 unchanged-C combiner differentials, final combiner and 16 clip masks passed. |
| Catalog | Seven recipe/catalog entries and thumbnail notices/hashes passed metadata checks. This does not admit their engines. |
| Current app smoke | 2048 and Arkanoo signed, verified, installed in an isolated library, compiled natively and completed 120 headless frames each. |

No actual Burnout gameplay, physical controller/rumble, audible audio, visible
postprocessing, Windows hardware or measured frame pacing is established by
these checks. No game media, saves, private signing keys or screenshots are
included in this work.

## Required before enabling the recipe

1. Replace the unsafe/FFI/TLS/raw-pointer portions of the generated game and
   compatibility code with dependency-free Rust accepted by the source policy.
   The current kit is not accepted merely because it contains Rust files.
2. Connect controller translation plus keyboard/mouse/touch, audio,
   in-place read-only ISO, timers/tasks, networking and private save operations
   to the shared client. Preserve saveguard recovery and flush/replace behavior.
3. Complete actual surface/state/NVI1 draw translation, finite verified pipeline
   descriptors, texture/cube handling and original SMAA/CAS/FXAA/presentation.
   These candidate programs do not yet implement the full postprocessing path.
4. Supply the complete checked source kit, signed production shaders, exact
   pipeline descriptors and reviewed entry/task table. Connect both platform
   recipes to the app's separate-object audit, receipts, private transport,
   host-owned window/providers and confinement. Keep every game object audited.
5. Requalify original visuals/saves and actual menus/driving/crashes, controls,
   audible audio and paced 60 FPS on Linux and both Windows devices. Component
   tests and historical direct-device builds cannot substitute for this gate.

The generic builder and launcher exist in the app; the complete migrated game
input needed to select them does not yet exist here. Do not re-enable the old
direct-device executable while this work remains.

## Child-local deadline/wakeup interface coordination

The app SDK now provides `local_clock_state`, `local_clock_wait` and
`local_clock_wake` (local operations 39/40/41). Use the app's PORT_SERVICES.md
section "Independent child-local deadline and wakeup lane" for the exact ABI,
errors and lifetimes. Current SDK identity is
`10c4f4ca7ea37783c08ea2cafd1b129ca9b3160cbe4d3cf30d6c1a5da8c51e88`
(CLI/GUI native identity `c4edee3e44be562b`). Rebuild against a matching app;
do not copy generated bindings into game source or reuse an older SDK receipt.
App implementation commit: `3e9a8c021ac486af5de8cb76b6978af42c203d3f`.

This supplies the generic primitive needed to replace the original host_time
WakeableSleep in the timer worker. It uses one runtime-owned timing lane, a
raw-counter absolute deadline, its exact frequency and a retained wake
generation. Maximum future horizon is one second, one waiter is permitted, and
the state/wait/wake operations remain independent of the remote window-request
mutex. Waits belong in the confined game's worker, never the app window owner.
Windows uses a private high-resolution timer/event pair and explicitly reports
Unsupported when unavailable; Linux uses monotonic condition-variable waiting.
There is no game OS import, timer handle, callback, spin loop or fallback.

Keep the existing periodic tick's separate 1ms local event wait: consuming the
single deadline lane there would contend with the original timer worker. The
port still owns timer/DPC queues, guest skew and vblank scheduling, DPC register/
stack/TIB setup and finite native call routing. Wire wake notifications into
that actual port state machine; the new primitive alone does not execute a DPC
or migrate the timer thread. No KSV3, root, generated helper or recipe changes
are made by this interface handoff.

Snapshot the wake generation before checking/draining queue work, and pass that
observation to the wait after deciding to sleep. Refreshing the counter must not
silently replace the observed generation after the queue check: a concurrent
notification could otherwise be lost. Retain the generation returned by a wait
and recheck work; multiple wakes may coalesce, as with the original pending flag.

App qualification covers local wait/wakeup/cancellation/teardown on Linux and
Windows x64/i686 (Windows execution under Wine), source admission and every
optimized game-object audit on all four targets, and real confined Linux
x64/i686 execution before any owner request is dispatched. The socket regression
also completes all three local operations while a real window request holds the
remote client mutex. Neither test establishes Windows AppContainer behavior,
physical controls/audio, hardware wake precision or measured gameplay pacing.

The existing full partial generated kit also passes this SDK's four-target
source/object/link matrix and actual confined Linux x64/i686 execution, including
the 38-function/4,864-vector original native witnesses and existing FPU/vector/
context/adversarial controls. Its 19,307 closed definitions leave 4,709 not
closed, and the main import plan still has 108 unresolved functions. This app
update introduces no newly supported kernel export. The matching optimized
CLI/GUI are installed locally; signed 2048 and Arkanoo each pass 120 headless
native frames using that installation. The catalog's seven metadata/thumbnail
checks pass. Burnout remains disabled pending complete migration/qualification.

Existing periodic-tick, clock and dispatcher component regressions pass against
this SDK: respectively 2,048/256/768 original C vectors per target, all four
source/object audits, and actual confined Linux x64/i686 live/lifetime controls.
The installed matching app also passed isolated signed native 2048 and Arkanoo
builds with 120 hidden frames each. All seven catalog entries and thumbnail
notices/hashes pass. This does not change the 108 unresolved main imports or
4,709 not-closed generated definitions; production Burnout remains unadmitted.


## Typed event service coordination (2026-11-16)

The app supplies child-local ABI 1 operations 27 (`local_event_wait`) and 28
(`local_event_state`). Current full SDK identity:
`51d880fd16a9cfacef7fb2d9582be1e26e32728685082248ce8f83e510ab1606`.
App implementation: `8164988a4dd6f9f8863f8c98c51e4db216d7f299`; matching
installed CLI/GUI native identity: `324b3c58fe2292b1`.
Use the exact framing, errors and lifetimes in app `docs/PORT_SERVICES.md`,
"Typed child-local event observation and waiting". Rebuild against this SDK;
old executable receipts must not be reused or relabelled.

Event wait accepts only an opaque event ID and 0..1000 ms timeout. It cannot
acquire a mutex; cancellation, release and close return Denied and wake blocked
waiters. Auto-reset signals are consumed only by a successful wait, while manual
signals persist until reset. The snapshot copies manual/signaled booleans
without consuming or resetting, allocating a sync slot or granting a lifetime
lease. A subsequent call rechecks revocation and task cancellation. Existing
128-object quotas, source restrictions and all-object auditing are unchanged.
The original generic wait 23 retains its prior semantics.

This is a generic primitive for the original timer shadow-event behavior, not
an implemented timer adapter. Keep the original 32 timer slots, periodic rearm
and one-shot removal before callback, callback outside the timer lock, and guest
SignalState publication after shadow signaling in checked port code. Preserve
signed relative due-time and host-clock/skew truncation order. Do not infer a
multi-event atomic operation from a snapshot or silently skip failed signals.
The original shadow-map capacity and SDK sync quota differ; resolve resource
admission explicitly rather than adding an OS event fallback or widening quotas.
The timer/DPC queue shares one actual worker: a consuming SDK join must occur
once before either component closes borrowed resources. Independent double joins
are not an acceptable lifecycle adapter.

The current source-checked event/deadline fixture passes all four target builds
and every emitted optimized game-object audit; Linux x64/i686 run with actual
GamePKG confinement before owner dispatch. Standalone runtime and wire tests
pass on Linux x64/i686 and Windows x64/i686 through Wine, including cancellation
handoff, invalid framing, wrong-kind non-acquisition and signal preservation.
Removing the cancellation/value-lock synchronization makes the regression test
fail. These results do not establish production timers, title startup, Windows
confinement, physical controls, audible audio or frame pacing. Burnout and Halo
remain unadmitted; no source archive, generated helper, ROM, save, recipe or
launch default changes in this interface checkpoint.


Current-SDK regressions pass for retained drain (384 original-C witnesses per
target), notifications (512 per target) and callbacks (256 per target), including
negative mutation controls, all eight optimized game-object audits on each of
four targets and actual confined Linux x64/i686 fixtures. Windows component
execution uses Wine; Windows prepared executables are compile/link qualification.
A callback witness test previously treated its single-slot channel's Busy as a
fatal worker error and could lose the error report too. The test transport now
retries bounded backpressure without increasing capacity; deliberate full-queue
and delayed-consumer checks plus drop/immediate-failure controls verify it.
The installed app also native-builds signed 2048 and Arkanoo in a private test
library and completes 120 headless frames each. Catalog metadata and all seven
thumbnail hashes/notices pass. No gameplay or physical-device claim follows.


## Original timer-table and shared-worker join checkpoint

`service-kernel-timers/README.md` specifies the new checked port-owned `Timers`
and `Scan` contracts. A fixed 32-slot table retains original set/cancel/ordered
fire behavior and publishes periodic rearm or one-shot removal under its mutex
before returning a firing to the caller. Callback execution belongs outside
that lock. Production `clock.rs` preserves the separate original host-ms and
skew-ms truncations using the existing host-owned counter/KSV3, rather than
rounding a combined guest-nanosecond value.

The existing DPC queue supplies a private-field `Joined` witness only after its
actual consuming join. Multiple owners of the same timer/DPC worker can close
without a second native join. Missing/premature or wrong root/queue/worker
witnesses reject. This introduces no new SDK operation or native object; shared
SDK identity remains `51d880fd16a9cfacef7fb2d9582be1e26e32728685082248ce8f83e510ab1606`
and installed CLI/GUI identity remains `324b3c58fe2292b1`. Existing quotas,
object/source checks, profile gates and the pinned ports submodule stay intact.

The maintainer reference uses pinned original set/cancel/fire and host/guest
clock bodies, explicit Xbox signed 32-bit period width, and an observational
hook before controlled callbacks. The shadow lookup is absent. Tests compare
all 768 slot bytes, results, clock-read counts and callback-order/state traces
for 512 operations per target, plus 64 original clock cases. Invalid/reserved/
stale/foreign metadata, a signaled event forged as the mutex, pre-start access,
CAS interference, clock failures, aborted scans, exact single join and cleanup
after KSV3 revocation are covered. Mutation controls must reject changed
relative rounding, old-due periodic drift, null cancellation, missing firing
publication, premature join witnesses and combined clock/skew truncation.

This is timer-table readiness, not complete timer or title integration. Original
shadow-map/initialize/reset/signal/cancel ordering, lazy first-set startup, actual
DPC handlers/profiling and the retained worker's timer/vblank/spin/TIB/context
loop remain required. No main import or generated source closure is enabled.
Four target component tests and source/object builds, confined Linux fixtures
and Windows Wine runs do not qualify real Windows confinement, title gameplay,
physical controls, audible audio, graphics fidelity or measured frame pacing.


## Checked shadow-map and event adapter checkpoint

`service-kernel-shadow/README.md` now specifies the port-owned `Shadow`, scoped
`Guard` and typed event/header adapters. The map preserves all 4,096 original
logical slots with lazy root-bound pages; its tokens alone confer no native
authority. Real events remain SDK-owned and subject to the unchanged active-sync
quota. Guard counts prevent premature owner close, even on a recursive same-task
hold. Creator-only deletion guards and retained cleanup IDs support teardown
after KSV3 revocation. No new SDK operation or native import is introduced.

Event initialization publishes the event mapping before guest-header writes;
timer initialization clears all 40 guest bytes first. Type 2 remains an auto-reset
event despite guest Type 8. Inline first users double-check the mapping under the
mutex and share one event. Normal set/reset do not mirror guest SignalState;
post-set timer reset leaves that word unchanged. Post-callback signal happens
before writing SignalState=1, so a failed write is an explicit fatal prefix.
Old events close under the map mutex before replacement/removal. Unknown guest
VAs never become native handle/pointer fallbacks.

Map qualification covers all 4,096 slots, 512 original operations/close traces,
eight exact full-slot snapshots per target, hostile metadata/page/type/schema
controls and three required-failing variants. Event qualification separately
covers 256 pinned header/lazy/reset/signal cases per target, actual SDK worker
first-use/auto-reset contention with real joins, exact quota/leak accounting,
stale/wrong-kind handling, read-only prefix failures, revoked cleanup and four
required-failing variants. The complete sources precede generated bindings;
all eight optimized game objects per target precede trusted runtime linking.
Linux x64/i686 execute with actual confinement; Windows x64/i686 component runs
use Wine and Windows prepared binaries are compile/link qualification only.
The strict object audit rejected a compiler slice-copy panic helper; the game
helper now uses bounds-checked fixed-width byte copying instead. The import
allowlist and all profile/admission enforcement remain unchanged.

`CANCEL_REVIEW.md` records a newly confirmed migration gap: the legacy bridge
passes a guest KTIMER to a host-native struct helper whose Inserted offset and
pointer fields differ by target width. It cannot be copied into checked Rust or
justified by a new raw native timer-queue service. Neither examined cancellation
body resets the shadow event, despite the old introductory comment. The complete
cancel/guest wait/object contracts, actual timer callbacks and retained timer/
vblank/spin/TIB/context loop still need integration and qualification before main
imports can close. No current main import is enabled by this checkpoint.
Shared SDK identity, installed CLI/GUI, source archive, recipe, launch defaults,
ROMs/saves and pinned ports submodule are unchanged. These fixtures do not qualify
title startup/gameplay, physical controls, audible audio, GPU fidelity, actual
Windows confinement or measured frame pacing; both title profiles remain blocked.


## Actual native timer/callback/event composition

`service-kernel-timers/firing.rs` and `FIRING.md` now connect the previously
separate checked table, actual native DPC adapter and SDK event shadows. The
pass validates worker/controller/stack/CPU and matching queue bindings before
publication. `callback_queue_id` and `timer_worker_binding` are read-only port
identity getters, not new host permissions or SDK operations. The pass preserves
slot update/unlock, native callback, current shadow lookup, native signal and
guest SignalState publication order. A failure after scan start aborts the actor
without rolling back side effects or fabricating pass completion. Ownership of
retained stack release, actual single join and cleanup remains with the worker.

Nine actual-SDK scenarios execute on Linux/Windows x64/i686 (Windows through
Wine), including replacement/removal from native callbacks, earlier/later timer
mutation, null cases, wrong-queue preflight, native errors and a read-only signal
word. A separate actual producer acquires the timer mutex inside each controlled
native callback; the callback is not running under that lock. All four targets
produce identical witness bytes. Five required-failing controls break signal
order, queue binding, error propagation, actor abort or lock release. The complete
source/object matrix and actual confined Linux x64/i686 execution pass without
any policy/import exemption. Original timer-table and callback differential
checks are requalified for the added getters.

This is checked native composition with controlled finite routines, not original
whole-loop or actual title qualification. Fixture clock/IRQ/profiler providers
cannot be used as production handlers. Cancellation/guest wait contracts,
retained timer/vblank/spin/TIB/context loop and full source/build/launcher closure
remain required. No main import or native title profile is enabled; SDK identity,
installed app, pinned ports submodule, ROMs/saves and launch defaults are unchanged.


## Shared-worker join evidence

The app SDK now mints private-field `LocalTaskJoin` after the actual consuming
join. Queue and pacing owners validate and borrow this same receipt instead of
joining a shared job twice. Timer close retains its existing queue-scoped witness.
See `service-kernel-pacing/SHARED_JOIN.md`. Full SDK is
`fe309935dd602e2cf098e50065f6035de33dac88b51d45a2bc1ba47cf026742f`.
Four-target component tests, complete source/object qualification and actual
confined Linux x64/i686 runs pass; Windows component tests use Wine. Pacing,
queue and timer-firing regressions have fresh receipts for this SDK. This
supersedes previous statements that the installed SDK remained unchanged.
Production worker-loop/context/IRQ/handler closure and title builds/launchers
remain required; no native title profile or pinned submodule is enabled.


## Original worker TIB/TLS setup

`service-thread-tib/thread_tib.rs` now implements the original per-thread TIB
allocation through the existing serialized heap actor and GamePKG-owned checked
RAM. The caller supplies the applied bootstrap's TLS template/extent, retains
its normal context and installs the returned guest address into CPU.fs_base only
after success. Copy order, current inherited bytes, own SEH/slot zero/+4, zeroed
thread data, no-TLS and allocation exhaustion match unchanged original C.

Seven modes use two allocations on an actual SDK worker and a separate heap
actor, comparing both addresses, allocation counters and the complete 64MiB RAM
hash. Invalid metadata, absent heap and revoked-root controls pass. Four
intentionally broken variants fail. Linux/Windows x64/i686 native component
vectors agree (Windows via Wine). Complete source checks, all eight separately
optimized game objects per target and actual confined Linux x64/i686 execution
pass on SDK fe309935dd602e2cf098e50065f6035de33dac88b51d45a2bc1ba47cf026742f.
See `service-thread-tib/README.md` for the exact API, failure-prefix contract and
maintainer command. No C oracle enters a game object.

Production context publication, retained timer-worker stack and CPU/TIB setup,
spin/pause/profiler/IRQ handlers, full source closure and builders/launchers still
need integration. This helper does not start or admit the title, prove physical
input/audio/graphics or measure frame pacing. App SDK/binaries and the pinned
ports submodule are unchanged; the locally installed app already has this SDK.
