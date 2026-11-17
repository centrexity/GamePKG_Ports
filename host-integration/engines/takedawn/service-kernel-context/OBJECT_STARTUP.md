# Opt-in authenticated object-owner startup

KIC3 binds the checked tagged table and typed event domain into the authenticated
image/import startup owner. This is dependency-free **game code**, not trusted
host engine code, an event-import implementation or a playable release. KIC1,
KIC2 and KPL4 remain strict; no reserved field is repurposed. Native profiles stay
blocked, and event imports are not classified ready merely because this binding
exists. The remaining full-title main import gap is still 106.

## Exact context contract

`kernel_pool::KernelContext::kernel_context_new_with_objects(&Memory,
&Connections,&Shadow,table:u64,event_domain:u64)` creates an explicitly versioned
80-byte copied record. Every field is LE u64:

| Offset | Meaning |
| --- | --- |
| 0 | magic 0x3343494b (KIC3) |
| 8 / 16 | game call root / KSV3 owner IDs |
| 24 / 32 | ISR1 connections / shared shadow record IDs |
| 40 / 48 | HTR2 tagged table / EVD2 event-domain record IDs |
| 56 / 64 / 72 | reserved zero |

The constructor and every observation validate live same-root service ownership,
ISR/shadow identity/shape, ready HTR2 structural binding and active EVD2 structural
binding to that same table/root/KSV. Zero, foreign, high-bit forged, wrong-sized,
wrong-version and reserved-field records are rejected. The KIC3 check does not
turn copied records into immutable/native authority, validate every event row,
pin a resource or prove any task joined. Typed consumers must additionally borrow
the complete table/domain and validate actual SDK kind/liveness on use.

`kernel_context_objects(&Memory) -> Result<[u64;2],i32>` returns the validated
`[table,event_domain]` copied IDs. A live strict KIC1/KIC2 returns Unsupported.
`kernel_context_registry` and `kernel_context_shadow` preserve their existing
contracts and work with KIC3. Creator-only `kernel_context_close` releases only
the context record; it neither owns nor closes those referenced resources.

## Startup API and ownership

`Startup::startup_prepare_with_objects(&mut self,&mut Memory,&Image,&Data,
remap:&[u16],video:u32,speakers:u32,scratch:u64,offset:u64)` opts in explicitly.
Scratch must be the exact fourth 1 MiB window of the host-selected
`TakedawnServices` preset. The chosen 256 KiB slot must already be committed,
read/write and unclaimed. Offset is one of 0, 262144, 524288 or 786432. Scratch
zero is invalid; it cannot silently select a legacy path. The caller retains
RAM/commit/heap/image/data ownership. No window base is chosen by game metadata.

The owner retains each accepted prefix immediately: KSV, ISR registry, shadow,
unbound tagged table, partial scratch initialization, event domain, KIC3 and
finally the published import map. A failed bind or later allocation keeps its
partial owner for explicit close. Failed startup stays failed and cannot retry,
issue ready IDs or enter game code. This does not enable additional imports.

`startup_objects(&Memory) -> Result<(Handles,Events),i32>` first validates ready
owner IDs, exact KPL4/context/registry/shadow associations and the exact *owned*
table/domain pair. It then borrows both complete typed representations. A
same-root, independently valid substitute pair is still rejected. Returned views
have no owner-close authority. The existing `startup_ids()` four-ID result is
unchanged: `[KSV3,ISR1,KIC3,KPL4]` for this path, KIC1/KIC2 for the older paths.
The existing worker call-context codec is also unchanged.

All subsystem jobs must be registered before they are resumed. Shutdown returns
Busy before changing phase while any tracked job lacks an actual consumed join
proof. A finished poll or unrelated/reused proof is insufficient. Following that
barrier, cleanup revokes the import map/context, drains real typed events and
wait records, clears the tagged table's accepted scratch pages, then drains the
older shadow/registry/services. An incomplete resource close retains its owner
and cleanup prefix; it is not dropped or marked closed. After revocation,
completed cleanup retains its explicit error even if resources are released.
Creator-only close is stable on repeat. Untracked guards/waits may make cleanup
Busy after map/context revocation; this is a closing owner, not a revived ready
one. Abandoned jobs/guards or corrupt backing can require session teardown.

The typed event domain supplies last-alias ownership, null-output orphans and
bounded cancellable waits; exact contracts are in
[service-kernel-events](../service-kernel-events/README.md). Startup owns that
domain and backing; it does not implement Xbox event/NTSTATUS/alert/pulse behavior.

## Explicit four-window topology and allocator path

`Layout::layout_services(&Memory)` requires exactly the reviewed four unique,
nonzero window IDs with sizes 64 MiB/64 MiB/1 MiB/1 MiB.
`layout_scratch()` returns only the fourth opaque ID. The old `layout_current`
requires exactly three windows and rejects this preset; its `layout_scratch`
returns Unsupported. `layout_view` preserves the original three-bank guest
aliases and creates **no scratch guest alias**. In particular, a guest number
that happens to equal a host preset base still resolves through the checked guest
mapping, not to that host address or scratch backing.

`Contiguous::contiguous_services(&Memory,window:u64)` is the corresponding strict
four-window constructor, still selecting only the second 64 MiB bank. The old
`contiguous_new` remains strict three-window. Do not construct competing allocator
state over a live bank; the existing reviewed allocator job remains the owner.

`HeapClient::heap_contiguous_services(window:u64)` sends private allocator
**version 2, operation 19**, exactly the existing 32-byte request header with the
opaque u64 split into the two LE u32 argument fields and no extra payload. The
receiver uses the existing actual-task membership/sequence checks and requires
the primary owner slot, an uninitialized physical arena, the exact second bank
and strict four-window topology. Version 1 operation 19 remains invalid; unknown
versions, malformed lengths, stale sequences, ordinary clients and arbitrary
window IDs remain rejected. Existing version-2 operations 8..18 are unchanged.
A second initialization is invalid. The SDK transport/identity is unchanged;
this is a checked game-owned allocator protocol extension.

The image fixture's explicit `imports_memory_services` and
`imports_owner_services` use these paths. Host preparation must explicitly choose
`LocalLayout::TakedawnServices`; it charges all windows and per-page metadata.
No host quota or copied-record limit is increased, and no raw pointer/device,
crate, source exemption, LTO bypass or direct system call is added.

## Qualification scope and handoff

`service-kernel-imports/check_objects.py` stages the complete existing image,
CPU/bootstrap/allocator/import/startup helpers plus the checked table/event code.
All source is checked before SDK injection; every separately optimized game
object is audited before trusted runtime/transport linking. Windows scope is
source/object/link, not actual Windows execution. Linux uses actual confined
children with a synthetic image or an independently authenticated read-only ISO
in place. No title entry, window, GPU, physical controls, audible audio or measured
frame pacing is established by these fixtures.

Remaining integration must connect actual event imports and exact NT behavior,
then the remaining native source/loop/renderer/media/build/launcher paths and
actual gameplay. Keep the app's existing ports submodule and public source-kit
archive pinned until their consumers explicitly adopt a reviewed integration.
The installed SDK remains
`1e15c059ca66907d45e603cec9f993ca54b9ffa66bae72c4a36cee67b944fd3f`;
no app binary update is required for these port-owned APIs. Cache receipts must
rebuild checked source-kit and exact audited-object identities.

The object-startup qualification covers 16 synthetic modes plus one independently
host-authenticated private ISO check per Linux architecture: **34 actual confined
positive sessions**, all four source/optimized-object/link targets and seven
source-valid/audited required-failing Linux64 copies. It creates real SDK events
from a tracked native worker and the startup caller, retains a null-output orphan,
writes the original first guest tag into the bootstrapped stack, creates a fresh
alias and verifies actual native objects/backing disappear only after cleanup.
The controls cover six record-quota prefixes, synchronization quota exhaustion,
wrong/readonly/partly occupied scratch, same-root substitute owners, strict legacy
formats, forged KIC3 fields, lost setup prefixes, join barriers, incomplete close,
revocation and full native-sync budget recovery. The private title is authenticated
and explicitly rejected before entry, with exactly 106 unresolved main imports.

Combined-source admission initially rejected colliding local identifiers; bounded
renaming fixed the game code rather than relaxing checks. Windows32 object audit
also rejected a stack-probe import after the larger owner was introduced; bounded
fixture snapshots replaced the large stack arrays. The rejected attempts remain
private historical evidence, not relabeled as qualified current source.
