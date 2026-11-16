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
