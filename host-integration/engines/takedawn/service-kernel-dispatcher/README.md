# Checked dispatcher-object initialization

Three complete original exports now run as dependency-free native Rust through
GamePKG memory/record/diagnostic functions. Routine/context values remain guest
u32 data. No callable native pointer, OS/SDL call, additional crate, emulator,
shader bypass or engine/media exemption is introduced. This initializes objects;
it does not execute a DPC, deliver an interrupt or schedule a timer.

| Ordinal | Arguments / pop bytes | Exact result |
| --- | --- | --- |
| 44 HalGetInterruptVector | level, optional IRQL pointer / 8 | Write low eight level bits when pointer !=0, return full level in EAX |
| 107 KeInitializeDpc | object, routine, context / 12 | Clear 32 bytes; LE Type u16=0x13 at +0, routine u32 at +12, context u32 at +16; EAX=0 |
| 109 KeInitializeInterrupt | object, routine, context, vector, IRQL, mode, share / 28 | Clear 44 bytes; routine/context/vector u32 at +0/+4/+8, remaining inputs ignored by the body; EAX=0 |

ESP also consumes the four-byte dummy return. Other CPU fields are unchanged.
The existing generic dispatcher validates all declared argument bytes before
mutation, including the interrupt body's three ignored inputs. Mediation errors
leave CPU unchanged, but earlier accepted memory writes/caller metadata are not
rolled back. Stores preserve original clear/type/routine/context/vector order.

## Buffer behavior

`kernel_buffer_mapped(address:u32, bytes:u32) -> bool` preserves the pinned
`bridge_va_mapped` rule for this reviewed retail profile: default 64MiB RAM,
map_size=0, plus the separate 64MiB contiguous range at 0x80000000. A zero-size
buffer is accepted without access; otherwise the address must be >=0x1000 and
the widened end must fit one of those ranges. Ordinary mirrored/tiled views do
not enlarge this original buffer rule. Configuring a different memory profile
requires a corresponding reviewed extension, not guessing the span geometry.

For invalid DPC/interrupt buffers, the original void export succeeds with EAX=0,
writes no guest bytes and warns once for that export. The numeric warning is
best effort, level 2, code `0x544b1700`, values `[ordinal,address,bytes]`.
The seen bit changes before emitting it, as in the original helper. The two
available exports are fewer than the original helper's 16 distinct-name slots.
Concurrent callers enforce the intended once rule with bounded CAS rather than
reproducing the original unsynchronized C static-table race.

A logically valid buffer must still pass the host's current owned-window,
view/range, commit and page checks. Denial of the initial clear leaves RAM and
CPU untouched, including complete scatter validation across backing segments.
Denial after a successful clear retains that clear and any successful earlier
field stores, with CPU unchanged. The HAL byte output is also capability-checked;
a null output performs no access. No C-fault recovery or out-of-grant access is
invented for invalid original native pointers.

## KSV3 service metadata

The warn-once state must survive kernel-map reinstallation and be shared by
callers. KSV3 keeps the existing **64 bytes**, replaces the old reserved u32 at
+4 with warning bits (bit 0=DPC, bit 1=interrupt), and uses magic `0x3356534B`.
All other fields retain their previous offsets/semantics. Old KSV1/KSV2,
unknown warning bits, malformed configurations and invalid TSC fields reject;
wrong root is Denied. The 288-byte root, 244-byte CPU, four-ID context, SDK
identity and quotas are unchanged. This is private transient port state, not a
host ABI/save format.

107/109 require this configured root before import publication and on invocation.
44 remains independent and can prepare/run without configuration. A warning
claim pins the selected service ID and uses at most 64 CAS attempts with the
existing bounded retry pause. Other writers' video/audio/skew/TSC state is
preserved. Busy/exhaustion/invalid/revoked/wrong-root failures propagate without
changing CPU or replaying accepted operations. A failed warning claim does not
undo another writer's accepted skew changes. An accepted operation can finish
concurrently with owner revocation; stop/join users before releasing owners.

## Qualification

Maintainer `check.py --app APP --app-lib EXACT_DEBUG_RLIB --wine` authenticates
the source kit, complete original buffer helper/initializer bodies, header
constants, handler lookup and exact stdcall counts. Original C runs only in an
inert bounded backing; no oracle/override enters the checked game package.

- 768 calls per target compare every CPU byte and all 262,144 backing bytes,
  including unaligned addresses, argument/output overlap, aliases, nulls,
  low-page/upper-bound/overflow refusals and warning state across map reinstalls.
- Maintainer-only injections check later-write prefixes, Failed/Denied/Unsupported
  metadata errors, 63/64 CAS boundaries, accepted competing skew and retained TSC
  origin. They are not actual platform-failure or gameplay measurements.
- Uninjected fixtures scan every source before host bindings, separately optimize
  game objects and audit all before trusted linking on Linux/Windows x64/i686.
  Actual confined Linux x64/i686 calls match the C CPU/RAM-hash/seen-state corpus.
- Real negative controls cover read-only host/guest views, missing scatter bytes,
  incomplete ignored arguments, two concurrent callers with a live same-root tick worker, malformed/old schemas,
  foreign/revoked configuration, independent HAL operation without configuration
  and refusal to publish unconfigured 107/109 thunks.

Windows runtime component comparisons execute under Wine; actual Windows
AppContainer, physical controls, audible audio, visual fidelity and frame pacing
remain unqualified. The supplied ISO is read in place by the separate import
check: 108 main functions remain unresolved. KeConnectInterrupt, DPC execution,
timer/DPC/IRQ scheduling, complete generated graph, device/renderer/media and
production title build/launch must migrate before enabling Burnout.

Current admission compares 352-byte witnesses per initializer call: the complete
CPU v5 record (316 bytes) plus the existing 36-byte RAM/state witness. The older
fixed 280-byte admission expectation was corrected; no CPU bytes were truncated
or native comparison weakened. The 768-call native/source/object/confined suite
is requalified with the shared KPL4-compatible kernel source.
