# Shared-service migration status — 2026-11-14

This is tested integration work, **not a playable Burnout release**. The legacy
profile remains blocked. No source pin, signature, cached executable or engine
exception may bypass that block. The source kit and catalog URLs are unchanged.

Incremental save migration: [service-save/README.md](service-save/README.md).
App `75d180311f5e29765ad4778f265908daaa0afb31` adds scoped metadata and duplicate
file handles. The save helper source passes all four target object audits and
three real-service helper tests; the complete recovery/worker guard is pending.

The current application contract is GamePKG commit
`7d350e10572cf61e451f623324d78858fd327784`:

- `docs/PORT_SERVICES.md`: exact wire/API, resource, permission and error contracts.
- `docs/TAKEDAWN_SERVICE_HANDOFF.md`: builder, launcher, rendering and remaining work.
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
python3 tools/check_catalog.py
python3 tools/check_current_app.py --cli /path/to/gamepkg
```

Results against that app revision:

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
