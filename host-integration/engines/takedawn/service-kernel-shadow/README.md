# Checked shadow-map and event adapters

These are dependency-free port sources. They use GamePKG-owned local records,
typed mutexes and events, checked guest RAM and reviewed workers. They introduce
no SDK operation, native pointer, OS handle, Cargo dependency, profile exemption
or change to the app's pinned ports submodule. Neither title is admitted here.

## Logical map

`Shadow::shadow_new(&Memory)` owns one mutex and a 152-byte root record, bound
to the current root/KSV3 and creator. `shadow_id` exposes only its record ID.
`shadow_borrow(&Memory,id)` is nonowning: it validates root metadata and actual
mutex kind; pages are validated when used. A copied root cannot adopt another
root record's pages. `shadow_closed` reports completed resource cleanup.

`shadow_lock(&Memory)` returns an actual-task-bound `Guard` holding one recursive
mutex depth. Every method validates the live root/KSV3 and schema. It preserves
all **4,096** original logical slots without restricting nonzero guest VAs to an
assumed alignment or truncating u64 tokens. Guard methods are:

- `shadow_lookup(&Memory,guest) -> Option<u64>`: first matching token, None for
  absent/zero guest. This is a copy, never a lease or native-object reference.
- `shadow_insert(&Memory,guest,token) -> Option<u64>`: replace an existing guest
  before considering the first hole; otherwise use the original first free
  slot. Zero guest/token is Invalid, full map is Busy. Return the displaced token.
- `shadow_remove(&Memory,guest) -> Option<u64>`: clear the matching slot and
  return its token; absent/zero is None. Empty slots retain neither field.
- `shadow_entries` copies nonempty `[guest,token]` pairs in original slot order;
  `shadow_slots` copies all 4,096 pairs, including holes.
- `shadow_guard_close` decrements the active-guard ledger and unlocks exactly
  one depth. It is explicit and idempotent after closure. While held, a foreign
  task cannot unlock or mark it closed. Cleanup of a held guard remains possible
  after KSV3 revocation/cancellation. `shadow_guard_closed` reports the result.

**Tokens are port metadata, not new authority.** This map does not create, type,
close, join or extend the lifetime of resources named by them. Typed callers
must close a displaced resource before publishing replacement/removal while
still holding the guard, and must handle full-map incoming-resource cleanup.
Arbitrary/synthetic tokens still fail the SDK's actual handle/type checks.
This separation preserves logical capacity without pretending that 4,096 native
events fit the unchanged 128-active-sync-ID quota. Thread/mutant/semaphore shadow
adapters and their reference lifetimes are not provided by this event module.

The owner can obtain `shadow_cleanup_lock` after KSV3 revocation. Only the
creator/owning view can open this deletion lane; it permits reads/removal and
explicit guard close, but refuses insertion. Stop/join producers first.
`shadow_close(&mut self,&Memory)` requires an empty map and zero active guards,
including guards held recursively by that same task. Busy leaves the resources
intact. It marks closing, retains page IDs in its cleanup ledger and releases
pages/root/mutex. Partial cleanup retains remaining IDs for retry; after KSV3
revocation it completes cleanup then reports Denied. An unavailable/corrupt root
or failed guard-ledger publication is fatal: report and tear down the confined
session rather than fabricating completion or discarding ownership.

Root `KHS1` fields are LE u64: magic `0x3153484b` at 0, game root 8, KSV3 16,
mutex 24, active guards 32, closing boolean 40, and 13 page IDs at 48. Pages are
allocated lazily as a contiguous prefix. Each `KSP1` page has magic u32
`0x3150534b`, index u32 and owning shadow-root record ID u64. Twelve pages have
340 12-byte entries; the last has 16. Each entry is guest u32 and token u64,
both zero iff empty. Full metadata is 49,512 bytes across 14 records, within
existing 64-KiB/64-record budgets; allocations can still fail under other shared
charges. No quota is widened and no missing page or failed allocation is a
native-memory fallback. Mutex waits remain bounded to 1,000 ms.

## Native event adapter contracts

`events.rs` owns only events it creates through the SDK. It rejects an existing
mutex/task token presented as an event instead of closing an unrelated resource.
Errors retain Invalid -1, Busy -2, Failed -3, Denied -4 and Unsupported -5.

- `event_replace(map,&Memory,guest,manual,signaled) -> id` creates the new event
  before acquiring the map lock, closes any old **event** under that lock, then
  publishes the new mapping. New-event failure preserves the old mapping.
  Publish failure after old-event closure is an explicit fatal partial prefix:
  the map may still name the revoked old ID. New unpublished events are released;
  callers must stop/report and clean up, never blindly retry that prefix.
- `event_lookup` copies a mapped event ID and validates its actual kind/lifetime.
  `event_for` additionally shadows a valid inline 16-byte KEVENT on first use.
  Existing mapping wins; otherwise Type 0/1 is sampled outside the map lock,
  mapping is checked again inside it, and current SignalState is read there.
  Concurrent first users therefore share one event. Other header types return
  None; unmapped/protected/overflowing guest reads return an error, not a pointer.
- `event_initialize(map,&mut Memory,guest,type,initial)` creates/replaces the
  event **before** writing the 16-byte guest header: Type is `type!=0`, Size 16,
  Inserted 1, SignalState normalized to 0/1, other header fields zero. Manual
  reset is true only when the original type is exactly zero.
- `timer_initialize` treats null guest as a no-op, otherwise clears the complete
  40-byte object and writes Type `8+(type&1)` and Size 40 **before** creating the
  event. Manual reset still means `type==0`, so noncanonical type 2 is Type 8
  with an auto-reset event, matching the original bridge. Allocation failure
  preserves this guest-memory prefix and any prior shadow mapping.
- `event_set`/`event_reset` use the inline-event path, returning false if it
  does not identify an event. They do not mirror changes into guest SignalState.
  `event_wait` is only the typed bounded SDK wait (0..1000 ms; absent non-event
  is Unsupported), not the full guest Ke/Nt wait/alert/timeout implementation.
- `timer_reset_after_set` performs lookup only (no lazy creation), resets an
  existing event, and leaves guest SignalState unchanged, even after a full
  firing table rejected a new timer. Call it after the unlocked set operation.
- `timer_signal_after_callback` looks up the current mapping after the callback,
  signals it, then writes guest SignalState=1. Missing mapping is false and has
  no guest write. Signaling precedes the write: a protected guest word can fail
  after the event became signaled. Do not roll back or acknowledge that prefix.
- `event_remove` is available to normal live callers and closes the actual
  event before removing its mapping under the same guard. A stale event is
  removed for cleanup but reports Denied. `event_close_all` is creator-only
  shutdown using the deletion lane, including after KSV3 revocation. Drain it
  before closing the logical map. No native event survives session teardown.

These are the checked shadow paths. They deliberately do not cast guest VAs to
native handles or reproduce the legacy raw-pointer fallback. A full bridge must
resolve other approved object kinds explicitly, not reinterpret an unknown VA.
The original cancellation helper also needs a separate layout review; see
[CANCEL_REVIEW.md](CANCEL_REVIEW.md). No native timer-queue API is justified by
that legacy mixed-layout helper.

## Validation scope

`check_map.py` pins original map bodies and compares all 4,096 logical slots,
512 operations, close-order traces and eight complete ordered slot snapshots per
target. Its synthetic opaque tokens are not native events. Hostile schemas,
stale/foreign roots, cross-root pages, wrong mutex kind without signal
consumption, guard-ledger overflow, nested guards and revoked cleanup are checked.
Required-failing controls permit early close, remove page ownership, or fail to
clear a slot. Bounds-checked fixed-width byte copies avoid an unapproved compiler
panic helper; the object import allowlist is unchanged.

`check_events.py` separately compares 256 pinned header/lazy/reset/signal cases
per target, including all guest bytes, creation sequence and manual/signaled
state. Its C event-state shell does not qualify full guest waits or raw-pointer
fallbacks. Actual SDK workers test six simultaneous first users and exactly one
auto-reset consumer, with real joins. Controls verify exact unchanged sync quota,
no leaked replaced/failed-new events, wrong-kind refusal, stale removal, read-only
failure order and revoked-owner cleanup. Required-failing variants change timer
kind interpretation, leak an old event, clear only 16 timer bytes, or publish the
guest signal before native signaling.

Both commands accept `--app APP --app-lib CURRENT_APP_RLIB --wine --evidence DIR`.
All admitted files are scanned before bindings and every optimized object is
audited before trusted linking. C/Python are maintainer tools, not user
requirements. Confined Linux x64/i686 runs and Windows x64/i686 Wine components
are separate from real Windows confinement, title gameplay, physical controls,
audible audio, GPU visuals or measured pacing. Actual timer callbacks, full
wait/object adapters, retained timer/vblank/spin/TIB/context loop and source-kit/
builder/launcher closure remain required before enabling a title.

Preserve the toolkit attribution in `../service-native/UPSTREAM_LICENSES.txt`
when incorporating these sources into a complete port source kit.


### Retained guard retirement

The KSH1 guard now retains native depth release and count retirement separately,
like the tagged HTR2 guard. Failed unlock keeps its count; stale accounting after
successful unlock retries only the count CAS. Closed requires both, and another
guard's depth is never released by that retry. Exact corrected lifecycle and
scope: [mixed-owner contract](../service-kernel-mixed-waits/README.md).


Root-count increments/retirement and guarded root-page linking tolerate bounded
uncommitted version conflicts (64 attempts), preserving the latest counts and
all other bindings. Native actions, read failures, non-conflict SDK errors and
actual page edits do not gain retry/fallback behavior. A retained count remains
owned on Busy; root-page linking does not allocate another page on each retry.
