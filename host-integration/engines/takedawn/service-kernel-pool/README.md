# Checked pool kernel bridges

This component connects four original pool operations to the existing
[shared heap task](../service-heap/SERVICE.md) through the actual finite native
call/jump router. Game functions remain compiled Rust functions; no instruction
fetch/decode loop, native handler pointer, OS heap or system call is exposed.
No app SDK extension is needed beyond implementation `38a84d4` (shared-service
identity `0db7bde13239a9ee...`, composite native recipe `dd1e2928b1a6fb54`).

The original dispatch table, not its stale comments, establishes these ordinals:

| Canonical ordinal | Original bridge | Arguments after dummy return |
| --- | --- | --- |
| 14 | ExAllocatePool | size u32 |
| 15 | ExAllocatePoolWithTag | size, tag u32 |
| 17 | ExFreePool | address u32 |
| 23 | ExQueryPoolBlockSize | address u32 |

Both allocation calls use the original 16-byte alignment. The tagged call reads
both arguments even when logging is disabled. Free returns EAX zero; query
returns remaining bytes in a live block, preserving interior-pointer behavior.
Ordinary allocation exhaustion returns EAX zero with normal stack cleanup.

`MmQueryAllocationSize` (180) also asks the separate contiguous arena first.
It remains Unsupported until that allocator exists; answering from only this
heap would be a downgrade. Kernel data exports are not callable pool bridges.
No other ordinal is replaced by a successful stub.

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
unresolved markers for those four pool ordinals**. Unsupported function/data
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

The private KPL1 map is LE: magic u32 `0x314C504B`, count u32, root call-record ID
u64, table u32, reserved-zero u32, then exactly count ordinal u32 values. Maximum
size is 1,536 bytes within the unchanged 4 KiB/64-record limits. It exposes no
host code address. `kernel_ordinal(&Memory, slot)` checks its live record, exact
length/magic/count/range/reserved word and root association on every use. Only
the supported ordinals can reach a handler; zero/out-of-range/unmigrated slots
remain explicit Unsupported. Bad metadata is Invalid and revoked records deny.

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

`reference_inputs.json` pins the source kit, complete `kernel_bridge.c`, four
exact bridge bodies, complete original argument-size function, lookup body and
original main import-patch loop. The C oracle reuses the pinned original heap
helpers from `service-heap`. It runs only supported pool tables in bounded
synthetic RAM, with inert logging/no OS services. Native C handler pointers exist
only in this maintainer oracle, never admitted or trusted runtime objects.
The original interrupt/profiling/watch/diagnostic wrapper is not claimed by these
four bridge comparisons; it remains separate migration work.

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

The full native matrix passed complete-source admission and every-object audits
on Linux32/64 and Windows32/64, with actual confined Linux32/64 vector/routing
runs and malformed-vector controls. The focused fixture separately passed the
same four build targets and both confined Linux runs. Existing heap data,
shared-task lifecycle, image loading and CPU-startup regressions also passed,
including authorized private ISO reads in place for the latter two components.
The title's native entry was never run by these tests.

The native matrix retains all earlier 38-function / 4,864 reference cases per
target. Its full generated candidate, helper/context code and reviewed task
entries go through complete source admission before bindings, separate game-unit
optimization, every-object audit and trusted linking. Actual confined Linux
execution is checked separately from Windows build audits/Wine references.
No source/import rule, resource threshold, compiler-only budget, confinement or
legacy game-profile gate is relaxed. `service-native/check.py` is authoritative
for those full-source/confinement results; `check.py` here runs the original bridge references; adding `--app-lib` and
`--call-targets` also builds the focused checked-source/audited-object fixture
on all four targets and executes it under actual Linux32/64 confinement.
That focused fixture is additional evidence, not a substitute for the full
generated-candidate matrix.

```sh
python3 host-integration/engines/takedawn/service-kernel-pool/check.py \
  --app /path/to/GamePKG/app --wine
```

Run the existing native matrix with its reviewed full candidate and verified
call-target inputs for admission/routing/confinement qualification. Python/C are
maintainer-only; users receive Rust/native tools. These pool fixtures use no ROM,
window, physical input, real audio or game assets. The prior image/CPU-startup
regressions independently read the authorized ISO in place.

## Remaining work

Complete original kernel data initialization, all other function exports,
contiguous allocation, import count/remap/lookbehind semantics, IRQ safe points,
scheduling/profiling/watch and bounded allocation/OOM/trace diagnostics. Connect
authenticated image/bootstrap to the complete resolver and guest thread/TIB
startup; this pool-only setup must not be passed off as a complete resolver.
Finish generated/manual/media and renderer paths before native entry/title
admission. These tests establish neither actual gameplay nor physical controls,
audible audio, rendering fidelity or measured frame pacing. The legacy title
build/play/cache/export gate remains enforced.
