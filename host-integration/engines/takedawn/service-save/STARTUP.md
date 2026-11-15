# Startup recovery and history export

`recovery.rs` runs before title activity. `Recovery::begin` borrows the fresh,
idle SaveGuard exclusively, verifies the selected grant, scans only the supplied
canonical title ID under UDATA and TDATA, and validates candidate paths before
mutation. Errors keep the same candidate for retry. It removes unfinished
`.~new` siblings and atomically publishes newer/equal complete `.~pend` files.
Published-but-not-durable status is counted and exposed as a notice; its source
has already been consumed and must not be blindly retried.

Blocked newer or older pending files return `RecoveryStep::History`. Use a
direct call to `history::archive_pending`, then acknowledge the exact returned
`Archived` proof with `history_done`. Wrong-grant/path proofs fail. The proof
also rechecks its completion marker and compares the archived pending bytes
before an older pending file may be discarded. A changed or deleted snapshot
cannot authorize deletion. `history_failed` keeps older files beside the real
save, or registers blocked newer files for read redirection/retry and warns
immediately. Always inspect the final Report, including uncertainty and history
failures. No generic I/O error is interpreted as absence.

`history.rs` copies the complete title tree into a fresh `save-history/<stamp>`
folder. Only UDATA/TDATA for that title are included; root settings, partition
images, other titles and temporary/pending siblings are excluded. For recovery,
the pending file replaces the corresponding real-file content **inside the
snapshot**, leaving the active real save unchanged. Each copied file is flushed
and byte-compared. `save-export.txt` is published last, with the original title,
made and note fields. Migrated listings require that marker. An interrupted
copy is not advertised as complete merely because some UDATA files exist.

`archive_before_replace` provides the same full-title snapshot for the normal
before-replace hook. Labels are supplied in YYYYMMDD-HHMMSS form; collisions use
suffixes without overwriting an old export. Timestamp acquisition/formatting,
retention/pruning, imports and original UI integration remain to be connected.
These flushed copies are not a filesystem-wide power-loss transaction.

Both scanning and export fail explicitly at resource bounds (32 levels, 10,000
entries, 1 GiB copied work, 99 same-label destinations), in addition to host
quotas. No incomplete tree is returned as a successful snapshot. Failed exports
attempt explicit cleanup through file removal and **empty-only** directory
removal; filesystem cleanup failure can leave an uncompleted folder. No proof
is issued on export/publication failure, including -6.

Production source is `paths.rs`, `files.rs`, `state.rs`, `adapter.rs`,
`history.rs`, and `recovery.rs`. `paths.rs` uses bounded checked segmentation,
without out-of-line standard-library search/panic imports. None of the modules
contains game-owned crates, aliases, macros, indirect callbacks or platform
calls. Host-generated bindings are still injected only after source admission.
Required app: `5bfdf4dd8d4ef9759549bb376757f65906f6a699` or a compatible SDK.
See `app/docs/STARTUP_RECOVERY_HANDOFF.md` and `PORT_SERVICES.md` for operation 18.

## Validation

The existing `check.py --app ... --app-lib ...` command now runs:

- 18 transition tests, including 4,096 events compared with unchanged pinned C.
- 21 actual-file/service tests, including full title-tree snapshots, pending
  overrides, changed proofs, quota cleanup, denied grants and synthetic I/O faults.
- Exact production source admission and separate optimized object audit for
  Linux64/Linux32/Windows64/Windows32; source bypass cases remain rejected.
- `native_probe.rs`: actual audited/confined Linux64 and Linux32 children, five
  process terminations and five subsequent startup recoveries at each width.
  Checkpoints cover opened shadow, first write, queued close, flushed before
  publication, and completed publication. The expected whole save survives;
  unfinished shadows disappear. Test-only output is recorded in memory.

`native_game.rs`, `native_probe.rs`, test includes and the C oracle are fixtures,
not production engine units. The native probe runs a synthetic main entry, not
the final save worker or actual Burnout. Process termination is not power loss.
Windows evidence is target preparation/audit, not execution on Windows. No real
game saves, ROMs, physical audio/controls, graphics or frame pacing are qualified.

Still required before enabling the game: reviewed worker task and bounded
shutdown connection; clock/retention/import/UI fidelity; full source, renderer,
input/audio and launcher migration; actual Linux/Windows gameplay and save tests.
The source kit, catalog recipe and legacy profile gate remain unchanged.
