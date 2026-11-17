# Shared mutex observation and typed retirement

Use app `docs/PORT_SERVICES.md`, section “Non-consuming child-local mutex state
and typed retirement”, for the full contract. SDK identity is now
`49a313ff062e83e3bcbaadb125f4ace190847b0321cc25877e45f92023f0234a`.
All game objects, runtime/transport and receipts must use that exact identity.
Existing host wait-set acquisition, abandonment, cancellation and budgets remain
unchanged. No engine code moved to the trusted runtime.

## Exact host APIs

- `gamepkg::local_mutex_state(id:u64) -> Result<LocalMutexState,i32>`: op 48,
  8-byte LE ID request; 16-byte reply of LE u32 locked/owned_by_caller/abandoned/
  depth. Non-consuming actual mutex-kind validation. Ownership means the actual
  calling runtime thread. Canonical booleans; locked iff depth != 0, owned implies
  locked, abandoned implies unlocked. No lifetime/ownership is granted by a copy.
- `gamepkg::local_sync_release(id:u64,kind:u32) -> Result<(),i32>`: op 49,
  12-byte LE ID/kind request; empty reply. Actual event=1/mutex=2/semaphore=3
  checked before retirement. Wrong kind Invalid, held mutex Busy. This does not
  unlock or decrement an owned depth. It revokes the registry entry and wakes
  retained waiters; runtime-owned references keep in-flight storage safe.
- State query denies cancelled tasks. Typed retirement permits task-cancelled
  cleanup in a live runtime; closed runtimes/foreign/stale IDs deny. Malformed
  format or kind Invalid, internal inconsistency/protocol failure Failed, older
  SDK Unsupported. No fallback. A protocol error may follow an accepted native
  action; preserve accepted prefixes and never invent rollback/reacquisition.

## Port integration requirements

Do not probe a mutex type by acquiring it: that would consume abandonment and
alter recursive ownership. Validate actual type through the query during live
admission, then publish/retain all wait leases before releasing table guards.
Use explicit typed retirement from an owning cleanup journal after cancellation;
query denial alone is not proof that an object has been retired. Do not convert
Denied caused by cancellation into “object absent” and discard the native ID.

Keep every successful guest acquisition owned until its actual release/actual
task-exit abandonment. Last aliases, active waits, recursive depths, unpublished
creation prefixes and creator shutdown need explicit retained obligations.
Neither a state copy nor HTR2 kind 2 alone establishes a typed owner. Extend the
checked mixed-owner transaction only after that actual owner exists; preserve
its retained-slice/no-replay contract and the SDK's abandoned result.

Pinned `kernel_bridge.c` identifies NtCreateMutant/NtReleaseMutant as 192/221.
The current private KIC4 main table does not import those two directly; required
waits 233/234 are still pending. The source kit's original kernel_sync.c contains
CreateMutexW initial-owner and release behavior for eventual compatibility work,
but do not mistake implementing unused exports for finishing the active game.
Prioritize actual file/thread/async ownership and alert/APC paths alongside any
mutex ownership demonstrated by those active paths. NT clock/status/CPU mapping,
generic close and complete generated loop/render/media/build/launcher remain.

## Qualification scope

The host has four-target real-runtime/binding/client checks, 21 required-failing
copies, and actual C/Rust local-ABI tests including full IDs on i686 and unaligned
replies. Synthetic internal corruption/revocation/cancellation prefixes are
explicitly separate from real public-cancellation cleanup. The signed game fixture
uses both APIs after source/object admission and executes confined Linux32/64
children without window-owner dispatch. Windows components use Wine; signed
Windows artifacts are build/audit-only. Existing port fixtures are rebuilt for
this SDK separately; old SDK receipts must not be relabelled as current.

These are infrastructure checks, not title entry, physical controls, audible
sound, original rendering or measured frame pacing. Main KIC4 still has 98 pending
imports. No title admission, source archive/catalog update, pinned submodule
change or legacy runtime bypass is authorized by this prerequisite.

## Existing owner consumer correction

EVD2/SSD1 retiring rows now use op 49 with explicit actual kinds 1/3. This removes
a reproduced false-completion path when cancellation happens after the creator
holds its cleanup guard: query Denied previously skipped native retirement while
clearing its ownership ledger. `service-kernel-retirement` verifies the actual
native object is gone after domain close and actual consumed join; restoring
either old path must fail. Creator/root/table obligations after cancellation may
still require outer runtime teardown. Do not generalize that domain correction
into complete title shutdown or mutex/file/thread ownership qualification.


The pinned bridge/main intersection also includes active critical-section exports
277/291/294 (RtlEnter/Initialize/LeaveCriticalSection). Review their actual guest
layout, recursion and error/CPU contracts when consuming mutex services; these
are more relevant to the active title than assuming unused 192/221 are required
main imports. Shared SDK mutex observation/retirement does not itself implement
those checked adapters or qualify their guest semantics.
