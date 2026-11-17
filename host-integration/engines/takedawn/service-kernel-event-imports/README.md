# Checked native event imports

The actual checked import dispatcher now implements **189 NtCreateEvent** and
**225 NtSetEvent** through the GamePKG-owned native event service. These are game
functions compiled to native Rust, not host engine code, a CPU interpreter, raw
native handles or an exception to admission. Complete title playback stays blocked.

## Admission, context and ABI

`kernel_supported(189) == Ok(16)` and `kernel_supported(225) == Ok(8)` describe
argument bytes only. Both require services and a live **KIC3** with the complete,
same-root tagged-table/event-domain binding. KIC1/KIC2/context-free plans fail
readiness before patching. Forged legacy plans also fail at actual invocation.
Classification alone never makes a new import ready. KPL4 and the 80-byte KIC3
format are unchanged; exact owner contracts are in
[OBJECT_STARTUP.md](../service-kernel-context/OBJECT_STARTUP.md).

`kernel_invoke(&mut Registers,&mut Memory,slot)` validates the existing frame and
reads the dummy return/all arguments before effects. Normal NT status returns
change only EAX and ESP: creation pops 20 bytes including the dummy return;
signaling pops 12. Other registers, flags, MMX/x87 state and FS data are preserved.
Control-plane or memory errors leave CPU state unchanged and retain/report any
accepted memory/resource prefix. They do not invent a normal NT success/failure.

### NtCreateEvent

Guest arguments are `[output_handle_ptr,object_attributes_ptr,event_type,initial]`.
ObjectAttributes is ignored, matching the pinned original implementation; it is
not dereferenced or translated into file/system authority. Type zero selects
manual reset and all other u32 values select auto reset. Only the low eight bits
of initial state determine signaling, preserving the original BOOLEAN conversion.

Creation reserves its owned transaction, accepts a real opaque SDK event and
publishes it into the shared domain before fallible tag/output publication. A
nonnull output receives exactly four LE bytes containing the guest tag. A null
output still creates and retains an owned orphan, consumes no tag and succeeds.
A failed guest write retains any accepted domain/tag prefix. Transaction close
runs after either successful or failed publication; it releases untransferred
objects or leaves transferred objects with the domain, then releases the guard.
Unrecoverable cleanup errors remain fatal/session teardown obligations.

Success returns `0`. Actual native event-capacity failure returns
`0xc000009a` (STATUS_INSUFFICIENT_RESOURCES) and still writes zero to a nonnull
output, matching the original bridge's null local HANDLE. Stale publication,
revocation, invalid backing and other control-plane failures are **not** recast as
ordinary allocation failure. Namespace IDs remain full u64 internally; no native
handle is reconstructed from the guest's u32.

### NtSetEvent

Guest arguments are `[guest_token,previous_state_ptr]`. A nonnull previous-state
pointer receives four LE zero bytes **before** attempting the signal, including
on invalid handles. The original explicitly reports zero rather than querying
previous state. A failed output write therefore cannot signal the event.

Only a live, owned event tag is signaled. Success returns `0`. Invalid/raw/pseudo,
empty, wrong-kind or valid-but-unowned event tokens return `0xc0000008`
(STATUS_INVALID_HANDLE). They never become host pointers. A private typed result separates ordinary invalid tags from failed service
operations. Inaccessible or malformed table pages propagate Denied/Invalid
errors; they never become STATUS_INVALID_HANDLE. Native liveness failures are
classified only after rechecking the owner, table entry and domain row under
the guard. Service/owner revocation remains fatal, with CPU state unchanged.

## Shared checked implementation

The table, guards, domain, creation transaction and wait types now live beside
the checked dispatcher in **ports `service-kernel-pool/kernel_pool.rs`**. The
existing `handles` and `events` modules re-export those same public types without
renaming them, following the existing checked shadow-map pattern. There is one
implementation and one ownership graph; standalone callers and actual imports
use it. This code remains in the untrusted checked game-source set and every
compiled game object is audited. Nothing moved into the app's trusted runtime,
SDK namespace or transport. No source, dependency, object or confinement rule was
relaxed. Public module declarations preserve the type interfaces with strict
compiler warnings; suppression attributes are not injected into game code.

## Original source and qualification

`reference_inputs.json` pins the public source kit, complete bridge/sync files
and ten unchanged original function bodies. The maintainer C oracle compiles
those exact bodies with a synthetic bounded event provider, guest words and
handle table. Its 126-event bound matches this fixture's unchanged 128 SDK sync
objects minus the shadow/table mutexes. It is **not a physical Windows event or
original-gameplay benchmark**, and it is never linked into a game object.

The 1,764-byte comparison checks 24 type/initial-state combinations, exact tags,
four-byte writes/sentinels, previous-state behavior, invalid/pseudo tokens,
null-output ownership and actual creation-budget failure. Real SDK state and
CPU-frame assertions accompany the bytes. Protected-error controls cover unmapped/
readonly output and previous-state pointers, inaccessible and corrupt tagged-table
metadata, retained accepted creations, wrong kinds, valid foreign IDs, alias
lifetime, strict legacy readiness and revocation.

Three maintainer-only SDK interference cases exercise revocation immediately
after an accepted previous-state write, a stale tag publication after accepted
native transfer, and a stale domain transfer before publication. They verify
fatal-error/status separation, unchanged CPU state, retained output prefixes and
actual cleanup without a leaked guard/object. No production fault hook exists.
Twelve broken native copies must fail. Runtime/fault oracle shells are rejected
if packaged even as unused source. These native negative controls are separate
from the positive admitted fixture; they are not claimed as confined mutants.

Positive signed fixtures scan every dependency-free game helper before binding
injection, separately optimize/audit all eight game objects on each target, then
link trusted runtime/transport objects. Linux32/64 execute in actual confinement.
Four-target standalone SDK/C comparisons and faults execute natively on Linux and
through Wine on Windows. Windows signed-artifact/image scope is build-only.
Authenticated KIC3 image mode 68 additionally invokes both imports through its
real published map and compares the same original-C result. The private ISO is
read/authenticated in place and still rejected before title entry.

No game window, physical input, audible audio, GPU fidelity or measured frame
pacing is established. SDK/quotas/installed app/public source archive/catalog/
app submodule/defaults/profile gates are unchanged. Exact checked source-kit and
all audited-object identities rebuild. The current main import gap is **104 with
KIC3**, **106 with KIC2**, and 107 with KIC1. Only 189 and 225 were removed from
the KIC3 missing set. NtClose, NT waits, pulse, typed file/thread resources and
the remaining full native source/loop/renderer/media/build/launcher/gameplay path
remain required. Do not admit NtClose from an event-only shortcut or fake a wait.
