# Checked native timer-table component

This is dependency-free port code, not trusted host runtime or a playable-title
checkpoint. It retains the original 32 kernel timer slots and ordered scan,
using GamePKG records, typed recursive mutexes, actual reviewed task identities
and host-owned clocks. No OS/thread/SDL/GPU/file/network access, callback pointer,
new SDK operation, crate dependency or admission exemption is introduced.

## Contracts

- `Timers::timers_new(&Memory, &Queue)` owns one mutex and one 832-byte record,
  bound to the current root/KSV3 and that queue's opaque record ID. Allocation
  failure releases a newly created mutex. `timers_id` returns only the record ID.
- `timers_borrow(&Memory, id)` is nonowning. It validates schema, root/KSV3,
  reserved bytes and a typed mutex acquisition; an event is rejected without
  consuming its signal. `timers_state` returns a copied complete record.
- `timers_start(&mut self, &Memory, &Queue)` is creator-only and one-time. It
  binds the actual worker already assigned to the same DPC queue. Prepare both
  components while that worker is suspended, then resume it. A failed start
  after selecting the worker retains the join requirement; stop/join and close
  this owner rather than discarding partial ownership or starting another worker.
- `timer_set(&self, &mut Memory, timer:u32, due:i64, period:i32, dpc:u32)`
  returns `[was_set, stored]`. It prefers an existing matching timer over the
  first empty slot, samples the clock **inside** the mutex only when a slot is
  available, and publishes before unlocking. Negative due values truncate
  relative 100ns units toward zero in milliseconds; nonnegative due values are
  immediate, matching the pinned bridge's existing absolute-time limitation.
  Period is an explicit signed Xbox 32-bit LONG on every target. A full table with no matching timer
  returns `[0,0]` without sampling the clock. Null timer matches the first empty
  slot (`was_set=1`) and preserves the original stale DPC/due/period fields.
- `timer_cancel(&Memory, timer)` returns false before start, otherwise clears
  **all** matching timer VAs under the lock without clearing stale slot fields.
  Null cancellation matches empty slots, including the original return value.
  This is the firing-table half, not the complete KeCancelTimer bridge/export.
- `timer_fire_begin(&Memory, now:i64)` creates a caller-retained `Scan` only in
  the assigned actual worker. Keep one scan for an original firing pass. It
  holds no mutex between calls. `timer_fire_next(&Memory, &mut Scan)` scans in
  original slot order and returns `Some([timer_va,dpc_va])` only after publishing
  periodic rearm (`now + positive period`) or one-shot removal and unlocking.
  Null DPCs still count as fired timers. Non-due armed slots and periodic rearm
  update the original signed `next_due=-1` accumulator, including its negative
  sentinel behavior. `None` means all 32 slots have been visited.
- Between returned firings, the caller must execute a nonzero DPC through the
  existing native callback adapter, then signal the current shadow event and
  publish guest SignalState=1 **only if a mapping exists**, in original order.
  Callbacks run outside the table mutex and can rearm/cancel any slot. This
  cursor does not execute or acknowledge callbacks or substitute event behavior.
  An error after slot publication is a fatal partial prefix: abort/report the
  scan and stop that actor; blindly retrying a firing can duplicate side effects.
- `Scan::timer_scan_result(&Memory)` returns `[fired_count,next_due_bits]` only
  after a complete scan; partial is Busy. Every use rechecks actual worker,
  live record/schema, root/KSV3 and cancellation. `timer_scan_abort` makes future
  use Failed. A failed scan step poisons the cursor, while wrong-owner access
  cannot change it. Dropping a scan releases no resource and fabricates no
  completion; table mutations already published are not rolled back.
- `timers_close(&mut self, &Memory, Option<&queue::Joined>)` is creator-only.
  An assigned worker requires a witness from that queue's **actual consuming
  join**, not merely a task-finished flag. The witness validates root, original
  queue, worker and creator without joining again. It remains usable after the
  queue closes. Stop all producers before close. Close releases the record and
  mutex even after KSV3 revocation, then reports Denied; `timers_closed` exposes
  completed cleanup. Partial cleanup retains remaining resource IDs for retry.

The queue adds `queue_worker_binding(&Memory) -> [queue_id,worker_id]` and
`queue_join_witness(&Memory) -> Joined`. Borrowed queues cannot obtain witnesses.
A witness is a private-field Rust value, not a host handle or creator-supplied
completion flag. `queue_join` keeps its existing result and consumes exactly
one native task join; a second join remains Invalid. The witness does not keep
any SDK object alive, allocate quota or make arbitrary game jobs joinable.

Errors retain SDK meanings: Invalid -1, Busy -2, Failed -3, Denied -4,
Unsupported -5. Source admission precedes generated bindings, every optimized
game object precedes trusted linking, and existing record/task/sync/process
budgets apply. A CAS conflict despite the mutex is Failed, never silently retried.
Locks are explicitly unlocked on errors. No lock ownership or game-supplied
identity substitutes for the actual reviewed task.

## Record and clock

`TMR1` is exactly 832 little-endian bytes. Header u64 fields: magic
`0x31524d54` at 0, root at 8, KSV3 at 16, mutex at 24, queue at 32, worker at 40,
started boolean at 48 and reserved zero at 56. Each of 32 24-byte slots at 64
has timer VA u32, DPC VA u32, due-ms signed i64 bits, period signed i32 bits,
and reserved zero u32. Empty slots retain their old fields except timer VA.

Production `clock.rs` samples the host counter before loading shared skew.
`timer_ms_value` preserves the original separate truncations:
`(counter/frequency)*1000 + (counter%frequency)*1000/frequency + skew_ns/1000000`.
Converting a combined guest nanosecond value to milliseconds can round across
a boundary differently and is not equivalent. Unsupported/invalid clock samples
are errors, never a guessed clock. Fixed-width wrapping negation/addition define
Rust arithmetic without architecture-dependent `long`; C differential claims
are limited to the tested native input domain, not a claim about C signed UB.

## Validation and remaining integration

`python3 check.py --app APP --app-lib CURRENT_APP_RLIB --wine --evidence DIR`
uses hashed original source-kit members for set/cancel/fire and host/guest
clock bodies. Reference instrumentation observes slot state immediately after
original publication/unlock and before its callback. The shell normalizes the
original period parameter to explicit Xbox signed 32-bit width, keeps one cached
frequency per original clock instance and uses controlled literal callbacks.
Its shadow lookup is deliberately absent; **no shadow-event fidelity is claimed**.

512 original operations per target compare all 768 slot bytes, results, clock
read counts and callback-order/state traces. Callbacks rearm the current timer,
arm another slot or cancel a later slot. Another 64 original clock witnesses
cover separate fractional truncations. Malformed/stale/foreign schemas, wrong
lock kind without signal consumption, pre-start behavior, missing/wrong join
witnesses, CAS interference, clock errors, scan abort and revoked-root cleanup
are checked. Required-failing controls change relative rounding, periodic rearm,
null cancellation, omit firing publication, forge a premature join witness or
combine clock/skew truncation. All four source/object builds and
confined Linux x64/i686 fixtures are separate from Windows Wine component runs.
They establish neither real Windows confinement nor gameplay/physical input/
audible audio/GPU fidelity/measured frame pacing.

The complete migration still needs the original shadow-object map and timer
initialization/reset/signal/cancel bridge ordering, lazy first-set worker startup,
actual retained-stack DPC callbacks/profiling, timer/vblank loop and spin/TIB/
native-context setup. Keep the existing event quota and explicitly handle
admission failures; no raw event fallback or silently reduced shadow capacity.
No generated source-kit/import closure, recipe, launch default or profile gate
is changed here. Burnout and Halo remain unadmitted pending their full paths.


The checked composition in [FIRING.md](FIRING.md) now connects these primitives
to the actual native callback adapter and current shadow-event lookup. It
provides fatal-prefix actor cleanup and preserves callback-outside-lock order;
production handlers and the complete worker loop remain required.
