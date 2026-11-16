# Checked SHA kernel exports

Port-owned dependency-free Rust now implements the original SHA initialization,
streaming update and finalization exports, ordinals **335, 336 and 337**, in
`../service-kernel-pool/kernel_pool.rs`. This is ordinary checked game code,
not a native crypto dependency, host exemption or success stub. Source and
object checks still apply to the whole dispatcher and helpers. The production
title remains gated on its other imports and complete native path.

## Contracts

`kernel_supported(335/336/337)` returns the original stdcall argument byte counts
4/12/8. Existing KPL2 preparation and KPL3 complete-image planning accept these
three alongside the nine allocator exports. Both main remapping and the raw
lookbehind use the same finite classification. Synthetic targets remain
`0xFE000000 + slot * 4`, not function pointers. `kernel_physical(ordinal)` is true
only for 165, 166, 168, 171 and 180: hash exports require no HeapClient,
Contiguous owner, host crypto API, file grant or additional resource authority.
Unimplemented functions remain Unsupported.

`kernel_invoke` reads and validates the CPU, live root-bound import record,
dummy return and complete arguments before calling the appropriate handler.
Successful SHA calls set EAX=0 and pop the return plus original argument bytes;
all other 244-byte register-state fields remain unchanged. A failed call leaves
the CPU unchanged. This does **not** promise rollback of earlier RAM effects.

The guest context is exactly 92 bytes: five little-endian u32 chaining words,
two little-endian u32 bit-count words and 64 retained buffer bytes. SHA block
words and the output digest use big-endian byte order. All arithmetic that
wraps in the original is explicitly fixed-width/wrapping. Context addresses may
be unaligned. No host struct layout, native pointer or native word width leaks
into that representation. SHAInit sets the original five constants and clears
both counters and every buffer byte. Update preserves the original retained
buffer contents, count update/carry and full/partial-block order. Final captures
the old count, pads, appends its big-endian length, mutates the context and emits
20 bytes; it does not clear/reset the context. Repeated Final follows that
mutated state, as in the original.

Null context is a successful no-op for all three. Update with null input or
zero length is a successful no-op even when its unused context/input value is
otherwise invalid. Final with null digest is likewise a no-op. Non-null active
context/input/digest extents must not overflow the 32-bit guest address space.
Each actual read/write still passes through Memory and the GamePKG-owned RAM
service. Buffer/block transfers are at most 92 bytes and use existing limits.
The maximum update length is the existing u32 input type, not a new chunk cap;
consumed offsets use checked extents and non-overflowing loop bounds.

Alias ordering is retained: Update publishes counters before reading input,
reads each full input block after previous state changes, and Final reads each
state word afresh for each digest byte. Tests include input aliases of Count,
State and retained Buffer plus digest aliases of State. The corpus avoids
partially overlapping memcpy source/destination ranges, whose original C
behavior is undefined. Checked copies use copied slices, never unchecked native
memcpy pointers. Concurrent writers to one context require caller coordination,
just as the original does; these operations do not introduce a transaction or
lease across the separate checked memory calls.

An extent error rejects before SHA writes. A later denied input read can leave
the earlier count update, and a denied digest write can leave the finalized
context or partial output. CPU still stays unchanged. Abort that private call
path on error rather than replaying it or treating it as an untouched context.
Neither source validation nor an allocation grants access through a RO view,
protected page, unmapped span or revoked root.

## Reproduction and evidence

```sh
python3 host-integration/engines/takedawn/service-kernel-sha/check.py \
  --app /path/to/GamePKG/app --app-lib /path/to/current/libgamepkg.rlib --wine
```

`reference_inputs.json` pins the source kit, exact original context declaration,
SHA bodies, bridge wrappers, argument-size switch and lookup membership. The C
oracle is maintainer-only and is never linked into admitted game objects.
Its i686 Linux link retains stack checking via the platform's ssp helper.
Python/C are not player requirements.

Native Linux32/64 and Windows32/64 under isolated Wine compare **64 setups / 254
original bridge calls**. Every call compares all **262,144 RAM bytes**, EAX/ESP
and every other register byte. This includes lengths 0..131,071 at padding/block
boundaries, split updates, unaligned contexts, arbitrary chaining/buffer values,
low/high counter wrap, null no-ops, defined context/input/output alias cases and
repeated finalization. The standalone oracle keeps its 4 MiB runtime budget and
reserves zero unused task slots.

The separate production-style fixture scans all ordinary sources before SDK
bindings, optimizes game units separately, and audits every object before
trusted linking for Linux32/64 and Windows32/64. Actual confined Linux children
compare copied context/digest/CPU witnesses with the C results and check known
empty/abc digests, missing heap independence, overflow/unmapped/RO/translation-
RO/revoked accesses, invalid CPU/stack/slot, unchanged CPU on failure and explicit
late-failure effects. Full RAM comparison belongs to the native oracle; the
confined pipe checks compare the documented witnesses. Existing signed/child
limits, runtime quotas and import/source vocabulary stay intact.

The image/import fixture's synthetic mode 6 authenticates and loads an image,
initializes CPU/allocator/data, publishes all three SHA imports via KPL3 and runs
`abc` through the actual finite Call router, verifying the independent expected
digest. Both Linux widths execute it; four targets pass source/object admission.
Your private ISO is read in place and remains Unsupported before any patch or
title entry: its missing main-function inventory is now **115** (121 before SHA, then 118 before RC4/HMAC).
It still needs remaining kernel calls, guest/device/runtime semantics and
complete generated/manual/media/renderer/native-entry migration.

Existing allocator, complete import/protection and generated-function/source-
enforcement regressions remain separate checks. These are silent synthetic/
component tests. No title entry, game window, real graphics, physical controls,
audible audio or measured frame pacing was qualified. Wine is not real Windows
AppContainer/gameplay evidence. The shared dispatcher now also contains RC4/HMAC and requires the clearing SDK
in app `2c646c0ec78cb39b1251ccd7ba8662da5fc62ffa`:
`dfdfb8933b8e744173dde07a3f59749deef10d4fc2fc120581353750a03fbda2`,
composite recipe `867d8027dd05fc8c`. See `../service-kernel-crypto/README.md`.
No production profile is enabled.
