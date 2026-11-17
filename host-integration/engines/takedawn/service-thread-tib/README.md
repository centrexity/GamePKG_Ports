# Per-worker guest TIB and TLS setup

`thread_tib_new(&mut Memory, template: u32, total: u32) -> Result<u32, i32>`
ports the source kit's `xbox_AllocThreadTib`. Call it on the actual reviewed
worker through that worker's memory view and lazily registered heap client.
The heap allocator is the existing serialized port-owned actor; GamePKG owns
its tasks, mailboxes and checked non-executable RAM. No OS TLS, host pointer,
native thread handle or additional host permission is exposed.

Obtain `template` and `total` from the applied bootstrap's `boot_layout()`
indices 7 and 8, respectively. The production context/bootstrap must retain and
convey those values. Do not infer them from uninitialized main-TIB bytes or a
fixture constant. This helper does not publish that production configuration,
start the worker, allocate its retained execution stack, set CPU.fs_base, or run
a game routine. The caller must initialize those parts and handle every error
before entering native generated code.

A live root is required even without TLS. Zero total returns zero without
accessing the template or taking a heap client. Nonzero total must be a reviewed
bootstrap extent: 4 modulo 16, at most 65536, a nonzero template and no u32 range
wrap. Invalid metadata returns Invalid (-1) before allocation. An absent heap
service or revoked root retains the existing checked service errors. The
exclusive heap client is put back even if its allocation call returns an error.

The allocation is `64 + total + 64` guest bytes, aligned to 16. Allocation failure
returns zero, as in the original; it does not publish a usable worker context.
After allocation, preserve this order:

1. Copy the current main TIB's 64 bytes from guest 0x4000.
2. Copy the current TLS template, including its current slot zero.
3. Clear the new 64-byte per-thread data block.
4. Replace the new TIB's SEH word with 0xffffffff.
5. Replace new TLS slot zero with its own per-thread data address.
6. Replace the inherited TIB word at +4 with the new TLS block end.

All other inherited bytes remain unchanged. Reads occur at each call; this is
not a constructor-time snapshot. The caller installs the returned guest address
as its own CPU.fs_base only after success. Ordinary checked RAM denies overlapping
copies, unmapped or protected bytes. Failures after allocation can leave an
allocation and a published write prefix; there is no fabricated rollback or
usable TIB on error. Abort that worker path. Any resource release must respect
the actual worker join and guest heap lifetime; this helper does not free blocks
that another routine may still reference.

## Qualification

`check.py --app APP --app-lib APP_RLIB --wine --evidence DIRECTORY` authenticates
the public source archive and exact original function bytes. It also pins and
uses the original heap allocator in a separate maintainer-only C oracle. Neither
C code nor its reference harness enters admitted game objects.

Seven modes cover no TLS, minimum and larger TLS blocks through the maximum
bootstrap extent, and an exhausted heap. Each makes two allocations on an actual
SDK worker using a separate heap actor. Main-TIB and template bytes change between
allocations. Every mode compares both returned addresses, all allocation counters
and a hash of the complete 64 MiB RAM image with original C. Five malformed
metadata cases must reject before allocating. Additional controls check no-TLS
with an invalid template, an uninstalled heap service, and root revocation even
when TLS is absent. Four intentionally broken copies
must fail: SEH, slot zero, inherited +4 and per-thread data clearing.

All four Linux/Windows x64/i686 targets are compiled and compared; Windows
component execution uses Wine. The complete dependency-free game/helper tree is
scanned before generated SDK injection. All eight separately optimized game
objects per target are audited before trusted linking. The fixture executes in
actual confined Linux x64/i686 children and an unused reference harness rejects.
Full SDK: `fe309935dd602e2cf098e50065f6035de33dac88b51d45a2bc1ba47cf026742f`.

This component is not production worker-loop integration or title qualification.
Native callbacks, actual IRQ/device handlers, context/bootstrap publication and
full source/build/launch closure remain required. It establishes no title
startup/gameplay, graphics fidelity, physical input, audible audio or measured
frame pacing. Both unmigrated title profiles remain blocked.

## Checked system-thread launch component

The authenticated startup fixture now drives the PsCreateSystemThreadEx
argument layout through the game-owned launcher in
../service-kernel-context/thread_launch.rs. The launcher reads the ten Xbox
arguments and the return word before allocating. The reviewed caller supplies
the matching generated guest routine, audited Plan entry, and initialized
TLS template. The supported shape uses a 16-byte-aligned guest stack from
4 KiB through 256 KiB, with a 64 KiB default. It requires the requested TLS
size to match the provided template and refuses extra or debug-stack fields.

The creator owns one combined guest stack/TIB allocation and the copied
launch record. The same TIB initializer used by the differential tests lays
out the worker data. A real GamePKG task is created suspended, tracked by the
authenticated Startup owner, and associated with a typed kernel handle. The
worker validates its actual task identity against the copied record and uses
a fresh guest RAM view and fresh CPU state. Its original stdcall frame contains
a zero return sentinel and the two guest context words. Only a compiled direct
dispatch route runs. No OS handle or native function pointer crosses into
guest state.

The launch owner can resume the matching live handle or cancel a still-gated
task during teardown. Resource cleanup consumes the real join before removing
the handle and releasing the launch record and stack/TIB. A failed launch
after acquiring resources still requires owner drain. Busy remains retryable.
The 32-bit output ID is a runtime task identifier and the handle is a typed
guest token. Neither is a host pointer.

The signed fixture checks explicit resume, immediate start, cancellation of
a suspended job, rejection of a mismatched route and handle, the paired guest
outputs, and parent register preservation. It runs a short existing translated
stdcall body. The real Burnout system routine and import routing remain to be
connected. General guest wait, suspend, close and termination are separate
integration work. The full title import gate remains closed.

## First owned-image system-thread route

The pinned translated parent reaches the system-thread wrapper through
sub_001D18B0. In the authenticated image its import cell at guest address
0x0036b89c contains the raw ordinal 255 thunk. The parent passes
0x001D1818 as the system routine and 0x001D2793 as the guest start context.
The image fixture reports the original cell before import publication.
It does not execute the parent or the requested routine.

The prepared graph retains the parent and sub_001D18B0. It excludes
sub_001D1818 because of pointer-based operations still outside the lowering
rules. It excludes sub_001D2793 because sub_001D3F2F still has unreviewed
CLI/STI lowering and pointer types. A recognized guest address therefore
does not establish that the call can run. The generator now emits
routine_available from the same accepted address partitions used for
direct dispatch. Both the launcher and the actual worker require that
proof. The confined fixture submits the known but absent 0x001D1818 route
and verifies that no task is created and the guest outputs remain untouched.
