# Checked native generated-function migration

Latest bounded-debug checkpoint: 19,295 literal-dependency-closed candidates;
38 original functions / 4,864 native reference cases per target. The named
entrypoint's literal chain now closes. Runtime routes, bootstrap, graphics and
media remain incomplete, so Burnout is not enabled. Earlier checkpoint sections
below preserve their narrower coverage and are superseded by the final section.

This is the first **incomplete** source component, not a playable source kit.
It converts 32 of the 50 native function definitions in pinned unit
`prepared/src/recomp_0000_001.rs`. All 18 remaining definitions are explicitly
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
- `registers.rs`: explicit integer/floating values and fixed 244-byte version-4 LE test
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

Requires app implementation `38a84d4` or a compatible later SDK, including
local task-completion snapshots (op 16), gather/scatter (9/15), fill (8) and
bounded byte diagnostics (service 64). Shared-service digest is
`0db7bde13239a9ee091c18840f476a6844ff7af1015945a62b6de5e768e7b4bd`;
the installed composite native recipe identity is `dd1e2928b1a6fb54`.
These are different identities. Missing operations are insufficient; stale
executable receipts must be rebuilt.

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

The original single-unit fixture now contains 32 differentially checked functions.
The new maintainer `--inventory` and `--prepare-closed` modes inspect all **485**
kit-pinned generated units. `maintainer/reviewed_units.tsv` binds every input
hash; a full preflight completes before parsing or creating output. Each file
is rechecked before parsing, limited to 16 MiB, and parsed independently.
Symlinks, changed inputs and existing output directories fail. Duplicated native
symbols are never selected by archive order: all owners and their dependent
callers are rejected, including identical definitions.

The current inventory covers **24,016 definitions**: 24,015 distinct
`sub_XXXXXXXX` bodies, including recovered/unresolved sources, and the exact
unit-bound `xbe_entry_point`. Five `_gen` variants still need inclusion; this
count is not the entire runnable title. Existing lowering
rules recognize 23,008 locally; removing missing dependencies transitively leaves
**19,295 candidates closed over literal direct dependencies**, with 5,456 direct edges, including 3,692 across units.
Runtime routes remain partial: 843 retained functions contain rewritten runtime
calls, and known unmigrated destinations fail Unsupported. This is not a fully
closed runtime graph. The other **4,721** definitions remain explicitly
unconverted. There are no
substitute bodies, dynamic native function pointers or unknown-target fallbacks.

Prepared candidates use 2,437 Rust files (121,975,559 bytes). At most eight native
functions per module plus byte/token-aware splitting bound source size. An AST pass rewrites only direct function
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
  --prepared /path/to/new-private-candidate-directory \
  --call-targets /path/to/new-private-targets
```

The whole graph passes source admission and separate optimized object auditing/
trusted linking for Linux64/Linux32/Windows64/Windows32. Confined native Linux
children execute 4,864 vectors with the complete graph linked, including 1,024
cases for original `sub_00244DF0` and 512 cases for original `sub_001FBAE0`.
**Semantic differential coverage is 38 functions**; compiling/auditing
19,295 functions does not establish the other functions' behavior. Windows
reference comparisons execute under Wine; the prepared service executables
are build evidence, not physical Windows gameplay. The full engine, ISO loader,
kernel/media bridges, verified renderer, saves, input/audio, pacing and the
remaining 4,721 functions still require migration/qualification before enablement.

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
Windows executions use Wine. The 32-function kit reference suite has 2,304
vectors per target. Private TKN9 vectors add signed extrema around the actual
`0x463b14` load; register records remain 244 bytes, now version 4. Test files are not a
public SDK/save ABI. No physical game/device/pacing evidence is implied.

The whole-kit admission fixture uses
`Plan::with_compiler_memory_bytes(12 * 1024 * 1024 * 1024)` on a 64-bit compiler
host. The expanded graph exhausted an 8 GiB compiler cap with both four and
two independently optimized objects; two objects alone did not solve peak
memory use. The earlier, smaller graph fit 8 GiB after exhausting 4 GiB.
This compiler-only budget is validated,
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
bytes 46..48 in the unchanged 240-byte prefix; v4 appends the guest FS word. No native layout
or host FP state is shared. All 65,536 control words round-trip as data; this is
not qualification of their rounding/exception behavior. TKN9 private vector
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
  --app-lib /path/to/target/debug/deps/libgamepkg-HASH.rlib --wine \
  --call-targets /path/to/new-private-targets
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

- Twenty-six regular converter/emitter tests cover pointer/TLS/alias/macro/platform/indirect-call
  rejection, address-width proof, mismatched memory widths, direct dependency
  recording, signatures and attributes. Maintainer strict clippy passes.
- Every converted function matches its unchanged pinned reference in 2,304
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
release gate; these 32 functions do not enable the title.

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
does not qualify arbitrary matrix NaN propagation. Private TKN9 vectors add a
seventh synthetic constant span; register records remain version 4.
`copy_checks.rs` is itself source checked, separately optimized/object audited
and run in confined Linux children: 1,048,593-byte copy with every byte checked,
overlapping mirror/readonly/unmapped/overflow rejection and guest-address edges.
Host tests additionally exercise exact 64 MiB copies, overlap in both directions,
full permission checks before writes, malformed requests and revocation on all
four targets (Windows under Wine). These are asset-free CPU-memory tests.

The full-kit graph closes 147 additional definitions. The large guest-copy
function now lowers but still depends on an unconverted dynamic-call resolver;
so does the high-fanout precision path. No body or bridge was stubbed to close
the graph. Burnout remains gated pending complete loader/kernel/renderer and
platform service integration plus physical gameplay qualification.

## Control-word rounding and signed integer stores

Port-owned `scalar::round_integer(value: f64, control: u16) -> f64` implements
all four control-word rounding selections: nearest with ties to even, floor,
ceiling and truncation. It operates on binary64 value bits, retains signed zero,
and returns nonfinite inputs unchanged. It neither reads nor changes host CPU
rounding state. This is a value helper for already compiled Rust functions,
not an instruction decoder or a system service.

`scalar::integer_store(value: f64, control: u16, width: u32) -> Result<i64, i32>`
accepts 16, 32 or 64 bits. It rounds first, then returns the requested signed
minimum for NaN, infinity or values outside that signed range, matching the
pinned FIST helper. Other widths return Invalid before a shift or conversion.
The converter requires exact unshadowed helper calls, a proven u16 control word
and a reviewed literal store width; no raw function address or host import is
introduced. The helpers remain ordinary source-checked/audited game code.

For each of Linux32/Linux64 and Windows32/Windows64 under Wine, 278,936 exact
rounding-bit cases and 836,808 signed integer-store cases match the unchanged
`recomp_frndint`/`recomp_fist` bodies from individually pinned unit
`recomp_0007_001.rs`. Cases cover all exponent classes and all 65,536 control
words, signed zero, subnormals, quiet/signaling NaNs, infinities, ties and
neighbors of all three signed range limits. Invalid store widths reject.
Private TKN9 vector trailers carry these results in bounded batches to actual
confined Linux32/Linux64 children; register records remain version 4/244 bytes.
This qualifies these helpers' returned values, not x87 exception flags or the
entire game's floating environment and behavior.

Preparation now partitions by both source bytes/tokens and a maximum of eight
functions, retaining static direct routes and exact module ownership. It reserves
space for generated routing/remapping within unchanged app source limits.
Oversized individual functions fail before output rather than being omitted or
exempted. Object audits, compiler budgets, source admission and legacy gates
remain independent and mandatory.

The prepared-graph comparison suite additionally executes the unchanged original
`sub_00244DF0` against its exact prepared body on each reference target. Its
1,024 cases cover all four rounding selections, randomized FP/register state,
integer-range edges, signed zeros, subnormals, infinities and NaNs. Register bits
and all committed 8 MiB RAM compare exactly; its 1,024 records also run in
confined Linux32/Linux64 children linked with the complete candidate graph.
The single-unit golden remains 32 functions/2,304 vectors; prepared runs cover
37 functions/4,736 vectors, without treating other compiled functions as
semantically qualified. `sub_00244DF0` has no native function dependencies; its
reference is extracted only from its exact hash-pinned unit, and its prepared
body from an inventoried/hash-checked source file. No extra body is admitted
by the reference harness: production admission still stages and checks the
complete prepared tree before host bindings.

## Next resolver migration requirements

The exact reviewed trace/lookup sequence is now lowered from pinned
`sub_001FBAE0` in `recomp_0005_013.rs`. Unmatched or changed sequences remain
rejected. The replacement preserves these original requirements:

- Record the guest target, advance the bounded trace ring and wrapping call
  counter before validating the target. These need owned, bounded value state;
  raw globals/volatile pointers must not survive.
- Preserve the code-range test, including kernel-address classification and
  the original zero-upper-bound condition. Disc/header-derived ranges must be
  validated guest numbers, never host executable addresses.
- Preserve lookup precedence: reviewed manual overrides, generated functions,
  then reviewed kernel handlers. A finite router may call compiled Rust paths
  directly; game code must not acquire callable native pointers.
- Distinguish an original missing/non-code target from a known but unmigrated
  target. The former retains the original diagnostic, saved guest stack-pointer
  restoration and zero-EAX behavior. The latter must report an explicit required
  migration fault; it cannot masquerade as the original lookup's absence.
- Retain the surrounding original argument setup, stack cleanup and register
  logic. Kernel/manual bodies still need source admission and GamePKG services;
  a router is no exemption or replacement for those implementations.

No resolver implementation, title enablement or physical gameplay qualification
is included in this checkpoint.

## Resolver data and host diagnostic boundary

`call_state.rs` now owns the original 16-target ring, wrapping u32 ring index,
wrapping u64 total, and 16-offender hit table. `call_record` writes/advances
before checking the original code predicate (kernel threshold, zero-high bypass,
and half-open code range). `call_noncode_values` reproduces the original logger's
first-16-distinct-target policy and powers-of-ten throttle, including wrapping
hit counts. `call_missing_values` retains the raw ring and its index so the
original chronological nonzero trace can be recovered without losing zeroes.
Malformed nonzero code ranges reject; all numbers remain guest values.

`call_emit_noncode` and `call_emit_missing` use GamePKG's new `diagnostic` API
(op 63), not native console I/O. Codes are `0x544B0001` for non-code and
`0x544B0002` for lookup absence. Their payloads are respectively
`[target, hits, total]` and `[target, total, ring_index, 16 raw ring values]`.
Service failures propagate; there is no stdout/stderr fallback. The host bounds
each session to 128 records with at most 32 numeric values, tracks evictions,
and includes an escaped-by-construction numeric summary in existing owner error
reporting. These are diagnostics only, with no filesystem/network/device grant.

The separately pinned `takedawn_call_targets` maintainer binary parses the exact
Rust dispatch AST, validates its array length, sorted unique addresses/symbols,
explicit widths and direct zero-argument function casts, and checks all input
hashes before output. It emits dependency-free numeric classification code,
never callback pointers. The original table has 24,001 entries: 24,000 numeric
`sub_` labels plus `xbe_entry_point` at `0x001D2807`. Five labels name manual
wrappers. None of the 13 unresolved source stubs belongs to this lookup table;
a source definition must not be exposed merely because its name has an address.

The generated `targets::original_target(u32) -> u32` returns zero for original
absence. The high byte tags generated/manual-wrapper/entrypoint/soundtrack/
kernel/unresolved identities as 1/2/3/4/5/6; low 24 bits are the exact table row
or reserved-method slot. Classification preserves original lookup precedence.
Soundtrack methods occupy 10 aligned four-byte slots at `0xFE7F5700`; kernel
lookup has 378 slots from `0xFE000000` and deliberately accepts byte addresses
within each slot, matching the pinned bridge. These tags are port-owned values,
not native callable objects or a public SDK ABI. No handler executes here.

```sh
python3 host-integration/engines/takedawn/service-native/check_call_targets.py \
  --target-dir /path/to/target --output /path/to/new-private-targets --wine
python3 host-integration/engines/takedawn/service-native/check.py \
  --app /path/to/GamePKG/app --app-lib /path/to/target/debug/deps/libgamepkg-HASH.rlib \
  --prepared /path/to/prepared-native-graph --call-targets /path/to/new-private-targets --wine
```

The target tool validates 106,272 original-table, neighbor/miss, random and
reserved-range/alignment cases on each reference target. The admitted fixture
checks those vectors through read-only private file grants in real Linux32/64
children and verifies the host received the exact two synthetic diagnostic
records plus original-call diagnostic records. Source checks run before host bindings; all target-classifier and
call-state objects participate in the same strict audits as generated functions.
Call-state tests on all four targets cover range edges, ring/counter wraparound,
16-offender saturation, power-of-ten output and diagnostic error propagation.
Windows reference execution is under Wine. There are no physical controls,
audible audio, visible rendering, FPS or actual title-handler results here.

The maintainer matcher compares all four original trace/lookup/fallback
statements as one AST after narrowly scoped temporary/label normalization.
Target and saved-stack values must be proven u32; TLS aliases must name ESP/EAX.
Changed guards, masks, counters, lookup order, fallback assignments, marker
strings, partial sequences or shadowed globals remain rejected. It emits one
safe `calls_runtime::invoke_call` with direct generated dispatch. Original
non-code and absent-table destinations preserve EAX/ESP fallback; known but
unmigrated generated/manual/entry/kernel/soundtrack destinations return
Unsupported rather than absence or success. No native callback or instruction
interpreter is introduced.

One title's trace/throttle state uses an opaque GamePKG atomic record. Memory
views and workers explicitly copy its ID; 280-byte LE snapshots and bounded
64-attempt compare/exchange updates retain coherent counters without importing
platform mutexes into game objects. Release revokes all copied IDs. No lock is
held across game execution or broker diagnostics. Numeric diagnostics follow
a committed update; concurrent tasks need not publish them in counter order.
The host reserves and limits records as documented in PORT_SERVICES.md.

Reference execution now adds 512 original `sub_001FBAE0` cases on each of four
targets, checking whole 8 MiB RAM, every register bit, trace and numeric logger
fields/order against the unchanged pinned routine. Selected original generated
callee, true table absence, non-code guards, zero/high ranges and fallback
stack cleanup are exercised. Separate guard checks cover known unmigrated
categories and revocation; they do not implement those handlers. Private
vectors use TKN9 with code bounds and expected trace values. The named roots
below have explicit reviewed units: the entrypoint is now inventoried but not
retained, while the five `_gen` bodies still require inclusion:

| Symbol | Pinned source unit |
| --- | --- |
| `xbe_entry_point` | `recomp_0004_013.rs` |
| `sub_001537C0_gen` | `recomp_0003_002.rs` |
| `sub_001D0554_gen` | `recomp_0004_012.rs` |
| `sub_001F65A0_gen` | `recomp_0005_009.rs` |
| `sub_002FAAEF_gen` | `recomp_0022_001.rs` |
| `sub_0030211F_gen` | `recomp_0022_004.rs` |

`generated/call_targets_summary.json` is compact source/classification evidence,
not an executable, admission receipt or title enablement. Full classifier source
and vectors remain private verification outputs; players need no maintainer
converter or Python. Source-kit/catalog pins and legacy gates remain unchanged.

## Compacted call-region proof

The maintainer `icall_compacted` pass reconstructs only the complete reviewed
region from pinned `recomp_0002_001.rs` / `sub_000CDDD0` (unit SHA-256
`6b8df098cc94321e31a452d2c764367f7961a97897de5dc2f9d94a08790d99c2`).
It verifies the entry's four trace/guard groups and all eight lookup/fallback
arms together after bounded temporary/state-name normalization. Changed guards,
counters, state edges, call order or extra effects fail the whole comparison.

The selector must have exactly one explicitly typed private binding. Only
literal-variant assignments and match selection may use it; observation,
borrowing, inferred declarations and shadowing are rejected. Qualified incoming
variant expressions outside the complete region reject reconstruction, including
initialization and shared lookup nodes. This proof is cached only within one
unchanged original function, never across inputs or converter identities.

The pass reconstructs the already reviewed contiguous template and a common
exit assignment. Existing u32/TLS/shadowing proofs must still accept it. Only an
exact, now-unused `let mut _fn: recomp_func_t = None` declaration can be removed;
any surviving function-pointer type is explicitly rejected. Former internal
arms become proven-impossible traps, then ordinary lowering emits Err for them.
No engine exemption or new game callback mechanism is introduced.

Native synthetic reference checks compare 7,680 original/reconstructed cases
per target with controlled handlers, covering lookup precedence, code bounds,
fallback registers, trace/counter wrapping and diagnostics. These are graph
transformation tests, separate from original-function qualification.

Prepared runs additionally compare 256 original `sub_00243450` cases with bounded
arrays and actual registered `sub_00015590`, `sub_000155A0`, `sub_000155E0` and
`sub_00015600` callbacks with matching stack conventions. This adds signed
comparison branches and repeated real call/return paths. An exploratory fixture
with absent callbacks crashed in the original routine after its stack fallback
and caller cleanup disagreed; those invalid callback-contract combinations are
not normal equivalence vectors. Original absent/non-code fallback remains
covered by `sub_001FBAE0` and the separate graph/route checks.

The complete prepared tree must still pass source checking before bindings,
all separate object audits, trusted linking and confined execution. The optional
maintainer `--reference-only` diagnostic explicitly omits those checks and
cannot establish admission or production readiness. New module/template hashes
are bound into both golden provenance and whole-kit inventory.

Other lookup shapes and shared/state graphs outside the reviewed proofs remain
rejected. Complete remaining
native functions, named entry dependencies/_gen bodies, manual/kernel services, loader and
verified renderer integration before enabling Burnout or qualifying gameplay.

## Alternate guards and calls at a block tail

A second exact four-statement template pins the inverted guard layout in
`recomp_0000_001.rs` / `sub_000146E0`. It has the same typed target/saved-stack,
ESP/EAX alias, lookup-order and unshadowed-global requirements as the original.
No general boolean, pointer-call or control-flow simplification is accepted.

Nine statements at the exact tail of their current labeled block can reuse the
original complete-region proof: three trace statements plus the six lookup/
fallback statements are isolated under that same label for checking. A fallback
break and normal fallthrough both leave that exact block. Ancestor targets,
loop labels, unlabeled nested blocks and trailing effects reject the rule. Block
labels are removed only when no break/continue references remain after lowering.

Normalization now rejects every uncaptured label and any uncaptured identifier
that already has a canonical replacement spelling. This prevents an unrelated
jump or value from being mistaken for a captured name. The compacted-region
proof uses the same capture check. Dedicated adversarial cases cover these
collisions, scope changes, mutated conditions, ordering and fallback fields.

Controlled native comparisons add 7,680 guarded-layout and 7,680 tail-layout
cases per target, including observable effects after the block. They remain
separate from original-function semantic qualification. The single-unit golden
now retains original `sub_00015570`, checked in 128 register/whole-RAM/trace/log
cases with null/non-null objects, original registered callees, absent/non-code
targets and code-range guards. Prepared runs cover 37 original functions and
4,736 vectors in bounded batches, as documented below.

The single-unit component now contains runtime calls too, so `check.py` always
requires `--call-targets` from the hash-verified classifier tool. A sparse fake
target table is not an alternative. All game sources still pass checks before
bindings, and every optimized game object is audited before trusted linking.
The guarded template hash is recorded in golden and inventory provenance.
The installed app SDK/API is unchanged by these maintainer/port changes.

## Guest thread-local memory and private context v4

`Registers::fs_base: u32` is an ordinary guest TIB/PCR address. The reviewed
`gamepkg_tls_g_fs_base` accessor lowers to that field; the existing proven-u32
address rule and exact relocation pattern still route every access through
`Memory` and GamePKG-owned RAM IDs. No host FS register, native thread pointer,
new opcode, permission or SDK bypass is exposed. Record decoding accepts guest
address bits, not memory authority; page/span rights are checked on each access.

The private register record is v4, 244 bytes LE: the old 240-byte prefix is
unchanged, followed by `fs_base` at 240..244. FP control remains at 46..48.
Wrong record lengths and invalid FP stack positions reject before mutation.
Four-target tests round-trip 65,536 FS patterns, including high address bits.
The registers source hash is bound into golden provenance and whole-kit reports.
This is not a public save format or a service-protocol ABI change.

Prepared comparisons add 128 unchanged original `sub_002447BF` epilogue cases,
including unaligned guest TIB addresses and aliases within the checked test RAM.
Source-checked game-side cases exercise that actual migrated function with
wrapping/out-of-range addresses and a read-only view; denied writes preserve
bytes. Native references additionally verify revocation leaves context intact.
These data checks do not initialize the real title's thread structures or prove
startup, physical input, audio, graphics or frame pacing.

Private vectors now use TKN9: magic, LE u32 batch count (1..16), then for each
batch a LE u32 case count (0..4096) and its existing records using 244-byte
contexts. Scalar trailers follow all batches; the reader requires exact EOF.
RAM, registers and the trace ledger persist across batches. Prepared runs now
execute 4,736 cases in two batches, preserving all earlier coverage and the
4,096-case limit per batch. No production resource limit was changed. The same
audited Linux child rejects old magic, zero/oversized batch counts, oversized
later batches and truncated records, with bounded waits and dummy output.

Both direct startup helpers `sub_001D18B0` and `sub_001D19FD` are now retained.
The entrypoint is now explicitly inventoried, and `sub_001D2E6F` remains blocked
by `sub_001CFF88`. That routine now lowers locally, but its default branch needs
`sub_001D3970 -> sub_00243816 -> sub_00246622`. The loop callback helper
`sub_0024490A` and the memory-fill helper `sub_001D5707` are now retained. The
formatter's remaining dependency chain goes through `sub_002440A5`,
`sub_002486AE`, `sub_00246EE4` and `sub_0024B275` to `sub_001D2955`'s
unmigrated `recomp_debug_service`. None of those bodies are
replaced by stubs. Complete the remaining functions and service/renderer/loader
integration before enabling a title; retained literal-call closure is not full
runtime routing or gameplay qualification.

## Calls with an explicit common exit

An additional exact rule handles nine trace/lookup/fallback statements followed
immediately by an unconditional labeled `break` at the end of the same block.
All fallback exits must name that same destination. The original full-region
AST, fixed-width target/stack, register-alias and capture checks still apply.
Comparison copies normalize only optional trailing commas in generic argument
lists; argument types, count and order remain exact. This accepts the equivalent
spelling in the original startup routine without accepting changed reads/calls.
Only the call machinery becomes a checked direct helper; the explicit break
remains in its original lexical scope. This preserves skipped caller effects
when leaving an ancestor block or loop. The earlier nine-statement rule still
rejects ancestor and loop exits where no explicit common jump exists.

Negative cases reject mismatched fallback destinations, unlabeled/value breaks,
trailing statements and changed guards. Native controlled-handler comparisons
add 7,680 cases per target, including the inner skip path, observable intervening
effects and post-exit effects. They run on Linux32/64 and Windows32/64 under
Wine; these are synthetic control-flow checks, not complete original-function
or physical gameplay qualification. No host API, runtime permission or legacy
admission gate changes.

The common-exit checkpoint retained 95 additional functions. All 18,556 previously
retained function/dependency records, including their lowered-body hashes, are
unchanged. This does not expand original-function semantic qualification beyond
the 37 functions/4,736 vectors recorded above.

## Untraced computed jumps

`calls_runtime::invoke_jump(&mut Registers, &mut Memory, target: u32) ->
Result<(), i32>` is a separate port-private contract. It checks the live private
ledger without recording a trace entry or applying ordinary call code bounds.
Original table absence emits the existing missing-target diagnostic, adds four
to guest ESP with u32 wrapping and clears EAX. Logging/ledger failures propagate
before that fallback mutates registers. Registered generated targets use the
finite direct router; known unmigrated manual/kernel/entry/soundtrack targets
remain Unsupported rather than becoming absence. No pointer or instruction
interpreter is introduced, and the GamePKG service ABI/SDK is unchanged.

The maintainer pass pins the formatter's entry tail and six lookup/fallback arms
as one complete AST region. It verifies typed private selector ownership, exact
transitions and that no outside edge enters removed states. Caller effects
before the region and the common continuation remain in place. Reconstruction
uses a separate full lookup template, then the normal u32 operand, register-
alias, name-capture and unshadowed-function checks. Only the exact now-unused
function-pointer declaration can be removed. Altered lookup order, stack pops,
branches, aliases, incoming edges, observer/borrow uses and changed markers
remain rejected. Both new template hashes participate in provenance.

Controlled native comparisons exercise 7,680 original/reconstructed untraced
cases per target, including manual precedence, generated/kernel handlers,
absence, wraparound and unchanged seeded traces across code-bound settings.
These handlers are maintainer fixtures, not migrated kernel implementations.
The source-checked `jump_checks.rs` additionally invokes real generated
`sub_00015590` at five stack boundaries outside ordinary call bounds, checks
absence diagnostics/stack wrapping and verifies revocation before any callee or
fallback runs. That fixture receives no source/object exemption and runs in the
admitted Linux children. This is component evidence, not startup, ISO loading,
full formatting behavior, physical devices, audible audio or frame pacing.

The untraced-jump checkpoint added 207 retained functions with all 18,651 prior
function/dependency records and lowered-body hashes unchanged. The current
literal-call closure contains 19,295 functions; semantic reference qualification
is 37 functions and 4,736 vectors. The named entrypoint is now inventoried but
blocked by dependencies; five `_gen` helpers still need inclusion. Known missing
runtime handlers still fail Unsupported.

## Callback tails in the current loop

A separate context distinguishes labeled blocks from labeled `loop` bodies.
Only nine statements at the exact end of the current loop can use the new
rule. Both fallback paths must continue that same loop. On comparison copies,
those two continues become exits from an isolated block and reuse the complete
existing trace/lookup/fallback proof. Normal completion and both fallbacks then
reach the next iteration. Breaks, ancestor targets, nested blocks, `while`
contexts, trailing effects and changed guards remain rejected. Loop partitioning
and all source, object, confinement and resource limits remain unchanged.
Controlled original/reconstructed checks cover 7,680 three-iteration cases per
target with observable per-iteration and post-loop effects.

Prepared comparisons additionally run 512 unchanged original `sub_0024490A`
cases with its actual `sub_0024499E` dependency. Bounded chains cover 0..5 table
entries, skip flags, early stop indices, registered callbacks, original absence,
non-code bounds, guest thread-head restoration, unaligned records and history
writes. Registers, traces, diagnostic fields/order and the whole committed
8 MiB match. A separate assertion verifies that no observable original write
escapes the eight recorded spans; the converted RAM image is compared in full.
This does not qualify arbitrary exception handlers or complete startup.

Private vectors now use **TKN9**, preserving v4/244-byte contexts and the 1..16
batches of at most 4,096 cases. Each case's former seven input/output RAM spans
now includes an eighth 64-byte history span at guest `0x3C6100`. Old TKN8 is
rejected before payload interpretation. The added span uses a forked RNG, and
new loop cases follow all prior cases. Current prepared coverage is 37 selected
original functions and 4,736 vectors in two batches. The same audited child
checks old magic, malformed counts, truncated records and exact EOF.

The inventory now includes the exact `xbe_entry_point` from pinned unit
`recomp_0004_013.rs`, at original address `0x001D2807`. Unknown names and wrong
owner units are rejected; duplicate numerical route addresses fail before
emission. Numeric partition order and static direct routing are tested with a
named entry between ordinary address ranges and a call across partitions.
`dispatch::entry_available()` describes literal closed-graph presence only; it
is false for the current kit because the remaining dependency reaches
`recomp_debug_service` in `sub_001D2955`. Existing runtime entrypoint migration faults and the
legacy profile gate remain in force. The five `_gen` helpers still need review
and inclusion. All 18,858 previous lowered-body/dependency records are unchanged;
the loop-entry checkpoint retained six additional functions without enabling Burnout.

## Host-owned memory fill

`Memory::fill(address: u32, value: u8, length: u32) -> Result<(), i32>` lowers
only the reviewed, unused-result native memset shape. The destination must be
an exact mutable c_void guest relocation with a proven u32 guest address. Value
must have the original u8-to-c_int casts; the size_t argument must contain a
proven u32 count. Changed pointer types/relocation, wider values/counts, result
uses, shadowed memset/relocation names and nested attributes reject conversion.
The surrounding native branch, register increments and reverse-direction loop
remain intact. No general pointer, libc-call or system-import exception is added.

A nonempty fill resolves the full logical writable span, then calls the host's
`local_memory_fill(window, offset, bytes, value)` (local op 8). That exact 32-byte
LE request carries four u64 words; value must be 0..255, length at most 64 MiB,
and the reply is empty. The trusted child runtime validates every committed
writable page before changing anything under the same lock as protect/decommit/
destruction. No pointer escapes and no payload-sized temporary is allocated.
Ordinary transfer limits and reserved-memory charges are unchanged. Port-level
zero length is an inert success, matching existing empty byte/copy adapters;
nonempty invalid, unmapped, read-only and revoked access fails explicitly.

`fill_checks.rs` is ordinary checked game source, including twenty value/length
combinations, low-byte truncation, unaligned and >1 MiB fills, logical read-only
views, complete-page validation, overflow, caps and unchanged bytes on failure.
Native references also check post-shutdown denial. The app additionally tests
malformed wire requests/values, maximum-size fills, scoped IDs, revocation and
concurrent page-protection races on Linux32/64 and Windows32/64 under Wine.
The actual admitted Linux children run the same game-side checks; this is not
physical gameplay, graphics, audible audio or measured pacing evidence.

`sub_001D5707` now lowers and is retained with its actual dependencies. This
step adds 216 retained functions while preserving all 18,864 previous lowered
function/dependency records. The named entrypoint remains dependency-blocked by
`recomp_debug_service` in `sub_001D2955`; all other native/runtime/media gates
remain unchanged. Completing a fill adapter does not qualify the full original
startup helper, debugger contract, renderer or title.

## Bounded debug-service migration

The original `recomp_debug_service(service, argument)` behavior stays in
ordinary checked Rust `debug.rs`; the generic host stores bounded diagnostic
bytes. Only unused-result calls with two proven u32 arguments, an exact plain
callee, no shadowed names and no nested attributes lower. Wrong widths, generic
arguments, altered arity and returned-result use reject conversion.

Service 1 reads the original counted ANSI descriptor through checked RAM.
MaximumLength is ignored, as in the pinned implementation. Output stops at the
first NUL or Length, but newline insertion independently examines the last
counted byte. Null descriptors, null buffers and zero lengths preserve their
defined quiet behavior. Unknown service numbers preserve the original unsigned
decimal/uppercase-hex ignored notice and read no RAM. Required read failures
propagate before any partial message is recorded. The adapter exposes no host
pointer, format string or native console; its fixed-length 65,544-byte buffer
needs no growable per-byte allocation helper or expanded object-import rule.

GamePKG op 64 takes LE level/code/byte-count u32 fields followed by at most
128 KiB exact bytes; the reply is an 8-byte sequence. Byte and numeric events
share 128 records and a 1 MiB payload ring. Human summaries escape controls and
non-UTF-8 bytes and display at most 256 input bytes per record. Host drains retain
exact complete payloads; eviction is explicit. The player charges a separate
2 MiB diagnostic reservation. Unsupported, denial and transport failures
propagate; direct stdout/stderr is never a fallback.

The maintainer reference verifies both the whole pinned C file and the exact
helper body hashes before inserting that unchanged body into an inert stdio
capture template. This C is oracle-only and never admitted or linked into game
or trusted runtime objects. Seven adapter requests cover ignored services, NUL
precision, independent newline decisions, raw non-UTF-8/control bytes and the
maximum counted string. Quiet, bounded-view, read-only and denied-tail cases
also execute as ordinary checked source in the confined Linux children. Private
GPB1 diagnostic fixtures have bounded fields and require exact EOF.

Prepared native comparisons now add 128 unchanged original `sub_001D2955` cases,
including empty/unaligned strings, literal percent characters, non-UTF-8 and
newline endings. Total reference coverage is 38 original functions and 4,864
vectors per target, preserving all earlier case inputs and the TKN9/v4 record
format. Linux32/64 and Windows32/64-under-Wine compare registers, full 8 MiB RAM,
trace data and exact diagnostic bytes. The source/object admission and runtime
checks are separate from these reference comparisons. None establishes physical
controls, audible output, gameplay, ISO boot or frame pacing.

The bounded-debug checkpoint retains 215 additional functions, including the
named entrypoint, with all 19,080 prior body/dependency records unchanged.
Literal closure now contains 19,295 functions. Runtime entry/manual/kernel/
soundtrack migration faults and the legacy admission gate remain enforced.

The next data-path component is [the checked in-place ISO reader](../service-disc/README.md).
It uses the same SDK without new host APIs or privileges. Actual confined
Linux32/64 reads match all 716 files of the supplied image after independent
pinned manifest authentication, and grant revocation denies subsequent reads.
This supplies the reader for future XBE loading; it does not yet load the
original sections, initialize the kernel or execute the startup entrypoint.

The [checked XBE image loader](../service-image/README.md) now supplies the
next data step using the same shared SDK. Actual Linux32/64 confined loads
match the original section loop across all 64 MiB by SHA-256, including the
supported private image's 17 sections. It remains separate from stack/FS/TLS,
heap/kernel initialization, import bridging and native entrypoint execution;
no title admission or gameplay claim follows from it.

The [checked CPU startup data component](../service-bootstrap/README.md) now
initializes the toolkit region, stack and main-thread FS/TLS through the same
SDK. Confined Linux32/64 checks compare all RAM, layout and initial registers
with the pinned original on synthetic cases and the authenticated private ISO.
Heap allocation, kernel exports, memory/device models and native entry remain
unqualified; the component does not enable the legacy profile.

## Explicit cross-span scalar/data transfers

The memory adapter now uses GamePKG's reviewed gather/scatter services when a
single read/write crosses adjacent explicit guest spans. Single-span accesses
retain their original path. The host service implementation is app commit
`c775eca`; shared port-service identity is
`86521a787ece4bd01a1164790b21294e3f1b696641412fc97cceb186d30aeca4`.
Older SDKs may return Unsupported and must not fall back to direct memory access.

The adapter resolves the complete guest range before calling the host. It
rejects 32-bit overflow, holes and port-level read-only spans before any access;
it never masks arbitrary addresses into RAM. Only explicitly attached spans
are traversed, irrespective of attachment order. The existing 32-span maximum
is retained. The host independently checks every opaque window, backing range
and committed page permission under one mapping/protection lock. A late denial
cannot leave half a scalar write or a partially filled read result. Segments
name backing offsets, never host addresses. The actual full-title mirror,
contiguous memory and device models remain separate required integration work.

Reads remain bounded to 1 MiB. Writes retain the single-span 1,048,560-byte cap;
a cross-span write must also fit its descriptor header plus payload inside the
same 1 MiB local-request limit (`4 + 24 * segment_count` header bytes). No
threshold, memory permission, import audit or game-profile gate is relaxed.
Fill and nonoverlapping bulk-copy still require one explicit containing span;
they are not silently split with different overlap or partial-failure behavior.

The admitted copy checks now exercise an unaligned little-endian scalar across
a reversed-order alias pair, backing-byte agreement, a late guest hole, a late
read-only span, top-of-address-space overflow, 32 adjacent one-byte mappings and
the unchanged 33rd-span rejection. Test scratch bytes are restored afterward.
These checks run through the same source/admission/object pipeline as the other
native component vectors. They do not execute a game entrypoint or qualify
hardware controls, audio, rendering or frame pacing.

The cross-span checkpoint passed the full generated candidate's complete-source
scan and separately optimized game-object audits/trusted linking on all four
native targets, with the same 12 GiB compiler-only plan and four codegen units.
No runtime memory ceiling or source/object policy was increased. Actual confined
Linux32/64 children replay all 4,864 native vectors, the explicit mapping checks,
malformed-vector rejections and original-target lookups. Native references also
pass Linux32/64 and Windows32/64 under Wine. Windows confined service execution
and actual gameplay remain unqualified. The checked XBE loader and CPU startup
matrices were rerun successfully against this adapter, including the private
ISO in place and every RAM hash; their image/startup behavior is unchanged.

The [checked heap data component](../service-heap/README.md) now supplies the
next owned data component after stack/TLS initialization. Its state and whole
RAM match the pinned original helpers. The shared-task serializer and four
pool kernel bridges are qualified separately below; complete public kernel
wrappers and title startup remain required before entrypoint admission.

The [checked pool kernel bridge integration](../service-kernel-pool/README.md)
now routes nine configured allocator ordinals through the actual finite native call/jump
path and shared per-caller heap clients. Other imports remain explicit required
migration work; installing that bounded map does not enable the title.


The optional three-bank host preset is now available in app implementation
[`f4bdfea`](https://github.com/centrexity/GamePKG/commit/f4bdfea):
`LocalLayout::TakedawnMemory` enumerates independent 64 MiB main RAM, 64 MiB
contiguous RAM and 1 MiB flash backing, initially inaccessible. Full SDK digest
is `c36219e59748d51107cbc5be62593dbcc119cd58b4d9763a09c3d959b3bb73e8`;
composite native recipe identity is `e5624d54d141c27b`. Current one-bank component
fixtures retain `TakedawnRam` and its 16-byte enumeration response. A future
three-bank bootstrap must validate the 48-byte ID/length response and explicitly
select its backing before committing pages. This does not silently migrate any
existing fixture, resolver or launcher.

The app qualifies all four build targets and actual confined Linux32/64 bank
isolation/protection/copy/decommit behavior. Standalone native Linux32/64 and
Windows32/64-under-Wine runtime tests also exercise all-bank charging, collision
rejection and teardown/reuse. None establishes complete Xbox guest aliasing or
device behavior. Port code must still supply exact guest views (including the
contiguous/tiled aliases), flash initialization and device behavior. The checked
contiguous allocator and shared owner are described below. Keep the 32-span ceiling, reject
unmapped MMIO and do not turn NV2A/MCPX registers into zero-filled successful RAM
fallbacks. The complete native entry/title gate remains closed. See the app's
[preset contract](https://github.com/centrexity/GamePKG/blob/f4bdfea/docs/PORT_SERVICES.md#child-local-runtime-separate-abi-namespace).

The [checked contiguous allocator data](../service-contiguous/README.md) now
preserves the original page/range/reserve/claim/free/query behavior over the
reviewed second RAM bank, with original-code and confined replay qualification.
It now joins Heap in the existing shared owner through the explicit private
version-2 protocol; constructing independent allocators per guest thread is
incorrect. The allocator bridge now connects five contiguous/combined wrappers
in addition to four pool wrappers, including all required stack reads, fixed-range
clears, instance padding and the first eight refusal dumps. Its focused fixture
compares 2,115 pinned-original calls per target, audits every game object on four
targets and runs concurrent/error controls in confined Linux32/64. The full native
candidate matrix retains its four-pool routing fixture and 38-function/4,864-vector
regressions. Neither establishes complete guest memory/device views or native entry.


The [retail guest layout](../service-memory/README.md) now constructs all 32
reviewed guest views over those three opaque banks: main RAM and 28 mirrors,
Contiguous with its tiled alias, and independent zeroed flash. No MMIO or masked
fallback is added. CPU-startup and nine-wrapper allocator fixtures stage and use
this helper as ordinary checked game source; workers preserve their shared
context and independently acquire clients. Its own native/confinement matrix
checks alias geometry, protection/decommit, cross-mirror transfer atomicity and
holes. This qualifies the retail RAM topology, not NV2A/MCPX device behavior,
complete guest-thread/kernel startup or native title entry. See its contract for
page commitment, root-versus-RAM lifetimes and precise proof limits.


The [checked kernel data component](../service-kernel-data/README.md) now
preserves all 24 data-export addresses and original initialization memory effects,
including command-line launch allocation and optional console-key/certificate
signing. Four-target original-code comparisons include 24 cases, every main-RAM
byte, all 400 ordinal probes and 1,024 fixed certificate-signature vectors.
Complete-source/object audits pass on all four targets and exact original-data
witnesses plus ownership/failure controls run in confined Linux32/64. The module
uses existing checked RAM and the shared heap owner; it introduces no host API,
crate, direct environment/file/clock access or accepted native import.

The nine-function map does not yet accept those data exports. Connect initialized
data to full import/remap/lookbehind preparation and periodic host-time updates,
then finish other kernel functions/thread/device and generated/media/renderer
paths before title entry. Key/EEPROM configuration parsing and coordinator status
reporting remain separate port work. This component does not enable title
admission, and its synthetic witnesses do not establish actual gameplay.


For the next complete import resolver, the app now supplies child-local op 17 /
`local_memory_page_states(window, offset, pages)` in host implementation
[`8e741b5`](https://github.com/centrexity/GamePKG/commit/8e741b5). The current
shared digest is
`22be2884a8921bfbcacb3b8f3f1fe8da111840f0bb3b600269a21d36221cc0be`, composite
native recipe `d8f147d4eed6560b`. It snapshots 1..256 aligned pages of an owned
opaque RAM window: 0 uncommitted, 1 committed/no-access, 2 read-only, 3 read/write.
These are state bytes, not the existing protect modes (0 no-access, 1 RO, 2 RW).
The exact 24-byte request is window u64, offset u64, page count u32,
reserved-zero u32. No memory contents, rights or native addresses are returned.
The copy is atomic with protection updates, not a lease for later operations;
setup must serialize writers and handle every subsequent failure. Unsupported
from an older SDK must not become guessed protection or direct OS access.

This fills the missing generic prerequisite for the original temporary import
writeability and restoration. The port resolver itself is still pending. Preserve
the pinned original's asymmetric behavior: remap and data classification apply
to the main run, while the 16-word lookbehind uses raw ordinals and function
routing, at synthetic slot `main_count + distance` (leaving slot `main_count`
empty). Unsupported imports and capacity overflow must not be concealed by
truncation or successful stubs. The original zero-count setter retains its
previous/default count, so it cannot be treated as an empty title table without
review. Derive the actual run from the authenticated image, preserve ordinary
non-marker bytes, and verify the original full setup before enabling entry.

The migrated kernel-data component was requalified against this new SDK:
24 pinned-original initializations / all 400 ordinal probes / full 64 MiB RAM
and 1,024 certificate-signature vectors per native Linux32/64 and Windows32/64
under Wine, plus complete source/object audits on all four targets and actual
confined Linux32/64 witness/ownership controls. The app separately qualifies
query framing, isolation, all states, maximum count, revocation, nonmutation and
concurrent protection snapshots without raising any quota. These remain silent
infrastructure checks, not complete title startup, Windows AppContainer runtime,
physical controls, audible output or measured frame pacing. Legacy title gates
and the app's pinned submodule remain unchanged.


The [complete import preparation component](../service-kernel-imports/README.md)
now connects initialized data and supported function imports, exact remapping,
raw 16-word lookbehind, full count/capacity checks and per-page protection
restoration. It publishes strict KPL3 metadata understood by the existing finite
router, retaining KPL2 compatibility and every quota. An image adapter binds
setup to the loaded executable bounds and its derived kernel-data region.
Four-target original-code comparisons/source-object audits and actual confined
Linux32/64 checks qualify the component, not all title imports. The private
image currently still rejects on 121 unmigrated function exports; no title
entry has run. See its contract for explicit per-page restoration semantics,
partial-publication failure/abort requirements and exact remaining work.


SHA kernel exports 335/336/337 now use checked guest RAM and the original
92-byte context/counter/buffer semantics; see `../service-kernel-sha/README.md`.
They participate in complete import preparation and the actual finite Call
router. The private main-table inventory is down to 118 unmigrated functions;
this does not enable the title or qualify gameplay. Shared SDK and all
admission/confinement limits remain unchanged.

RC4/HMAC exports 338/339/340 are checked game Rust as well, with exact context
alias ordering and explicit caller-owned SDK clearing. Current app requirement:
`2c646c0ec78cb39b1251ccd7ba8662da5fc62ffa`, recipe `867d8027dd05fc8c`.
See `../service-kernel-crypto/README.md`; 115 main functions and the remaining
runtime/device/generated/manual/media/renderer/native-entry path still need
migration. No title profile or enforcement exception is enabled.

The next timing/settings migration has a coordinated host clock prerequisite:
see `CLOCK_SERVICE_HANDOFF.md` for app `da4864d`, SDK `969defa5bb1d6421`, exact raw
counter/calendar units, worker-only routing and the remaining shared root/skew/
settings integration. This service addition does not register new kernel calls;
the private main-function gap remains 115 and native gameplay stays gated.


The shared root services/NV extension adds original export 24 with configured
video/audio flags and one shared offset. See `../service-kernel-settings/README.md`
for the exact 288-byte private root codec, 64-byte record, unchanged four-ID
context, readiness/lifetime/error contracts and C/confinement controls. CPU
records stay 244 bytes and all quotas stay intact. The private main gap is now
114; time exports and full gameplay remain gated.

## Interrupt register preservation: private CPU record v5

The next gate/IRQ integration requires the original `kernel_hal.c` register
snapshot to survive callbacks. `registers.rs` now includes the previously absent
`eflags: u32` (initial 0x202), `fp_cmp: i32` (initial 0), and eight MMX bit values
`mm0` through `mm7`, each `[u32;2]`, initially zero. This is explicit native
function-call state; it does not fetch/decode guest instructions or grant native
pointers. MMX operation lowering is still rejected until individually qualified.
Only exact pinned TLS accessors for scalar `eflags` and `fp_cmp` are additionally
converted to owned Rust fields. Forged names, arguments, qualified accessors,
alias bindings and MMX accessor calls remain rejected.

CPU record **v5 is exactly 316 bytes**, using explicit little-endian fixed-width
fields. Bytes 0..243 preserve the v4 layout, including guest `fs_base` at 240.
New offsets: eflags u32 at 244, fp_cmp i32 at 248, then mm0..mm7 starting at
252, eight bytes each (low u32 first). No NaN, signed comparison, flag or MMX bits
are normalized. The existing fp_top range 0..7 remains validated. Decode rejects
old 244-byte records, short/trailing input and invalid fp_top before mutation.
All host resource quotas, root288, KSV3 and four-ID contexts are unchanged.

`Registers::irq_prepare(stack_top:u32)` applies the original IRQ worker prefix:
set esp to the supplied guest stack and zero eax/ecx/edx/ebx/esi/edi. It leaves
other state unchanged. `irq_restore(&mut self, saved:&[u8]) -> Result<(),i32>`
restores all recorded arithmetic state but retains the **current** holder guest
fs_base, matching the original IRQ save/restore functions, which omit that
field. Normal decode still restores fs_base. Malformed restoration is atomic.
These primitives do not allocate a worker stack, publish PCR.Irql, acquire a
dispatch gate, deliver an interrupt or execute a callback.

Private native vectors advance from **TKN9 to TKNA** to bind both the 316-byte
context and an appended captured RAM span at 0x36c0e0. Old markers reject before
replay. Generator provenance records v5/316 and the exact register source hash;
whole-kit admission checks all three before staging source. Every game object
still undergoes separate optimization and import auditing before trusted linking.
The source-kit archive remains unchanged; checked generated candidate identities
must be regenerated, never patched to reuse an earlier receipt.

`check_irq_state.py --wine` binds the archive, original kernel_hal.c file and
exact bytes for its IRQ state type/save/restore and stack-preparation prefix.
It compares 4096 full-state transitions per target with original C, covering
arbitrary flags, comparison values, MMX bits, floating NaN payloads, clobbered
callback state and deliberately changed holder FS. A corrupt-holder-FS copy must
fail the original-C test. Codec tests cover 65536 arbitrary records and atomic
rejection. `irq_state_checks.rs` supplies dependency-free preservation controls
for actual checked/confined game fixtures, without reference hooks.

The full native harness also adds the unchanged original `sub_002490E1` from
reviewed unit recomp_0007_003.rs: 1024 floating compare vectors spanning negative,
equal, positive and unordered results, signed zero, subnormals, infinities and
NaNs, all fp_top values, full CPU/RAM effects and retained-state sentinels. Its
extra input seed and new RAM span are forked so the previous 38 functions retain
their original inputs; the new function is tested last. The remaining production
gate/IRQL/IRQ callbacks and timer integration still need to consume these helpers.
Do not treat register-state qualification as full interrupt delivery or gameplay.

The isolated state component can be checked without rebuilding the whole candidate:
`python3 check_irq_state.py --wine --app /path/to/app --app-lib /path/to/libgamepkg.rlib`.
This also scans both game-source modules, audits all emitted optimized objects
on four targets, rejects an unused maintainer test, and runs 4096 state controls
in actual confined Linux32/64 children. It does not substitute for the complete
candidate's build or gameplay. The fixture uses a fixed-size buffer, avoiding
the unapproved allocation-growth import without changing any audit allowlist.

The v5 checkpoint passes the complete four-target build/audit matrix and actual
confined Linux x64/i686 execution: 39 original functions, 5888 full CPU/RAM
vectors, scalar/vector/call/context/protection/revocation controls and malformed
old-format rejection. Source/dependency checks precede binding injection and all
eight optimized objects pass before trusted linking. Linux compiler scheduling
uses the app's private jobserver under the unchanged 12 GiB limit; four/eight
unrestricted partitions had exceeded it. A newly emitted allocation helper was
removed by allocating the fixture buffer at fixed size; the allowlist did not
change. The 2453-file candidate closes 19420 definitions, leaving 4596, with 108
main imports still unresolved. Windows execution here is Wine component work;
full Windows builds are compile/object/link qualification, not gameplay.

`check_irq_state.py --wine --app /path/to/app --app-lib /path/to/libgamepkg.rlib`
also runs the isolated state source/object audit on four targets and 4096
preservation controls in actual confined Linux32/64. It rejects its unused
maintainer harness. Neither suite enables production Burnout.

## Indexed guest byte copy in system-thread startup

The converter now accepts a byte assignment between two tracked guest
temporaries indexed by the original unsigned 32-bit count cast to isize.
The generated code reads the source before calculating and writing the
destination. The original loop and its direction stay in the generated
function, so an overlapping forward loop still observes its earlier writes.
The existing nonoverlapping bulk-copy route is unchanged.

The address adapter rejects null scratch values, offsets wider than positive
i686 isize, and dereferences of one-past guest addresses. Ordinary Memory
checks still enforce permissions and alias mapping for each byte. Standalone
pointer loads, reference creation, arbitrary pointer offsets, and compound
writes through a pointer remain outside this lowering.

This lowering is scoped to the pinned system-thread wrapper sub_001D1818,
which uses this assignment alongside two already-supported bulk copies.
Extending it to other routines needs a separate check of the newly reachable
source.

## Thread-start interrupt lowering

For sub_001D3F2F in the pinned kit, the converter matches all seven source
statements corresponding to the CLI, memory patch, STI, saved original ESP,
and complete PUSH EBX. It verifies the register bindings and generated
successor, then emits one call to the checked task-bound IRQ adapter and
continues at the original next state. A changed address, width, register,
statement order or successor does not lower. The original three traced
calls and the direct dependency on sub_001D3ECB remain.

The closed graph still excludes sub_001D3F2F through sub_001D6AAC,
which has separate guest SGDT and far-reload behavior. The start callback's
other branches through sub_00156400 and sub_001D2EE5 still need MMX, guest
pointer and finite call lowering. Startup still needs to bind the live
worker and IRQ queue before dispatch. The port remains gated.
