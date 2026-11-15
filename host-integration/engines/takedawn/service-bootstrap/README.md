# Checked CPU startup data

This dependency-free port component initializes the original toolkit region,
stack, main-thread TIB/FS and TLS data through GamePKG-owned non-executable RAM.
It follows the authenticated [XBE image loader](../service-image/README.md).
It does not execute the entrypoint, allocate a host thread, resolve kernel
imports, initialize devices or enable the Burnout profile. The current reused
memory adapter requires app `c775eca`; see the cross-span contract below. CPU
initialization itself adds no permission, host API or import exemption.

## API contract

`Bootstrap::boot_plan(&Image, &Memory) -> Result<Bootstrap, i32>` requires a live
checked call record with the image loader's published executable bounds. It
writes no RAM. It computes fixed-width guest addresses, validates stack/heap
space and TLS extents, and snapshots at most 65,520 initialization bytes in
child-owned memory. The snapshot and record identity remain private. RAM and
its record remain caller-owned. No raw pointer or executable mapping escapes.

The toolkit region starts at the greater of `0x700000` and the image high
address rounded up to 64 KiB. The stack starts at region + `0x80000`; its normal
size is 8 MiB. The original low-space stack decision is preserved, including its
unsigned subtraction, but an impossible resulting stack/heap is rejected before
writes. In particular, the original underflow for some available sizes below
8 MiB cannot publish an out-of-RAM stack here. At least 1 MiB must remain for
the heap. This component records the heap base; it implements no heap allocator.

A nonzero TLS directory must fit 24 bytes inside the loaded image. Initialized
TLS data must also fit the loaded image unless its length is zero. Reversed
ranges, overflow and a rounded TLS block larger than its 64 KiB reserved slot
are Invalid (-1). A zero-length initializer does not dereference its source
address. The thread-data address at TLS block + `0x200` and its 64 bytes must
fit the reserved toolkit region.

`boot_layout() -> [u32; 14]` returns, in order:

1. Toolkit region base.
2. Stack base.
3. Stack byte count.
4. Initial ESP (stack end minus 16).
5. Initial heap base.
6. Heap upper bound (`0x4000000`).
7. Main-thread FS base (`0x4000`).
8. TLS template address, or zero without TLS.
9. TLS total bytes, or zero without TLS.
10. Thread-data address, or zero without TLS.
11. Thread-data byte count (64).
12. PRCB address (region + `0x61000`).
13. Fake-TLS address (region + `0x60000`).
14. Kernel-data address (region + `0x40000`).

`boot_apply(&mut Memory) -> Result<Registers, i32>` requires the same live call
record used by the plan. It writes the original TIB fields, clears the PRCB,
sets the fake-TLS pointers, and then performs TLS initialization in the original
order: clear the TLS block, clear the thread-data block, copy the snapshotted
initializer, then replace the first TLS word with the thread-data pointer.
This order matters for initializers overlapping the fixed thread-data address.
The original TLS convention then changes TIB `[FS+4]` to the TLS block end;
it does not change the real ESP. The TLS index variable remains untouched,
as in the original initializer. Fresh returned registers retain their normal
zero/default floating-point state, with FS and ESP set only after all writes
succeed.

Revoked/stale records or a different RAM record are Denied (-4); checked-memory
and transport errors propagate. A failure during application can leave earlier
writes. The operation is not an atomic RAM transaction: callers must abort and
must not enter the game with a failed or incomplete bootstrap. This API does not
make guest initialization immutable against other game-owned RAM operations.

## Local qualification

`reference_inputs.json` pins the source kit, original C/header and exact byte
ranges of the original region and TIB/TLS initialization blocks. The maintainer
oracle inserts those unchanged blocks into an inert, bounded RAM wrapper with
silent diagnostics. It has no OS/device/window/audio calls and is never linked
into game or trusted runtime objects. Invalid ranges never enter that C oracle.
The source kit's upstream notices are retained in the test package.

Actual confined Linux32/64 children each passed 33 cases: 19 valid synthetic
layouts, eight invalid synthetic layouts, five lifecycle/bounds/permission
controls, and the supplied private ISO. Every case compares all 64 MiB of RAM
through 64 independent SHA-256 chunks. Successful cases additionally compare
the complete 244-byte register record and all 14 layout fields against the
original initialization. Rejected plans and the tested denied applications
leave the loaded image unchanged and publish no successful CPU context.

The synthetic matrix covers absent TLS, empty/nonempty initialization, padding
boundaries, initializers overlapping thread data, the maximum accepted TLS
extent, one byte beyond it, arithmetic overflow, reversed/out-of-image/truncated
ranges, a directory exactly at the image end, an unused invalid source address,
and the original stack-size decision boundaries. Controls revoke the record
before planning or application, use a different RAM record, protect the first
TIB page read-only, or replace the published code bounds.

The real ISO is independently authenticated against its pinned 716-file
manifest before granting the same open file to the child. Its startup layout
matches the original: region `0x780000`, stack `0x800000` with 8 MiB, initial ESP
`0xFFFFF0`, heap `0x1000000`, FS `0x4000`, TLS block `0x7F0000` with 148 bytes and
thread data `0x7F0200`. The image stays in place; nothing is extracted or
published. This test stops before native entrypoint execution.

The complete game source tree is scanned before bindings are injected. Each of
two separately optimized game objects is audited before trusted linking for
Linux32/64 and Windows32/64. An unused direct-filesystem/unsafe source also
remains rejected. Windows results are build/admission checks only, not Windows
runtime, AppContainer, controls or gameplay qualification.

```sh
python3 host-integration/engines/takedawn/service-bootstrap/check.py \
  --app /path/to/GamePKG/app \
  --app-lib /path/to/target/debug/deps/libgamepkg-HASH.rlib \
  --iso /path/to/your/own.iso
```

Python and the C reference are maintainer-only. Users receive prepared Rust and
native GamePKG tools. The ISO argument is optional for synthetic qualification.

## Remaining integration

Implement the original heap allocator and kernel data/export semantics, the
reviewed RAM mirrors and distinct device/contiguous-memory windows, and the
remaining generated/manual/media functions. Connect the verified renderer,
audio/input and launch lifecycle without direct SDL/OS calls. Only then qualify
native entry, saves, physical controls/rumble, audible audio and frame pacing.
The legacy profile gate remains enforced. These CPU-data tests establish none
of those gameplay claims.

The reused memory adapter now also requires app commit `c775eca` for its
explicit cross-span fallback (shared service identity `86521a787ece4bd0`).
The CPU startup APIs and original reference semantics above are unchanged.

The [checked heap data component](../service-heap/README.md) now preserves
allocation/reuse/coalescing/query and zero-fill semantics against the original
helpers. The shared-task owner/client now has separate concurrency and lifecycle
qualification. Public kernel wrappers, actual title-thread startup and diagnostic
reporting still require integration.


The current fixture uses the [retail three-bank guest layout](../service-memory/README.md).
Main RAM plus 28 mirrors share one 64 MiB backing; Contiguous/tiled/flash spans
are attached but remain uncommitted during this CPU-data test. For successful
startup it observes the loaded image, TIB/TLS and stack through every main mirror,
while preserving all original CPU and whole-main-RAM comparisons. This is an
explicit fixture migration to `TakedawnMemory`, not a legacy-launcher exemption.
The title's native entry and other kernel/device work remain gated.
