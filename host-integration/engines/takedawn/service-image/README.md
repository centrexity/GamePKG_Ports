# Checked XBE data loader

This is ordinary dependency-free game Rust over the checked ISO reader and
GamePKG-owned RAM. It is an image-data loader, not a runnable bootstrap or an
enabled Burnout profile. It never executes original machine bytes, opens a host
path/device, maps executable memory or links the original C into game/runtime
objects. The shared SDK remains `f9b87d0fde634662` (app implementation `7f6b19c`).

## API and ownership

`Image::image_plan(&Disc, file: u32, expected: [u8; 32]) -> Result<Image, i32>`
requires the expected original XBE SHA-256 from the reviewed content identity.
A creator flag, computed hash of an unknown file or a test receipt is not a
substitute for that identity. The plan borrows the disc and owns an immutable
file snapshot in child memory; it writes no extracted file. The snapshot is
bounded at 16 MiB and prevents later section copies from mixing changed reads.
A changed prefix during snapshot acquisition fails Failed (-3); wrong content
identity and malformed layout fail Invalid (-1), before RAM publication.
Normal file/transport denial and I/O errors propagate.

Images must have XBEH magic, base `0x10000`, headers from `0x178` through 64 KiB,
1..64 complete section records within the headers, and image extents within the
64 MiB guest window. Section virtual ranges and initialized copy bytes must fit;
names must be NUL-terminated inside the header. The entrypoint is decoded with
the original retail/debug keys and must name raw-backed executable-section data.
The thunk is decoded using the original range heuristic. TLS and thunk getters
return original metadata only: they do not validate a TLS structure, resolve
imports or initialize any kernel object.

Getters return the digest, decoded entry/thunk/TLS, section count, and
`[image_low, image_high, code_low, code_high]` using explicit fixed-width values.
The borrowed disc and owned snapshot remain private. The caller owns the ROM
handle; the plan neither duplicates nor closes it.

`image_load(&mut Memory) -> Result<(), i32>` checks the live source capability
again before publication, copies the original header at `0x10000`, then processes
sections in their original table order. Each section is fully zeroed before
copying `min(raw_size, virtual_size)` initialized bytes in <=64 KiB chunks.
Overlaps, header overlap, BSS and raw padding preserve the original copy order;
they are not silently normalized or skipped. The min/max executable bounds are
published to the private checked call ledger only after successful copying.
Guest bytes stay in GamePKG-owned non-executable RAM. No raw pointer escapes.

Unlike the legacy loader's corrupt-header fallback and truncated-section warning,
invalid plans fail before touching RAM. A failure during load can leave earlier
successful writes; callers must abort, never enter the game or claim a completed
load. The image load is not an atomic memory transaction. Revoking the ROM before
publication denies the load, even though an earlier snapshot exists. Already
read data cannot be retroactively forgotten, but no new I/O authority is granted.

`sha256.rs` supplies fixed-width, endian-explicit value computation without any
crate, intrinsic, macro, trait/callback or platform service. The image digest
checks the full immutable file snapshot. Streaming length overflow is checked
before state mutation; its result is the standard 32-byte digest. This does not
replace the production launcher's signed source/asset identity checks.

## Validation

`probe.rs` is ordinary checked test game code. It commits the fixed RAM window,
fills 64 MiB with a sentinel, loads or rejects the supplied image, and returns
64 SHA-256 digests covering every 1 MiB of RAM. No raw image bytes leave the child.
The full source, including the reused `disc.rs`, `memory.rs` and `call_state.rs`,
is scanned before SDK bindings. Both optimized game objects are audited before
trusted linking on Linux32/64 and Windows32/64. Accidentally packaging an unused
unsafe/direct-filesystem reference remains rejected; there is no image exception.

The maintainer C oracle inserts the exact hash-bound original section loop from
`xbox_memory_layout.c` into an inert RAM-only wrapper with silent diagnostics.
The small header-copy wrapper reproduces the valid-header behavior. The original
loop is never admitted or linked into game or trusted runtime objects. Invalid
inputs never enter the unsafe oracle. `reference_inputs.json` pins the whole
source file, exact loop and source kit; the notices in
`../service-native/UPSTREAM_LICENSES.txt` are retained in the test package.

The local matrix covers seven valid synthetic layouts (ordinary BSS, raw padding,
virtual/header overlaps, zero raw data, multiple executable ranges and debug
keys), twenty malformed layouts, wrong content identity and grant revocation
between planning and load. Actual confined Linux32/64 children compare all
64 MiB of RAM using SHA-256 against the original loop, or against the unchanged
sentinel for rejected inputs. Entry/thunk/TLS/count and image/code-bound fields
are checked independently against known fixtures. Failed inputs do not become
successful no-op loads.

The supplied private ISO is authenticated with the existing 716-file host
manifest. Its XBE snapshot must match that pinned executable digest. The actual
4,308,992-byte XBE loads all 17 sections in the confined Linux32/64 children;
whole-RAM chunk hashes and bounds match the original section loop. The expected
entry remains `0x001D2807`. No extracted asset or original binary is published.

SHA qualification includes 2,415 comparisons against the app's independent
SHA-256 implementation and 2,439 streaming replays on each of Linux32/64 and
Windows32/64 under Wine, covering padding/block boundaries, empty updates and
inputs beyond 1 MiB. Windows service executables are source-checked, audited and
linked only; Wine value tests are not Windows AppContainer/gameplay proof.

```sh
python3 host-integration/engines/takedawn/service-image/check.py \
  --app /path/to/GamePKG/app \
  --app-lib /path/to/target/debug/deps/libgamepkg-HASH.rlib --wine \
  --iso /path/to/your/own.iso
```

Python, the C oracle and reference drivers are maintainer-only. Users receive
prepared Rust and the app's native tools. The ISO argument is optional; synthetic
checks still use the normal admission and confinement path.

## Remaining bootstrap work

Initialize the original toolkit region, stack/FS/TLS, heap and kernel data, then
resolve actual import ordinals with correct data-export versus native-handler
semantics. Complete missing generated/manual/kernel/media and renderer paths
using shared services; preserve original behavior rather than substituting
success stubs. Only then connect and qualify the native entrypoint, saves,
physical controls/rumble, audible audio and frame pacing. This component opens
no game window/audio device and establishes none of those gameplay claims.
