# Host clock contract for the remaining native timing migration

App implementation: `da4864db4e17a5d5f0c615a8383976d0a9e4f3cf` on
`feature/halo-shared-input`. Shared SDK digest
`fe1d3ea8e80629e53576dd9a49d3689099e4df5edc7d8ba76bf6d68898889320`;
composite native recipe `969defa5bb1d6421`. The matching CLI/GUI are installed
locally. This resolves the host raw-clock/calendar prerequisite; it does **not**
implement or register kernel ordinals 24/125/126/127/128, migrate the scheduler,
or enable native title entry. The private main-import gap remains **115**.

## Available interface

The host-generated Rust client exposes:

```rust
pub struct ClockSample {
    pub counter: u64,
    pub frequency: u64,
    pub uptime_ms: u64,
    pub utc_100ns: u64,
}
pub fn clock_sample() -> Result<ClockSample, i32>;
```

ABI 1 op 65 has an empty request and exact 32-byte LE reply in that field order.
Counter/frequency use the actual positive host performance-counter units, not
an invented Xbox/CPU rate or session epoch. Unix uses CLOCK_MONOTONIC ns / 1 GHz;
Windows uses QPC/QPF and GetTickCount64. Calendar is in 100ns since 1601-01-01 UTC,
anchored on the first successful sample and advanced from that original counter
anchor. Later wall-clock corrections do not move it. It has the original anchored
calendar model, not a continuously corrected realtime value. OS SystemTime
precision determines the initial anchor's precision; do not assume Unix ns
precision on Windows. Arithmetic is checked and rounding does not accumulate.

Game operation 65 reads platform clocks on the private connection worker. Owner-thread dispatch
returns Busy. Malformed request/capacity is Invalid before any anchor; closed
sessions deny new samples. Invalid/regressing counters, frequency changes,
out-of-range calendar arithmetic and source errors fail. Unsupported hosts or
old SDKs are not permission to use direct std::time, libc, SDL, Win32, TSC or
native symbol lookup. The client rejects malformed lengths and zero/out-of-range
frequency/counter before returning a typed sample. No handle/address is exposed.

Op 60 and `monotonic()` still return **session ns and wake generation**. Op 61
`wait_until` takes that timebase and at most one second of future deadline.
Never pass QPC counts, uptime or UTC directly to it. Convert relative durations
with the sampled frequency and obtain a session deadline/generation. Existing
lost-wake/revocation/one-waiter bounds are unchanged. Measure transport cost under
the real port workload before making pacing claims; never hide delays by
inventing clock values or relaxing timeouts.

## Original consumers that must be integrated next

The original source kit remains pinned at
`6b37d24c0984c686c7ed6fd73cd3a5b252b5631ceefa2798e5b7a6e0adff8dc4`.
`toolkit/src/kernel/kernel_hal.c` and the corresponding bridge define:

- 125 `KeQueryInterruptTime`: unsigned boot milliseconds * 10,000 plus guest
  skew ns / 100; return the full value as EDX:EAX, no arguments.
- 126 `KeQueryPerformanceCounter`: raw counter plus skew scaled to its actual
  frequency. Preserve the original split seconds/remainder calculation and
  defined signed range, returning EDX:EAX with zero argument bytes.
- 127 `KeQueryPerformanceFrequency`: actual raw frequency, EDX:EAX, no args.
- 128 `KeQuerySystemTime`: one pointer argument; null is a successful no-op
  before querying time. Otherwise store the host anchored UTC plus guest skew
  ns / 100 using checked guest LE memory. EAX=0; EDX/other CPU fields preserved.
- `xbox_ReadTimeStampCounter` is a **separate** guest counter scaled to
  733,333,333 Hz with its original shared first-use origin and skew. Do not feed
  raw QPC directly to generated RDTSC helpers or conflate it with ordinal 126.
- Periodic tick data must advance from the appropriate host/guest time through
  the checked kernel-data API. Leaving the synthetic initializer's 123 forever
  is not timer integration. Timers/wakeup/pacing belong off the window owner.

Guest skew/time-warp state must be shared by the actual title's threads and
bound to the live root; do not put a per-view copied mutable clock in Memory,
forge guest RAM holes for metadata or silently assume warp is always zero.
Coordinate a bounded port-owned record/context schema before registering these
exports. Preserve the 244-byte CPU record, strict root/foreign/stale checks,
finite dispatch and all quotas. Any internal record-format change needs full
codec, alias, task-sharing, revocation and regression qualification. Source scan
still precedes host bindings and every optimized game object is audited before
trusted linking. The legacy production profile stays gated meanwhile.

Ordinal 24's NV settings also remains: preserve source-defined short-buffer,
null/alias/write ordering and status behavior; supply actual coordinated video
and audio settings instead of hardcoding a synthetic fixture's values. The
remaining shared launch/settings context is separate work, not part of this
read-only clock API. All game-side kernel algorithms remain ordinary port Rust.

## Evidence and boundaries

Six tests using byte-identical host clock source pass each on Linux32/64 and Windows32/64 under Wine,
including real source reads, long uptime, rounding, regression/frequency/overflow,
worker frames and revocation. The C header and Rust client carry the same wire
contract. Both actual confined Linux widths read samples consistent with the
host's exact counter/calendar anchor; all four targets pass complete source
admission and independently optimized every-object audit before trusted link.
A broker test proves valid sampling needs no owner dispatch or owner wake.
Malformed client replies, direct-clock source attacks and build/admission
regressions pass; no enforcement was relaxed.

Existing image/import/crypto checks remain separate component evidence. These
are silent service tests, not title entry, graphics, physical controls, audible
output, real Windows AppContainer or measured gameplay/frame pacing. Python/C
remain maintainer-only; ROMs remain read-only/in place and outside publication.


The subsequent shared-services/NV implementation is documented in
`../service-kernel-settings/README.md`: root codec 288 bytes with a trailing
service ID, the same four-ID context, configured video/audio and shared skew,
and original export 24. The main gap is now 114. Timing 125..128 remains
unavailable: it additionally needs a performance-only host read that does not
anchor absolute time before the original first SystemTime query. The existing
combined op 65 contract is unchanged; do not use its calendar side effect as an
unreviewed timing approximation.

## Performance-only prerequisite completed

App `b617c3e450fb198689803c8890e01c0c5bebd52f` supplies ABI 1 op **66** /
`gamepkg::performance_sample() -> Result<PerformanceSample, i32>`. Request empty,
reply exactly 24 bytes: LE u64 counter, actual counts/second frequency, boot ms.
It uses the same platform units as op 65, with counter <=i64::MAX and frequency
1..i64::MAX, but never reads wall time, initializes the calendar anchor or updates
its last-sample state. Op 65's first successful call still anchors at its own
counter. Neither operation uses op 60 session-deadline units. Do not guess a
frequency or substitute guest TSC units; TSC conversion/origin stays port-owned.

Both operations run on the private connection worker. Direct owner-thread calls
are Busy; malformed requests/short capacities Invalid; closed sessions Denied.
Malformed SDK reply shape/ranges are Failed. Unsupported hosts/platforms must
not fall back to direct OS reads. Quotas, waits, source checks and object/import
rules are unchanged. Complete host contracts are in app PORT_SERVICES.md.

Shared SDK digest is
`91f55ad297aeb2109e0622778c9a967ebc831e44975dc5edc6d8573013d2988a`,
recipe `9fef7a199380e4b4`. Seven exact-source host clock tests pass on each of
Linux32/64 and Windows32/64 under Wine, including untouched calendar state before
and after counter-only reads. Actual confined Linux children each return 16 raw
and 16 calendar samples bounded by host counters; all four targets pass complete
source checks, separate optimization and every-object audit before trusted link.
The settings and complete image/import matrices also pass with this SDK, while
the private image still rejects 114 remaining main exports before title entry.

Next work belongs in checked port code: register exact 125..128 math/CPU effects,
shared skew consumption, separate guest TSC origin/units, periodic tick and real
timers/scheduler. Choose op 66 for QPC/uptime-only paths and op 65 only where the
original first calendar query belongs. Do not enable these exports or the title
on the strength of infrastructure tests. No gameplay, visible rendering, physical
input, audible audio, real Windows AppContainer or measured pacing is qualified.
