# Checked kernel clock bridges

Original exports 125..128 now run as ordinary dependency-free Rust in
`../service-kernel-pool/kernel_pool.rs`. They use GamePKG's worker-owned clock
services and checked guest RAM. No C implementation, direct platform clock,
raw pointer, native function pointer, new game dependency or engine exception is
linked into admitted game objects. The complete title remains blocked.

## Contracts

`kernel_supported` reports argument bytes 0 for 125/126/127 and 4 for 128.
`kernel_requires_services` is true for 125/126/128: their import preparation and
publication require the existing valid root-bound KSV2 configuration/skew record
before patching. Frequency (127) does not depend on that record. The root stays
288 bytes, CPU record 244 bytes, context four opaque IDs and KSV2 64 bytes.
No record, task, stack, process, transfer or diagnostic limit was increased.

| Export | Clock / shared-state ordering | Result |
| --- | --- | --- |
| 125 KeQueryInterruptTime | op 66 performance sample, then shared skew | Original unsigned `uptime_ms * 10000 + skew_ns / 100`, with defined u64 wrap |
| 126 KeQueryPerformanceCounter | shared skew, then op 66 | Raw counter plus split seconds/remainder scaling using the actual frequency |
| 127 KeQueryPerformanceFrequency | op 66, independent of skew | Actual raw counts per second, never a guessed console or host frequency |
| 128 KeQuerySystemTime | non-null: op 65 calendar sample, shared skew, checked LE output | Anchored UTC100ns plus `skew_ns / 100`; null is a successful no-read/no-write |

125..127 return the full u64 as EDX:EAX and pop return + 0 argument bytes.
128 writes exactly eight bytes, returns EAX=0, preserves EDX and pops return +
four argument bytes. Other CPU fields remain unchanged. Caller/argument reads
are validated first. Any mediation failure leaves all CPU bytes unchanged;
this does not roll back prior clock reads, calendar anchoring, caller metadata
or concurrent operations. Abort rather than replay a failed call.

A null 128 pointer does not request either clock or read skew during invocation,
even when the service record was subsequently removed or clock sampling would
fail. Initial import admission still requires configuration for potential
non-null calls. Non-null output denial can occur after a calendar read has
anchored the clock. The eight-byte copied store validates the entire range,
including distinct physical segments and page protections, before mutation;
invalid/denied outputs remain unchanged. Invalid original native pointers do
not authorize access outside GamePKG memory or define a C-fault recovery path.

`kernel_time_value(ordinal, base, frequency, skew) -> Result<u64, i32>` is the
ordinary game-side arithmetic helper. Base is uptime ms (125), raw count (126),
raw frequency (127) or already anchored calendar100ns (128). Counter/frequency
and signed sums/products use the original C's defined signed-positive domain.
Zero/oversized frequencies, counter/calendar values over i64::MAX and overflowing
signed intermediates/sums are Invalid (-1), before CPU/output changes. Original
unsigned 125 arithmetic wraps explicitly; unsupported ordinals return -5.
This never invents results for the original C's signed-overflow undefined cases.

The actual SDK is app `b617c3e`, recipe `9fef7a199380e4b4`, shared digest
`91f55ad297aeb2109e0622778c9a967ebc831e44975dc5edc6d8573013d2988a`.
Op 66 never reads/anchors calendar time. Op 65 establishes its original first
successful calendar origin and advances it with raw counters. Their copied
values are not op 60 session nanoseconds or op 61 deadlines. Errors propagate;
no direct OS fallback is available. See the app PORT_SERVICES.md and
`../service-native/CLOCK_SERVICE_HANDOFF.md` for exact host lifetime/error rules.

## Evidence

```sh
python3 host-integration/engines/takedawn/service-kernel-clock/check.py \
  --app /path/to/GamePKG/app --app-lib /path/to/current/libgamepkg.rlib --wine
```

Hash pins cover the unchanged original four bridges, original argument switch /
lookup membership, kernel clock implementations, calendar anchor function and
its static state. The C clock-source shims supply deterministic host readings;
they do not replace original kernel math. Native comparison intercepts only
sample acquisition and records the exact raw/calendar/no-read selection.
C/Python, sample injection and the C oracle are maintainer-only. Windows32's
oracle links GCC division helpers after its C object; that is never a game-link
exception or an extra library in admitted game objects.

**256 original bridge calls per target** pass on Linux32/64 and Windows32/64
under Wine, comparing every 262,144 RAM byte and every CPU byte after each call.
Inputs cover zero/fractional/large shared offsets, eight supplied frequency scales,
high counter words, unsigned uptime wrap, old/current UTC epochs, unaligned and
page-straddling output, aliases that overwrite argument-stack bytes and nulls.
All five clock errors propagate with CPU/output unchanged; null 128 makes no
sample request even against the failing fixture. Defined signed-overflow
boundaries and unsupported ordinals are checked separately, without executing
undefined C arithmetic as an oracle.

Complete production/helper source is scanned before SDK injection, optimized
separately and every game object audited before trusted linking on all four
targets. The admitted fixtures use the actual SDK, not the sample shim or C code.
Actual confined Linux32/64 children each compare 256 C arithmetic witnesses and
perform **80 real kernel calls across five views**, including four concurrent
workers sharing one root-bound nonzero offset. The parent independently bounds
all reported results with its own host samples, verifies per-view/per-ordinal
counts, every stage and final status. The child checks every CPU byte.

Other controls cover signed overflow, unsigned wrap, zero/invalid frequency,
missing configuration before preparation with untouched table/map, independent
frequency preparation/invocation without skew, null after record removal,
unmapped/overflowing output, read-only outputs and eight-byte stores across
separate physical backing ranges, including unchanged bytes on denied second
segments. Existing source/object/allocator/import/settings/crypto/native matrices
remain separate regressions, with their original enforcement and limits.

Image mode 10 carries all four time exports through authenticated synthetic XBE
loading, complete import publication, checked CPU/data/allocator initialization
and the finite native Call router; it preserves full CPU completion and real
clock bounds. Existing negative modes still reject malformed/unsupported input
before patching. The private ISO stays read-only/in place and rejects **111
remaining main function exports**, down from 114: its main table uses 126..128;
125 is additionally implemented, not an extra removed main import.

These are silent synthetic/service/native-component checks. They do not qualify
full title entry, real menus/driving/crashes, graphics/SMAA/sharpening, physical
controls/rumble, audible audio, real Windows AppContainer, saves or measured frame
pacing. No new app binary is required for this port-only consumer; the matching
CLI/GUI 0.1.24 services are already installed locally.

## Remaining work

The separate guest TSC conversion/shared first-use origin and all five generated
callers now exist at `e206014`; see `../service-kernel-tsc/README.md` for exact
KSV2 contracts and differential/confinement evidence. Raw QPC must not replace
that guest counter. Periodic tick data must
advance through the existing checked data API rather than stay at the synthetic
initializer's 123. This component does not implement timer queues, waits,
scheduling, IRQ/DPC/device semantics, time-warp wakeups or suspend recovery.

The other kernel/generated/manual/media/renderer paths, real launch settings,
complete source kit and reviewed Linux/Windows production builder/launcher
selection remain unfinished. Continue ordinary checked Rust migration, preserve
in-place ROM/private save grants and all admission/confinement rules, and
qualify actual gameplay before enabling the title. No skipped operation, fake
success, direct-device fallback or engine exemption is authorized.
