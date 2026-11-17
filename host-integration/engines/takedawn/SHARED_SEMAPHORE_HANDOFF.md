# Shared semaphore interface for the checked port

The app now supplies counted child-local semaphores required by the reviewed
`xbox_NtCreateSemaphore` / `xbox_NtReleaseSemaphore` source. The exact SDK identity
for this qualification is
`7baf98fe4ac2504829b299197b35c524ebad6b1f08b4995cda9a560af7960199`.
Use app `docs/PORT_SERVICES.md`, section “Counted child-local semaphores”, as the
host contract. The generated client is injected after complete game-source
admission; never copy it into the game or add a Cargo dependency.

## Available contract

- `gamepkg::local_semaphore_create(initial: u32, maximum: u32) -> Result<u64,i32>`:
  initial <= maximum, maximum 1..=0x7fffffff, one opaque sync ID.
- `gamepkg::local_semaphore_release(id: u64, permits: u32) -> Result<u32,i32>`:
  positive permits, atomic complete addition <= maximum, returns previous count.
- `gamepkg::local_semaphore_wait(id: u64, ms: u32) -> Result<(),i32>`:
  0..1000 ms, cancellable, consumes exactly one permit on success.
- `gamepkg::local_semaphore_state(id: u64) -> Result<LocalSemaphoreState,i32>`:
  copied `count: u32` and `maximum: u32`; no consumption or lease.
- Existing local op 25 releases the object and revokes every copy of its ID.

Local ops 43/44/45/46 use exact LE requests of 8/12/12/8 bytes and replies of
8/4/0/8 bytes. Invalid requests/counts/kinds and over-release return -1, quota or
wait deadline -2, internal failure -3, stale/foreign/revoked/cancelled -4, older
unsupported SDK -5. No partial release occurs on a failed SDK release. Creation,
snapshot, release and typed wait deny cancelled reviewed tasks. Cleanup via op 25
remains possible. Counts allocate no counter-sized memory and share the unchanged
128-object total with events/mutexes. All waits stay off the window owner thread.

Events/mutexes reject semaphore operations without state changes. Semaphore IDs
reject event/mutex operations. Legacy untyped wait op 23 rejects semaphores; do
not use it to avoid cancellation or substitute an event for a count. Host IDs
are full u64; guest tags remain separately checked four-byte values.

## Required next port work

1. Add a checked typed semaphore owner/creation transaction and tagged-table kind,
   preserving accepted creation, tag, guest-write and cleanup prefixes. Register
   owners immediately and integrate their shutdown with actual worker joins.
2. Bind that owner into the actual context/startup graph using a reviewed codec.
   EVD2 owns events; do not insert semaphore IDs and pretend they are events, or
   infer ownership from a copied ID or state snapshot.
3. Adapt actual imports 193/222 from the pinned original C, including signed LONG
   inputs, ignored ObjectAttributes, null-output semantics, PreviousCount writes,
   CPU frame/stack preservation and NT status distinctions. Verify original
   behavior with unchanged original-C witnesses. A successful SDK release is an
   accepted count change before any later guest PreviousCount write; do not fake
   rollback or conceal memory/metadata/service errors as ordinary invalid handles.
4. Extend NT wait and generic close only with complete typed object dispatch,
   lifetime accounting and matching compatibility tests. Event-only NtClose is
   not sufficient; file/thread and other synchronization kinds remain required.
5. Rebuild through complete source admission, separate optimized game objects,
   collective object audit before trusted linking, exact receipts and OS
   confinement. No direct OS/SDL/threading fallback or engine exemption exists.

## Evidence and limits

All four native component targets pass runtime/ABI/malformed-reply tests; Windows
execution is through Wine. Six altered runtime copies compile and then fail the
real tests. The signed dependency-free game fixture is audited on all four targets
and runs actual confined Linux32/64 workers without owner dispatch. The app's
existing full preparation/enforcement checks pass; current event imports and
private authenticated KIC3 startup are rebuilt against the new SDK. Old receipts
are invalidated by the changed SDK identity.

These are SDK/infrastructure tests. No NT semaphore import is ready yet, and the
KIC3 main import gap remains 104. No physical controller, audible audio, hardware
frame fidelity, measured pacing or title gameplay is established. The public
source archive/catalog, app ports pin, canonical checkout and blocked profiles
remain unchanged.
