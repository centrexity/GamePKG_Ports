# Saveguard file operations

`files.rs` ports the pinned saveguard's file helpers to the host-supplied
dependency-free GamePKG client. It performs no direct filesystem or system calls.
It is not yet connected to the engine, and is not the complete saveguard.

Requires app commit `75d180311f5e29765ad4778f265908daaa0afb31` or a compatible
later SDK. Exact API contracts: `app/docs/SAVEGUARD_SERVICE_HANDOFF.md` and
`app/docs/PORT_SERVICES.md`. Validation on 2026-11-14: all three service tests,
all four target builds, and all four source rejection cases passed.

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

Still required: the original guard's bounded entry/handle tables, worker queue,
pending-save retry/read redirection, startup sweep/history, title hooks and
shutdown synchronization, then engine integration and interruption/recovery
tests. Source kit, recipe and profile admission remain unchanged. These tests
do not establish actual Burnout saves, gameplay, Windows runtime, audio or pacing.
