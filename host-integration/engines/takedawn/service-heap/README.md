# Checked guest heap data

This dependency-free Rust component preserves the pinned toolkit's heap
allocation, reuse, free/coalescing, allocation-size queries and RAM zeroing.
It follows the [CPU startup data](../service-bootstrap/README.md) component,
whose layout supplies the heap base and upper bound. It does not bind kernel
ordinals, run the native entrypoint or enable Burnout. No allocator, thread,
filesystem or platform implementation is moved into trusted host code.

The current shared service implementation is app `c775eca`, shared-service
identity `86521a787ece4bd0...` (composite native recipe `5c7fc32a2c8b443f`).
This component requires no SDK extension or new game permission. All guest
bytes stay behind the existing checked non-executable RAM services.

## API and ownership

`Heap::heap_new(&Memory, base: u32, top: u32) -> Result<Heap, i32>` requires a
live checked RAM call record and `65536 <= base <= top <= 67108864`.
An empty heap (`base == top`) is valid. The initial next pointer is the supplied
base. No guest RAM is written by construction. The returned value owns a fixed
196,608-word child-memory vector: 65,536 records of address, size and free flag,
786,432 payload bytes. This does not use or enlarge GamePKG's small atomic-record
quota. The record ID and table remain private. The signed child memory limit
continues to bound this ordinary Rust allocation.

The value has exclusive mutable ownership. It is not a shared-memory allocator
or a substitute for the original cross-thread heap lock. The integration must
keep one instance owned by a reviewed game task and serialize requests through
bounded GamePKG messages, or provide an equally reviewed ownership arrangement.
Creating one independent Heap per guest thread would allocate overlapping
addresses and is not a valid integration. There is no Clone/static/unsafe or
native callback path for sharing this value.

`heap_allocate(&mut Memory, size: u32, alignment: u32) -> Result<u32, i32>`
requires the same live call-record ID used at construction. Size is clamped to
16, alignment to 4. The original bit-mask alignment rule is preserved even for
non-power-of-two values; the port does not invent a power-of-two restriction.
Freed blocks are searched in their original table order. A large enough,
aligned free block is reused without splitting or shrinking its recorded size;
only the requested byte count is zeroed. Reuse does not advance the bump pointer
or increase the original new-allocation counter.

If no block can be reused, the next pointer is aligned, the entire allocation
is checked, and its requested extent is zeroed through `Memory::fill`. The next
pointer and block/counter updates are published only after that fill succeeds.
Ordinary exhaustion returns `Ok(0)`, preserving the original null result.
Arithmetic that would wrap the original u32 alignment/end computation is Invalid
(-1) before a write, rather than corrupting RAM. All successful extents fit the
supplied upper bound and the fixed 64 MiB guest window.

At the original 65,536-record limit, bump allocation continues without adding
a table entry, exactly as in the pinned code. Those untracked allocations cannot
subsequently be found by free/size queries. This behavior is tested; neither the
table limit nor any host quota is raised to hide it. The comment claiming a
4096-byte allocation minimum is stale: the original code actually uses 16.

`heap_free(&Memory, address: u32) -> Result<(), i32>` checks the same live
record. Null, unknown, interior and already-free addresses are quiet no-ops.
An exact live starting address becomes free, then coalesces with its immediate
next free table entry and its immediate previous free table entry when their
extents are adjacent. Absorbed records retain the original zero-address,
zero-size free tombstones; they are not compacted or skipped. Freeing does not
clear RAM, lower the bump pointer or restore table slots.

`heap_size(&Memory, address: u32) -> Result<u32, i32>` checks the same live
record. A live block's interior address returns the remaining bytes from that
address to the recorded block end. Unknown, null, freed and untracked addresses
return zero. It does not ask a host virtual-memory service for a mapping size.

Wrong/revoked call records are Denied (-4). Checked RAM/transport failures
propagate and are never converted to exhaustion. A failed allocation fill leaves
both RAM and bookkeeping unchanged: the existing host fill validates every
page under the mapping/protection lock before writing, and this value updates
its table only after success. The caller must still use the intended guest
mapping; the call-record ID is a lifecycle association, not an immutable map
seal. There is no promise of a transaction across several allocator calls.

`heap_stats() -> [u32; 3]` returns copied next pointer, table count and original
new-allocation count. `heap_record(index: u32) -> Result<[u32; 3], i32>` returns
one copied address/size/free-flag record; indexes at/above count are Invalid.
These are value diagnostics, not mutation or authority. Cached diagnostics can
still be inspected after revocation; allocator operations themselves cannot.
No native struct layout, byte order, raw pointer or platform integer width is
part of these interfaces.

## Validation

`reference_inputs.json` pins the original source kit, complete source and exact
byte ranges/hashes of `heap_alloc_unlocked`, `heap_block_size_unlocked` and
`heap_free_unlocked`. The C reference template inserts those unchanged bodies
into an inert RAM/value wrapper. Original diagnostic output is silenced in the
maintainer oracle, not redirected to user devices. The original helper bodies
are never admitted or linked into game/runtime objects. Unsafe original overflow
cases are excluded from C execution and must be rejected by checked Rust.

Eight synthetic cases contain 77,759 allocation/free/query/scribble vectors,
including three checked-only overflow rejections. They cover the actual minimum,
interior/end queries, double/interior/unknown frees, coalescing and tombstones,
reusing a larger block while zeroing only its requested prefix, alignment
boundaries including non-power-of-two values, exact exhaustion, an empty heap,
12,000 randomized operations, filling/passing the 65,536-record boundary and a
zero-operation fixture. Every result and next/count/counter state is compared
per operation. Final comparisons include every table record and all 64 MiB RAM,
so clearing an excessive range or silently changing reuse policy is detected.

Direct native reference comparisons passed Linux32/64 and Windows32/64 under
Wine. These maintainers' executables compare the entire RAM byte-for-byte and
all table records, not sampled regions. The same ordinary game source also
passes complete source admission before bindings and separate optimized-object
audits/trusted linking on all four targets. No game Cargo dependencies, source
exception, import whitelist change, direct platform call or legacy-profile
exemption was added. An unused forbidden filesystem source is still rejected
by the packer before SDK injection.

Actual confined Linux32/64 children replay all vectors with per-operation state
checks, then return 64 independently checked 1 MiB RAM SHA-256 digests and a
canonical LE table digest. The test host uses the app's independent SHA-256;
the child uses the previously qualified dependency-free implementation. No raw
pointers or mutable table memory cross the boundary. Windows confined-service
execution remains unqualified; Wine reference comparisons do not prove Windows
AppContainer, native game graphics or gameplay.

The admitted denial checks additionally exercise invalid construction bounds,
a different RAM record, a read-only page late in both a fresh allocation and a
reused block, and record revocation. They verify unchanged bookkeeping and RAM
on failure, restore their scratch bytes and keep the original quotas. Nine
malformed/forged fixture cases reject bad/short headers, excessive counts,
trailing/truncated data, invalid heap bounds, unknown operations and forged
result/state witnesses without a successful completion witness. The private
GPH1 fixture is bounded to 200,000 fixed 32-byte records with exact EOF.

```sh
python3 host-integration/engines/takedawn/service-heap/check.py \
  --app /path/to/GamePKG/app \
  --app-lib /path/to/target/debug/deps/libgamepkg-HASH.rlib --wine
```

Python, C and direct filesystem access are maintainer-only. Users receive the
prepared Rust/native tools. These tests use only synthetic RAM/data; they do
not read, extract or publish any game image, asset or save. Original notices
from `../service-native/UPSTREAM_LICENSES.txt` are retained in the test package.

## Remaining runtime integration

The [shared heap task and client](SERVICE.md) now serialize one owned heap
through bounded GamePKG task/message services, including zero-fill. Wire actual
kernel/manual allocation/free/query exports and thread/TIB allocation against
that shared owner. Adapt the
original rate-limited allocation logs, OOM histogram and optional heap-trace
settings through bounded diagnostics; the core value component above does not
yet implement those reporting paths. Do not claim the original public wrappers
or concurrency are migrated just because unlocked data semantics match.

The separate contiguous allocator, explicit RAM mirrors/device mappings, kernel
data/exports, generated/manual/media and renderer paths remain required work.
Native entrypoint, saves, physical controls/rumble, audible output and measured
frame pacing remain unqualified. The legacy admission gate stays enforced.
