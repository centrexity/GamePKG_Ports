# Checked retail guest memory views

`layout.rs` supplies the original 64 MiB retail RAM topology over GamePKG's
three-bank `LocalLayout::TakedawnMemory`. This is port-owned address translation,
not a device model, host mapping API, CPU interpreter or title admission path.
No app SDK change is needed. Use shared-service digest
`c36219e59748d51107cbc5be62593dbcc119cd58b4d9763a09c3d959b3bb73e8`, composite
native recipe `e5624d54d141c27b`, available from app `f4bdfea`.

## Contract

`Layout::layout_current(&Memory) -> Result<Layout, i32>` requires a live valid
call record and the exact 48-byte host enumeration: distinct nonzero opaque IDs
for 64 MiB main RAM, 64 MiB Contiguous RAM and 1 MiB flash. An incompatible
one-bank profile returns Unsupported (-5); malformed sizes/IDs return Invalid
(-1). Host and call-record errors propagate. There is no truncation, independent
allocator, guessed backing or larger-memory fallback. The plan binds its root
call-record ID and these backing IDs. It allocates or commits no pages.

`layout_banks() -> [u64; 3]` copies the IDs in main/contiguous/flash order. IDs
are not host pointers and remain subject to host ownership/lifetime checks.
Reading this copied metadata does not renew a grant or un-revoke a runtime.

`layout_view(&Memory) -> Result<Memory, i32>` requires the same live root and
constructs a fresh view. It preserves the source's shared call/kernel/heap
endpoint context through `call_view()` but copies no exclusive HeapClient,
response queue, sequence or in-flight operation. It does not modify or transfer
the source's existing spans/client. Install the primary owner into the completed
coordinator view; worker callers independently adopt their context and create a
view, then lazily register their own client. Failure drops only the partial
view, without releasing the borrowed root or modifying RAM.

Exactly 32 nonoverlapping guest spans are attached, all at backing offset zero:

| Guest extent (exclusive end) | Backing |
| --- | --- |
| `[0, 0x04000000)`, plus 28 consecutive 64 MiB spans through `0x74000000` | The same main bank |
| `[0x80000000, 0x84000000)` | Contiguous bank |
| `[0xF0000000, 0xF4000000)` | The same Contiguous bank |
| `[0xFF000000, 0xFF100000)` | Independent flash bank |

Guest zero is represented as in the original default layout. Explicit host
page protections can deny it through every main alias. No arbitrary address
masking is used. Device apertures at `0xFD000000` (NV2A) and `0xFE800000` (MCPX),
the synthetic kernel function range and all other holes have no RAM span.
They fail rather than returning invented register values. A future device
adapter must supply actual semantics; it must not add zero-filled MMIO views or
increase the 32-span limit to make device accesses appear successful.

All spans permit writes at the translation layer, matching the original mapping;
GamePKG independently validates every page's commit/protection state. The plan
changes no protections and initializes no memory. Initially inaccessible banks
must be explicitly committed by startup. Fresh host commits are zeroed. Flash
is the original zeroed writable aperture, not a BIOS image. Committing the same
live page is not an authorized method of resetting a game. Root-record
revocation prevents creating further views; it does not itself revoke already
attached RAM. Host page protections/decommit/runtime teardown control those RAM
lifetimes. Shutdown must coordinate callers, owners and runtime teardown.

## Integration and qualification

The CPU-startup fixture now stages this module as ordinary checked source and
uses all 32 views. It commits only main RAM and proves authenticated image,
TIB/TLS and initial stack writes are visible through all 29 main aliases.
Contiguous, tiled and flash remain inaccessible until explicitly committed;
device holes stay denied. Original CPU/layout and whole-main-RAM comparisons
are retained for all synthetic cases and the authorized ISO read in place.
This does not run the native title entry.

The nine-wrapper allocator fixture and its four concurrent callers also use
this layout, preserving per-caller context and the single shared owner. Its
2,115 pinned-original C wrapper comparisons per target and all rejection,
concurrency and bounded-refusal controls remain unchanged. The C oracle is
maintainer-only; no C is admitted into game or trusted runtime objects.

`layout_checks.rs` is ordinary admitted game source. It exercises every ordered
main-alias pair at zero/unaligned/page/end offsets, all 28 cross-mirror boundaries,
contiguous/tiled aliasing, separate-bank isolation, flash edges, a rejected 33rd
span, unbacked device/hole addresses, wrong-root/revoked-root plans, page
protection/decommit/recommit, actual aliased-copy overlap rejection and no partial
RAM/output changes on denied cross-mirror scatter/gather. The native harness
also checks the incompatible one-bank profile and runtime teardown/reuse.
`reference_inputs.json` pins the exact original source/header and relevant
mirror/tiled/flash regions. Those pins establish reviewed provenance, not replay
of the original platform mapping calls.

```sh
python3 host-integration/engines/takedawn/service-memory/check.py \
  --app /path/to/GamePKG/app --app-lib /path/to/libgamepkg.rlib --wine
```

The matrix compiles/runs synthetic conformance on Linux32/64 and Windows32/64
under Wine, then independently scans complete fixture sources before bindings,
optimizes separate game objects and audits every object before trusted linking
on all four targets. Actual confined Linux32/64 execution is separate from Wine.
The child remains 256 MiB and the signed fixture limit 384 MiB; all 129 MiB
backing and metadata remain charged under the existing runtime limits.

These checks open no window/audio/device and render no frame. Windows
AppContainer execution, device fidelity, physical controls, audible audio and
frame pacing remain unqualified. Complete kernel/data exports, guest-thread/
TIB/IRQ startup, generated/manual/media/renderer routes and native entry are
still required. Keep the title build/play/cache/export gate closed. Python is
maintainer tooling only; users receive Rust/native tools.

## Explicit service scratch topology

`Layout::layout_services(&Memory)` is the opt-in four-window companion for
host `LocalLayout::TakedawnServices`; `layout_scratch()` returns its fourth opaque
1 MiB ID. The original three-window entry remains strict. Guest views keep the
same three-bank aliases and do not map scratch. A matching guest number still
resolves to its checked guest backing, never to a host address. Caller-owned
commit and partial resource cleanup contracts are described in
[OBJECT_STARTUP.md](../service-kernel-context/OBJECT_STARTUP.md).

## Guest page protection imports

The checked import dispatcher binds MmQueryAddressProtect (179, one 32-bit
argument) and MmSetAddressProtect (182, three 32-bit arguments). The pinned
source-kit reference is toolkit/src/kernel/kernel_memory.c. These adapters use
the same GamePKG local pages as ordinary guest reads and writes.

Query takes a guest virtual address and returns the current backing page's
Xbox protection flag. Reserved backing reports 0, no-access and unmapped
addresses report 1, read-only reports 2, and read/write reports 4. Runtime
revocation and service errors propagate.

Set accepts a guest address, positive byte length, and exactly one of
PAGE_NOACCESS, PAGE_READONLY, or PAGE_READWRITE. It rounds the guest range
outward to 4 KiB boundaries and applies one checked request to a single
translated backing span. Aliases of those pages observe the change immediately.
A range crossing a span boundary is explicitly unsupported by the adapter.
Wrapping, unmapped or uncommitted ranges are rejected before modifying any
page. Executable, guard, and cache attributes are unsupported, and no host
address or executable mapping is exposed. The checked dispatcher advances
the guest stack and records the caller only after a successful operation.

The authenticated startup fixture covers read/write, read-only, no-access and
reserved queries, a two-page unaligned change through an alias, content
retention, and refusals for invalid flags and ranges. The real disc check
prepares imports in place. Full title execution remains gated.
