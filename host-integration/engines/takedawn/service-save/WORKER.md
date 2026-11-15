# Reviewed worker integration contract

App commit `f1dd3fb8359f6524d7a5ca00f7a282b99a9f124b` supplies the missing
child-local copied-message primitive. It is not a connected Burnout worker.
The exact SDK contract is `app/docs/PORT_SERVICES.md`, local operations 30–33.

The host-generated Rust helpers are:

- `local_mailbox_create(maximum: u32, depth: u32) -> Result<u64, i32>`.
- `local_mailbox_send(id: u64, message: &[u8]) -> Result<(), i32>`.
- `local_mailbox_receive(id: u64, milliseconds: u32, reply: &mut [u8]) -> Result<usize, i32>`.
- `local_mailbox_release(id: u64) -> Result<(), i32>`.

Maximum message size is 1..65536 bytes, depth 1..32, at most 16 live queues
and 1 MiB total declared capacity per child runtime. Capacity and bookkeeping
are reserved before game entry. Released queues still referenced by a receiver
remain charged. IDs never repeat; no OS endpoint, raw pointer or callback escapes.
Queues copy bytes and preserve FIFO order. Sends return Busy when full. Receives
wait at most one second, return Busy when empty at the deadline and preserve
the oldest message on a short-buffer error. Empty messages are valid. Task
cancellation, queue release and runtime shutdown wake receivers with Denied.
These waits execute in the confined child, never on GamePKG's window owner.

## Port work still required

Connect a statically reviewed task entry using the existing local spawn/join
services. The worker should own its guard state; title code sends bounded,
correlated requests through these queues. Do not add a mutex crate, closures,
function pointers, global mutable state or direct OS channels to game source.
The guard and file/recovery/history modules remain port-owned game code.

Preserve the existing sequencing: successful title close queues the save;
background work flushes retained file identity, runs the history hook and
publishes. A close acknowledgement must not be advertised as durable publication.
Keep the exact Job across an incomplete I/O error and apply a bounded wait
before retry. Keep source-consumed durability uncertainty explicit. Sequence
startup recovery before accepting title opens. Shutdown must settle or report
remaining work under a deadline, preserve pending saves, and poll/consume the
reviewed task result before releasing its queues. Cancellation is cooperative;
session shutdown can terminate the containing process, not an arbitrary thread.

Clock/date formatting, retention/import/UI, Windows original rename retry
timing, actual worker protocol and full engine integration remain incomplete.
The app's legacy build/cache gates stay enabled. This handoff does not replace
those requirements or authorize an engine/media exception.

## Validation scope

Six runtime tests pass on native Linux64/Linux32 and Windows64/Windows32 under
Wine. They cover bounds, FIFO, stale/foreign IDs, capacity/slot reclamation,
retained in-flight charges, cancellation, release and shutdown. ABI tests cover
malformed messages and short reply buffers. A source-checked, optimized and
object-audited synthetic game exchanges messages with a reviewed static task in
actual confined Linux64/Linux32 children; preparation/link checks pass for all
four targets. Source/object bypass fixtures remain rejected. This is service
evidence, not Burnout gameplay, physical controls, audible audio or frame pacing.
