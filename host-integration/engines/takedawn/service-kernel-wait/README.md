# Checked host clock and wakeable timer sleep

Dependency-free Rust replacement for the host_time clock conversion and the
one-sleeper wake notification contract. All actual counter reads, waits and
wakes use GamePKG. This is a timer-worker component, **not a complete timer/DPC
scheduler or playable title**, and enables no new kernel import or recipe.

The original code and normative single-sleeper/pending-wake comments are in
the pinned source kit's `toolkit/src/platform/host_time.c` and `host_time.h`.
`reference_inputs.json` pins the kit, complete C file and unchanged host/guest
clock function byte ranges. The upstream notices are preserved in
`../service-native/UPSTREAM_LICENSES.txt` and included in admitted test packages.
There are no title assets, ROM contents, Python player requirements or new crates.

## Exact port API

`wait.rs` uses the host-generated client as `crate::gamepkg`, and the existing
checked `memory` and `kernel_pool` modules. It adds no SDK operation or host
implementation. Require a matching app with local operations 38–41; current full
SDK is `10c4f4ca7ea37783c08ea2cafd1b129ca9b3160cbe4d3cf30d6c1a5da8c51e88`
(native recipe `c4edee3e44be562b`). Its authoritative contract is the app's
PORT_SERVICES.md. The installed matching CLI/GUI remain valid without rebuilding.

- `host_time_value(counter:u64, frequency:u64) -> Result<u64,i32>` preserves the
  original QPC quotient/remainder expression and u64 multiplication/addition
  order, including wrapping. Counter/frequency are at most i64::MAX; zero
  frequency rejects. No guessed rate, nanosecond epoch or calendar anchor.
- `guest_time_value(host_ns:u64, skew_ns:u64) -> u64` preserves the original
  wrapping host-plus-skew expression.
- `host_now_ns(&Memory)` validates a live configured root then samples local
  operation 38. `guest_now_ns(&Memory)` samples host time before loading the
  current shared KSV3 skew, in the original order. They do not initialize TSC
  or anchor calendar time. Missing configuration returns Unsupported (-5).
- `sleep_deadline(counter, frequency, ns) -> Result<u64,i32>` constructs an
  absolute raw-counter request, rounding the requested relative ns upward with
  bounded u128 arithmetic. Accept 0..1,000,000,000 ns and positive frequency
  through i64::MAX; reject unrepresentable deadlines rather than wrapping into
  the past. This converts a request to the SDK domain; it is not guest TSC.
- `Sleeper::sleep_new(&Memory)` binds root ID, KSV3 owner ID, current reviewed
  task ID (or the child entry's ID 0), raw frequency and observed wake generation.
  Initialize it in the one timer worker **before enabling queue producers**.
- `sleep_pending(&Memory) -> Result<bool,i32>` observes a changed generation
  without acknowledging it. `sleep_observed()` returns only this instance's
  last acknowledged generation and performs no service call.
- `sleep_ns(&mut self, &Memory, ns) -> Result<u32,i32>` returns 0 for an elapsed
  deadline, 1 for one or more coalesced wakes. Zero consumes a pending wake
  without blocking. A fresh state sample refreshes counter/frequency but never
  overwrites the saved generation before waiting. On success, ownership is
  checked again before advancing the acknowledged generation.
- `sleep_wake(&Memory) -> Result<u64,i32>` validates the producer's root and
  configuration, then posts local operation 41. It returns the host generation.

The timing lane is **per confined child**, not per Memory view/root. There must
be one active timer sleeper; this component does not allocate additional lanes
or make multiple root objects independent clocks. The host enforces one blocking
waiter; Busy (-2) propagates without consuming a notification. Other threads may
post wakes. Keep the periodic KeTickCount worker's existing local event wait.
Stop/join the timer task before closing its root/configuration or queue state.
The Sleeper itself owns no task, event, record, native handle, pointer or callback.

Wrong root/task, cancellation and replaced/revoked ownership return Denied (-4).
Even if configuration closes between the preliminary owner read and the values
read, a bound sleeper reports revocation, not a new unconfigured owner. Malformed
metadata remains Invalid (-1). Native/transport Failed, Busy and Unsupported
errors propagate. An error leaves the acknowledged generation unchanged.
Validation is not a transaction with arbitrary subsequent owner teardown: an
operation already accepted at its final check may complete concurrently.

No OS/platform calls, SDL, native function pointer or executable memory are
exposed. Native high-resolution timer support and clock failures remain explicit
host errors; no coarse fallback, extra quota or game import exception is added.

## Qualification

From the ports review checkout:

```sh
python3 host-integration/engines/takedawn/service-kernel-wait/check.py \
  --app /path/to/app --app-lib /path/to/debug/deps/libgamepkg-HASH.rlib \
  --wine --repeat 32
```

The maintainer-only C reference compiles the unchanged original clock bodies.
4,096 vectors per target compare host/guest arithmetic, including overflow-sized
intermediate products and nontrivial raw frequencies. 36,864 deadline cases per
target check upward rounding, signed-domain bounds and non-early requests.

Each repeated actual-runtime fixture checks 64 coalesced double-wake cycles,
64 reviewed-task wake handoffs, expired/2ms/invalid waits, exact current skew,
no TSC initialization, foreign owners, single-waiter contention, cancellation,
malformed KSV3 and stop/join/reinitialization/revoked-root controls. A separate
maintainer-only test moves a sleeper to a different reviewed task and requires
Denied without generation mutation. Deterministic maintainer-only injection
unlinks/releases configuration on the exact second root read, preserving a real
pending wake and requiring Denied from both pending and wait operations.
Neither injection nor any oracle is included in admitted game sources.
The checker also compiles a temporary regressed copy with the old premature
error return; the isolated deterministic control must reject its Err(-5) in
place of the required Err(-4). That copy is never an admitted linker input.

The complete actual fixture/helper tree passes scanning before SDK injection,
separate optimization and every-object auditing before trusted linking for
Linux/Windows x64/i686. Confined Linux x64/i686 outputs match all 4,096 original
C vectors and pass the same lifecycle/error controls. Their deadline worker also
continues sleeping and consuming its own pending wakes while the parent holds a
real fullscreen request and the remote client mutex for at least 60ms. No window
is opened. Windows component execution is under Wine; Windows admission builds
do not establish actual AppContainer behavior.

## Required integration still outstanding

Wire these calls into the actual checked `kernel_timer_loop_ns` and queue/DPC
worker. The port must still implement original timer/shadow-event tables,
gate/IRQL/IRQ behavior, native DPC argument/stack/TIB dispatch, full vblank/display
schedule and signaling, time-warp/suspend listeners and profiling/pacing policy.
This helper does not create fake callback success, run a timer queue or replace
that behavior with a generic host engine. Full generated/manual call closure,
renderer/media, production Linux/Windows build/launch, actual visuals/controls/
audio and measured pacing remain required before admission. No physical inputs,
audible audio, display cadence or playable 60 FPS are qualified by these tests.
