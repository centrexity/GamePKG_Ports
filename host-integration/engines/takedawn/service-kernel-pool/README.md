# Checked allocator kernel bridges

This component connects nine original allocator operations to the existing
[shared heap task](../service-heap/SERVICE.md) through the actual finite native
call/jump router. Game functions remain compiled Rust functions; no instruction
fetch/decode loop, native handler pointer, OS heap or system call is exposed.
The contiguous paths use `LocalLayout::TakedawnMemory`, available in app
implementation `f4bdfea`: shared-service identity
`c36219e59748d51107cbc5be62593dbcc119cd58b4d9763a09c3d959b3bb73e8`,
composite native recipe `e5624d54d141c27b`. No new host API or quota is added.

The original dispatch table, not its stale comments, establishes these ordinals:

| Canonical ordinal | Original bridge | Arguments after dummy return |
| --- | --- | --- |
| 14 | ExAllocatePool | size u32 |
| 15 | ExAllocatePoolWithTag | size, tag u32 |
| 17 | ExFreePool | address u32 |
| 23 | ExQueryPoolBlockSize | address u32 |
| 165 | MmAllocateContiguousMemory | size u32 |
| 166 | MmAllocateContiguousMemoryEx | size, low, high, alignment, protection u32 |
| 168 | MmClaimGpuInstanceMemory | bytes, padding-address u32 |
| 171 | MmFreeContiguousMemory | address u32 |
| 180 | MmQueryAllocationSize | address u32 |

Both allocation calls use the original 16-byte alignment. The tagged call reads
both arguments even when logging is disabled. Free returns EAX zero; query
returns remaining bytes in a live block, preserving interior-pointer behavior.
Ordinary allocation exhaustion returns EAX zero with normal stack cleanup.

Ordinal 165 allocates with alignment 4096 and the full physical range. Ordinal
166 reads all five arguments, including the otherwise unused protection word.
Its fixed-range branch preserves the original u32 wrapping test and address
calculation, then performs the shared owner's exact clear-and-claim operation.
Other ranges use the original contiguous allocator with alignment at least 4096;
reversed bounds return zero. Invalid fixed ranges fail through checked RAM rather
than falling back to arbitrary addresses. Ordinal 168 writes zero to a nonzero
padding address and computes `0x84000000 - bytes` with u32 wrapping; MAX bytes
means 128 KiB. The original wrapper does not allocate or claim that range.
Ordinal 171 frees Contiguous first and otherwise Heap, returning EAX zero.
Ordinal 180 queries Contiguous first and falls back to Heap only for zero.

Any table containing these five ordinals requires a configured shared Contiguous
owner before publication. Kernel data exports and all other ordinals still fail
Unsupported. No other import is replaced by a successful stub.

## Context and ownership

The port's `Memory` now owns an optional exclusive `HeapClient`, shared endpoint
IDs and one opaque kernel-map record ID. `heap_install(&mut Option<HeapClient>)`
accepts the initialized primary client only when its root call-record association
matches this RAM context. Success moves ownership out of the caller's Option;
failure preserves caller ownership for cleanup. Existing bindings cannot be
overwritten by that operation.

`call_view()` shares call-record/kernel-map/endpoint IDs but starts with no
client and a fresh per-caller kernel-caller value. `thread_context() -> [u64; 4]`
returns `[call_record, kernel_map, heap_inbox, heap_job]`; `with_context` validates
the live record IDs and paired endpoints and rejects an already-finished heap
task. Kernel-map contents/association are checked on invocation. No response
queue, sequence, native pointer or native layout crosses this setup message.
Each caller lazily registers its own reply queue at its first heap operation.
Guest-thread startup must adopt this context once per caller, then attach its
reviewed guest views. Creating a separate Heap per guest thread remains invalid.

`heap_take()` moves out the caller's exclusive client (or lazily constructs one);
`heap_put()` restores it after the operation, including failure/poison state.
They are internal port ownership operations, not new host APIs. The caller must
pair them and must not replace an outstanding client. `heap_close()` unregisters
a worker client or stops the primary owner, then revokes its response queue.
`kernel_clear()` explicitly revokes the shared map. Only the coordinator should
clear that shared map at shutdown; worker views borrow its ID. `kernel_caller()`
holds the original dispatcher caller value in each RAM/caller context, replacing
its original TLS global. These value/context IDs do not grant additional host
permissions; GamePKG enforces its runtime/page/grant checks independently.


Contended call-record updates keep their existing 64-attempt bound. A stale CAS
commits nothing; before retrying, the caller uses local ops 20/23/25 to create an
unsignaled event, wait for a 1 ms timeout, and release it. Release occurs even
when waiting fails. This avoids consuming the retry bound in a tight loop
under contention. Quota, runtime-closure and release errors still propagate;
there is no retry after a heap mutation is accepted. No host API or quota change
is needed. These waits stay in game worker threads, away from the window owner.

## Import preparation and invocation

`kernel_prepare(&mut Memory, table: u32, count: u32) -> Result<(), i32>` requires
live RAM, an unconfigured context, an aligned table at/above `0x10000`, 1..378
slots and a complete extent inside 64 MiB. The caller must derive the complete
import range from the reviewed authenticated image; it must not truncate a real
title's table to conceal unsupported imports.

At this checkpoint **the entire supplied table must contain only zero slots or
unresolved markers for those nine allocator ordinals**. Unsupported function/data
imports fail Unsupported (-5) before writes; non-marker values and invalid
bounds fail Invalid (-1). Already resolved markers are not silently re-resolved.
This is not a complete Burnout kernel resolver or a title-admission mechanism.
The actual title's other imports still prevent bootstrap completion.

The plan reads/validates the complete original table before creating metadata.
It records canonical ordinals in a bounded opaque GamePKG record and patches
supported function slots to `0xFE000000 + slot * 4`. Zero slots stay zero. The
whole table write uses existing checked RAM transfer validation, so a late
read-only page cannot produce a half-patched table. Record quota exhaustion fails
before publication. Failed publication releases its new record. A later failure
(such as root-record revocation during setup) still requires abort before entry;
this is not a transaction across independent runtime calls.

The private KPL2 map is LE: magic u32 `0x324C504B`, count u32, root call-record ID
u64, table u32, reserved-zero u32, exactly count ordinal u32 values, then a shared
u32 refusal counter. Length is `28 + 4 * count`, at most 1,540 bytes inside the
unchanged 4 KiB/64-record limits. The counter must be 0..8. Live metadata,
length/magic/count/range/reserved word and root association are checked on every
invocation. Only supported ordinals reach a handler; empty/out-of-range/unmigrated
slots remain Unsupported. Invalid metadata is Invalid; revoked records deny.
No host code address is stored. `Memory::with_context` accepts this new bound;
the four opaque context IDs and CPU/call-state layouts do not change.

The original ordinal-166 refusal path reads 35 additional stack words for the
first eight allocation failures even when ordinary kernel logging is disabled.
A bounded whole-record CAS grants those eight tickets across all callers; the
counter saturates at eight. A ticket is consumed before the extra guest reads,
matching the original increment order. Failure to read those bytes propagates
without changing EAX/ESP; no allocator mutation is replayed. After eight tickets,
the extra range is never read. Numeric diagnostic `0x544B0A00` carries caller,
size, low, high, clamped alignment and protection; byte diagnostic `0x544B0A01`
carries exactly 140 little-endian stack bytes. Output errors are best effort,
as the original ignores console errors. No console or filesystem access is added.

`calls_runtime` routes classified kernel targets to
`kernel_invoke(&mut Registers, &mut Memory, slot)` only with a configured map.
Without configuration, the old explicit migration fault remains. The existing
lookup's unaligned-byte floor-to-slot rule is preserved. Manual, entrypoint,
soundtrack and other unfinished routes are not enabled by installing a map.

Invocation checks valid register state, mapped ordinal and complete stack bounds
before asking the heap to mutate. It observes a nonzero dummy return address,
then reads all required argument bytes through checked guest views. ESP zero
preserves the original skipped dummy-return read; argument reads are still
required. The original dispatcher caller is retained per caller. Success changes
only EAX and ESP, popping four dummy-return bytes plus the original stdcall
argument size. Fixed-width end arithmetic rejects ranges beyond 2^32; a valid
end exactly at 2^32 has the original u32 wrapped ESP. Other registers/FP/XMM
values are preserved. Failures leave the CPU result/stack unchanged and propagate;
an indeterminate accepted heap mutation must cause abort, never replay.

## Validation

`reference_inputs.json` pins the source kit, complete `kernel_bridge.c`, nine
exact bridge bodies, complete original argument-size function, lookup body and
original main import-patch loop. The C oracle reuses the pinned original Heap and Contiguous helpers. It runs only supported pool tables in bounded
synthetic RAM, with inert logging/no OS services. Native C handler pointers exist
only in this maintainer oracle, never admitted or trusted runtime objects.
The original interrupt/profiling/watch/diagnostic wrapper is not claimed by these
component comparisons; it remains separate migration work.

Linux32/64 and Windows32/64-under-Wine each passed 2,944 original bridge calls
across five cases: normal/reused/random allocations, tight/empty heaps, maximum
378-slot metadata, unaligned stacks and ESP zero. Every call compares the full
244-byte CPU record, allocator counters and dispatcher caller. Final comparisons
cover all 64 MiB RAM byte-for-byte, including original thunk patching. Unsafe
malformed inputs never enter the C oracle.

`pool_checks.rs` is ordinary admitted game source using the real verified target
classifier and actual traced/untraced native router. It checks all four low-byte
kernel-target offsets, stack cleanup, untouched CPU fields, zeroing/reuse and
query/free results. Four reviewed tasks independently adopt the shared context
and execute 512 allocation/query/free lifetimes through kernel dispatch (1,536
kernel calls), checking held memory remains exclusive and the shared trace advances
by exactly 1,536 commits without replay/lost updates. Controls cover unsupported
function/data imports, bad markers/bounds/counts, maximum map size, record quota,
a late read-only table page, revoked records, missing dummy/tag reads, ESP zero,
malformed map fields, empty slots and incomplete handler routes. A temporary
name collision caught by the conservative source scanner was removed without
changing that scanner or accepting indirect calls.

The extended `--memory` fixture separately compares 2,115 original calls on
Linux32/64 and Windows32/64 under Wine. Each call compares all 244 CPU bytes,
caller, stack arguments and both allocator counter sets. Final comparison covers
both complete 64 MiB banks and every page record. Zero/unaligned stacks, range
and alignment edges, fixed claims, combined fallback, GPU padding and all eight
original refusal dumps are included. This is a synthetic RAM oracle, not a title.

Its ordinary `memory_checks.rs` fixture runs actual finite kernel call/jump
routing with four independently registered callers: 512 iterations hold 1,536
Heap/Contiguous lifetimes, with exactly 1,024 contiguous allocation/free pairs.
Negative controls cover unconfigured backing before table writes, inaccessible
fifth arguments, u64 stack bounds, late read-only padding/clear pages, invalid
fixed ranges and corrupt metadata. One denied extra-stack read consumes the
first refusal ticket; concurrent callers consume the remaining seven exactly.
After saturation an inaccessible extra-stack range is correctly ignored.
Exclusive held data, tiled aliases, exact post-join counts, client restoration
on errors and explicit cleanup are checked. This focused source passes admission
and every-object audits for all four targets and executes under actual confined
Linux32/64. Windows results here are cross-build audits and Wine oracle runs,
not Windows AppContainer execution.

The full partial generated-candidate native matrix retains its 38 original
functions / 4,864 register-and-whole-RAM vectors per target, complete-source
admission before bindings, separate optimized objects and collective audits
before trusted linking. Its routing fixture exercises the four original pool
ordinals; the additional five wrappers are exercised by the focused extended
fixture above. The full matrix is not evidence of executing all nine through
the title. Existing Heap/shared-owner/image/bootstrap regressions remain required.
No source/import rule, budget, confinement or legacy game-profile gate is relaxed.

```sh
python3 host-integration/engines/takedawn/service-kernel-pool/check.py \
  --app /path/to/GamePKG/app --wine --memory \
  --app-lib /path/to/libgamepkg.rlib --call-targets /path/to/verified-targets
```

Omit `--memory` for the original four-pool reference/control suite. Supply the
reviewed full candidate and target report to `service-native/check.py` for its
separate full-source build/admission/confinement matrix. Python/C are maintainer
only; users receive Rust/native tools. These tests use no ROM, window, physical
input, real audio or game assets. Image/startup regressions independently read
the authorized ISO in place.

## Remaining work

Complete original kernel data initialization, all other function exports,
import count/remap/lookbehind semantics, IRQ safe points, scheduling/profiling/
watch and other bounded allocation/trace diagnostics. Connect authenticated
image/bootstrap to the complete resolver and guest-thread/TIB startup; this
allocator-only setup is not a complete resolver. Finish generated/manual/media
and renderer paths before native entry/title admission. These tests establish
neither gameplay nor physical controls, audible audio, rendering fidelity or
measured frame pacing. The legacy title build/play/cache/export gate stays closed.


The extended nine-wrapper fixture now stages the [retail guest layout](../service-memory/README.md)
and uses its complete 32-span topology for the coordinator, four independently
registered worker contexts and maintainer original-code comparison. The older
four-pool fixture and full generated-candidate four-pool regression remain
independent. This changes no kernel record/stack/allocator protocol, budget,
source/object rule or title-admission gate.


The dispatcher additionally implements SHA ordinals 335/336/337; see
`../service-kernel-sha/README.md` for exact behavior and the separate original-C,
source/object and confined-call checks. The nine-allocator corpus here remains
a distinct regression, not the SHA qualification. `kernel_physical` now lists
only 165/166/168/171/180 so hash preparation does not require a heap owner.

RC4 key/streaming and HMAC-SHA1 (338/339/340) also use checked RAM and the finite
router. See `../service-kernel-crypto/README.md`; the dispatcher now requires the
current app clearing SDK. Its maximum argument copy is 28 bytes for HMAC,
without changing any record, runtime, signed or child budget.


The shared root services/NV extension adds original export 24 with configured
video/audio flags and one shared offset. See `../service-kernel-settings/README.md`
for the exact 288-byte private root codec, 64-byte record, unchanged four-ID
context, readiness/lifetime/error contracts and C/confinement controls. CPU
records stay 244 bytes and all quotas stay intact. The private main gap is now
114; time exports and full gameplay remain gated.

Clock consumer checkpoint `b70e07e` also implements exports 125..128 using the
actual host clock SDK, shared root-owned skew and original register/stack rules.
See `../service-kernel-clock/README.md` for exact APIs, signed arithmetic domains,
unsigned wrap, no-read null behavior, output protections and original-source /
confined-worker evidence. Allocator behavior, quotas and complete-title gating
are unchanged; no direct platform clocks or engine exemptions were added.

Timestamp checkpoint `e206014` adds the port-owned guest TSC helpers and strict
64-byte KSV2 metadata. It preserves configured flags/skew, shares one first-use
origin and rejects old/malformed state without a host API or quota change.
All five generated consumers are compared with their originals and exercised
through the finite router in actual confined Linux children. See
`../service-kernel-tsc/README.md` for exact contracts, failures and scope.
