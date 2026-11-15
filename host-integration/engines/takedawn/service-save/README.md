# Saveguard file operations

`files.rs` ports the pinned saveguard's file helpers to the host-supplied
dependency-free GamePKG client. `state.rs` implements its bounded open/close,
worker and pending-save transitions without I/O or callbacks. Neither performs
direct filesystem or system calls. They are not yet connected to the engine,
and are not the complete saveguard.

Requires app commit `75d180311f5e29765ad4778f265908daaa0afb31` or a compatible
later SDK. Exact API contracts: `app/docs/SAVEGUARD_SERVICE_HANDOFF.md` and
`app/docs/PORT_SERVICES.md`. Validation on 2026-11-14: all three service tests,
all four target builds, and all four source rejection cases passed.

The combined file/transition component also passes all four target audits.
Fifteen transition tests pass, including a differential test comparing 4,096
policy events in 128 deterministic traces against unchanged pending-policy
functions extracted from the hash-pinned C saveguard. These are synthetic state
events, not interrupted disk writes or actual game-save recovery tests.

Contracts:

- `copy_shadow(grant, real)` copies the complete opened real file into
  `real + ".~new"` with a bounded 64 KiB buffer, truncating only that shadow.
  It handles short reads, propagates failures, closes both handles, and removes
  partial shadow output on failure. The real save is never truncated. The caller
  must serialize the path and settle previous pending/queued work first.
- `same(grant, first, second)` compares complete contents in 32 KiB blocks.
  It handles short reads and propagates read/open/close errors. A failed read
  is not interpreted as equal content. Both opened handles are closed.
- `retain_for_flush(handle)` preserves the opened identity with a second opaque
  handle. `sync_close` flushes and closes it even after the title handle closes,
  and reports flush/close failure. It does not bypass revocation or access rights.
- `modified(grant, path)` returns signed floor epoch seconds and nanoseconds.
  Recovery can compare this tuple; errors never become a zero timestamp or
  evidence that a pending save is obsolete. Filesystem time precision remains
  filesystem-dependent.
- `publish(grant, shadow, real)` requests source flush and durable atomic replace.
  `Publication::Durable {}` means the host reported success.
  `Publication::CommittedNotDurable {}` means the source was already consumed:
  do not retry its old name. Other errors propagate to the guard's pending/retry
  state machine. This helper never performs an automatic retry.

Scratch buffers have fixed heap capacities instead of large stack arrays:
the latter required a rejected compiler stack helper on 32-bit Windows.
The object-import policy was not changed to admit it.

## Transition API and worker obligations

- `Guard::prepare_write` reserves an opening and returns its seed/shadow path.
  `opened` acknowledges success or failure using that exact ticket. Up to four
  handles/reservations share a shadow. A failed reopen never deletes a shadow
  that already represents the newest pending save. Stale tickets are rejected.
- `wrote` and `delete_on_close` record title changes. Before closing a service
  handle, the adapter calls `last_handle`; pending opens return Busy. The last
  handle acquires a distinct retained flush handle first. After successful
  service close, `closed` queues it. The state machine never closes handles.
- `next_work` permits one worker action at a time, commits queued saves before
  retry batches, and returns immutable path/counter/history data plus a unique
  token. Perform flush, comparisons, the direct history-hook call if requested,
  and publication off the window owner thread, using the shared file helpers.
  Pass only the observed result to `complete`. Rejected or stale completions
  do not consume the action. `RetryLater` preserves queued work after incomplete
  I/O; the adapter must wait before retrying, not spin on the worker/window.
- Failed replacement records the exact preserved `.~pend` or fallback `.~new`
  path. Reads and the next write seed use that newer file. A close-triggered
  retry batch includes saves that failed during the preceding queued commits,
  matching the original worker. Four failures expose a problem sequence;
  `exit_retry` schedules an unconditional final pending retry.
- A confirmed missing pending file can complete a retry as `Missing`; a generic
  I/O error must never be converted to missing. Published-but-not-durable is a
  distinct published result and is never queued as an ordinary pending retry.
  Its notice persists until explicit acknowledgement. Returned obsolete paths
  may be cleaned up only after the newer complete save has superseded them.
- Startup's disk scanner first compares timestamps, attempts publication and
  preserves history where required. `recovered_pending` registers only a newer
  complete file still blocked there; it warns immediately and permits one more
  close-triggered retry. The scanner/history operation is still to be ported.

Recovery capacity is reserved before writing a new shadow: at most sixteen
distinct active-or-pending paths. Full tables and a fifth handle return Busy,
without the old guard's unprotected in-place fallback. This deliberate
backpressure preserves the whole-save contract under resource exhaustion.
The adapter must supply canonical title-relative paths; these transition checks
do not replace the host's portable-path/capability containment enforcement.

Run from the repository root against the current app library:

```sh
python3 host-integration/engines/takedawn/service-save/check.py --app /path/to/app --app-lib /path/to/debug/deps/libgamepkg-HASH.rlib
```

The maintainer harness uses the app's actual Session and exact unmodified client
over private local pipes. It tests whole/short reads, content comparison, bounded
quota failures and partial-output cleanup, retained flush, atomic publication,
path/error handling and repeated operations to catch handle leaks. A separate
synthetic `-6` response checks explicit publication handling without pretending
to cause a real disk durability failure. The same production Rust source is
scanned before binding injection, optimized and object-audited for Linux64,
Linux32, Windows64 and Windows32; closure/macro/alias/filesystem attacks must
remain rejected. Python and the harness are maintainer tools, not player inputs.

Still required: connect these transitions to the actual disk adapter and reviewed
worker task; port the startup sweep/history and direct title-hook dispatch;
connect bounded shutdown waits, then integrate the engine and test real
interruption/recovery behavior. Source kit, recipe and profile admission remain
unchanged. These tests do not establish actual Burnout saves, gameplay, Windows
runtime, audio or pacing.
