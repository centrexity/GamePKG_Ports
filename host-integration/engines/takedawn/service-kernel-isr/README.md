# Guest interrupt registration and native ISR calling convention

These checked dependency-free port modules implement the reviewed registration
and ISR calling convention. They add no trusted host engine, OS call, native
pointer, function value, game dependency or SDK operation. Component qualification
is complete for the scope below; production device handlers and title admission
remain unfinished. Preserve the current builtin/native profile gates.

`reference_inputs.json` pins the public source archive, original bridge/HAL/type
files and exact function/structure ranges. The new checked guest implementation
is qualified against those inputs for this component. Full source-kit/import
integration must retain the exact identities and pass its own complete source,
object and confinement checks. Compilation alone is insufficient.

## Contracts

`connections.rs` implements a root/KSV-bound 32-vector table of **guest u32
addresses**. `Connections::connections_new`, `connections_id`,
`connections_borrow`, `connection_get`, `connection_connect`,
`connections_close` and `connections_closed` own or borrow one ISR1 record.
The 192-byte record has eight LE u64 header words (magic 0x31525349, root, KSV,
five reserved zeros) followed by 32 LE u32 slots. No routine is converted to a
host address. Valid serial behavior follows the bridge: connect overwrites its
selected vector; null or out-of-range connect still reports 1; out-of-range get
returns zero. Changing an object's vector does not clear its earlier slot.

`connection_connect` returns [result, vector or u32::MAX for null, routine,
context]. This retains the original successful connection's diagnostic values
for a later bounded reporter. Out-of-range requests do not read routine/context.
Slot publication precedes those original diagnostic reads; a later checked-read
or revocation error can leave a published slot and must not be retried blindly.
Updates use bounded full-record CAS with the existing retry service. Ordinary
operations require live root/KSV and an uncancelled task. Close is actual-creator
only, including cleanup after cancellation/revocation, and releases the copied
record before reporting its liveness error. Borrowed close rejects. Stop/join
callers before owner teardown; a copied slot is not a routine or memory lease.

`interrupt_initialize(&mut Memory, guest, routine, context, vector)` clears and
writes the original 44-byte **guest** object at +0/+4/+8. It does not cast a host
KINTERRUPT. Null returns false without access. Other invalid ranges or permissions
return checked errors; final import-ABI invalid-buffer behavior still requires
review. Published write prefixes are not rolled back. `interrupt_vector` returns
the original full u32 level and, for a nonzero guest destination, writes its low
u8 IRQL byte. Neither helper registers an IRQ queue or delivers a device event.

`call.rs::isr_call(&Connections,&mut Memory,&mut irql::Task,&mut Registers,
&Lease,vector) -> Result<i32,i32>` ports `kernel_call_isr`. A missing connection,
null routine or absent finite reviewed route returns **Ok(-1)** without argument
writes, as in the original lookup path. This is not permission to omit title
handlers: complete generated targets/routes and full admission remain required.
The caller must already hold its checked single-CPU gate, as in the original
vblank/IRQ path; this helper does not acquire a second caller guard. A callable
route validates the actual task and retained guest stack/TIB, pushes
context/object/dummy return in original order, enters IRQL16, starts the supplied
port profiler, calls a literal native Rust route, ends profiling and restores
IRQL. Native stdcall must consume 12 bytes. Success returns EAX & 1. Errors after
publication attempt IRQL/gate abort and preserve the partial prefix; they do not
report an uncallable ISR or fabricate successful completion. The caller retains
stack release, actual join and owner teardown obligations.

The `isr_routes` and `isr_observer` modules still need real finite generated
handlers and original profiling adaptation. Fixture providers must not be
promoted to production. This layer does not implement NV2A register changes,
acknowledgements, vblank counters, APU/OHCI, spin wake or the timer loop.

## Disconnect layout gap

The original bridge's `KeConnectInterrupt` updates `g_connected_isr`; it does not
call the HAL struct helper to set a native `Connected` field. Its disconnect
wrapper calls the separate HAL `xbox_KeDisconnectInterrupt`, which returns and
clears that struct field and does not clear the bridge's vector table.

The exact original structure and disconnect body were compiled and executed
with their fixed-width scalar/pointer typedefs on four targets:

| Native target width | Struct bytes | Connected offset |
| --- | --- | --- |
| Linux/Windows 32-bit | 20 | 16 |
| Linux/Windows 64-bit | 32 | 24 |

In every case an initial byte 0xa5 was returned unchanged and cleared; null
returned zero. Windows execution here used Wine. These are original structure
layout checks, not a title or disconnect-ABI qualification. Do not implement
ordinal 100 by selecting a host-size-dependent guest offset, silently deleting
registry entries or guessing a replacement guest layout. Resolve the intended
fixed-width contract and its original behavior explicitly before enabling it.

## Qualification and remaining integration

Run `check.py --app APP --app-lib APP_RLIB --wine --evidence DIRECTORY`.
The command authenticates every original file and exact inserted bridge function.
Maintainer C asserts and function pointers stay in the separate oracle, never in
game objects or the trusted host runtime. Python/C are not player requirements.

128 original-C cases per target compare the entire 32-vector table, both 44-byte
objects, null/out-of-range/overwritten connections, missing/null/primary/manual
routes, low-byte vector output, three stdcall arguments, full EAX and masked
return value, ESP, previous IRQL/PCR and profiling order. Cases span old levels
0/1/2/16, profiling off/on and vector 3/31. A real reviewed SDK worker retains its
stack and own heap-allocated TIB throughout, with a separate serialized heap
actor. Complete CPU records are checked for unintended clobbers. The caller-held
single-CPU gate is part of the fixture setup and released after each normal call.

Ten native error/lifetime modes cover body failure, a bad stdcall return with
profiling **off**, profile-begin/end errors, insufficient guest stack, read-only
argument and PCR pages, cancellation and KSV revocation while native code holds
the actual gate, and a missing TIB. They verify partial argument/EAX/ESP/PCR and
profiling state, error precedence, actor abort, rejection of replay, successful
retry after preflight failure, released gates/counts, retained-stack lifetime,
real joins and stable worker finish results. These typed-error cases have no
original C exception equivalent and are explicit port failure contracts, not an
independent original-C error-path comparison.

Registry controls reject borrowed close, foreign roots, malformed magic/reserved/
root/KSV metadata, arithmetic overflow and read-only initialization. A partial
memory view verifies that connect publishes its vector **before** a later
routine-read failure. No rollback or success is fabricated. Null initialization,
no-destination vector output, creator idempotence and revoked close are checked.

All Linux/Windows x64/i686 targets agree on 35,968 witness bytes. Windows
component execution uses Wine. Eight required-failing copies alter EAX masking,
argument order, stdcall validation, profile-end error propagation, fatal abort,
manual-route support, vector 31 or borrowed close. Complete source scanning is
before SDK injection; all eight separately optimized game objects per target are
audited before trusted linking. Actual confined Linux x64/i686 children run the
same cases. An unused maintainer reference shell in game source rejects.
SDK remains `fe309935dd602e2cf098e50065f6035de33dac88b51d45a2bc1ba47cf026742f`.

The literal ISR routes, profiler and IRQ hooks here are controlled fixtures,
not real title/device/profiler implementations. Remaining: production shared
registry/context publication, import-ABI wiring (including invalid-buffer policy
and the separate disconnect review), actual finite generated handlers and NV2A
register/ack behavior, APU/OHCI, timer/vblank/spin/pause/warp/wait integration,
full generated source/build/launcher closure and actual gameplay/GPU/input/audio/
pacing qualification. No new import or title profile is enabled; candidate
closure counts, installed app SDK/binaries, pinned submodule and user ROMs/saves
are unchanged.
