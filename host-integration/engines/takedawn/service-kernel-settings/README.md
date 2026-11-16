# Shared root services and checked NV settings

Ordinary dependency-free Rust now implements original kernel export **24**,
`ExQueryNonVolatileSetting`, in `../service-kernel-pool/kernel_pool.rs`. It uses
checked guest RAM plus a private root-bound configuration record, not SDL,
platform settings, host files, native pointers or a trusted-engine exemption.
All sources are checked before bindings and every optimized game object remains
audited before trusted linking. The complete title stays gated.

## Exact record/API contracts

The port-private CallState codec is now **288 bytes**: its original 280-byte
trace/bounds/counter prefix is unchanged, followed by the LE u64 service-record
ID at offset 280. Zero means unconfigured. Decode requires exactly 288 bytes;
old/truncated/oversized records are rejected, not silently upgraded. The actual
CPU register record remains **244 bytes**, and the opaque context stays four
IDs `[root, kernel_map, heap_inbox, heap_task]`. Views/tasks retrieve shared state
through that root; no mutable clock/response queue/client is copied per view.
This is transient port-owned metadata, not a new host ABI or saved-game format.

`Memory::services_handle() -> Result<u64, i32>` validates/reads the root codec.
`services_link(expected, next)` uses at most 64 versioned CAS attempts with the
existing bounded retry pause; unexpected current IDs deny, equal expected/next
is Invalid. Trace and bounds updates preserve this field. Root publication and
record CAS are separate calls, not one lease/transaction. Metadata IDs are opaque
local records, never pointers. Existing 64 records / 64 KiB payload / 4 KiB each
and all other limits remain unchanged. Records still cannot be resized by CAS.

`kernel_services_initialize(&Memory, video: u32, speakers: u32)` returns one
`KernelServices` owner. It requires a live unconfigured root, no installed kernel
map and resolved mixer channels 2 or 6. It applies the original video setter's
`0x001F0000` mask; stereo reports 0, six-channel mixing reports `0x00010002` from
the original speaker helper. Skew and TSC state/origin/frequency start at zero;
old KSV1 is rejected. The 64-byte KSV2 record is:

| Offset | LE field |
| --- | --- |
| 0 | magic u32 `0x3256534B` |
| 4 | reserved-zero u32 |
| 8 | owning root record ID u64 |
| 16 | immutable masked video flags u32 |
| 20 | immutable audio flags u32 (0 / 0x10002) |
| 24 | shared guest clock offset ns u64 |
| 32 | first-use TSC origin counter u64 |
| 40 | first-use TSC frequency u64 |
| 48 | initialized u64 (0 / 1) |
| 56 | reserved-zero u64 |

Malformed magic/shape/reserved/flag values are Invalid; wrong owning root is
Denied. Allocation/publication failure releases the new record and leaves the
root unconfigured. Constructor configuration is a resolved per-title snapshot;
real launch/settings values still need production coordinator wiring. This API
does not implement physical output, live settings editing or time-warp scheduling.

`kernel_service_values(&Memory)` copies `[video, audio, skew]`; missing state is
Unsupported. `kernel_advance_services(&Memory, delta)` and the owner's
`service_advance` perform bounded checked-add/CAS on skew. Overflow does not
change bytes/version. The selected record ID stays pinned across snapshot/CAS,
so a stale owner cannot copy another same-root record into its old record, even
when versions match. Owner methods also bind the root. Other flags remain fixed.
This stores one shared offset; it does not yet feed the time kernel exports.

`service_close` clears the matching root link and releases only that owner's
record. Repeated close is harmless; another active link denies. A revoked root
still allows explicit cleanup of that owner's known record, returning Denied.
IDs are never leases. Stop/join all users before clear/release; an already
accepted update may finish. Do not replay a failed call or claim global rollback.
Runtime teardown remains the final cleanup on an unrecoverable invalid root.

## Original NV call behavior

Ordinal 24 has five guest u32 arguments / 20 stdcall argument bytes:
`index, type_pointer, value_pointer, value_length, result_length_pointer`.
Preparation and complete import planning/publication require valid services
before any thunk patch. Missing/wrong/corrupt configuration cannot authorize an
otherwise known call. Success/status results use EAX, ESP pops return + 20;
all other CPU bytes remain unchanged. Mediation errors leave CPU unchanged.

Null value returns `0xC000000D` before logging or other writes. Known indices
retain the original constants/configuration: timezone -300, English language,
video/audio flags, unrestricted parental values, miscellaneous zero and DVD
region 1. For these, length <4 succeeds with **no pointer writes**. At length >=4,
store Value, optional Type=4, then optional ResultLength=4 in that exact order.
A larger declared known value length still writes only four bytes.

For unknown indices, preserve the original default: zero `value_length` bytes,
then optional Type=4 and ResultLength=the supplied length. Zero length need not
access its non-null value address, but still writes Type/Result. Zeroing uses
bounded copied chunks through checked segmented memory, including adjacent guest
views with distinct physical offsets. It retains the prior 64 MiB fill-work cap
and checks 32-bit extent overflow; no quota was raised. Each chunk validates its
ranges, while errors after earlier chunks/stores may leave those earlier effects.
Denied Type/Result writes likewise retain the preceding original writes. Abort,
not replay. Host/translation RO, unmapped and foreign/stale state still deny.

The original debug `(index,length)` and unknown-index warning are preserved as
bounded best-effort numeric diagnostics `0x544B1300` (level 0) and `0x544B1301`
(level 2). Logging failures are ignored like the original void logger. No format
string, console descriptor, native log call or arbitrary file access is exposed.

## Evidence

```sh
python3 host-integration/engines/takedawn/service-kernel-settings/check.py \
  --app /path/to/GamePKG/app --app-lib /path/to/current/libgamepkg.rlib --wine
```

Reference pins cover the original query/bridge, video setter/getter, speaker
mapping/wrapper, argument switch, lookup membership and constants in the reviewed
source kit. C/Python and diagnostic-capture shims are maintainer-only; no C oracle
or fake SDK is linked into admitted game objects. **96 original calls per target**
pass on Linux32/64 and Windows32/64 under Wine: every 262,144 RAM byte, full CPU
and ordered numeric log calls. Cases cover all defined indices, original default
indices, short/zero/large declared lengths, unaligned/overlapping outputs and
null/unused pointers, including outputs that overwrite argument stack bytes.

All four targets pass complete source checks before SDK injection, separate
optimization and every-object audit before trusted linking. Actual confined
Linux32/64 runs match the C output/CPU/log witnesses and all control stages.
Whole-RAM comparisons belong to the native oracle; confined witnesses copy the
entire affected corpus regions. Controls cover wrong/old record shape, fixed-size
CAS rejection, foreign/same-root replacement, reserved fields, quota exhaustion
without partial linking, missing/late configuration, all three output-pointer
protection failures, invalid spans/extents, split physical views and translation
RO. Four workers perform exactly 256 shared increments; separate trace workers
perform 1,024 updates while the root service pointer is published/cleared 256
times. Final trace/offset/ID values survive. Overflow, repeated close and revoked
root/orphan cleanup also pass. No record/task/stack/process limit was increased.

Five codec tests pass on all four targets, retaining the exact original trace
prefix while validating the trailing ID, malformed lengths and compare/set.
Existing allocator, import/protection, image, bootstrap, SHA/RC4/HMAC and generated
native/source-enforcement matrices remain separate regressions. Synthetic image
mode 8 routes video/audio queries through authenticated loading, complete import
publication and the actual finite Call router; mode 9 rejects missing services
without patching. The private ISO stays read-only/in place and stops before title
entry on **114 remaining main function exports**, down from 115.

The current installed app SDK `969defa5bb1d6421` is unchanged by these port-only
helpers; image reused-source pins track the new Memory/CallState hashes. No app
binary reinstall or host API exemption is required. These tests are silent and
establish no title gameplay, graphics, physical controls, audible audio, original
save compatibility, real Windows AppContainer or measured frame pacing.

Next: implement a performance-only host read that does not initialize the
absolute-calendar anchor, then consume shared skew in 125..128 and the separate
guest TSC/tick/timer paths. Using combined clock_sample() for an earlier QPC query
would anchor UTC before the original first SystemTime call; do not silently
change that behavior. Source launch flags, scheduler/IRQ/device/generated/manual/
media/renderer/entry and complete reviewed production builder/launcher admission
still remain. Keep all unmigrated profiles gated; never use direct platform
calls or fake success to fill those gaps.

Update: app `b617c3e` now provides the required non-calendar performance read,
op 66 / `performance_sample()`. See the exact contract and validation in
`../service-native/CLOCK_SERVICE_HANDOFF.md`. The full settings and image/import
checks pass again with SDK recipe `9fef7a199380e4b4`. Consume that API explicitly
when implementing 125..128; this extension does not register them or qualify
title gameplay. The historical SDK above describes the initial port-only change.

The next port consumer now exists at `b70e07e`: exports 125..128 consume shared
skew through the SDK with original register/stack, null and arithmetic behavior.
See `../service-kernel-clock/README.md`. Guest TSC/origin, tick/timers/scheduling
and the other complete-title requirements remain pending. No record schema,
quota, app SDK, production recipe or launch gate changed in that checkpoint.

The subsequent shared timestamp consumer uses the KSV2 fields above. See
`../service-kernel-tsc/README.md` for exact first-use CAS, lifetime, errors,
original-body comparisons and generated-call integration. Video/audio/skew APIs
retain their signatures; this ephemeral format is not a saved-game format.
