# Checked native generated-function migration

This is the first **incomplete** source component, not a playable source kit.
It converts 31 of the 50 native function definitions in pinned unit
`prepared/src/recomp_0000_001.rs`. All 19 remaining definitions are explicitly
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
- `registers.rs`: explicit integer/floating values and fixed 240-byte version-3 LE test
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

Requires app commit `9952b70f520f5fd7060bbf76adbd3496d64f56b5` or a compatible
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

The original single-unit fixture now contains 31 differentially checked functions.
The new maintainer `--inventory` and `--prepare-closed` modes inspect all **485**
kit-pinned generated units. `maintainer/reviewed_units.tsv` binds every input
hash; a full preflight completes before parsing or creating output. Each file
is rechecked before parsing, limited to 16 MiB, and parsed independently.
Symlinks, changed inputs and existing output directories fail. Duplicated native
symbols are never selected by archive order: all owners and their dependent
callers are rejected, including identical definitions.

The pinned kit contains **24,015 unique native definitions**. Existing lowering
rules recognize 21,197 locally; removing missing dependencies transitively leaves
**17,269 closed candidates**, with 1,749 direct edges, including 1,144 across units.
The other **6,746** definitions remain explicitly unconverted. There are no
substitute bodies, dynamic native function pointers or unknown-target fallbacks.

Prepared candidates use 2,178 Rust files (66,263,343 bytes). Eight native
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
children execute the existing 2,176 vectors with the complete graph linked.
**Semantic differential coverage is 31 functions**; compiling/auditing
17,269 functions does not establish the other functions' behavior. Windows
reference comparisons execute under Wine; the prepared service executables
are build evidence, not physical Windows gameplay. The full engine, ISO loader,
kernel/media bridges, verified renderer, saves, input/audio, pacing and the
remaining 6,746 functions still require migration/qualification before enablement.

## Compacted control-flow contract

Private `C2Rust_Block_N` enums with unique data-free `S_N` variants lower to
lexically scoped u32 ordinal values. Payloads, explicit discriminants, generics,
attributes and aliases remain rejected. Label suffixes are not discriminants.
A loop consisting only of an unguarded match on its local state is partitioned
into ordered native match statements; a matched arm finishes that iteration
before another tag is tested. Guarded matches keep their original shape.
This bounds source expression size without decoding any CPU instructions.

Only `unreachable!("GamePKG compacted dispatch state reached")` is recognized
as the pinned generator's fault marker. It becomes propagated Invalid, never
success. An impossible-state pattern becomes a wildcard only in the final arm;
all other macro forms still fail. Original enum-unrepresentable integer values
also fault. Primitive fixed-width `size_of` folds to literal bytes; pointer,
platform-sized, unknown/alias and layout-dependent sizes stay unsupported.

An additional explicitly invoked native test compares three original/converted
CFG variants on 4,096 inputs each for every target (49,152 comparisons), covering
ordinal values, first/final trap ordering, guarded effects and fault propagation.
Windows executions use Wine. The 31-function kit reference suite has 2,176
vectors per target. Private TKN5 vectors add signed extrema around the actual
`0x463b14` load; register records remain 240 bytes, now version 3. Test files are not a
public SDK/save ABI. No physical game/device/pacing evidence is implied.

The whole-kit admission fixture uses
`Plan::with_compiler_memory_bytes(8 * 1024 * 1024 * 1024)` on a 64-bit compiler
host after proving the 4 GiB cap exhausted memory. This budget is validated,
OS-enforced and bound in Plan/receipt identity, separate from game memory. The
app's source checks, exact object audit, import vocabulary and confinement stay
mandatory; no creator flag can grant a larger compiler budget or exempt code.

## Scalar value helpers and control words

`scalar.rs` remains dependency-free checked game code. It provides:

- `round24(f64) -> f64`: nearest/even 24-bit significand rounding over the
  double-backed model's full binary64 exponent range. It preserves signed zero
  and infinity; NaNs retain sign and the payload kept by f64-to-f32-to-f64
  conversion and are quieted. Integer bit operations avoid an overflowing or
  underflowing plain float cast for large/tiny finite inputs.
- `parity8(u32) -> i32`: even parity of the low byte; higher bits are ignored.
- `rotate_carry(u32, u32, &mut i32, u32, bool) -> Result<u32, i32>`: exact pinned
  count masking/modulo, value masking and carry normalization for 8/16/32-bit
  widths. Invalid widths return Invalid before modifying carry. No CPU flags,
  intrinsic, assembly or raw pointer is exposed.

The converter recognizes only unshadowed exact helper calls. Parity/rotate
values must be proven u32. Carry must be the exact mutable address of a proven
local i32; it becomes a safe mutable borrow. Width/direction must be reviewed
literal constants. Aliases, fields, indirect calls, unproven widths, immutable
or wider carry values and malformed helper signatures remain rejected.

`fp_control_word` is an explicit u16 register value. Version-3 test records use
bytes 46..48, previously reserved, keeping the total 240 bytes. No native layout
or host FP state is shared. All 65,536 control words round-trip as data; this is
not qualification of their rounding/exception behavior. TKN5 private vector
files include bounded, batched scalar conformance records after function cases;
old test containers fail their magic check. No public SDK/save ABI changed.

Per target, the unchanged kit helpers are compared bit-for-bit against 114,688
round24 cases across every exponent, signs, ties, NaNs and random inputs; 65,536
parity inputs; and 261,120 rotate/carry cases. Reference bodies are extracted
from individually hashed kit units. Confined Linux children also execute every
record through the same checked helper code. The Windows references run under
Wine. FP exception flags and nondefault host rounding remain **unqualified**;
the complete title remains gated. No helper was moved into trusted host code.

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

- Fifteen regular converter/emitter tests cover pointer/TLS/alias/macro/platform/indirect-call
  rejection, address-width proof, mismatched memory widths, direct dependency
  recording, signatures and attributes. Maintainer strict clippy passes.
- Every converted function matches its unchanged pinned reference in 2,176
  register-bit/whole-8-MiB-RAM vectors per target. A branch-heavy routine covers
  1,024 combinations of flags/output bytes; two matrix/copy functions cover
  128 finite bounded input vectors each; the other 28 cover 32 vectors each.
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
release gate; these 31 functions do not enable the title.

## Checked guest-copy fast path

The SDK's new `local_memory_copy(source, source_offset, destination,
destination_offset, bytes)` uses child-local op 7: five LE u64 words, exact
40-byte request, empty reply, at most 64 MiB. GamePKG validates both complete
ranges and all page permissions before copying under the RAM lifetime lock.
It permits overlap; the port's `Memory::copy_nonoverlapping` deliberately
rejects overlapping backing offsets, even through guest mirrors, preserving
the original caller's explicit forward/backward loops. Existing read/write
transfers remain limited to 1 MiB. No host address or payload buffer escapes.

Reviewed local `*mut u8` scratch values become `Option<u32>` guest addresses.
Only exact pinned constructors, assignments, null tests, nonnegative checked
byte offsets, ordering comparisons and discarded-result `memcpy` idioms lower.
Null scratch and guest address zero stay distinct. Unknown widths, aliases,
raw escape, pointer equality, malformed casts and value-used copies reject.
Numeric ordering uses u64 and permits one-past 2^32; access still requires an
actual mapped range. Counts negative on 32-bit hosts and overflow fail.

The original matrix/copy routines `sub_00013D10` and `sub_00013E50` now have
128 finite bounded input vectors each, covering both direction modes; all
register bits and the whole committed 8 MiB remain exact comparisons. This
does not qualify arbitrary matrix NaN propagation. Private TKN5 vectors add a
seventh synthetic constant span; register records remain version 3.
`copy_checks.rs` is itself source checked, separately optimized/object audited
and run in confined Linux children: 1,048,593-byte copy with every byte checked,
overlapping mirror/readonly/unmapped/overflow rejection and guest-address edges.
Host tests additionally exercise exact 64 MiB copies, overlap in both directions,
full permission checks before writes, malformed requests and revocation on all
four targets (Windows under Wine). These are asset-free CPU-memory tests.

The full-kit graph closes 147 additional definitions. The large guest-copy
function now lowers but still depends on an unconverted dynamic-call resolver
and floating-to-integer conversion;
so does the high-fanout precision path. No body or bridge was stubbed to close
the graph. Burnout remains gated pending complete loader/kernel/renderer and
platform service integration plus physical gameplay qualification.
