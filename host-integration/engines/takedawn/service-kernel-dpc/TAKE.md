# Locked dequeue observation

`queue.rs` now exposes the original drain's locked observation/publication
boundary. This is checked port code over the existing DPC1 record and typed
recursive SDK mutex, not a new host ABI, permission or trusted engine adapter.
The 1,616-byte record, 64-slot/63-usable ring and existing SDK quotas are unchanged.

## Exact calls

- `Queue::queue_nonempty(&Memory) -> Result<bool,i32>` reports head != tail,
  including holes, **also on the assigned timer**. Use this for drain decisions.
  Existing `queue_pending` still returns false on that timer to suppress its
  self-release wake; it is not interchangeable with this call.
- `queue_take_begin(&Memory) -> Result<Option<Take>,i32>` requires the assigned
  actual reviewed consumer. It checks that role before a bounded mutex wait and
  rechecks after acquisition. Empty returns None with the lock released. Some
  transfers exactly one still-held recursive depth to the observation. Nothing
  has advanced and the guest Inserted byte is unchanged.
- `Take::take_depth(&Memory) -> Result<u32,i32>` returns the captured depth 1..63.
- `take_entry(&Memory) -> Result<[u64;4],i32>` returns captured
  `[dpc,arg1,arg2,timestamp_ns]`, with guest values as fixed-width u32 in u64 lanes.
  A cancelled hole has dpc=0. Both getters check the root/KSV3, current actual
  owner, live observation and exact captured record version/data.
- `take_publish(&mut Memory) -> Result<[u64;4],i32>` advances head through the
  original CAS, then clears a nonzero DPC's Inserted byte, then attempts unlock
  on every result. It returns the original entry only on full success. A stale
  version is Failed; it never refreshes and consumes another entry.
- `take_abort(&mut self) -> Result<(),i32>` releases just this owned lock depth
  without advancing or changing RAM. It is idempotent after closure. Cleanup
  deliberately needs no live root/KSV3 or uncancelled task, but still requires
  the actual owner. The SDK independently enforces mutex ownership.
- `take_closed() -> bool` reports whether this view has completed its unlock.

The observation owns a copied bounded snapshot and a borrowed lock depth. It
allocates no SDK record, event, task, native pointer or new capability. Moving
it to another actual worker does not transfer ownership: reads, publication and
unlock reject there, leaving the original owner able to finish cleanup.

Errors retain Invalid -1, Busy -2, Failed -3, Denied -4 and Unsupported -5.
New work denies cancellation/revocation. Same-owner cleanup remains available.
An interfering/reentrant publisher invalidates an observation even if it writes
identical bytes with a new version. Close every acquired depth; a recursive
reentrant call is not permission to reuse a stale observation.

## Required ordering and errors

The drain must already hold its original dispatch gate before taking this queue
observation. It can now record the original queue-depth metric **while locked,
before head and Inserted publication**, without passing a function pointer into
the queue. Keep this work local/bounded; never call a native title routine or
block on window/render/network work while holding the observation.

Every Some must be published or explicitly aborted on every path. There is no
implicit game-code Drop handler. If an observer fails, abort the observation
before aborting the IQL actor: those are different mutexes. Inspect `take_closed`
after errors. Failed unlock needs actual-owner cleanup or session stop; do not
fabricate closure or try another worker's unlock.

Publication is not a transaction across the record and RAM. A denied flag clear
can leave head advanced with Inserted still set, matching the original queue
prefix. It reports an error after attempting unlock. Stop/report the affected
actor/session; do not blindly retry, roll back, run or acknowledge that DPC.
Wrong-root publication does not advance, but still attempts cancellation of an
observation owned by the caller. A wrong-task attempt cannot cancel the owner's
observation. Stop producers and actually join the consumer before closing the
queue/root. The observation extends no resource lifetime.

## Qualification

`check.py --app APP --app-lib LIB --wine --evidence PRIVATE_NEW_DIRECTORY`
retains the original 1,024-operation vectors per statistics mode/target. It
interleaves the old pop with observation/abort/stale-CAS/reacquire/publication,
matching complete guest-RAM/ring hashes, values, pending and clock use. It also
checks partial read-only publication, revoked KSV association, wrong root,
exhausted record/sync quotas and actual competing-task lock exclusion/release.
The cancelled-hole mutation remains a required failing original-C control.

Complete fixture sources are scanned before binding injection; eight optimized
game objects are collectively audited before trusted linking on Linux32/64 and
Windows32/64. Actual confined Linux fixtures compare statistics-off original
vectors plus the boundary controls. Statistics-on C witnesses and Windows
execution use the separate maintainer component shell; Windows artifacts are
also source/object/link qualified. All current source/SDK/compiler/target/object/
binary identities are retained in private receipts.

`check_take_lifetimes.py --app APP --wine --evidence PRIVATE_NEW_DIRECTORY`
separately runs 64 actual SDK worker lifetimes per target. A maintainer-only
shared value moves an observation to another actual task to prove rejection,
returns it to its owner, and cancels an owner while it holds a dequeue. Foreign
read/publish/unlock deny; cancellation denies publication but permits real owned
unlock, verified by subsequent mutex acquisition and actual join. This shell
uses native test coordination and is not admitted game code or a confined game
fixture. Required failing controls remove ownership, cancellation cleanup,
head-before-flag ordering or publication unlock; none is admitted.

Python/C/native test coordination are maintainer-only, not player requirements.
No real title callback, timer loop, hardware graphics, physical input, audible
audio or frame-pacing qualification follows from this protocol alone.
