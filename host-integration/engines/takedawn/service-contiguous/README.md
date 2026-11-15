# Checked contiguous allocator data

This dependency-free Rust component preserves the pinned original contiguous
page allocator's data behavior using GamePKG-owned RAM access. It exposes no
OS allocation, native pointer, raw page-map reference, device access, Cargo
dependency or successful unmapped-memory fallback. It now connects to the existing
[shared allocator owner](../service-heap/SERVICE.md#contiguous-extension-private-protocol-version-2).
**Kernel wrappers, guest-thread startup and native entry remain incomplete**.

Requires the app's reviewed three-bank `LocalLayout::TakedawnMemory` preset,
implementation [`f4bdfea`](https://github.com/centrexity/GamePKG/commit/f4bdfea).
Full shared-service identity:
`c36219e59748d51107cbc5be62593dbcc119cd58b4d9763a09c3d959b3bb73e8`;
composite native recipe identity: `e5624d54d141c27b`. These are different
identities. The old one-bank preset is insufficient. The game still receives
only opaque window IDs and offsets. All backing/metadata remain charged under
the existing runtime budget, child cap and signed limit.

## Exact owned-data API

`Contiguous::contiguous_new(&Memory, window: u64) -> Result<Contiguous, i32>`
requires the live root call record and the exact three-bank enumeration
(64 MiB, 64 MiB, 1 MiB). `window` must be the second bank's opaque ID; the first
64 MiB main bank, flash, zero and guessed host addresses are rejected. The
constructor does not commit pages or change permissions. It owns 16,384 byte
page states and 16,384 u32 block lengths, approximately 80 KiB plus fixed fields.
Only page zero initially has state 2 (reserved). The initial high-water mark is
4 KiB; the first free page is 1. This owned state is not Clone or a shared static.

All allocation/claim/free/size methods require the same live root call-record
association. Another or revoked root returns Denied (-4) before mutation.
These IDs coordinate game state; they do not replace host capability checks.

- `contiguous_allocate(&Memory, size, alignment, phys_low, phys_high) -> Result<u32, i32>`:
  size zero becomes one byte; round size to whole 4 KiB pages. Alignment is at
  least 4 KiB and non-powers-of-two round upward using the original u32 behavior,
  including the original 0x80000000 fallback on rounding overflow. Physical
  lowest rounds upward to a full page and highest is inclusive. First fit begins
  at the original first-free bound and honors both alignment/range constraints.
  Ordinary exhaustion returns zero. A successful guest VA is
  `0x80000000 + page * 4096`, never a host address.
- General allocations exclude the top 128 KiB GPU-instance reserve. Requests
  whose page-range limit extends above 16 MiB skip the 15..16 MiB low reserve;
  requests constrained below it may use that reserve. Page zero is never handed
  out, even with a physical lowest of zero. Returned pages are fully zeroed,
  including bytes beyond the requested byte count up to the rounded page end.
- `contiguous_claim(&Memory, address, size) -> Result<(), i32>` records an existing
  block without clearing it. Unaligned byte ranges claim every intersected page.
  Zero size, below-window, overflow/out-of-window and any already-used/reserved
  page are quiet no-ops, preserving the original void-call semantics. Explicit
  claims may occupy the GPU-instance reserve. They cannot claim page zero.
- `contiguous_clear_claim(&Memory, address, size) -> Result<(), i32>` validates
  a complete bound-bank extent, clears exactly those bytes, then performs the
  original intersected-page claim. A zero-sized request does not claim pages;
  original overlapping/no-op claims do not undo the requested clear. This is
  the data action used by the fixed-range kernel bridge, not qualification of
  its branch conditions, stack cleanup, returned VA or diagnostics.
- `contiguous_free(&Memory, address) -> Result<bool, i32>` releases only a
  page-aligned allocation/claim start. Null/outside/unaligned/interior/double free
  returns false; success returns true. Freed RAM is not cleared. First-free can
  move backward; high-water and peak never shrink.
- `contiguous_size(&Memory, address) -> Result<u32, i32>` returns bytes remaining
  to the end of the containing live allocation, including unaligned interior
  addresses. Reserved page zero, free pages and outside addresses return zero.
- `contiguous_stats() -> [u32; 6]` copies in-use pages, high-water bytes,
  first-free page, allocations, frees and peak pages. Operation counters retain
  original unsigned u32 wrapping behavior. This is copied diagnostic metadata.
- `contiguous_record(page) -> Result<[u32; 2], i32>` copies page state and start
  length in pages, with page < 16,384. `contiguous_pages(first, &mut [u8])` copies
  at most 4,096 page-state bytes and validates the complete range before writing
  the output. Empty copies at the end are valid. Neither returns a mutable table
  reference or the original native volatile page-map pointer.

Complete allocation zeroing uses `local_memory_fill` on the bound contiguous
bank before publishing any metadata. A late read-only or uncommitted page
therefore leaves both all RAM bytes and metadata unchanged. This is explicit
error behavior for invalid host page access, not an oracle claim about recovery
from an original native fault. Checked errors propagate; no successful result
or platform fallback is substituted. A claim is metadata-only and does not
implicitly clear, commit or grant access to its RAM pages.

## Qualification

`reference_inputs.json` pins the complete source kit, memory source, kernel
header, allocator function bodies and relevant macro values. `check.py` verifies
those hashes, exact selected byte spans and constants before constructing its
maintainer-only C oracle. It uses the exact original take/allocate/default-
allocate/claim/free/size/high-water/in-use functions. Logging/reporting and locks
are inert only in the single-threaded value oracle; C and its raw pointers never
enter checked game source or trusted runtime objects.

Native Linux32/64 and Windows32/64-under-Wine each compare 92,213 operations in
five cases, including an empty case. Every operation compares result and all six
state counters. Final comparisons cover every byte of 64 MiB RAM and every one
of the 16,384 page-state/length records. Cases cover all-size/alignment edges,
physical-range boundaries, both reserves, complete exhaustion/reuse, fragmented
claims, unaligned/interior/duplicate frees and deterministic mixed workloads.

The admitted Rust replay fixture is separate from that C oracle. Complete source
is checked before SDK injection; two game units are optimized independently and
every game object is audited before trusted linking on Linux32/64 and
Windows32/64. Actual confined Linux32/64 children replay all cases, with per-
operation result/state comparisons plus independent host SHA-256 comparisons
of all RAM chunks and the complete canonical page table. The host also verifies
the final counters. Main and flash banks are not used as allocator storage.

Controls deny wrong-bank handles and wrong/revoked call records. Fresh and reused
allocation failures on late read-only pages preserve metadata and earlier RAM;
uncommitted-page failure, copied-map bounds and unchanged failed output are
checked. Ten malformed/forged fixture cases (header, magic, count, length,
opcode, result/state witnesses and oversized fixture writes) reject without a
success witness. An unused filesystem source still fails packing; no scanner,
import, resource-budget, confinement or legacy profile exception is introduced.

```sh
python3 host-integration/engines/takedawn/service-contiguous/check.py \
  --app /path/to/GamePKG/app --app-lib /path/to/libgamepkg.rlib --wine
```

Without `--app-lib` this runs only the original-code reference comparisons.
Python and C are maintainer tooling; users receive generated Rust/native tools.
All fixtures use synthetic RAM, hidden output and no ROM, game window, physical
input, real audio or timing measurement. Windows builds/Wine do not qualify
Windows AppContainer execution or actual gameplay.

## Shared-owner qualification and remaining work

Add `--shared` to the command above to use the actual version-2 owner/client path.
`service_reference.rs`, `service_probe.rs` and `service_admission.rs` separately
replay the same original corpus through copied requests. Every operation and
six quiescent counter observations match the C oracle; complete RAM and page
records are independently compared again. The four extra vectors use exact
pinned clear/claim statements from the original kernel source, including an
unaligned/overlapping range, reserved page-zero bytes, the final bank byte and
zero length. The surrounding kernel branch/stack/return behavior is not claimed.

`shared_checks.rs` exercises mixed v1/v2 framing, rejected owner replacement,
unsupported unconfigured operations, missing/trailing payload, bad versions,
sequence gaps/replays, reserved fields and explicit combined-free/size fallback.
Four callers hold 1,000 paired Heap/Contiguous allocation iterations plus four
fixed-range claims. They verify exclusive held bytes in both banks, correct
interior sizes and exactly 1,004 Contiguous allocations/frees after joining.
Ownership checks stop when a block is released: another caller may immediately
reuse it. Final accounting is checked only after all callers finish.

Invalid fixed clears and late read-only pages preserve counters and earlier
bytes. Root revocation denies observations/mutations while shutdown still works.
Five fake-peer modes test wrong reply version/sequence, short response, invalid
boolean and invalid status, including no further request after poisoning. A
separate peer acknowledges consuming a v2 request before the caller is cancelled;
the caller returns Denied, poisons its client, sends no replay, releases its queue
and remains joinable. These controls use the same finite reviewed task table,
GamePKG RAM/queues/tasks and existing budgets, not platform fallbacks.

The original v1 owner checks still exercise 2,000 concurrent heap lifetimes,
queue/membership/correlation/revocation/fatal-peer and cancellation controls.
All dependent image/bootstrap/heap/native fixtures now stage Contiguous as normal
checked game source before host-generated bindings. No dependency, source,
object-import, budget, confinement or legacy admission rule is exempted.

Next wire reviewed original contiguous kernel ordinals, fixed-range/instance
claims, free fallback and the contiguous-first size query through these methods.
Ordinal 180 remains Unsupported until its complete kernel wrapper is qualified.
Preserve original reporting, bounded OOM/allocation diagnostics and device
page-map use. Guest mirrors/tiled views, full device semantics, kernel/data
imports, thread/TIB/IRQ startup, remaining generated/manual/media/renderer paths
and native entry remain required. The title build/play/cache/export gate stays
closed; neither shared-owner tests nor Wine establish actual gameplay, physical
controls, audible output or measured frame pacing.
