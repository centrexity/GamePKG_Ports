# Checked typed native-event ownership

This is dependency-free **game code** using GamePKG-owned events, records,
mutexes, memory and reviewed jobs. No OS handle, device API, dependency, raw
pointer or trusted-host engine exemption is introduced. It builds on
[the tagged metadata table](../service-kernel-handles/README.md).
It is an ownership component, **not an admitted kernel import or playable title**.
The main import gap remains 106; legacy profiles remain blocked.

## Domain and transaction APIs

All APIs return `Result<_,i32>` unless the accessor is explicitly infallible.
SDK errors stay SDK errors; these functions do not yet map Xbox NTSTATUS values.

- `Events::events_new(&Memory,&Handles) -> Events` requires the live same-root
  KSV and actual table creator. It allocates one bounded 2,112-byte copied ledger.
  No additional mutex or larger quota is introduced. `events_id()` returns its
  opaque record ID; `events_closed()` reports actual ledger release.
- `Events::events_borrow(&Memory,&Handles,id) -> Events` validates the fixed
  representation and root/KSV/table binding. Borrowed views can operate and
  begin creation transactions from reviewed worker tasks, but cannot close the
  domain. Copied IDs and metadata are not SDK authority or resource leases.
- `events_begin(&Memory,&Handles) -> EventCreation` reserves an empty ledger row
  while holding an actual-task tagged-table guard. The returned transaction owns
  that reservation and guard. All fallible task discovery precedes the accepted
  reservation. The transaction must be explicitly closed on every path.
- `EventCreation::event_create(&Memory,event_type:u32,initial:u32)` attempts
  creation once. `event_type == 0` selects manual reset, all other values select
  auto reset. Only `initial & 255` controls initial signaling, preserving the
  original `BOOLEAN` conversion. A successful call immediately retains the
  opaque SDK event in the transaction. Failure retains the reservation for close.
- `event_publish(&mut Memory,guest_output:u32) -> u32` attempts transfer once.
  It publishes the accepted event into the domain ledger **before** allocating
  a guest tag or writing guest memory, immediately retaining that transfer stage
  before any later fallible call. A null output retains an owned orphan and returns
  zero without consuming a tag. A nonnull output allocates the original first tag
  and writes exactly four LE bytes. Failed tag publication or guest writes leave
  the accepted ledger/native/token prefix owned rather than pretending rollback.
- `event_creation_id()`, `event_creation_token()`, `event_creation_phase()` and
  `event_creation_closed()` are infallible observations. Phases are 0 reserved,
  1 native event accepted, 2 transferred to ledger, 3 tag accepted, 4 output done
  (including null output). These observations do not grant native authority.
- `event_creation_close(&Memory)` closes an untransferred event, clears its
  reservation and releases the guard. After transfer it releases only the guard;
  the domain retains the native event and any accepted tag even after an output
  error. If reservation clearing fails after actual event release, the private
  released prefix prevents a second close attempt on that native object. Errors
  retain obligations and the transaction stays unclosed. Completed close repeats
  harmlessly. Work/cleanup requires the actual acquiring task, not a copied ID.

The table mutex serializes all domains/transactions using this metadata table.
A long-lived creation transaction therefore prevents other tasks from entering;
callers must finish it promptly, without waiting for work that needs that mutex.
No wait occurs on the host's window owner thread.

## Typed operations and shutdown

`Events::events_state(&Memory,&Handles,token) -> [bool;2]` returns manual/signaled.
`events_set(...,token,signal:bool)` sets or resets an actual SDK event.
`events_alias(...,token,fresh:bool) -> u32` preserves original same/fresh tagging.
`events_remove(...,token)` removes one alias; it closes the actual event only
when removing the last owned alias. Every operation requires a live same-root
KSV, matching domain/table, kind 1, a LIVE ledger entry and an actual SDK event
kind/liveness check. A valid event copied from outside this domain is not owned.
A mutex or other kind is never closed as an event. Raw/pseudo tokens stay denied.

Before last-alias close, the ledger publishes CLOSING. Native close, metadata
removal and ledger clearing then proceed in order. An interrupted removal leaves
that journal and accepted prefix. Ordinary operations reject CLOSING; owner
shutdown can retry draining it. It never fabricates a live or closed event.

`events_close(&mut self,&Memory,&Handles)` is actual creator-only. Callers must
stop and **actually join** all borrowers first; this component does not track jobs
or prove joins. It returns Busy before changing phase while any other table guard
or reserved creation remains. It then disables new operations, journals/drains
all live, orphaned and interrupted-closing events, removes their matching typed
aliases and releases the ledger. The table remains owned separately: close the
domain before closing its table. Creator/root-validated completed close is stable,
including after the table itself has closed. KSV revocation or an unexpectedly
missing LIVE event is reported even after completed cleanup; errors do not mean
resources were released unless `events_closed()` confirms it. Malformed records,
wrong native kinds or inaccessible backing can require fatal session teardown.
No lost guard, reservation, native event or backing is silently called cleaned up.

## Fixed representation and budgets

`EVD1` is exactly 2,112 bytes, all fields LE u64:

| Offset | Meaning |
| --- | --- |
| 0 | magic 0x31445645 |
| 8 / 16 / 24 | game root / KSV / tagged-table record IDs |
| 32 | actual owner task ID, bootstrap zero allowed |
| 40 | domain phase: 0 active, 1 closing |
| 48 / 56 | reserved zero |
| 64 + 16*n | opaque SDK event ID, n=0..127 |
| 72 + 16*n | row state: 0 free, 1 reserved, 2 live, 3 closing |

FREE and RESERVED require ID zero; LIVE and CLOSING require a nonzero unique ID.
Reserved and closing states are explicit resource obligations. Whole-record
versioned CAS reports stale publication as Failed, never successful transfer.
This record fits unchanged limits of 4 KiB each, 64 records and 64 KiB aggregate.
The unchanged 128 synchronization-object budget includes the table mutex:
one table plus 127 events exhausts it. Metadata rows do not enlarge SDK budgets.
Host resource IDs are opaque and nonreused; original guest tags reuse holes.

## Qualification and remaining integration

`check.py` verifies the pinned public source kit and exact original
`xbox_NtCreateEvent`/`bridge_NtCreateEvent` source slices. No original C is linked
into the checked game; these are provenance pins, not an executed C oracle.
Native Linux32/64 and Windows32/64 SDK components (Windows through Wine) check
24 manual/auto/initial-state combinations, real signal/reset and zero-time waits,
last-alias lifetime, invalid kinds/foreign domains, null-output orphans,
failed-output prefixes, forged records, four actual creating workers with consumed
join proofs, unchanged synchronization limits and revoked-KSV cleanup.

Six maintainer-only fault cases per target interfere through the actual SDK:
stale ledger transfer, stale tag publication after accepted transfer, stale tag
removal after native close, stale final ledger clear, stale reservation clear
following incoming-event release, and revocation immediately after transfer.
Ten deliberately broken native copies must fail. Fault facades and maintainer
runtime files are rejected if packaged even as unused game source; no production
fault hooks or exemptions exist.

The positive signed fixture scans every game helper before injecting bindings,
separately optimizes and audits all eight game objects on each target, then links
trusted runtime/transport objects. Receipts retain exact SDK, source-kit, compiler,
target, shader and audited-object identities with LTO off. Linux32/64 execute in
actual confinement. Windows component execution uses Wine; real Windows
confinement, physical controls, audible audio, GPU fidelity and frame pacing are
not established. There is no game window or full-title gameplay in these tests.

Remaining: shared wait leases and cancellation-safe wakeup/close coordination;
accurate pulse/NT status and previous-state/import behavior; new versioned
context/startup ownership; typed file/thread resources; migrated full native
source/build/launch/renderer/media and actual gameplay qualification. Do not wire
ordinal 189 or other imports as ready from this component alone, reinterpret
KIC2 reserved fields, allow raw native fallbacks, or enable blocked profiles.
SDK identity and installed app remain
`1e15c059ca66907d45e603cec9f993ca54b9ffa66bae72c4a36cee67b944fd3f`.
