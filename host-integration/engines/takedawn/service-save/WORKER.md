# Reviewed save worker

`worker.rs` is port-owned, dependency-free Rust. It owns its `SaveGuard`, startup
recovery, history copies and opened save handles in a reviewed static task. It
uses no game mutex, callback, function pointer, global mutable state or OS API.
The actual generated Burnout kernel/title still needs to call this component.
It is not a complete Burnout source-kit or launcher integration.

App commit `f1dd3fb8359f6524d7a5ca00f7a282b99a9f124b` supplies its bounded
child-local message primitive. Exact generic SDK contracts are in
`app/docs/PORT_SERVICES.md`, local operations 30–33. Register a root entry like
`pub fn save_worker(argument:u64)->u64 { worker::run(argument) }` in the host's
reviewed `Plan` task table. The port source is checked before generated bindings,
then separately optimized and object-audited before trusted runtime linking.
The host task index passed to `Connection::launch` is not a function pointer.

## Connection and wire contract

`Connection::launch(entry, save_grant, title, stamp)` creates its request/reply
queues, sends configuration, then starts the reviewed task. A failed spawn
releases both queues. Poll the sequence-1 startup reply before submitting title
requests. Startup recovery completes before that reply and before any title I/O.
Only the supplied eight-hex-digit title's UDATA/TDATA paths are accepted.
The supplied grant must be a host-listed private writable save grant.

`submit(operation, payload)` returns a sequence number. One request may be
outstanding at a time; another returns Busy. `poll(milliseconds)` returns
`None` on timeout or a `Reply { sequence, status, bytes }`. Use zero milliseconds
in a frame/input loop. Waiting is confined-child work, never GamePKG's window
owner thread. Callers must bound startup/request observation and handle a failed
or unresponsive worker; no reply is equivalent to a successful save.

Every request is LE operation u32, sequence u64, payload; every reply is LE
sequence u64, status i32, payload. Status is zero or the standard negative SDK
error. Sequences begin at 1, increase without reuse and are checked at both
ends. Invalid sequence/header aborts the task, rather than dispatching ambiguous
work. The configuration request is exactly 51 bytes: operation 0, sequence 1,
reply queue u64, save grant u64, title bytes[8], timestamp bytes[15]. No native
packing or pointers appear in the protocol.

| Operation | Payload | Successful reply payload |
| --- | --- | --- |
| 1 OPEN_WRITE | relative UTF-8 path | title file handle u64 |
| 2 OPEN_READ | relative UTF-8 path | read handle u64, using pending read redirection |
| 3 WRITE | title handle u64, offset u64, bytes | empty |
| 4 RESIZE | title handle u64, length u64 | empty |
| 5 CLOSE | read/title handle u64 | empty; title close only, not durability |
| 6 DELETE_ON_CLOSE | title handle u64 | empty |
| 7 STATUS | empty | 68-byte status record |
| 8 STAMP | YYYYMMDD-HHMMSS bytes[15] | empty; Busy during an extracted job |
| 9 SHUTDOWN | empty | final status record; Busy if work/errors remain |
| 10 OPEN_FILE | flags u32, disposition u32, relative UTF-8 path | opened handle u64 and disposition information u32 |

`OPEN_FILE` accepts `FILE_OPEN` (1), `FILE_CREATE` (2) or `FILE_OPEN_IF`
(3). Flags bit 0 grants a guarded writable handle and bit 1 requests
write-through on that handle. Read-only opens accept `FILE_OPEN` only.
Successful information is 1 (opened) or 2 (created). A zero handle with
information 4 means a create collided with the existing logical file.
Information 5 means a requested existing file was absent. Other failures
return the usual negative service status. Pending redirection is used before
probing for presence. Additional writers see the current shadow. Read-only
opens retain the previous published/pending read behavior. A failed probe
does not become a successful create.

Write-through flushes the checked file handle before acknowledging an open,
write or resize. Mutation stays in the shadow until the final close and
atomic publication. If a write or resize succeeds before a flush error,
the staged change can still be present. The failure is reported and stays
visible in worker status. A later close can commit that shadow after a
successful flush. The caller must not infer rollback or durable shutdown
from the failed acknowledgement.

Read handles may be read through the SDK. Both read and title handles must be
closed through the worker so it can track shutdown correctly. Writes/resize/
delete-on-close require a guard-owned title handle. There are at most 64 open
read handles plus the guard's existing four-per-path/sixteen-path write bounds.
The private file service independently checks handle rights, quotas and scopes.

Messages are at most 65536 bytes. Requests have at most 65524 payload bytes;
a WRITE chunk therefore has at most 65508 data bytes. Split longer title writes
at checked offsets and propagate failures; do not report partial acknowledgement
as a whole successful write. Request depth is 8 and reply depth 4, reserving
768 KiB of the runtime's 1 MiB mailbox allowance. Full queues apply backpressure.
The worker keeps a reply across Busy and waits in bounded 20 ms local intervals,
with cancellation checks and a one-second send deadline. It never spins on
an unchanged I/O failure or holds a host window-thread wait.

The status record is LE: busy u32, pending u32, last worker I/O error i32,
last history error i32, problem sequence u64, problem code i32, uncertain bool
u32, open reads u32, open title handles u32; followed by seven startup counts
u32 (unfinished removed, recovered, archived, pending, older kept, archive
errors, uncertain). Observed I/O/history errors are sticky for this worker
lifetime. A zero busy count alone does not establish durable shutdown.

## Save and shutdown ordering

A successful CLOSE reply is queued before any background flush, history copy
or publication. A subsequent worker step flushes the retained opened identity,
compares complete files, performs the original before-replace history hook and
atomically publishes. History failure is reported without silently suppressing
the title's publication, matching the original void hook. Incomplete I/O retains
the exact Job and retries after a bounded wait. Failed publication keeps the
actual pending/shadow path and the pinned original retry/problem policy.
Source-consumed durability uncertainty is reported, never retried by old name.
Obsolete-file cleanup is also retained across errors.

SHUTDOWN stops admission, gives existing work a two-second drain window, then
always schedules the final pending retry batch and gives that work a separate
two-second drain window. This second window is an explicit bound on the original
final retry phase. Existing jobs are not discarded to select a retry. Open title
handles, pending files, incomplete jobs, observed I/O/history errors and durability
uncertainty prevent a successful shutdown reply. They are reported, not silently
cleared or replaced with direct in-place writes. An unclosed shadow remains
unfinished; the next startup scanner keeps the previous published save.

These are cooperative checks between service calls, not a promise that a stuck
filesystem operation can be interrupted within four seconds. The containing
process owner must enforce any final application-exit deadline. `cancel()` wakes
local waits cooperatively; it does not acknowledge durability. `finish()` polls
and consumes actual task completion, then releases queues, and returns the task
result (0 clean shutdown, 1 reported incomplete/failed startup, 2 task/transport/
cancellation failure). It never treats a running task as joined. Inspect both
shutdown reply and task result. Dropping a Connection is not an implicit save or
join; the engine must explicitly shut it down or terminate its containing process.

## Validation and remaining integration

Eight real-file worker tests cover close-before-publication, flush failure,
complete title history, read/write limits, malformed paths/messages/handles,
failed pending redirection and the original warning threshold, history quota
failure, startup ordering, and post-publication uncertainty without source retry.
Together the suite has 29 actual-file/service tests and 18 transition tests,
including 4096 pinned-C policy events.

Exact production modules including the worker pass source checks, isolated
optimized object audits and trusted linking on Linux64/Linux32/Windows64/
Windows32. Each Linux width executes eleven actual confined worker scenarios:
normal save/history, cooperative cancellation, shutdown with an open write,
rejected bootstrap grant, twenty failed-spawn cleanup iterations followed by a
valid launch, newer pending recovery, older pending history, deletion, unchanged
save, invalid requests, and regular file dispositions with write-through. The previous five process terminations and five
startup recoveries per width also pass. Output is recorded in memory; no sound,
window, real game saves or user media is involved. Windows worker evidence is
compilation/audit only; generic mailbox tests ran under Wine separately.

Remaining: the actual clock/date provider (labels are supplied explicitly here),
retention/import/history UI, original Windows rename retry timing, generated
kernel/title connection, persistent file attributes and complete opened-file
metadata, and engine/media/renderer migration. Physical
controls, audible audio, graphics fidelity and measured game frame pacing remain
unqualified. Legacy build/cache gates remain enabled; these components provide
no engine/media exception or permission to publish an unfinished source kit.
