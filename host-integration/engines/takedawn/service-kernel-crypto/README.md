# Checked RC4 and HMAC kernel exports

Ordinary dependency-free game Rust in `../service-kernel-pool/kernel_pool.rs`
now implements RC4 key scheduling (338), in-place RC4 streaming (339) and
HMAC-SHA1 (340). It shares the previously compared pure SHA compression with
335/336/337. No native crypto crate/API, host pointer, engine exemption,
permissive import fallback or new runtime opcode is introduced. Unsupported
public-key/block-cipher stubs remain Unsupported; they were not promoted into
success/verification bypasses.

Requires app SDK implementation `2c646c0ec78cb39b1251ccd7ba8662da5fc62ffa`,
which adds the safe caller-owned clearing helper. Full shared identity:
`dfdfb8933b8e744173dde07a3f59749deef10d4fc2fc120581353750a03fbda2`;
composite native recipe `867d8027dd05fc8c`. Source/object/cache receipts include
the new bindings. Do not copy an old client into a package or exempt this code
when an SDK is missing. Older SDKs do not supply `gamepkg::zeroize`.

## Call and memory contracts

| Ordinal | Guest arguments | Original argument bytes |
| --- | --- | --- |
| 338 | context, key length, key | 12 |
| 339 | context, data length, data | 12 |
| 340 | key, key length, data1, length1, data2, length2, digest | 28 |

The dispatcher validates all argument bytes, CPU and live root-bound map before
computation. KPL2 and complete KPL3 accept these finite slots, including main
remapping and raw lookbehind. On success EAX=0, ESP advances by return plus
argument bytes, and other register bytes remain unchanged. No HeapClient or
Contiguous owner is required; only the original five physical allocator exports
have that requirement. Existing CPU/map/task layouts and all limits stay intact.

RC4 context is exactly 258 guest bytes: S[256], byte i and byte j. KSA writes the
identity permutation before reading the key, reads keys after prior swaps and
sets i/j to zero last. A declared key length can exceed 256: only the original
`i % key_length` accesses for i=0..255 are made. Its active key extent is thus
min(length,256), not an invented full-length read or clamp of the modulo.
Crypt updates i/j, swaps S and reads/writes each data byte in original order.
It rereads guest state on each step, preserving data aliases of S/i/j and key
aliases of the evolving context. Contexts can be unaligned; every operation
uses checked RAM, explicit u32 guest ranges and wrapping byte arithmetic.
There are no host struct layouts or native addresses in the game source.

Either RC4 call is a successful no-op for null context, null key/data or zero
length, even if an unused pointer/length is otherwise invalid. Active context
and accessed key/data extents may not wrap 32-bit addresses. Access still needs
a mapped live writable/readable capability at every step; an allocation or
numeric extent is not access authority. Errors leave CPU unchanged but may
leave earlier identity/state/swaps or output bytes. In particular, a bad key
read can follow identity publication and a denied data write can follow state
advancement. Abort such a call path; do not replay it as if untouched.

HMAC has the original null-key/null-digest no-op. Non-null zero-length keys are
valid without a key read. Non-null data buffers are hashed in full length1 then
length2; null or zero-length data are skipped. Keys longer than 64 bytes are
SHA-hashed first. Blocks use BE SHA encoding, independent of host width/endianness;
bit counts wrap explicitly. Input is read in bounded copied chunks of at most
4 KiB, with a fixed local hash/context rather than allocating input-sized data.
No new total-input cap is introduced beyond the u32 ABI, owned ranges and
existing process budgets. Inputs may overlap each other and the output: all
required reads finish before the final checked 20-byte write. A denied read or
output write does not change guest data. The checked final write rejects an
inaccessible output before publication; no claim about original C fault
recovery/partial native stores is made.

The four original temporary key/pad/inner-digest buffers are explicitly cleared
on success and checked errors with `gamepkg::zeroize`, as are the copied key-hash
scratch, input chunk and local hash buffer. The helper accepts exclusively owned
byte slices of at most 1 MiB, uses volatile stores and a compiler fence, and
performs no transport/allocation/system call. Clearing does not depend on a live
runtime/root or grant. It covers those specified slices, not every compiler
copy/register, swap, dump or historical allocation; no such guarantee is made.
The original read-only input key/data are preserved. Algorithms remain in
ordinary port code; the trusted helper only clears a borrowed byte slice.

## Reproduction and completed validation

```sh
python3 host-integration/engines/takedawn/service-kernel-crypto/check.py \
  --app /path/to/GamePKG/app --app-lib /path/to/current/libgamepkg.rlib --wine
```

`reference_inputs.json` pins the exact original SHA/RC4 context declarations,
crypto bodies, bridge wrappers, argument switch and original lookup membership
inside the reviewed source kit. The maintainer C oracle supplies only platform
byte/pointer/clearing shims around those pinned bodies; its clearing shim uses
volatile byte writes. No C oracle is linked into admitted objects. Python/C are
maintainer tools, not player requirements.

Native Linux32/64 and Windows32/64 under isolated Wine each pass **96 setups /
160 exact original calls**, comparing **every 262,144 RAM byte after every call**
and all CPU fields. Cases cover arbitrary permutations/counters, byte wrap,
streaming lengths through 8,192, key lengths through u32::MAX, evolving key/S/
i/j aliases, HMAC keys through 131,071, split and overlapping data, padding/chunk
boundaries, digest/input aliases, repeated calls and null/unused-pointer cases.

Complete ordinary sources are scanned before bindings, independently optimized
game units are all object-audited before trusted linking on four targets. Actual
confined Linux32/64 children compare original context/output/CPU witnesses and
exercise known RC4 Key/Plaintext and HMAC Hi There vectors, key-source retention,
missing-heap independence, full seven-argument validation, unmapped/overflow/RO/
translation-RO/revoked denial, unchanged CPU and documented late-failure effects.
Whole-RAM comparisons belong to the native oracle; confined checks compare the
copied witnesses. Signed/child memory caps and all runtime quotas are unchanged.

Synthetic image mode 7 composes authenticated in-place loading, bootstrap,
allocator/data setup, complete import publication and all six SHA/RC4/HMAC calls
through the actual finite Call router. Both Linux widths execute and verify
independent known digests/ciphertext; four targets pass source/object admission.
The supplied private ISO stays read-only/in place and rejects before any import
patch or title entry. Its remaining main-function inventory is **115**, down
from 118. Public-key/block-cipher stubs and other real kernel/device/runtime
work remain unresolved, with no fake-success substitution.

Separate SHA, allocator, complete import/protection, generated native-function,
source-enforcement and object-audit regressions pass. App clearing boundary
checks pass at four widths/OS targets; an optimized dead-buffer codegen test
retains all 64 volatile zero stores on all four targets. Windows tests run under
Wine and do not establish real AppContainer/graphics/gameplay. Tests are silent;
no title entry, game window, graphics, physical controls, audible audio or frame
pacing was qualified. Production build/play/cache/export remain gated until the
complete reviewed native path is admitted and qualified.
