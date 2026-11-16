# Shared-service migration status — 2026-11-16

This is tested integration work, **not a playable Burnout release**. The legacy
profile remains blocked. No source pin, signature, cached executable or engine
exception may bypass that block. The source kit and catalog URLs are unchanged.

## Current checkpoint: checked IRQL controller

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
