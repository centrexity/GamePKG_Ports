# Checked native generated-function migration

This is the first **incomplete** source component, not a playable source kit.
It converts 28 of the 50 native function definitions in pinned unit
`prepared/src/recomp_0000_001.rs`. All 22 remaining definitions are explicitly
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
- `vector.rs`: checked word/float lane access, arithmetic, scalar values,
  shuffles and RAM transfers. Ordinary native math, with explicit NaN policy.
- `registers.rs`: explicit integer/floating values and fixed 240-byte version-2 LE test
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

Requires app commit `cf2dca17930a4e3000c7a076e51a311fe9fc7e2b` or a compatible
later service SDK; stale executable receipts must be rebuilt.

The admission fixture requests `Plan::with_codegen_units(4)` and requires two
to four retained, independently optimized game objects on each target. Every
object is audited before trusted linking; cross-unit calls resolve only against
those exact definitions. Receipts bind both original compiler hashes and exact
audited hashes, the reviewed partition count, source kit, target/compiler and
SDK. MinGW symbol-hiding metadata is normalized only by the app's closed
`RustMingwSymbolExclusionsV1` validator; arbitrary directives and direct platform
imports remain rejected. The fixed-width SDK decoders need no error-formatting
imports, and no runtime vocabulary was expanded for this migration.

The four-target reference checks and confined Linux32/Linux64 execution pass
with this multi-object path. Windows comparisons execute under Wine; Windows
service artifacts are compiled/audited/linked here, not physically played. This
is synthetic component evidence, not full game, device, audio or pacing evidence.

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

## Whole-kit preparation

The original single-unit fixture remains 28 differentially checked functions.
The new maintainer `--inventory` and `--prepare-closed` modes inspect all **485**
kit-pinned generated units. `maintainer/reviewed_units.tsv` binds every input
hash; a full preflight completes before parsing or creating output. Each file
is rechecked before parsing, limited to 16 MiB, and parsed independently.
Symlinks, changed inputs and existing output directories fail. Duplicated native
symbols are never selected by archive order: all owners and their dependent
callers are rejected, including identical definitions.

The pinned kit contains **24,015 unique native definitions**. Existing lowering
rules recognize 18,048 locally; removing missing dependencies transitively leaves
**15,777 closed candidates**, with 376 direct edges, including 266 across units.
The other **8,238** definitions remain explicitly unconverted. There are no
substitute bodies, dynamic native function pointers or unknown-target fallbacks.

Prepared candidates use 1,991 Rust files (16,331,059 bytes). Eight native
functions per module bound source size. An AST pass rewrites only direct function
paths to their owning modules; literals are untouched. Binary selection routes
call statically compiled functions directly, with at most logarithmic selection
depth; unknown entries return Unsupported before any function executes. Invalid
register state returns Invalid. Routing does not fetch/decode instructions.
The emitter does not add game dependencies, aliases, macros, unsafe blocks or
OS/library calls. Input/tool/lock and every emitted-file hash are recorded in a
separate maintainer inventory. `generated/whole_kit_inventory.json` is its compact
reproducible summary, **not an executable or admission receipt**.

All candidates must subsequently pass the normal GamePKG source, dependency,
portability and object checks. The `admission.rs` harness optionally accepts a
prepared directory as its fifth argument, verifies each emitted source hash,
and stages those complete sources before bindings. Its fixture uses synthetic
RAM/vector data and no user ISO, window, physical controls or audible audio.

```sh
python3 host-integration/engines/takedawn/service-native/check_inventory.py \
  --target-dir /path/to/target \
  --output /path/to/new-private-candidate-directory
python3 host-integration/engines/takedawn/service-native/check.py \
  --app /path/to/GamePKG/app \
  --app-lib /path/to/target/debug/deps/libgamepkg-HASH.rlib --wine \
  --prepared /path/to/new-private-candidate-directory
```

The whole graph passes source admission and separate optimized object auditing/
trusted linking for Linux64/Linux32/Windows64/Windows32. Confined native Linux
children execute the existing 1,888 vectors with the complete graph linked.
**Semantic differential coverage remains 28 functions**; compiling/auditing
15,777 functions does not establish the other functions' behavior. Windows
reference comparisons execute under Wine; the prepared service executables
are build evidence, not physical Windows gameplay. The full engine, ISO loader,
kernel/media bridges, verified renderer, saves, input/audio, pacing and the
remaining 8,238 functions still require migration/qualification before enablement.

## Floating-point portability

The original Rust helper's choice between two NaN operands differs between
32-bit and 64-bit compiled references. Rust explicitly permits this payload
nondeterminism in arithmetic; see the [Rust f32 contract](https://doc.rust-lang.org/stable/core/primitive.f32.html#nan-bit-patterns).
Our add/subtract/multiply helpers, including scalar lane expressions, instead
quiet the first NaN operand and preserve its sign/payload; if only the second
operand is NaN, they quiet and preserve that operand. No-NaN inputs use the
original native arithmetic. Scalar construction, raw lanes, XOR and shuffle
preserve bits exactly and do not quiet signaling NaNs.

Helper tests independently assert this exact portable result for every NaN
input. Only the raw reference's choice between **two** NaN payloads is allowed
to differ, and only if it is one of those operands quieted. These differences
are counted and printed (32 in the tested Linux64 reference, zero in the other
three references). Finite/infinite results and every non-arithmetic operation
remain exact-bit baseline checks; full function/register/RAM vectors remain
exact. No build, source, dependency, object or confinement check is relaxed.
Native FP exception flags and nondefault rounding modes still need migration;
these comparisons use the default floating environment.

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

- Ten converter tests cover pointer/TLS/alias/macro/platform/indirect-call
  rejection, address-width proof, mismatched memory widths, direct dependency
  recording, signatures and attributes. Maintainer strict clippy passes.
- Every converted function matches its unchanged pinned reference in 1,888
  register-bit/whole-8-MiB-RAM vectors per target. A branch-heavy routine covers
  1,024 combinations of flags/output bytes; 27 functions cover 32 vectors each.
- 1,575 vector-helper edge cases and all 256 shuffle selectors pass, with the
  precise dual-NaN policy comparison above, checked lane bounds and bit views.
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
release gate; these 28 functions do not enable the title.
