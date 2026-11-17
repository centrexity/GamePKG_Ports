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
  KSV and actual table creator. It allocates one bounded 2,624-byte copied ledger.
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
or reserved creation remains. Registered ACTIVE waits also return Busy before
shutdown phase changes. It then disables new operations, retires completed wait
records, journals/drains
all live, orphaned and interrupted-closing events, removes their matching typed
aliases and releases the ledger. The table remains owned separately: close the
domain before closing its table. Creator/root-validated completed close is stable,
including after the table itself has closed. KSV revocation or an unexpectedly
missing LIVE event is reported even after completed cleanup; errors do not mean
resources were released unless `events_closed()` confirms it. Malformed records,
wrong native kinds or inaccessible backing can require fatal session teardown.
No lost guard, reservation, native event or backing is silently called cleaned up.

## Bounded wait leases

`events_wait_begin(&Memory,&Handles,token) -> EventWait` validates the owned live
native event, selects a free slot in the 32-entry wait graph and allocates one
80-byte WLT1 record while retaining the actual-task table guard. Its record
remains privately owned until publication. `event_wait_id()`,
`event_wait_published()` and `event_wait_closed()` are infallible observations.

`EventWait::event_wait(&Memory,milliseconds:u32)` accepts 0..1,000 ms and attempts
the actual wait once. It publishes ACTIVE ownership into the domain before
releasing its guard, and **releases the guard before waiting** through the typed
SDK event service. Other tasks can signal/reset or remove other aliases while
it waits. Removing the last alias returns Busy while its event has an ACTIVE
lease. Host waits remain worker-only; no window-owner-thread wait is introduced.
This is one bounded SDK wait, not an Xbox indefinite/absolute wait translator.

After the actual wait, live KSV/cancellation is rechecked. The observed result is
retained privately before fallible DONE publication, so an interrupted publication
can be retried without repeating a wait or consuming a second auto-reset signal.
`event_wait_close(&Memory)` must run on every path in the acquiring task. A
published lease records DONE with the retained result (or Denied for an abandoned
pre-wait transaction); an unpublished lease releases its private record. It also
releases any remaining guard. Completed close repeats harmlessly. DONE publication
uses only actual-task/root-bound copied-record CAS: it needs neither a live KSV
nor cancellation-sensitive mutex reacquisition. Actual task cancellation therefore
does not strand the wait graph. Total SDK/session revocation still requires the
outer process owner to terminate/reclaim that session.

Published record ownership belongs to the domain after the accepted list CAS.
The waiter must not release it. Under the table lock, domain operations move a
DONE slot to RETIRING **before** releasing its record, then clear that slot. A
failed final clear retains the exact RETIRING ID; retry permits a missing record
only in that already-journaled phase. ACTIVE missing records are errors. Domain
shutdown and last-alias close do not pretend an ACTIVE lease has finished from a
copied task ID or a task-finished poll. Callers must still consume actual join
proofs and stop borrowers. Lost/forged records or abandoned tasks can require
fatal session teardown. Records and copied event IDs are not immutable authority.

## Fixed representation and budgets

`EVD2` is exactly 2,624 bytes, all fields LE u64:

| Offset | Meaning |
| --- | --- |
| 0 | magic 0x32445645 |
| 8 / 16 / 24 | game root / KSV / tagged-table record IDs |
| 32 | actual owner task ID, bootstrap zero allowed |
| 40 | domain phase: 0 active, 1 closing |
| 48 / 56 | reserved zero |
| 64 + 16*n | opaque SDK event ID, n=0..127 |
| 72 + 16*n | row state: 0 free, 1 reserved, 2 live, 3 closing |
| 2112 + 16*n | wait-record ID, n=0..31 |
| 2120 + 16*n | wait-slot state: 0 free, 1 linked, 2 retiring |

The earlier 2,112-byte EVD1 representation is historical and rejected by this
version; its fields are not silently reinterpreted. Wait IDs are unique, nonzero
when linked/retiring, and zero when free. WLT1 is exactly 80 bytes, LE u64:
magic 0x31544c57 at 0; root/KSV/table/domain IDs at 8/16/24/32; native event ID at
40; actual acquiring task at 48; state ACTIVE=0/DONE=1 at 56; sign-extended i32 SDK
result at 64; reserved zero at 72. ACTIVE requires zero result; DONE retains the
observed result. Task zero is the bootstrap; u64::MAX is invalid. Completion is
not a guest pointer, event authority, joined-task proof or frame-time measurement.

FREE and RESERVED event rows require ID zero; LIVE and CLOSING require a nonzero unique ID.
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
join proofs, unchanged synchronization limits and revoked-KSV cleanup. Actual waiting workers
check concurrent signal, cancellation, late KSV-revocation denial, Busy last-alias
close, consumed joins and record reclamation. Zero-timeout and invalid/duplicate
wait controls exercise exact outcomes. Synthetic schema controls register 32
ACTIVE copied leases to test the bound; this does **not** claim 32 physical
waiting jobs. Global record exhaustion and duplicate lease IDs remain rejected.

Ten maintainer-only fault cases per target interfere through the actual SDK:
stale ledger transfer, stale tag publication after accepted transfer, stale tag
removal after native close, stale final ledger clear, stale reservation clear
following incoming-event release, and revocation immediately after transfer. Wait faults cover stale ACTIVE
publication, stale completion after an actual timeout, stale retirement marking,
and stale slot clearing after actual record release.
Fourteen deliberately broken native copies must fail. Fault facades and maintainer
runtime files are rejected if packaged even as unused game source; no production
fault hooks or exemptions exist.

The positive signed fixture scans every game helper before injecting bindings,
separately optimizes and audits all eight game objects on each target, then links
trusted runtime/transport objects. Receipts retain exact SDK, source-kit, compiler,
target, shader and audited-object identities with LTO off. Linux32/64 execute in
actual confinement. Windows component execution uses Wine; real Windows
confinement, physical controls, audible audio, GPU fidelity and frame pacing are
not established. There is no game window or full-title gameplay in these tests.

The opt-in authenticated KIC3 startup now owns this domain and its table; see
[OBJECT_STARTUP.md](../service-kernel-context/OBJECT_STARTUP.md). Remaining:
accurate pulse/NT status and previous-state/import behavior; typed file/thread
resources; migrated full native
source/build/launch/renderer/media and actual gameplay qualification. Do not wire
ordinal 189 or other imports as ready from this component alone, reinterpret
KIC2 reserved fields, allow raw native fallbacks, or enable blocked profiles.
SDK identity and installed app remain
`1e15c059ca66907d45e603cec9f993ca54b9ffa66bae72c4a36cee67b944fd3f`.
