# Shared guest timestamp counter and generated callers

The port now implements the original 733,333,333 Hz guest timestamp counter in
ordinary dependency-free Rust, with one root-owned first-use origin shared by
all game tasks. It consumes GamePKG's actual performance sample, not a native
RDTSC instruction, guessed host frequency, calendar anchor or instruction loop.
The five pinned generated callers are compiled native Rust; no game engine,
media bridge or original unsafe oracle is trusted/exempted.

## KSV2 metadata contract

`../service-kernel-pool/kernel_pool.rs` now uses an exact 64-byte **KSV2** record.
This is ephemeral port-private metadata, not a host ABI or save-file format.
KSV1 is explicitly rejected, never silently upgraded. The root remains exactly
288 bytes, CPU record 244 bytes and context four opaque IDs. Host quotas and
all source/dependency/object/confinement checks are unchanged.

| Offset | LE value |
| --- | --- |
| 0 | magic u32 `0x3256534B` |
| 4 | reserved-zero u32 |
| 8 | owning root ID u64 |
| 16 | immutable masked video flags u32 |
| 20 | immutable audio flags u32 |
| 24 | shared guest skew ns u64 |
| 32 | first-use raw counter origin u64 |
| 40 | first-use actual counter frequency u64 |
| 48 | initialized u64 (0 or 1) |
| 56 | reserved-zero u64 |

State 0 requires origin/frequency zero. State 1 permits origin zero, requires
origin <=i64::MAX and frequency 1..i64::MAX. Any other state, shape, magic,
reserved/configuration value is Invalid; wrong root is Denied. Existing settings,
clock-query and skew APIs retain their signatures, configuration and arithmetic.
Skew CAS preserves TSC fields, and TSC initialization preserves current skew.

`kernel_tsc_state(&Memory) -> Result<[u64;3], i32>` copies origin/frequency/state.
`kernel_tsc_read(&Memory) -> Result<u64, i32>` requires a configured live root,
pins its selected service ID and uses at most 64 metadata CAS attempts with the
existing bounded retry pause. The first winner publishes origin/frequency/state
in one CAS. A successful read then takes a second raw sample, preserving the original separate
origin/current-counter reads. Losers observe the same published pair instead
of creating per-task clocks. Current skew is fetched after the counter read,
as in the original implementation. No calendar request is made.

Publication and copied record reads are not leases or one global transaction.
A changed valid service ID/origin/frequency/state during the read is Denied;
missing/revoked/invalid records propagate their normal errors. Frequency changes
or a counter before origin fail (-3); no reset, guessed frequency or fallback
is used. Initialization can publish before a later sample fails: do not claim
rollback or replay. A publish on the final bounded iteration can return Busy
before the second sample; the complete origin remains for later requests. This
boundary is tested explicitly. Stop/join users before clearing the service owner. Runtime
teardown remains the final cleanup for unrecoverable failure.

`kernel_tsc_value(counter, origin, frequency, skew)` is the pure checked helper.
It validates the signed-positive host counter/frequency domain, then preserves
original u64 arithmetic, including unsigned intermediate/result wrap:

```
ticks = counter - origin
seconds = ticks / frequency; remainder = ticks % frequency
value = seconds * 733333333
      + (remainder * 733333333) / frequency
      + (skew / 1000000000) * 733333333
      + (skew % 1000000000) * 733333333 / 1000000000
```

Each unsigned multiply/add wraps exactly like the original C. In particular,
synthetic large-frequency cases must not silently widen the remainder product
and change its result. Invalid pure inputs return -1. These guest ticks are
separate from raw QPC, uptime, UTC100ns and op 60 session deadlines.

## Exact generated Rust consumers

`generated.rs` contains all five original timestamp-call bodies:
`sub_001D20AC`, `sub_001FC930`, `sub_001FC960`, `sub_001FC980`, `sub_00356AC0`.
Their three source units are hash-pinned. The maintainer AST converter accepts
only the exact direct zero-argument `xbox_ReadTimeStampCounter()` binding and
emits `kernel_tsc_read(guest_ram)?`. Shadowing, attributes, generic/qualified /
parenthesized callees and extra arguments reject. No source policy is relaxed.

`--prepare-tsc-leaves` selects only those exact unit-bound definitions, rejects
new literal dependencies, emits safe game bodies and a separate unsafe oracle,
and records source-kit/unit/converter/lock/helper and output identities.
`generated.json` pins the result. Its unsafe oracle is maintainer-only and is
explicitly rejected even when added as unused game source. Final admission
still scans all game source before SDK injection, optimizes separately and
audits every game object before trusted runtime linking.

The finite-router fixture uses the original target classifier and direct Rust
invokers, never function pointers or an instruction decoder. These generated
bodies retain their original **two 32-bit stores**. A denied second store keeps
the first store and preceding CPU effects; do not replace them with a globally
atomic store or pretend a failed call rolled back. Missing services preserve
all completed stack/register/memory prefixes for each of the five functions.

## Validation

Maintainer commands (Python/C are never player requirements):

```sh
python3 host-integration/engines/takedawn/service-kernel-tsc/check.py --app /path/to/app --wine
python3 host-integration/engines/takedawn/service-kernel-clock/check.py --app /path/to/app --app-lib /path/to/current/libgamepkg.rlib --wine
python3 host-integration/engines/takedawn/service-kernel-tsc/check_generated.py --app /path/to/app --app-lib /path/to/current/libgamepkg.rlib --units /path/to/hash-verified/original-units --tool /path/to/takedawn-safe-source-preparation --call-targets /path/to/reviewed-targets --wine
```

- **768 original C timestamp calls per target**, Linux32/64 and Windows32/64
  under Wine: eight independent origins/frequencies, fractional/long deltas,
  full skew domain and unsigned wrap. Only the function identifier is renamed
  per family; every original body byte and its local statics remain intact.
- **240 original generated-Rust body comparisons per target**, checking every
  CPU byte and all 262,144 mapped RAM bytes, with controlled service results.
  The oracle's unmapped gap is checked separately, not mirrored into game RAM.
  Counter arithmetic is qualified by the independent original-C suite above.
- Complete five-body/helper source and every-object audits pass on four targets.
  Actual confined Linux32/64 children each execute **80 finite-router calls**,
  full CPU/mapped-RAM checks, independent parent timestamp bounds, partial-store
  denial and missing-state prefixes. A startup barrier precedes parent timing
  bounds. Actual game objects contain neither the oracle nor service injection.
- The clock suite additionally passes four concurrent first users with one
  shared origin, all five clock errors before/after initialization, counter /
  frequency failures, 64-attempt contention exhaustion, retry preserving actual
  intervening skew writes, malformed/old metadata and post-initialization skew
  updates without origin loss. Fault injection is maintainer-only; the admitted
  worker tests use the actual SDK/runtime. Prior 125..128 behavior still passes.
- The refreshed partial native kit has 23,013 locally convertible / **19,307
  closed definitions of 24,016**, 2,439 emitted files and all five TSC consumers.
  The qualified five bodies match that candidate byte-for-byte. Its full
  source/object matrix and actual Linux reference/confinement paths pass without
  relaxing the existing 12 GiB compiler budget, object limits or confinement.

The main import gap remains **111**: TSC is a direct generated-code helper, not
an additional kernel export. The installed CLI/GUI SDK `9fef7a199380e4b4` remains
current; this port-only consumer requires no app binary reinstall.

These are silent synthetic/native-service checks. Windows execution above is
Wine comparison evidence, not real AppContainer/hardware testing. No complete
title entry, visible graphics/SMAA/sharpening, physical controls/rumble, audible
audio, original saves or measured 60 FPS/frame-work budget is qualified.

## Remaining work

Periodic tick data, real timer/scheduler/wakeup/IRQ/DPC/device and time-warp /
suspend handling remain. Do not leave tick data fixed at the synthetic 123.
The partial kit still has 4,709 definitions outside its literal closed graph;
computed native/manual/media routes and complete renderer/entry/production
builder/launcher integration remain required. A closed graph is not full game
qualification. Preserve all guards and in-place ISO/private-save grants, and
qualify real gameplay before enabling the production recipe.

## Independent child-local clock migration

The production helpers now use `gamepkg::local_performance_sample()` (local
operation 38) for uptime, raw performance queries, frequency and guest TSC.
The GamePKG runtime owns the platform read; no game OS import, dependency,
unsafe code, raw pointer, arbitrary selector or enforcement exemption is added.
This avoids serializing timer/TSC reads behind a pending window/broker request.
Calendar queries still use remote operation 65 and preserve their separate
first-use anchor. There is no remote or native-OS fallback if local reads fail.

KSV2/root/CPU layouts, 64-attempt CAS behavior, skew arithmetic, first-use TSC
origin, quotas and original partial-write/error semantics are unchanged. The
maintainer oracle injects local samples explicitly; actual confined admission
uses the real child-local SDK and separately audited game objects. Regenerated
provenance binds the updated kernel helper even though generated game bodies
are unchanged. Complete title entry and the periodic tick/scheduler/device/
renderer migration remain required before production admission.
