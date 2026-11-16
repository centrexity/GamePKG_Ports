# Checked display and vblank schedule

Port-owned, dependency-free Rust translation of the pinned kernel_pacing.c
display-learning and nanosecond vblank schedule. The shared-record consumer runs
on a reviewed GamePKG worker using the existing clock/wake adapter. It neither
raises an IRQ nor claims that the complete title timer/DPC thread is migrated.
The recipe, source kit archive and legacy admission gate remain unchanged.

The ten original C functions and their global declarations are pinned by exact
byte ranges/hashes in reference_inputs.json. Original code remains a separate
maintainer oracle. Its platform/environment/logging shell is never admitted as
game code. Preserve ../service-native/UPSTREAM_LICENSES.txt with derived source.

## Pure scheduling API

`Pacing::pacing_new(mode:i32)` accepts -1 (no explicit setting, presenter auto may
select), 0 (forced off) or 1 (forced display lock). `pacing_auto(bool)` preserves
forced settings. `pacing_locked()` returns the selected mode. `pacing_refresh`
accepts the original report timestamp/period ns values, including future refresh
timestamps, missing measured periods and invalid stated periods with the original
ignore/anchor behavior. It preserves measured-period filtering, shorter-period
candidates, neutral missed frames, sustained/consecutive rate-change detection,
irregular stated-report suppression/recovery and late-report anchor smoothing.

Refresh returns original diagnostic conditions as flags: 1 sustained changed
rate, 2 consecutive changed rate, 4 first irregular stated report. The caller
emits bounded GamePKG diagnostics after publishing; there is no native stderr.
`pacing_relearn()` defers dropping learned data to the next enabled report.
`pacing_rate_changed()` consumes the original one-shot flag. Relearning preserves
the original diagnostic-once and pending-rate-change behavior.

`pacing_step(now_ns) -> Result<[u64;3],i32>` returns due 0/1, next edge, restart
lateness. It preserves exact 60 Hz integer-grid arithmetic, the >100ms restart,
250ms display-report staleness, 1% display tolerance, display-refresh snapping
and continuation of the nominal grid when reports stop. Original unsigned
operation order/wrapping remains explicit. The repeated positive-period catchup
adds are calculated directly; an unrepresentable/nonterminating catchup returns
Invalid without changing state rather than hanging a game worker. No frames or
IRQs are silently emitted or skipped to handle that error.

Private fixed-width `pacing_encode/decode` uses 104 LE bytes:

| Offset | Field |
| --- | --- |
| 0 | u32 VPC1 magic 0x31435056 |
| 4..44 | eleven i32: explicit mode, selected lock, candidate count, off count, off/on runs, jitter, relearn, rate changed, diagnostic count, started |
| 48..96 | seven u64: report reference, learned period, candidate period, off/on sums, grid epoch, vblank count |

Malformed sizes/magic/modes/boolean fields/negative bounded counters reject.
Learned periods accept conservative reachable bounds 1.875..100ms: original
2..50ms stated input bounds are not invariants after smoothing. Candidates accept
2..100ms. Checks must not reject valid smoothing across the stated-input limits.
Both lower and upper crossings are included in original-C differentials.

## Shared-record ownership and worker integration

`shared::Schedule` uses the supplied `gamepkg`, checked `memory`/`kernel_pool`,
`pacing` and existing `wait_service`. No SDK/host implementation changes or
additional quotas. Require native SDK `c4edee3e44be562b`, full identity
`10c4f4ca7ea37783c08ea2cafd1b129ca9b3160cbe4d3cf30d6c1a5da8c51e88`.

- `schedule_new(&Memory,mode)` creates one 144-byte copied record and an owning
  controller bound to its creating reviewed task/child entry.
- `schedule_id()` is an opaque record ID. `schedule_borrow(&Memory,id)` creates a
  nonowning view only after validating live root/configuration and full metadata.
- Start the worker suspended. `schedule_assign(&mut self,&Memory,task)` validates
  its actual live reviewed task ID, publishes it by bounded CAS, then remembers
  the binding privately. Resume only after publication. Zero, missing/finished
  task IDs, rebinding, and assignment by a borrowed view reject.
- Only that assigned worker may call `schedule_step(&Memory)`. It samples real
  shared guest ns through wait_service and returns `[now,due,next,late]`. Root/
  configuration is checked around sampling, including errors, so revocation does
  not become a new unconfigured owner. A failed update does not advance state.
- Same-root reporters may call `schedule_auto`, `schedule_refresh`,
  `schedule_relearn` and `schedule_changed`. Complete-record CAS updates retain
  concurrent display and worker changes. Retry at most 64 times with the existing
  local 1ms retry pause; exhaustion is Busy. `schedule_state` returns a copied
  pure-state encoding, not a lease/pointer or mutable native object.
- Stop/cancel the worker. Creator-only `schedule_join(&mut self,&Memory)` consumes
  its actual nonblocking local join and returns the result; Busy retains owner
  resources. Never consume that assigned job through another join path.
- `schedule_close` refuses an assigned, unjoined job, even if it already finished.
  After the real join it revokes the owned record. Configuration revocation is
  reported after cleanup; `schedule_closed()` then returns true. A borrowed view
  cannot assign/join/close the owner. Runtime teardown is final cleanup if normal
  ownership or transport has been lost.

The 144-byte record is five u64 header fields (PCS1 magic 0x31534350, root ID,
KSV3 owner ID, assigned task ID, assigned 0/1), then the 104-byte pure state.
Unassigned means task zero; assigned means nonzero. Root/KSV3/CPU schemas and
their existing helper hashes remain unchanged. One timer worker owns the lane;
keep periodic KeTickCount on its separate event wait. Every metadata/role/error
path is scoped to this child and these opaque records.

Invalid (-1), Busy (-2), Failed (-3), Denied (-4) and Unsupported (-5) propagate
without a direct platform fallback. Ordinary accepted operations may complete
concurrently with subsequent owner revocation; this is not a transaction with
arbitrary teardown. Stop/join is the completion barrier before resource release.

## Qualification and remaining work

```sh
python3 host-integration/engines/takedawn/service-kernel-pacing/check.py \
  --app /path/to/app --app-lib /path/to/debug/deps/libgamepkg-HASH.rlib --wine --repeat 32
```

The checker compares 4,096 operations in each of auto/off/on modes: 12,288
original C state/output witnesses per target, including diagnostic conditions.
The C-local diagnostic count is mirrored for serialization; its actual original
first-warning output is captured independently. All other scheduling/display
fields come directly from unchanged C functions. Explicit cases cover 120-run
and three-run rate changes, candidate periods, eight unmatched steps, missed
frames, jitter recovery, exact staleness/tolerance edges, pause restart, learned
periods beyond stated bounds and malformed/overflow rejection.

The worker fixture exercises real local SDK clocks/tasks/records, assignment
before resume, concurrent display updates, shared skew pause restart, record
quota exhaustion, foreign/borrowed/unassigned/wrong-worker roles, corrupted
metadata, cancellation, revoked configuration, nonblocking join and cleanup.
Its synthetic display reports do not come from a display device.
Qualification runs 32 worker/lifecycle suites per target. A separate maintainer
hook revokes configuration during the actual clock read; the assigned worker
must receive Denied with its complete scheduling record/version unchanged. A
temporary regressed copy that propagates the earlier unconfigured error must
fail that deterministic control. The hook/copy never enters admitted source.

Actual package qualification scans the complete game/helper tree before SDK
injection, audits every separately optimized object before trusted linking and
records exact identities on all four Linux/Windows x64/i686 build targets.
Confined Linux x64/i686 compare all original witnesses and run the same worker
controls while the parent holds a real queued fullscreen request/client mutex
for >=60ms. Windows component execution uses Wine; Windows admission builds do
not qualify real AppContainer, display, controls or audio behavior.

Connect this state and worker contract to the complete original timer/DPC/IRQ
path, preserve native stack/TIB/IRQL/gate execution and timer/shadow-event tables,
and wire authentic host display-refresh reports when supported. Timer-side IRQ,
PCRTC/device updates, time-warp/suspend listeners, spin/pacing policy, profiling,
renderer/media and production entry/build/launch remain required. No physical
display cadence, rendering frame time, audible audio, controller response or
playable 60 FPS is proved by synthetic schedules or these service checks.
