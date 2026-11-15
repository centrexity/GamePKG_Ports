# Shared heap task and client contract

`service.rs` keeps exactly one `Heap` and an optional `Contiguous` exclusively
owned by one reviewed game task. All allocation, free and query requests run serially in that task,
including zero-fill. Caller tasks never share its Rust table or native pointers.
GamePKG owns every native task, copied queue, memory mapping and protection.
The allocator/protocol remain ordinary dependency-free game Rust, not trusted
engine code. Four pool ordinals use this owner through the checked native router; contiguous
kernel wrappers and guest-thread startup still require migration.

This component requires app implementation
`38a84d4acdce1bc3107dbf2725dc3a2e02848372` or a compatible later shared SDK
with **local op 16**,
`local_task_finished(u64) -> Result<bool, i32>`. Shared-service identity:
`0db7bde13239a9ee091c18840f476a6844ff7af1015945a62b6de5e768e7b4bd`.
That non-consuming snapshot leaves the actor's result available for the
supervisor's join. Older bindings/runtime are insufficient; never use a direct
OS/thread fallback or substitute a consuming join for this query.

## Initialization and ownership

The reviewed launch recipe must include a static game entry that calls
`heap_serve(inbox: u64) -> u64` in its fixed GamePKG task table. The service itself
has no hard-coded table index. `heap_inbox() -> Result<u64, i32>` creates a copied
queue with maximum message size 48 bytes and depth 32. The launcher creates it,
spawns the reviewed entry with that opaque ID, and constructs its owner client
using `HeapClient::heap_client(inbox, job)`.

`heap_client` creates a private 32-byte, depth-one response queue. It exposes no
queue pointer or native task handle. `heap_initialize(record, window, base, top)`
can be called only as the owner's first request. The launcher must obtain the
record/window from its reviewed RAM setup and base/top from authenticated CPU
startup layout fields 4/5. The actor adopts the live call record, attaches the
explicit 64 MiB RAM view and constructs its one Heap. It does not commit or
relax any page permission; the normal image/bootstrap path must do that first.
Initialization failure returns its actual error and terminates the actor.
Successful reinitialization/reset is not an available operation.

Each other caller constructs its own client and calls `heap_register()` once.
The client supplies its GamePKG logical task ID (zero for the bootstrap thread),
not an OS thread ID. Nonzero IDs must name a pending local task. There are at
most 12 active clients including the owner. Only registration scans for exited
client tasks: completed/joined task memberships and their response queues are
reclaimed before allocating a new slot. Bootstrap-thread clients (ID zero)
need explicit unregister. This permits repeated guest-thread lifetimes without
increasing GamePKG's 16-mailbox quota. The normal actor plus 12 responses uses
13 queues and under 2 KiB of reserved payload capacity; the host still enforces
its existing queue/count/memory limits independently.

These are game-owned logical protocol identities, not a new host authentication
or permission boundary. Host grant/page/runtime checks remain independent.
A caller's task association is for bounded lifecycle cleanup and must not be
used as an authorization substitute for filesystem/network/device capabilities.

## Typed client operations

All methods require exclusive `&mut HeapClient`; one request can be outstanding
per client. A client object must stay owned by its one caller and must not be
reconstructed/cloned while a request is pending.

- `heap_alloc(size, alignment) -> Result<u32, i32>`: preserves the core allocator's
  reuse/alignment/zero-fill rules and zero on ordinary exhaustion.
- `heap_release_block(address) -> Result<(), i32>`: original free semantics.
- `heap_query(address) -> Result<u32, i32>`: remaining bytes in the live block.
- `heap_status() -> Result<[u32; 3], i32>`: copied next/table/new-allocation counts,
  after checking the live RAM record.
- `heap_unregister() -> Result<(), i32>`: a non-owner releases its membership.
  It remains available after RAM revocation, since it performs no RAM operation.
- `heap_stop() -> Result<(), i32>`: only the owner stops the actor, also permitted
  after RAM revocation. The supervisor still joins the task normally.
- `heap_abandon() -> Result<(), i32>`: explicitly revokes the client's response
  queue and poisons the local object. It is idempotent, including when the actor
  already revoked that queue. Normally unregister/stop first. Abandonment is not
  rollback of an accepted request.

A full input queue returns Busy before acceptance and retains the client's next
sequence, so that unaccepted request may be retried. Membership exhaustion is
an accepted registration response returning Busy: close that unregistered
client and create a fresh one when capacity is available. A failed initialization
or registration must not be reused as a new logical client.

After accepting a request, the client waits in at-most-one-second local queue
waits and observes cooperative cancellation. On an empty wait it checks the
actor's completion without consuming its result. A malformed/uncorrelated reply,
cancellation, disconnection or stopped actor poisons the client. **Its mutation
outcome may be unknown: abort that use, never replay the accepted mutation.**
There is no default frame watchdog, instruction meter or arbitrary native wait.
All waits are in game tasks in the confined child, never on the window owner.
Known negative operation responses propagate; Unsupported is not converted into
a platform fallback. Overflowing request sequences fail before acceptance.

`heap_serve` returns zero on normal stop or the negative error encoded as u64.
It revokes the input queue on every exit. Normal stop revokes other client
response queues while retaining the owner's final acknowledgement. A fatal exit
also revokes the owner response queue. Pending clients wake or observe the task's
completion; no client consumes the supervisor's join result.

## Wire format (private game protocol)

All integers are LE. Initialization is exactly 48 bytes: version u32=1, op u32=0,
response ID u64, sequence u64=1, base/top u32, call-record ID u64, RAM-window ID
u64. Subsequent version-1 requests are exactly 32 bytes: version/op u32, response ID and
sequence u64, first/second u32.

| Op | First / second | Meaning |
| --- | --- | --- |
| 1 | size / alignment | allocate |
| 2 | address / zero | free |
| 3 | address / zero | size query |
| 4 | zero / zero | owner stop |
| 5 | zero / zero | state query |
| 6 | zero / zero | non-owner unregister |
| 7 | logical task ID low / high u32 | register new response ID, sequence 1 |

Replies are exactly 32 bytes: version u32=1, status i32, echoed sequence u64,
result u32, next/table-count/new-allocation-count u32. Initial/register/free/
state/stop/unregister results are zero; allocation/query return their u32 value.

Known clients must send exactly previous sequence + 1. Duplicates/gaps cannot
execute another mutation. Correctly framed requests consume their sequence even
if their operation/arguments are rejected. Bad lengths/versions with a usable
response header do not consume a sequence; unknown members are denied. Frames
shorter than 24 bytes or with zero response/sequence are uncorrelatable and fatal.
A failed response send is fatal; a stale response ID cannot silently become a
live membership. Registration publishes membership only after its reply send
succeeds. None of this grants access outside the existing session/runtime.

## Qualification and remaining work

`check_service.py` compiles/runs the same ordinary service/check source with a
maintainer fixed task table on Linux32/64 and Windows32/64 under Wine. The signed
package then passes complete source admission before bindings and separate
optimized-game-object audits before trusted linking for four targets. Actual
confined Linux32/64 executions run four callers through 2,000 allocation lifetimes,
checking zeroing, interior queries, exclusive held-memory patterns and reuse.

Controls cover duplicate/gapped sequences, malformed versions/lengths/ops,
invalid registration task IDs, membership limits/reclamation, 24 task exits
without unregister, full input queues before acceptance, invalid initialization,
revoked records before/after setup, wrong/short/uncorrelated replies, stopped
peers with preserved join results, actor cancellation, cancellation after a peer
consumes a pending request, stale response queues and fatal-frame cleanup.
No source/import rule, host quota, compiler budget or confinement was relaxed.
Windows confined service execution and physical devices remain unqualified.

```sh
python3 host-integration/engines/takedawn/service-heap/check_service.py \
  --app /path/to/GamePKG/app \
  --app-lib /path/to/target/debug/deps/libgamepkg-HASH.rlib --wine
```

Python and the direct runtime wrapper are maintainer-only. No window/audio/ROM
is used by these synthetic tests. Title kernel/manual allocation/free/query
exports and guest-thread stack/TIB startup still need to call this shared owner.
The original allocation/OOM/heap-trace reporting and the rest of the native
kernel, memory/device, media and renderer migration remain required. This is
component integration evidence, not native gameplay or measured frame pacing.

The [pool kernel integration](../service-kernel-pool/README.md) now moves the
primary client into Memory and gives each caller its own lazy client. Shared
thread context carries only endpoint/record IDs, never a pending reply queue or
sequence. The original title's other kernel/thread/device paths remain required.


## Contiguous extension: private protocol version 2

The extension requires the app's three-bank preset from implementation `f4bdfea`;
heap-only v1 use retains the earlier minimum SDK above.

The same reviewed actor can now own both allocator tables. It starts with
Contiguous absent; only the initialized primary owner may configure it once.
`heap_contiguous_initialize(window)` requires the exact second bank from the
app's `TakedawnMemory` preset. Wrong main/flash/zero/guessed IDs, another root or
an already-configured arena cannot substitute/reset that state. Other callers
register through unchanged v1 op 7 and keep their own exclusive client objects.
These are game bookkeeping/lifecycle identities, not new host authorization.
The actor constructs the [checked contiguous core](../service-contiguous/README.md)
as ordinary game source; it is not moved behind a trusted engine exemption.

The original v1 initialization, operations, replies, sequence and error behavior
remain intact, including unknown correctly framed v1 requests consuming their
sequence. The queue is still 48 bytes/depth 32, replies 32 bytes/depth 1, and at
most 12 clients fit within the unchanged 16-queue/1 MiB runtime limits.

Version-2 requests have the same first 32 LE bytes as v1. Only op 9 appends
physical lowest/highest u32, making exactly 40 bytes. All other v2 requests are
exactly 32 bytes. Valid new replies echo version 2, status, sequence, result and
the existing three **Heap** counters; the reply does not silently substitute
Contiguous counters. v1/v2 operations share one monotonically increasing
per-client sequence. The typed client chooses the version for each operation
and requires an exact 32-byte reply with that version and echoed sequence.

| Op | First / second | Additional words | Result |
| --- | --- | --- | --- |
| 8 | contiguous bank ID low/high u32 | none | zero; primary owner only, once |
| 9 | size / alignment | physical low / inclusive high u32 | guest VA, zero on ordinary exhaustion |
| 10 | address / byte count | none | zero; claim without clearing |
| 11 | address / zero | none | bool u32; free only from Contiguous |
| 12 | address / zero | none | bytes remaining in Contiguous |
| 13 | address / zero | none | Contiguous size first, then Heap only if zero |
| 14 | address / zero | none | zero; Contiguous free first, then Heap if not owned |
| 15 | address / byte count | none | zero; exact-byte clear then intersecting-page claim |
| 16 | counter index 0..5 / zero | none | one serialized Contiguous counter observation |
| 17 | first page / count 1..4 | none | copied page-state bytes packed LE into u32; unused bytes zero |
| 18 | page index / zero | none | allocation length at that page, in pages |

Ops 9..18 return Unsupported until Contiguous is configured; combined size/free
must not approximate that with only Heap. The same live root is checked before
all new observations/mutations. Reserved fields, record/map extents and complete
RAM access are independently validated. Invalid header versions/lengths do not
advance the sequence and produce the existing version-1 Invalid envelope. A
valid v2 replay/gap/unknown member produces a version-2 negative reply without a
mutation. Correctly framed, next-sequence operation errors consume their sequence.
No reset, native pointer, arbitrary window/address or device aperture is added.

Typed methods are `heap_contiguous_initialize`, `heap_contiguous_allocate`,
`heap_contiguous_claim`, `heap_contiguous_free`, `heap_contiguous_query`,
`heap_allocation_query`, `heap_allocation_release`, `heap_contiguous_clear_claim`,
`heap_contiguous_counter`, `heap_contiguous_pages` and `heap_contiguous_length`.
Allocation takes size/alignment/physical-low/physical-high u32; the combined
methods take one guest address. Claim/clear-claim take guest address and byte
count. Free-only returns bool and rejects any other wire value. Page copying
returns `[u8; 4]`; invalid local count/counter arguments fail before acceptance,
while the actor also validates raw requests. Counter indices copy in-use pages,
high-water bytes, first-free page, allocations, frees and peak pages. Six separate
counter reads are not an atomic snapshot when other writers run. Copied-map
chunks are independently serialized, not a frozen whole-map view.

The fixed-range clear/claim is one actor operation: validate the complete span,
clear exactly the requested bytes, and perform the original claim. Claims do
not clear rounded padding. Overlapping claims may be original no-ops, while the
requested clear still occurs. An inaccessible late page prevents the clear and
metadata mutation. Root revocation between independent runtime calls still
requires abort; no cross-call rollback is claimed.

All accepted operations retain existing response correlation, liveness,
cooperative cancellation, poison-on-indeterminate-result and explicit queue
cleanup. A bad version/sequence/length/status or invalid bool from a peer poisons
the client and prevents another request. Cancellation after a v2 allocation is
consumed also poisons and closes the client; the mutation is never replayed.
Unregister/stop remain v1 lifecycle operations permitted after root revocation.
No legacy builtin/native profile or kernel ordinal is enabled by this extension.


The [allocator kernel bridge](../service-kernel-pool/README.md) now connects this
owner to nine exact original wrappers, including contiguous allocation, fixed
clear/claim, instance padding, combined free and ordinal-180 size queries.
KPL2 carries a shared eight-ticket refusal counter; it does not change this
owner's v1/v2 messages or quotas. The extended wrapper fixture independently
qualifies source/object admission and confined Linux32/64 routing. Other kernel
imports, guest-thread startup and native title entry remain required and gated.
