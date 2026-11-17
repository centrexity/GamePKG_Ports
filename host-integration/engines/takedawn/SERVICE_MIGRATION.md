# Burnout shared-service migration handoff — 2026-11-14

Current component results and superseding app contracts:
[SERVICE_STATUS.md](SERVICE_STATUS.md). Interface tables below preserve the
earlier review and must not be used as the current SDK specification.

Status: **incomplete, not enabled in the recipe or launcher**. The legacy
integration is not compliant merely because the shared SDK exists. No app files,
generated game functions, source archive, ROMs, saves or launch defaults changed.
The rejected vertex-ring experiment has not been adopted. This handoff records
the initial migration baseline and the subsequent interface coordination below,
not gameplay qualification.

## Shared-service coordination update

The app task has added the retained-target, color-copy, whole-pass visibility,
save-replacement, clock/wait/wake and exact-object staging contracts. Reviewed
the current `app/docs/TAKEDAWN_SERVICE_HANDOFF.md` and `PORT_SERVICES.md`.
The original missing-interface table below is historical, not the current SDK.

The current port-side response is
[renderer contract review](service-renderer/CONTRACT_REVIEW.md), with a
reproducible source-hash/enum inventory. It enumerates vertex operations,
combiners, texture modes, render states, attachment identity/format requirements
and the full AA/sharpening variants found in the pinned sources. Static source
vocabulary is explicitly separate from observed gameplay usage.

Remaining coordination includes stencil and shared zeta/depth identity,
rendered-target copies into cubemap faces, sampler/state expressiveness,
surface CPU coherence, verified vertex/combiner programs and complete
postprocessing. The reviewed service has fixed RGBA8/Depth32Float targets and
only installed verified pipelines; the review does not authorize arbitrary
runtime shaders or an engine-in-host exemption. Launcher and builder integration
are still required. App regression results remain owned by the app task; its
reported offscreen tests do not qualify Burnout gameplay.

## Port-side component prepared here

`service-input/pad.rs` is dependency-free Rust game logic using the unmodified,
host-supplied native client as `gamepkg`. It translates logical service players
to the original Duke fields: original button ordering, full signed stick range,
Y-up axes, optional Nintendo face mirroring and the existing native XInput
trigger quantization. The original title's deadzones, control scheme remapping,
keyboard merge, overlay guard and rumble curve must still run in the input core;
this component does not replace them. Its caller owns serialization and copies
the returned fields into the existing title report.

It tracks per-player packets by controls and device identity, including neutral
focus reports, disconnects, replacements and recovery after service errors.
Rumble addresses logical players with the existing 250 ms effect lifetime;
errors, including Unsupported, propagate. There is no device-provider or OS
fallback. Callers must neutralize cached title state on errors or disconnection.

Run `sh host-integration/engines/takedawn/service-input/check.sh [app-path]`.
Four synthetic tests cover all 65,536 button masks and all 65,536 analog values,
packet/lifecycle transitions, player isolation and actual unbound-client denial.
The production path is also compiled with warnings denied. These checks do not
establish physical hotplug, focus delivery, rumble, keyboard/menu integration or
gameplay. The component is **not connected** to the current native input core.
Do not package it as a working migration or copy the transport into game source.

## Initial app requests (historical; superseded by coordination update)

| Boundary | Current evidence | Required shared interface or integration |
| --- | --- | --- |
| Render surfaces | `src/port_host/graphics.rs::render` creates and clears fresh color/depth attachments for each indexed frame. Texture uploads are sampled textures, not addressable render attachments. Pinned `toolkit/src/d3d/nv2a_pb_vulkan.c` uses per-surface retained targets, copies, partial attachment clears and target sampling. `media-bridge/native_d3d11.c` has equivalent retained-target and self-copy paths. | Add bounded, session-owned render-target IDs with render/sample transitions, copy/resolve and retained color/depth load operations, ordered multipass submission and partial clears. Return only opaque IDs. Preserve original surface ownership and postprocessing, including the default SMAA/sharpening path. |
| Visibility reports | The existing renderer begins/ends occlusion queries and polls counts asynchronously (D3D11 `occ_*`; Vulkan visibility path). ABI 1 operations 100–105 expose no queries/results. | Add bounded query handles and ordered begin/end/poll/result operations, including stale-query lifetime rules. Do not replace actual counts with always-visible results. |
| Native programs/pipelines | Vulkan `module_from_glsl`, `vs_build` and `fs_build` translate Xbox state/programs into GLSL/SPIR-V. The shared `Pipeline` is installed by the host before execution, with one uniform binding, fixed blend/depth/cull and a fixed verified program list. | Agree a verified representation for the required programs and state. A signed, validated static interpreter/template may avoid runtime compilation, but needs port fidelity tests. Otherwise extend shared preparation with bounded verified variants and opaque pipeline selection. Do not accept arbitrary game shader strings or move NV2A/the engine into `Backend`. |
| Save durability | ABI 1 has positional write, sync, resize and remove, but no rename/replace. Existing Windows adapter delegates `MoveFileExW`/rename to the OS; original saveguard relies on replacement semantics. | Add capability-relative atomic replace/rename with source/destination containment, symlink/race checks and explicit durability semantics. Preserve original profile/saveguard behavior; direct truncation is not an equivalent substitute. |
| Timing/waits | Input core requires monotonic milliseconds for probe, overlay and rumble refresh; the runtime uses `kernel_pacing.c` for waits/frame pacing. Input timestamps describe samples, not a clock that advances without input. ABI 1 has no clock/wait operation. | Provide a host-owned monotonic clock and bounded wait/wakeup/deadline contract suitable for native pacing. Keep blocking requests off the window owner thread. Audit thread/synchronization/runtime imports too; none gain an engine-wide exemption. |
| Launch/session | `src/takedawn_player.rs` currently builds a bwrap command exposing `/dev/dri`, `/dev/snd`, `/dev/input`, selected disc and writable state. Windows has a separate build/player path. Neither Takedawn path connects the new private service client. | Create the session on the app side, own the game's window/provider/graphics/audio, use `broker::channel` and bounded owner-thread dispatch, bootstrap private inherited pipes, grant ROM/assets/private saves, charge shared budgets, and remove legacy device/display/audio/file access while retaining OS confinement on both launchers. Attach host rumble provider; preserve Unsupported where absent. |
| Build/cache enforcement | `src/takedawn/build.rs` and `windows_build.rs` consume the host-pinned source kit and embedded bridge. Changes to this checkout's integration snapshot are not automatically consumed. | Stage the reviewed ports integration; source/dependency-check game code before injecting generated bindings; audit **all** game objects together with `port_host::audit::objects` and pinned nm before trusted-client linking. Record `port_host::build_identity()`, shader hashes, compiler/target identities and updated source-kit digest in cache receipts. No Cargo dependency exemption for the media bridge or engine. |

The SDK already has input snapshots/events, window commands, PCM16 output,
ROM reads, capability file operations and HTTPS/TCP/UDP; do not invent parallel
per-port OS implementations for those. Migration still needs port adapters:
HID keyboard/menu/drive mapping and event ownership, retained touch behavior,
audio chunk/backpressure handling, ROM-offset/index translation and all save
operations. The existing `media-bridge` includes app reader/input modules and
Cargo dependencies; linking that legacy archive into game objects is not the new
service boundary. Keep disc indexing/game translation separate from host file
access and preserve reads in place. Network permission remains false; online
play is not qualified, and direct socket imports must still be removed/denied.

## Validation and merge gates

Checkout write/delete probe succeeded. GitHub CLI is installed but its saved
accounts fail authentication; no network operation is needed for local work.
Catalog validation passes for all seven dependency-free recipes. New component
tests are synthetic, with no game window, selected ISO or physical devices.
No gameplay, audible audio, physical keyboard/controller, Windows runtime,
GPU fidelity, frame-pacing or performance validation was performed this session.
No performance change or optimization claim is made without a running migrated
path to profile. Historical private evidence is preserved and not republished.

`main` at `5deca49` shares Burnout preview ancestry (merge base
`d8672b5`) and has recipe 0.1.16 with the Xbox title. At session start `takedawn/`
was identical between `main` and `workspace/individual-ports`. The older local
`native-takedawn-rust-preview` tip `95c7e0d` is not an ancestor of main.
`git log main..native-takedawn-rust-preview` lists only that title commit;
main already has the same title through another commit. Its tree still has
recipe 0.1.4 and lacks the current launch defaults. There is no missing Burnout
feature to recover by merging that old branch.

After the app contract and launcher/build work land, wire and qualify this port
on the new boundary, including original saves/visuals, menus/driving, analog
extremes, hotplug/focus/rumble, audible output and measured CPU/GPU frame times.
Only then merge the scoped Burnout commits into local main. This session leaves
main and historical worktrees untouched because that completion gate is unmet.

## Checked critical-section checkpoint (2026-11-17)

The isolated `feature/takedawn-shared-services` integration adds checked CSD1
recursive critical sections and explicit KIC5 startup for active imports
291/277/294. See [the exact contract](service-critical/README.md). It consumes
unchanged app SDK `49a313ff062e83e3bcbaadb125f4ace190847b0321cc25877e45f92023f0234a`.
Normal source scanning, separate optimized game units, collective object audit,
SDK/object/source-kit receipts and process confinement remain mandatory. No
engine/media exemption or legacy player enablement was added.

KIC5 private image diagnostics report 95 pending imports; KIC4 remains at 98.
This is not title entry or a playable port. Next work includes demonstrated
active file ownership, thread creation/termination and actual NT wait/APC paths,
then complete generated-source, renderer, media and launcher integration. Existing
local GamePKG binaries already supply this SDK; these game-side changes do not
require replacing them or enabling blocked legacy ports.
