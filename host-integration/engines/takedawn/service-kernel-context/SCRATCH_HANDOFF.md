# Shared scratch service handoff

## Bounded scratch memory and atomic copied updates

The Rust SDK now provides
`local_memory_compare_exchange(window: u64, offset: u64, expected: &[u8],
replacement: &[u8]) -> Result<bool, i32>`. This is **child-local operation 42**,
a separate namespace from broker key-state operation 42. No pointer, mapped
slice, native object or GPU object escapes. Both slices must have identical
nonzero length, at most 65,536 bytes. The wire request is LE window u64, byte
offset u64, byte count u32, then exactly both copied slices. Reply is exactly
one LE u32: 0 stale, 1 replaced; malformed booleans/reply sizes fail.

The complete destination must be committed and writable before comparison,
even when its bytes would differ. The runtime memory lock covers validation,
comparison and the whole write, excluding other reads/writes/copies, protection,
decommit and runtime destruction. False has no write. Invalid bounds, overflow,
lengths or malformed packets return Invalid; absent/closed windows or denied
pages return Denied. This does not pin a window across later calls or provide a
version counter: callers needing ABA detection must compare a nonwrapping
version in their own copied format. It neither waits for a deadline nor blocks
the application's window owner; execution is inside the confined child.

The opt-in **host-only** `LocalLayout::TakedawnServices` preset provides the same
main 64 MiB/contiguous 64 MiB/flash 1 MiB windows as `TakedawnMemory`, followed
by a separate 1 MiB scratch window. All four reservations and per-page metadata
are charged before entry. Old presets and guest aliases are unchanged. A reviewed
port must explicitly consume the fourth descriptor and keep its scratch backing
out of the guest-address alias layout. The existing strict three-bank port
`Layout` reader still needs that adapter; do not switch a full title blindly.
Only the host chooses bases/layout/entry functions; game metadata cannot request
arbitrary executable memory, devices or native addresses.

Larger mutable port data belongs in an explicitly owned bounded memory range.
Opaque records retain **1..4096 bytes each, at most 64 records and 64 KiB total**;
there is no larger-record fallback or quota exemption. A failed dense handle-table
prototype demonstrated this distinction and is preserved privately rather than
published as admitted code. The ports task must still move its full 16,383-slot
tagged table onto scratch, bind root/range/page identities, retain partial setup
ownership, validate typed resource lifetimes, and stop/join borrowers before
releasing backing. No native handle may be reconstructed from a guest integer.

SDK identity is now
`1e15c059ca66907d45e603cec9f993ca54b9ffa66bae72c4a36cee67b944fd3f`.
Native source kits, audited objects and cache receipts must rebuild. Earlier
fe309935 component checkpoints remain historical, not relabeled as this SDK.
The image qualification tool derives its SDK from all four audited receipts
rather than stamping a fixed historical value.

Both native SDK suites pass on Linux32/64 and Windows32/64 through Wine (34 runtime
tests per target; 22 binding tests on Linux and 14 on Windows, with Unix-only client
tests omitted there). Tests cover concurrent single-winner CAS and
512 successful RMW increments, 64 KiB updates, stale unchanged bytes, overflow,
foreign IDs, malformed/trailing/truncated packets, short replies, cross-page
read-only/no-access denial, revocation and unchanged record bounds. A signed
fixture passes all four checked-source/two-object/link targets and actual confined
Linux32/64 execution, including the unchanged full 64 KiB record quota alongside
scratch. Windows confinement/device/gameplay and measured frame pacing are not
established. Existing authenticated startup is requalified separately on this SDK;
its full title remains blocked and the KIC2 main-import gap remains 106.
