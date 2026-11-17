# Checked tagged-handle metadata over bounded scratch

This is dependency-free **game code**, not trusted host engine code. It uses
GamePKG-owned memory, records, synchronization and reviewed jobs. It grants no
native event/file/thread authority and does not enable a title. Native resource
adapters, context/import/startup wiring and full gameplay remain required.

## Owner and backing contracts

- `Handles::handles_new(&Memory) -> Result<Handles,i32>` creates an unbound,
  creator/root/KSV-owned 80-byte record and one SDK recursive mutex. A failed
  record allocation releases the incoming mutex; it does not enlarge record quotas.
- `handles_bind(&mut self,&Memory,window:u64,offset:u64)` binds once. The window
  must be the fourth 1 MiB descriptor of reviewed `TakedawnServices`, distinct
  from the original three banks. Offset is exactly one of 0/262144/524288/786432.
  The whole 256 KiB range must be committed read/write before any claim. No guest
  alias, native pointer, device aperture or game-selected host base is created.
- Each accepted 16 KiB page is retained in the owner's private initialized count
  **before** later fallible progress publication. The first all-zero page is
  atomically claimed with the opaque root ID. Another table cannot claim that
  slot while page zero remains occupied. Failed bind is not retried: retain the
  owner, inspect `handles_progress()` and close or report/tear down.
- `handles_progress() -> [phase,accepted_pages]`: 0 allocated, 1 binding/failed,
  2 ready, 3 closing; accepted pages 0..16. `handles_id()` is the opaque record
  ID. `handles_closed()` means both record and actual SDK mutex are released.
- `Handles::handles_borrow(&Memory,id)` validates the ready root, exact full-u64
  scratch binding and actual mutex kind; it has no owner-close/bind authority.
  Copied IDs and returned metadata are not leases. `handles_lock(&Memory)`
  acquires a bounded 1,000 ms recursive mutex depth and an actual-task guard.

All borrowers/producers must stop and actually join before owner shutdown; this
metadata owner does not track jobs itself. `handles_close` is creator-only and
returns Busy before modifying state while guards or nonempty rows remain.
After entering closing it clears accepted pages **in reverse order**, preserving
the first-page claim until the other pages clear. A failed compare/permission
check retains the exact uncleared prefix for retry. No closed flag is invented.
Once backing is clear, it releases the record and mutex. Completed close is
idempotent; a KSV liveness error is retained after actual resource release.
Inaccessible/corrupt backing or lost guard-ledger publication can require fatal
session teardown rather than fabricated recovery or discarded ownership.

`handles_cleanup_lock` is creator-only after KSV revocation, only for a ready
map. It permits copied lookup/removal/guard release, not insertion. Ordinary
operations require live same-root KSV and an uncancelled actual task. A cancelled
creator may still be denied the SDK mutex acquisition; errors do not mean closed.
Guards always require the actual acquiring task for use and unlock.

## Guard APIs and original guest tokens

The original 16,383 usable slots are preserved: `0x48000001` through
`0x48003fff`, with high tag byte 0x48 and slot zero reserved. No slot contains a
native address. Each row is `[SDK object ID u64, kind u32]`; kind zero and ID zero
must appear together as an empty row. Kinds distinguish SDK namespaces (for
example event and file IDs); they are metadata, never proof of SDK authority.

- `handle_guard_insert(&Memory,object:u64,kind:u32,fresh:bool) -> u32` rejects
  zero identity/kind. Nonfresh insertion searches for the same full identity and
  kind before selecting the first hole. Fresh insertion creates a distinct token.
  Full tables return Busy; high u64 identity bits are never truncated.
- `handle_guard_lookup(&Memory,token) -> Option<[u64;2]>` copies object/kind.
  `handle_guard_remove` clears both fields atomically and returns that pair.
  Wrong-tag, pseudo, raw, zero-index and out-of-range tokens are Invalid rather
  than being interpreted as native pointers. An absent valid slot is None.
- `handle_guard_references(&Memory,object,kind) -> u32` counts matching aliases
  under this guard. `handle_guard_slots` copies all 16,383 rows in slot order.
- `handles_guard_close` releases exactly its active-ledger entry and one mutex
  depth; completed close is idempotent. `handles_guard_closed` observes that.

These operations **do not create, close, duplicate or transfer SDK objects**.
Typed callers must retain incoming ownership until publication, validate actual
SDK kind/liveness on use, release only after the last owned alias, and drain real
objects before removing metadata during shutdown. Numeric entries do not bypass
host permissions or budgets. The original lowest-hole guest-token reuse remains;
a stale guest token can name a later row after reuse, just as in the original
bridge. It is not a persistent object identity or generation lease.

## Fixed-width copied representations

`HTR2` is exactly 80 bytes, LE u64 fields:

| Offset | Meaning |
| --- | --- |
| 0 | magic 0x32525448 |
| 8 / 16 / 24 | game root / KSV3 / actual mutex IDs |
| 32 | active-guard count, nonwrapping |
| 40 | phase 0..3 |
| 48 / 56 | scratch window ID / slot byte offset |
| 64 | published initialized count 0..16 |
| 72 | reserved zero |

Each `HTP2` scratch page is 16,384 bytes: magic u32 0x32505448 at 0, page index
u32 at 4, owning root-record u64 at 8, nonwrapping version u64 at 16, reserved zero
u64 at 24. Rows start at 32: kind u32 then object u64, exactly 12 LE bytes each.
Pages 0..14 have 1,024 rows; page 15 has 1,023. Remaining padding must be zero.
Updates compare the complete expected page through SDK local-memory CAS 42;
stale comparisons return Failed with no invented publication. Version overflow
fails without wrap. Root updates use the existing versioned record CAS.

Scratch costs one explicit 256 KiB slot inside the charged 1 MiB window. The
fixed SDK record limit remains 4 KiB each / 64 records / 64 KiB aggregate;
the dense payload is not relabeled as oversized copied records. SDK identity,
root/KSV/CPU/four-ID formats, quotas, public source archive/catalog/defaults,
app submodule pin and blocked title profiles are unchanged. The installed SDK
`1e15c059ca66907d45e603cec9f993ca54b9ffa66bae72c4a36cee67b944fd3f`
already supplies these services. Checked source-kit/object identities rebuild.

## Evidence and remaining work

`check.py` verifies original source/archive/function hashes and compiles the
pinned original C token/lookup/take functions only in its maintainer oracle.
All 16,383 slots, 512 original operations and the complete final-slot snapshot
match across Linux32/64 and Windows32/64 (Windows component execution uses Wine).
Controls cover full capacity, same/fresh aliases, first-hole order, full-u64 and
kind identity, raw-token rejection, nested guards, actual four concurrent jobs,
foreign/forged roots and pages, version overflow, unchanged record/sync quotas,
wrong backing, a five-page failed initialization, retained ownership through a
read-only interrupted close and creator-only revoked-KSV cleanup.

A separate **maintainer-only** SDK facade causes real version interference or
KSV unlink using the real SDK: stale progress after accepted RAM, stale insertion
without false publication, stale clearing without early decrement, revocation
at the last scan read and revocation after the third accepted bind page. No
production fault hooks or signature/profile exceptions are present. The facade
and original-C oracle are rejected if packaged even as unused game source.
Twelve broken copies must fail their native behavior checks. Fault cases run on
all four native SDK targets, Windows via Wine; they are not confined gameplay.

The complete admitted fixture scans every game helper before SDK injection and
audits all **eight separately optimized objects** before trusted linking on each
of four targets. Actual confined Linux32/64 repeat the vectors, concurrency and
ownership controls. Neither Windows process confinement, original native event/
file/thread semantics, device input, audible audio, GPU fidelity nor measured
frame pacing is established. **Main import gap remains 106**. Next work must
add typed resource ownership/cleanup and bind this table into a new versioned
context/startup path before admitting event/object imports; no legacy raw-handle
fallback, ignored unknown object or engine/media exemption is allowed.
