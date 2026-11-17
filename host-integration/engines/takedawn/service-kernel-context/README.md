# Bound kernel-import context and KeConnectInterrupt

The complete import path now supports ordinal **98 KeConnectInterrupt** through
the reviewed guest registry. This is ordinary dependency-free port Rust using
GamePKG-owned records and checked RAM. No host interrupt, function pointer,
platform call, game dependency, source/object exemption or new SDK operation is
introduced. Other unmigrated calls still fail before publication.

## Production APIs

`kernel_pool::KernelContext::kernel_context_new(&Memory,&Connections)` creates a
root/KSV-bound copied context only after validating the supplied live registry.
`kernel_context_id()` returns its opaque u64 ID. `kernel_context_borrow(&Memory,id)`
validates the complete context and registry; `kernel_context_registry(&Memory)`
returns a borrowed registry, with no owner-close authority. Ordinary reads require
live root/KSV and an uncancelled actual task. Close is creator-only, including
cleanup after revocation/cancellation; it releases the record before reporting
liveness errors. Inspect `kernel_context_closed()` after error. Completed close
is idempotent. IDs and copied metadata are not resource leases or native pointers.
Stop and join all users before clearing map/context/registry/service owners.

The existing single `Connections` implementation now lives in the **checked
port** `kernel_pool.rs`, so its native import handler and ISR callers use the same
type and state. `service-kernel-isr/connections.rs` reexports that type; it does
not duplicate its state or move an engine into trusted host code. Its guest
object helpers retain their existing interfaces and component qualification.

`Imports::imports_scan_with_context(&Memory,table,&Data,remap,&KernelContext)`
preserves the entire original bounded scan, remap, data-export, lookbehind,
backing/protection, snapshot and publication checks. It selects KPL4 and binds
the supplied context. `image_imports::imports_image_with_context` is the real
coordinator adapter: it additionally derives the table from the authenticated
image, checks published executable bounds and derived kernel-data region and
validates the full main extent. Production must use that adapter; a synthetic
raw-table fixture is not a replacement for image authentication.

The older `imports_scan` / `imports_image` keep their context-free KPL3 behavior
and reject ordinal 98 before patching. Legacy `kernel_prepare` also rejects it.
`kernel_supported(98)` now returns its four argument bytes; this classification
is not a context-readiness assertion. `kernel_requires_context(98)` is true and
`kernel_requires_services(98)` is true. Complete planning still rejects any other
unsupported import even with a valid context. A context/creator/source signature
cannot waive source, dependency, object, confinement or builtin-profile checks.

`kernel_invoke` validates the complete map/context and CPU argument frame, records
the original caller, invokes the same registry's connect operation and only then
sets EAX/ESP. Ordinal 98 consumes one u32 argument plus the dummy return (8 bytes),
returns 1 even for null/out-of-range vectors, and emits the original connection's
bounded best-effort report: level 1, code 0x544b3200, values
[vector, object, routine, context]. Invalid vector/null produces no report.
Other CPU fields are preserved. Accepted registry/caller metadata prefixes are
not rolled back on a later error; do not replay a failed publication/call.

## Private formats

KIC1 is exactly 64 bytes, eight little-endian u64 words:

| Offset | Value |
| --- | --- |
| 0 | magic 0x3143494b |
| 8 | root call-record ID |
| 16 | KSV3 service-record ID |
| 24 | live ISR1 registry-record ID |
| 32, 40, 48, 56 | reserved zero |

KPL4 preserves KPL3 fields at 0..23, adds the bound context ID at 24..31, starts
canonical u32 ordinals at 32, and puts the existing 0..8 refusal counter after the
last ordinal. Total bytes are **36 + 4 * routing_count**, at most **1548** for
378 slots. Main/count/lookbehind/gap/data-versus-function rules remain intact.
Every KPL4 decode requires a nonzero, correctly typed, live same-root/KSV context
and registry. KPL2/KPL3 retain their exact original layouts and counter offsets;
extra bytes are not silently accepted. A forged context-free map containing 98
cannot invoke it. Full u64 IDs are decoded even on 32-bit targets.

The four-ID worker context remains [root, map, heap inbox, heap job]. Its initial
map snapshot bound rises from 1540 to 1548; the strict map decoder still checks
version, exact size, slot counts, counter and live associations before invocation.
No record quota, GPH1 ABI, root288, KSV3/64 or CPU v5/316 format changes. Old source
kits must rebuild for the changed checked source identity; never relabel receipts.
SDK remains `fe309935dd602e2cf098e50065f6035de33dac88b51d45a2bc1ba47cf026742f`.

A plan rechecks context/registry before publication. Snapshot/protection/patch/
install remain separate calls, not an atomic transaction or a context lease.
Revocation after a successful check can race accepted work; owner shutdown must
stop/join users before releasing shared owners. Invocation revalidates the live
binding. Stale context is never an excuse to fall back to context-free handling.

## Qualification

Run `check.py --app APP --app-lib APP_RLIB --wine --evidence DIRECTORY`.
Three complete-plan scenarios exercise aligned, unaligned lookbehind and maximum
378-slot/1548-byte maps, then **actual SDK workers** reconstruct the existing
four-ID context and invoke 109/98/44 through the map with exact CPU pops/state.
Null connect and the last/extra slot also run. Missing context, unsupported calls,
forged legacy maps, malformed magic/root/KSV/reserved/registry fields, borrowed
close, foreign root and context/registry revocation between scan/publication or
after binding reject. Full-width ID high bits cannot alias a valid low ID. The
shifted refusal counter accepts 8 and rejects 9. Rejections preserve CPU and
required pre-publication guest state.

All four Linux/Windows x64/i686 targets agree on 192 witness bytes; Windows
component execution uses Wine. Five broken variants must fail: contextless legacy
preparation, missing root check, ignored reserved fields, stale publication and
missing KSV check. Complete source scanning is before SDK injection; every one
of eight optimized game objects per target is audited before trusted linking.
Actual confined Linux x64/i686 children execute the same cases. Three connection
reports are checked through the private host transport; an unused maintainer
shell in game source rejects.

The authenticated-image suite additionally exercises bound context through
`imports_image_with_context`, preparation/publication and the real native
`calls_runtime::invoke_call` path, preserving the original image/root rejection
cases. It runs synthetic images in place in actual confined Linux x64/i686;
Windows builds pass complete source/object/link checks. This is not actual-title
ISO execution or Windows desktop/gameplay testing.

Current shared-source regressions pass: initializer calls/diagnostics/protection
(with the admission witness width corrected to preserve all 316 CPU bytes),
clocks, allocator/physical-memory/refusal behavior, complete legacy imports and
ISR original-C/native failure suites. Their original C remains maintainer-only.
No current source/recipe/profile/cached-player bypass is enabled.

Remaining: production root factory adoption and complete generated-source
closure, actual ISR/profiler/device/NV2A register/ack paths, disconnect contract,
timer/IRQ/vblank/spin/pause/warp/wait loop and full builders/launchers plus real
gameplay/GPU/physical-input/audible-audio/measured-pacing qualification. This
checkpoint establishes safe native ABI composition, not playable title admission.

## Authenticated startup ownership

`startup.rs` now composes service/registry/context creation and authenticated
complete import publication with explicit retained cleanup and actual-job join
barriers. See [STARTUP.md](STARTUP.md) for every API, phase, copied-ID format,
partial-error and cleanup contract. This is checked game code. The complete
native session factory still needs to adopt it and register its real users;
issuing an opaque ID is not a source/build/admission exemption or resource lease.

## Explicit timer-initializer import binding (KIC2)

Checked port code now classifies ordinal **113 KeInitializeTimerEx** as eight
argument bytes and invokes the existing checked `timer_initialize` operation.
It requires `KernelContext::kernel_context_new_with_shadow(&Memory,&Connections,
&Shadow)`. The new KIC2 record is exactly 72 bytes: little-endian u64 magic
`0x3243494b`, root, KSV3, ISR1 registry, shadow-map ID, then four reserved zero
words at 40/48/56/64. The existing strict KIC1/64 layout stays unchanged and
cannot admit 113. `kernel_context_shadow(&Memory)` returns a borrowed map;
it cannot close its owner. Stop and join all users before owner release.

`kernel_context_ready(&Memory,context_id,ordinal)` separates classification
from actual per-import readiness. Main-table and lookbehind planning use it
before patching, and invocation rechecks it before caller/CPU mutation. KPL4,
worker four-ID, CPU v5/316, SDK, host quotas and admission policies are unchanged.
A prepared plan revalidates the live map through its context before publication;
these copied IDs are not leases or an atomic lifetime transaction.

The one checked `Shadow`/`Guard` and event implementation now lives alongside
`Connections` in port `service-kernel-pool/kernel_pool.rs`. The old shadow path
reexports `Shadow`, and event functions forward to that implementation. No game
engine code enters the trusted host; no SDK operation or dependency is added.
The original map and event mutation tests now mutate that actual implementation.

The timer bridge reads both u32 arguments, retains the original 40-byte zeroed
header/type/size writes before event allocation, selects manual reset only for
kind exactly zero, and uses the same event map as other timer callers. A null
timer returns success without allocation. Success sets EAX=0 and advances ESP by
12; all other CPU bytes remain unchanged. Allocation, RAM, kind/lifetime and
publication failures remain explicit, with accepted guest/event prefixes retained;
callers must stop/report rather than retry or invent CPU success.

The actual four-ID worker fixture checks kinds 0/1/2/0xffffffff, exact header and
boundary sentinels, event signal consumption, null input, legacy-context planning
refusal, forged KPL4 legacy binding, full-u64 shadow identity, reserved fields and
shadow revocation between planning/publication. Seven required-failing context
copies include missing-shadow readiness and truncated-ID mutations. Linux32/64
run the admitted confined child; Windows32/64 component execution is Wine, with
separate source/object/link admission on all four targets. Map/event, timer-table
and retained timer/dispatch regressions pass on these exact sources. Legacy pool
and complete-import comparisons also pass. None is title gameplay, real Windows
confinement, device input, audible audio, graphics or measured frame pacing.

The authenticated Startup factory still creates KIC1. It must explicitly own or
bind the shared map and coordinate its event cleanup before using this new path;
raw-table component qualification does not migrate that factory. Its private
main-import gap remains 107 (108 without context); ordinal 113 remains a readiness
gap there despite its implemented classification. Complete generated-source,
handler/loop, renderer/media, builder/launcher and actual gameplay qualification
remain required. Both title profiles, catalog recipes, public source archive,
pinned submodule and launch defaults remain unchanged and blocked. Rebuild checked
source kits and receipts for the changed exact game-source/object identities.
The installed matching host SDK does not require a binary update for these APIs.
