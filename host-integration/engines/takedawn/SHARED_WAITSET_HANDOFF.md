# Shared mixed-object waits: port integration contract

GamePKG's audited host local runtime now provides the missing atomic multi-object
wait primitive. SDK identity:
`49a313ff062e83e3bcbaadb125f4ace190847b0321cc25877e45f92023f0234a`.
This includes typed cleanup from `SHARED_MUTEX_HANDOFF.md` and invalidates prior
SDK receipts (including waitset-only 9801... and 7baf...); source/object/link
qualification must be rebuilt. No game code moves into the trusted runtime.
The public source kit/catalog, app submodule pin, launch defaults and title gates
remain unchanged. This handoff does not enable actual NT waits or generic close.

Read app `docs/PORT_SERVICES.md`, “Atomic child-local mixed-object waits”, for the
complete host ABI, locking, abandonment, cancellation, limits and error contract.
Generated bindings expose:

- `LocalWaitObject { id: u64, kind: u32 }`: event=1, mutex=2, semaphore=3,
  reviewed task completion=4. Full IDs are revalidated against actual objects.
- `local_wait_many(&[LocalWaitObject], all: bool, milliseconds: u32)
  -> Result<LocalWaitResult,i32>`: 1..64 distinct IDs, 0..1000 ms.
- `LocalWaitResult { index: u32, abandoned: bool }`: WaitAny lowest ready caller
  index; WaitAll 0 normally, or lowest acquired abandoned-mutex caller index.
- Local op 47 exact LE request: count/all/ms u32, then ID u64/kind u32 per entry
  (24..780 bytes, no padding); reply index/reason u32 (8 bytes), reason 0/1.

WaitAll never consumes a prefix when any participant is unready/invalid/revoked
or recursion cannot increment. Auto-events reset and semaphores consume one only
on successful atomic acquisition. Manual events remain signalled. Actual native
thread ownership governs recursive mutexes. Reviewed entry return marks its
remaining mutexes abandoned, resets old recursion and wakes waiters; the new API
acquires/reports abandonment explicitly. Legacy unit-returning acquisition denies
an abandoned mutex without taking it. Do not conceal damaged guarded game metadata
or treat an abandoned mutex as ordinary successful admission.

Task completion is actual native completion and does not join. A successful join
revokes copied completion views and wakes pending mixed sets; no participant is
consumed by a denied set. Stop/join proof requirements remain unchanged. Host
notification epochs protect the check/sleep gap; waits never block the window
owner. Task entries use a sleeping <=1 ms native-completion recheck for the final
host-only exit tail, within the original deadline. Existing quotas are unchanged.

## Required checked port work

1. Implement combined typed wait admission across HTR2/EVD2/SSD1 and future
   file/thread owners. Resolve/validate complete guest arrays and timeout data
   before mutation. Deduplicate/reject repeated underlying IDs according to the
   reviewed original behavior; separate guest aliases do not create new objects.
2. Retain actual active-wait/join/close obligations for **every** participant,
   then release table guards before waiting. No raw ID/state snapshot is a lease.
   Do not serially call event/semaphore waits for WaitAll or synthesize rollback.
3. Keep original absolute/relative timeout, guest time-warp and NT status mapping
   in game code, using host monotonic/deadline services and bounded cancellable
   wait slices. Host Busy is not automatically the final NT timeout for a longer
   or infinite guest wait. Preserve ignored arguments only where pinned source
   actually ignores them; handle pseudo/synthetic tokens in reviewed game code.
4. Preserve original `xbox_wait_result_to_ntstatus`: ordinary object indices map
   to success+index, timeout to STATUS_TIMEOUT, I/O completion to STATUS_ALERTED,
   WAIT_ABANDONED_0 to STATUS_ABANDONED, failure/default to STATUS_UNSUCCESSFUL.
   The pinned converter's abandonment handling does not cover every offset;
   record a deliberate compatibility correction separately if one is necessary.
5. **Alertable APC dispatch is still a demonstrated integration gap.** Op 47
   does not execute APCs, arbitrary callbacks or raw file waits. Establish the
   actual pinned APC/async-file requirements and request generic wake/interrupt
   services where existing reviewed queues/timers cannot satisfy them. Never
   silently ignore alertable behavior or admit a partial engine as complete.
6. Generic NtClose must account for async overwrite completion, thread-object
   detachment, aliases, orphan owners and all actual resource types. An event/
   semaphore-only implementation cannot qualify ordinal 187 for the title.
7. Compare actual 233/234 (and Ke multiple waits) to pinned original C, including
   CPU/stack/status/guest-memory prefixes, mixed kinds, abandoned mutexes, stale
   views, cancellation, partial failures and teardown. Complete source scan,
   separate optimized objects, all-object audit and exact new-SDK receipts apply.

## Evidence scope

Native SDK/runtime, exact ABI and malformed-reply tests pass Linux32/64 and
Windows32/64 components through Wine; twenty altered runtime/binding/provider
copies compile and then fail. The signed ordinary game fixture passes complete
source and object checks on four targets and actual confined Linux32/64 execution,
with no owner dispatch until its final report. It exercises retained task waits,
atomic mixed acquisition, abandonment, cancellation and malformed calls.
This is infrastructure evidence, not native title entry, physical Windows controls,
audible audio, hardware visual fidelity or measured frame pacing. The KIC4 main
import gap remains 102 until checked actual adapters are qualified. Neither
Burnout nor Halo admission is enabled by this prerequisite.

## Original-source provenance for the next adapter

These pins identify reviewed source, **not a passed NT wait/close oracle**.
Source-kit SHA-256: `6b37d24c0984c686c7ed6fd73cd3a5b252b5631ceefa2798e5b7a6e0adff8dc4`.
Ranges are byte offsets [start,end) in the original UTF-8 member.

| Function | Source member | Bytes | SHA-256 |
| --- | --- | --- | --- |
| `bridge_NtClose` | `toolkit/src/kernel/kernel_bridge.c` | 46872..47516 | `36c911bb5a383ef19da2e110b8968d41294a2b70aa7e4b949649936f4d1eadec` |
| `bridge_NtWaitForSingleObjectEx` | `toolkit/src/kernel/kernel_bridge.c` | 85066..85737 | `4e855511a63d5464d493404fd11061c319418fa334c96de0e3ae28679b680d48` |
| `bridge_NtWaitForMultipleObjectsEx` | `toolkit/src/kernel/kernel_bridge.c` | 87759..89061 | `f10f9ec3655c7df32e4faec891406c161708fc0d88a1f019fb2df9a65b737b80` |
| `wait_one` | `toolkit/src/kernel/kernel_sync.c` | 2308..2731 | `9ea260c1d8724f9cdb50cdfe8b1f81a00ee064a9ad0ff364288fdae2069dbf98` |
| `wait_many` | `toolkit/src/kernel/kernel_sync.c` | 2733..3217 | `c8ebf0fe5678b9a7c6cd89081e3c6c68721d46325f51b903f54b91af6cda9861` |
| `xbox_wait_result_to_ntstatus` | `toolkit/src/kernel/kernel_sync.c` | 7044..7592 | `b85a28e82b09982f29158ec99ce87349a41988f5294a9d2040b8b46cfb94e472` |
| `xbox_nt_timeout_to_ms` | `toolkit/src/kernel/kernel_sync.c` | 1059..1811 | `ec3b7369a6dac4951e7f16fde535cae8a6554e508f1e9f9622f6d336bf574900` |
| `xbox_NtWaitForSingleObjectEx` | `toolkit/src/kernel/kernel_sync.c` | 7867..8417 | `7462f6824d810f38e581b55f3a5d9018f129d4f13e638cb39d075afa42ba9f31` |
| `xbox_NtWaitForMultipleObjectsEx` | `toolkit/src/kernel/kernel_sync.c` | 8419..8908 | `85894e429b78430e971cb41758ea7c95d34c8e8c7c32b13866cdaf5abaf62915` |
| `xbox_KeAlertThread` | `toolkit/src/kernel/kernel_thread.c` | 10690..11082 | `c680543c2ce22f5dd6c27aac52127ebdc56e42f69f1e947986ec449eb8e6e8ee` |

The pinned `xbox_KeAlertThread` queues the empty `xbox_alert_apc` through
`QueueUserAPC`. This is concrete evidence for a game-thread alert/wakeup path
when completing alertable waits; current op 47 does not provide that dispatch.
Keep alert queues/guest callback translation in checked port code and request
a narrowly scoped generic interrupt mechanism rather than exposing OS APCs or
arbitrary function pointers. `wait_one`/`wait_many` also slice time-warped finite
waits against the guest clock; zero/infinite and unwarped waits use their direct
Windows variants. Preserve those distinct time bases and alert results using bounded host waits;
repeated Busy is not successful completion. Complete typed ownership/status/CPU comparisons before
enabling 233/234 or the close path.


## Checked event/semaphore consumer checkpoint

`service-kernel-mixed-waits/README.md` now defines the checked combined EVD2/SSD1
ownership transaction, its failure-prefix and guard-retirement contracts, and
authenticated KIC4 mode 82. It supplies the event/semaphore portion of items 1/2
above without new SDK authority or larger quotas. Other typed resources, actual
NT timeout/status/CPU conversion and alert/APC integration remain required;
233/234/187 are not admitted by this component. The current KIC4 main gap remains
98 after the separately qualified RTL batch; the earlier 102 count above is the
historical waitset-prerequisite checkpoint. Full native title qualification is
still incomplete.


## Retained event/semaphore logical waits

The mixed-owner API now adds `mixed_wait_slice` and `mixed_wait_timeout`; see
`service-kernel-mixed-waits/README.md` for the exact state/error contract. Only
actual Busy plus a successful live check permits another slice, keeping all
original leases ACTIVE. Final guest timeout publishes Busy without another
native acquisition. Success, uncertain/protocol failures and post-wait liveness
errors are terminal. The one-shot API remains terminal even on Busy. NT clock,
status/CPU, alert/APC, mutex/file/thread and generic-close integration is still
port work; no ordinal or title admission follows from this component.
