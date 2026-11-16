# Checked guest worker-stack slices

This port-owned, dependency-free Rust component implements the pinned original
worker-slice allocator over GamePKG copied records and actual reviewed-task
identity. It allocates **guest address values**, not native stacks, host pointers,
executable memory or OS resources. It is a prerequisite for native IRQ, device
and timer callbacks, not a complete dispatcher or a playable Burnout integration.
The legacy profile remains blocked; no source/dependency/import exemption exists.

## API and original behavior

`stacks.rs` is staged as `stacks`, alongside existing `memory`, `kernel_pool` and
host-generated `gamepkg`. All fallible calls return `Result<...,i32>`:

- `Pool::stacks_new(&Memory, base:u32, bytes:u32) -> Pool` binds the pool to the
  current title root, live KSV3 record and actual creator. Supply the reviewed
  `Bootstrap::boot_layout()` stack base/size (indices 1/2). Base must be at least
  0x780000 and 64 KiB aligned; size is 0..8 MiB and 64 KiB aligned; end must fit
  the original 64 MiB guest RAM. Zero/partial-quarter sizes are supported by the
  allocator but do not constitute a usable title bootstrap.
- `stacks_id() -> u64`, `stacks_borrow(&Memory,id) -> Pool` provide opaque copied
  identity and a nonowning view. Borrowing validates format, root and KSV binding.
- `stacks_usage(&Memory) -> [u32;2]` reports [allocated, available slices].
  Availability is min(16, size / 262144), preserving shrunken large-image layouts.
- `stacks_allocate(&Memory) -> Option<Lease>` chooses the lowest free fitting
  slice using one atomic record CAS. All occupied returns **Ok(None)**, matching
  original -1 exhaustion. CAS contention has a separate bounded Busy result.
  There are at most 64 attempts; callers must handle failure, not invent a stack.
- `Lease::stack_slot(&Memory) -> u32`, `stack_guest_top(&Memory) -> u32` validate
  live ownership and return the index/top. Top is base + (slot+1)*262144 - 16,
  preserving original 16-byte alignment. Allocation/release never clear RAM.
- `stack_identity(&Memory) -> [u64;3]` returns [pool record, slot, generation].
  `Lease::stack_adopt(&Memory,id,slot,generation) -> Lease` reconstructs a token
  only for the **actual current owning task** and live generation. A copied token
  grants no cross-task authority. Slot >= 16 or generation 0 is Invalid.
- `stack_release(&mut self,&Memory) -> ()` frees only the matching actual task,
  slot and generation. It leaves the generation for stale-token detection. A
  released local Lease is idempotent; another still-active copy of an old token
  is Denied and cannot free a replacement. Generation overflow fails before
  mutation rather than wrapping. Inspect `stack_released()` after cleanup errors.
- `stacks_close(&mut self,&Memory) -> ()` is creator-only and refuses live leases
  with Busy. It atomically marks the pool closing, then releases the record.
  New work denies a closing pool; the owner can retry a partial close. Completed
  owner close is idempotent; inspect `stacks_closed()` after cleanup errors.

The original allocator uses per-slot atomic flags. This implementation publishes
allocation and actual owner/generation together, preventing duplicate ownership
or stale frees while preserving every valid serial original allocation choice.
It is independent of the native runtime's own reviewed worker stack allocation.
The title's ordinary thread stacks come from the heap in the pinned
`xbox_AllocThreadStack`, despite an older header comment claiming otherwise.
Do not create independent pools over the same title region: bootstrap must make
one pool and pass its opaque ID to every reviewed caller. This helper does not
deduplicate independently constructed pools or discover an unpassed bootstrap.

## State, errors and lifetime

One **576-byte WKS1** record contains eight LE u64 header words:
[magic 0x31534b57, root, KSV3, base, size, fitting count, closing, reserved zero].
Sixteen 32-byte slots follow: [used 0/1, actual task, generation, reserved zero].
Unused slots have owner 0 and retain generation history. Active bootstrap task 0
is distinguished by used=1. Active slots require nonzero generation and a fitting
index. Validation rejects malformed flags/reserved words, invalid bounds/counts,
foreign associations and missing/stale records. No root288, KSV3/64 or CPU v5/316
format, SDK operation, quota or full-candidate helper hash changes are introduced.

Errors retain SDK meanings: -1 Invalid, -2 Busy, -3 Failed, -4 Denied,
-5 Unsupported. Every ordinary operation checks live root/KSV and cancellation;
mutating allocation checks again before returning the lease. Cancellation or
KSV revocation denies new work. Actual owner release/creator close still clean
up, then return Denied even when cleanup completed. Cleanup failure that leaves
resources owned requires retry or whole-session shutdown. A failed post-create/
post-allocation liveness check tries cleanup; incomplete cleanup returns Failed,
never a fabricated successful allocation. Malformed/released storage cannot be
repaired implicitly; end the failed session rather than reusing affected tokens.

This is not a memory/protection lease. No pages are committed, made writable,
zeroed or exempted from access checks. Guest reads/writes must still use Memory
and its GamePKG backing; unmapped, read-only or revoked pages fail there. A top
value is not a host address or a promise that pages remain accessible. Validate
and preserve the actual bootstrap layout and image boundaries during integration.

The owner must arrange actual task cleanup and joins before code unload/pool
close. Device ISR callbacks allocate/release on the same actor. The original
kernel timer retains its slice for the worker's lifetime, including DPC calls;
retain that behavior and release on every exit/error in the migrated worker.
Do not free its stack between timer iterations or reclaim another actor's lease
on the strength of a creator-provided finished/joined flag. This helper introduces
no forced cross-task reclaim or generic host callback.

## Validation and next wiring

`python3 check.py --app /path/to/app --app-lib /path/to/libgamepkg.rlib --wine`
runs maintainer-only C/Rust comparisons. `--evidence /new/private/directory`
preserves all four exact native build receipts, source hashes and vector hashes.
Python/C/FFI and runtime bootstrap are not shipped as game requirements or allowed
in checked game source. The source-kit archive, complete original kernel/header
hashes and both original allocator function ranges are pinned.

128 cases / **16,384 operations per target** compare original allocation/free
results, guest tops, occupied masks and counts across zero, partial, shrunken and
full stack sizes and two region bases. A reversed allocation-order copy must fail.
Checked controls exercise invalid layout/record fields, foreign/stale/borrowed
objects, closing retry, no-clear behavior, held-close refusal, token adoption,
generation reuse/overflow, actual worker ownership, normal/cancelled cleanup,
KSV revocation and recovery after both existing record-count/byte quota limits.
Four actual reviewed workers compete for and retain four slices each; all 16
must be unique and remain owned until their matching releases and actual joins.

All checked fixture sources scan before host binding injection; each of eight
separately optimized objects audits before trusted runtime linking on Linux32/64
and Windows32/64. Unsafe unused oracle source still rejects. Confined Linux
children repeat witnesses/controls. Windows component execution uses Wine;
Windows artifacts are cross-target compile/link checks, not native Windows
confinement. No production IRQ/DPC handler, actual display, physical controls,
audible audio, gameplay or measured frame pacing is qualified.

Next connect this single pool to the IRQL/native callback epilogue and timer
worker, then implement coalesced pending lines, post/release race handling and
finite direct handler routes. Stack exhaustion must restore the current and
remaining pending bits, not drop an IRQ or report a callback as run. Original
callback/diagnostic/force options still need explicit integration and validation.
The complete candidate and its unresolved import counts are unchanged here.
