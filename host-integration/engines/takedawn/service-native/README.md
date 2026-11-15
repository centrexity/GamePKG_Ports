# Checked native generated-function migration

This is the first **incomplete** source component, not a playable source kit.
It converts 24 of the 50 native function definitions in pinned unit
`prepared/src/recomp_0000_001.rs`. All 26 remaining definitions are explicitly
listed in `generated/provenance.json` with reasons. There are no replacement
stubs, unsafe fallback links or exemptions. The game's legacy build/cache gate
and source-kit/catalog pins remain unchanged.

The original native functions and branches remain Rust functions and branches.
TLS pointer globals become explicit borrowed register values; guest memory
operations become GamePKG-owned RAM-service calls. The finite entry router
matches an original function label to a direct compiled call. It does not fetch
or decode instructions and is not the rejected virtual-CPU/native-gen path.
The original C2Rust reference bodies are used only by maintainer comparisons;
they are never included in admitted game objects.

## Files and ownership

- `generated/game.rs`: checked generated native function bodies; do not edit.
- `generated/dispatch.rs`: finite direct-call routing; unknown targets fail.
- `generated/provenance.json`: input/kit, converter source/lock, formatter and
  output hashes, coverage and excluded definitions. `production_ready` is false.
- `registers.rs`: explicit integer/floating values and fixed 112-byte LE test
  record. No TLS globals, native struct layout, pointers or aliases.
- `memory.rs`: up to 32 explicit, nonoverlapping guest spans referencing existing
  opaque GamePKG RAM windows. Mirrors can share backing offsets. No native
  addresses, allocations/protection changes, OS I/O or code execution occur here.
- `maintainer/`: source converter and its parsing/hash dependencies. This is
  maintainer tooling, **not game Cargo dependencies** or a player requirement.
- `comparison.rs`, `oracle_prefix.inc.rs`, `fixture.rs`, `admission.rs`, `check.py`:
  maintainer fixtures. The admitted fixture contains the exact production source;
  the raw reference is separately compiled for comparison and stays rejected by
  game admission even if accidentally packaged unused.

New adapters/tooling follow the repository license. Original Takedawn and
XboxRecomp notices are preserved in `UPSTREAM_LICENSES.txt`, and copied into the
temporary signed test package. No game media, ISO contents, assets, saves or
private keys are included. Reference data is synthetic and generated locally.

## Contracts

Requires app commit `62f3a61f002cd89707952ef415d772feafca8dc8` or a compatible
later service SDK; stale executable receipts must be rebuilt.

The app's host-controlled `LocalLayout::TakedawnRam` supplies one 64 MiB physical
RAM backing window (initially inaccessible, non-executable). It is separate from
the total process memory limit. Only opaque window ID and length reach the game.
The port loader must commit pages and configure its reviewed guest aliases.
Each scalar access is explicit LE and alignment independent. Overflow beyond
2^32 is Invalid; unmapped, read-only/protected and revoked access is Denied.
Reads are at most 1 MiB and writes at most 1 MiB minus the 16-byte local request
header. Empty byte transfers do nothing. Host page rights remain authoritative.

Converted functions receive `&mut Registers` and `&mut Memory`, return
`Result<(), i32>`, and retain ordinary native arithmetic/control flow. The
converter accepts only proven u32 guest-address expressions when removing the
original host relocation. Unsupported types, TLS, calls or dependencies do not
enter the output. Unknown entry labels and memory faults propagate; the engine
must stop/report a native fault rather than treat an error as success. Earlier
register/RAM changes in a faulting function are not rolled back.

## Reproduce

Run with current app library and its offline dependencies available:

```sh
python3 host-integration/engines/takedawn/service-native/check.py \
  --app /path/to/GamePKG/app \
  --app-lib /path/to/target/debug/deps/libgamepkg-HASH.rlib --wine
```

The checker verifies the entire kit SHA-256
`6b37d24c0984c686c7ed6fd73cd3a5b252b5631ceefa2798e5b7a6e0adff8dc4`
and exact unit SHA-256
`442f7812516f3c5157ee74e15aede23d0ac423105acb7d973733bbe9f01ad94d`,
then builds the locked maintainer AST converter. The converter also refuses
unreviewed input hashes before writing output. It runs rustfmt for readable
output and records that formatter's version. Checked-in files must regenerate
byte-for-byte. The generation happens before publication; users need only the
resulting Rust source and GamePKG compiler, with no Python or converter setup.

Executed checks:

- Four converter tests cover pointer/TLS/alias/macro/platform/indirect-call
  rejection, address-width proof, mismatched memory widths, direct dependency
  recording, signatures and attributes. Maintainer strict clippy passes.
- Every converted function matches its unchanged pinned reference in 1,760
  register-bit/whole-8-MiB-RAM vectors per target. A branch-heavy routine covers
  1,024 combinations of flags/output bytes; 23 functions cover 32 vectors each.
- Native Linux64/Linux32 and Windows64/Windows32 under Wine execute comparisons.
  Extra checks cover explicit aliases, unaligned LE values, transfer/range
  bounds, read-only/protected/uncommitted memory, invalid top indices and revoke.
- The exact game source is scanned before generated bindings, separately
  optimized/object-audited, then trusted-linked for all four targets. An unused
  unsafe/OS reference remains denied. No source/import allowlist changed.
- Actual confined Linux64/Linux32 children execute every reference vector via
  GamePKG-owned RAM and a private read-only synthetic vector grant. Output is
  recorded in memory; no physical audio/window/input or user media is involved.

These are synthetic component checks. Windows comparison under Wine is not
Windows hardware, AppContainer gameplay, graphics or driver qualification.
No visible fidelity, audible audio, controller/rumble, original save behavior,
frame pacing or performance is established. The first memory adapter uses the
existing per-access local service; game performance still needs measurement and
optimization after integration.

## Remaining before enabling Burnout

Convert the rest of this unit and the full generated call graph, including
vector/MMX/floating helpers and kernel compatibility. Complete original RAM
aliases and device-boundary handling, ISO loader, reviewed save-worker calls,
input/audio and verified surface/NVI1/postprocessing integration. Supply the
complete checked source kit, shader/pipeline descriptors and reviewed entry/task
table to both builders/launchers, preserving all object audits and confinement.
Only real game/save/graphics/controls/audio/pacing qualification can satisfy the
release gate; these 24 functions do not enable the title.
