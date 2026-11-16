# Raw shared dispatch gate

Port-owned dependency-free Rust over an actual GamePKG recursive mutex. This is
the raw lock/owner/depth component for the forthcoming IRQL/IRQ adapter, **not**
`xbox_DispatchGateLeave`: callers still need the original safe point before raw
release and the release point afterwards. It does not deliver an IRQ, execute a
DPC, change IRQL, publish PCR bytes or enable Burnout.

`Gate::gate_new(&Memory)` owns one local21 mutex and one 64-byte copied record.
It validates the root/KSV3 binding before and after allocation; failure releases
created resources. `gate_id()` exposes only the record ID. `gate_borrow(&Memory,
id)` returns a nonowning view, validates metadata and proves the mutex kind using
local26. A signaled event cannot replace the lock or lose its signal.

`gate_enter(&Memory, timeout_ms)` uses typed local26 with 0..1000 ms. Success
holds a real recursive mutex across calls and publishes actual reviewed task ID
and checked depth. The bootstrap task ID zero is legitimate: `[0,0]` is free,
`[0,n]` is bootstrap-owned. `gate_state` returns `[owner,depth]`. IDs remain u64;
there is no native thread-ID truncation. Contention is Busy, never permission to
continue; invalid timeout/kind is Invalid. Missing/foreign/revoked binding or a
cancelled task is Denied. Depth overflow or interfering CAS is Failed; no quota,
source/import exception or fallback is added.

`gate_leave_raw(&Memory)` publishes one decrement and releases one depth on the
same actual owner thread. Wrong-thread/empty release is Denied. This is explicit
cleanup: cancellation or KSV3 revocation does not prevent an already-owning task
from releasing. New work remains denied. The task must catch cancellation from
mailbox waits and release before returning; cancellation does not abandon a lock.

`gate_close(&mut self,&Memory)` is creator-only. It acquires zero-time typed
ownership before checking depth/releasing metadata, so close cannot revoke an
actively published gate. Busy preserves an existing recursive hold. It releases
record then mutex, retaining a partial cleanup handle on failure. Once closed it
is idempotent; `gate_closed()` reports that state. Revoked KSV3 still permits
cleanup, then returns Denied. Borrowers cannot close. Stop/join reviewed users
before unloading their code; this object does not replace their join barrier.

GAT1 is eight LE u64 fields: magic 0x31544147, root ID, KSV3 ID, mutex ID, owner
ID, depth (0..u32::MAX), zero, zero. Depth zero requires owner zero. Every read
checks exact size, magic, reserved/range and immutable identity. Runtime services
own lock lifetimes; no native pointer, OS function, callback or library enters
game code. Partial publication/unlock errors require abort/report, not a fabricated
successful transition. Arbitrarily corrupted shared data is not repaired.

`check.py --app APP --app-lib LIB --wine` runs 32 pairs of normal/cancelled worker
lifecycles per architecture. It covers recursive acquisition, actual contention,
wrong-thread release, creator/borrower and stale/foreign binding, close while held,
forged event, excessive timeout, explicit cancelled cleanup and revocation while
held. All four targets execute components (Windows through Wine); complete source
scan and every optimized object audit precede trusted linking. Actual confined
Linux32/64 repeat the checked worker fixture. None of these establishes original
IRQ ordering, PCR/IRQL behavior, callback delivery, graphics/audio or gameplay.

Continue with `../service-native/IRQ_SERVICE_HANDOFF.md`; do not wire raw release
as final GateLeave or promote a whole engine into trusted code. Upstream source
and notices remain in the pinned kit and `../service-native/UPSTREAM_LICENSES.txt`.
