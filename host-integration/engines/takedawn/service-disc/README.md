# Checked in-place ISO reader

This port-owned component indexes XDVDFS using only GamePKG file-size and
positional-read services. It has no crates, macros, aliases, raw pointers,
callbacks, filesystem/device imports or direct platform calls. It is not an
executable loader or playable source kit and does not enable a legacy profile.

## Contract

`Disc::disc_open(handle: u64) -> Result<Disc, i32>` borrows a caller-owned opaque
read-only ROM/file handle. A ROM grant (kind 3) is itself the file handle; it
must not be reopened by a filename. Construction neither duplicates nor closes
it. Dropping the index drops metadata only. The caller owns handle lifetime.
`disc_partition_offset`, `disc_count` and `disc_entry(u32)` return cached metadata.
Entries expose UTF-8 `disc_path_bytes()`, u64 image offsets, u32 lengths and a
boolean directory flag; these are data, never native paths or device pointers.

`disc_find(&str) -> Result<u32, i32>` matches ASCII case-insensitively and accepts
relative slash/backslash separators. It rejects empty, absolute, drive-prefixed,
control-containing and dot/dot-dot/empty path components. Missing paths return
Unsupported (-5); malformed paths and bounds return Invalid (-1).

`disc_read(index: u32, offset: u64, output: &mut [u8]) -> Result<(), i32>` reads
exactly the requested bytes from an indexed file, never a directory or beyond
its extent. Transfers are at most 1 MiB, split into <=64 KiB positional requests.
Short positive reads are retried; unexpected zero or oversized replies fail
(-3), never spin or report success. Service denial/revocation (-4) propagates.
Even valid empty file reads validate the live capability. Cached names remain cached after
revocation; no new data is available from a revoked handle.

For sequential I/O, `disc_open_entry(index)` and `disc_open_file(path)`
return an opened `DiscFile` that borrows the already indexed image. They
reject directories and validate the live read grant before returning. Each
open has an independent cursor at zero. Dropping a stream leaves the
caller-owned ROM grant open. Size and cursor are cached metadata.

- `disc_file_read(&mut self, output)` reads from the cursor and advances it
  only after a successful transfer.
- `disc_file_read_at(&self, offset, output)` keeps the cursor fixed. Both reads
  clamp to the file extent and return the transferred byte count. A read at or
  beyond logical EOF returns zero after validating the grant. Only the returned
  prefix of the buffer is written on success.
- `disc_file_exact(&self, offset, output)` retains strict extent checking for
  file-format reads. All transfers retain the same one MiB request and 64 KiB
  per-service-call limits. Unexpected physical EOF is Failed. A failed transfer
  can leave a partial output buffer and leaves sequential state intact.

An owner that needs an opaque ID can call `disc_file_open_handle()`. This
opens a separately closeable read-only host range over the file's validated
extent using SDK operation 19. The caller must close it, or hand it to a
handle owner that will close it. The original ROM grant can revoke all such
ranges together. The admitted reader probe compares each range with its
stream and keeps one open through the revocation checkpoint.

The XBE loader uses the stream for its section-plan snapshot, independently
validates the file digest, and checks the grant again before publishing
the loaded image to guest memory. Guest handles, asynchronous completion and
save writes require their own integration.

The seven known partition bases are tested in order, with both volume magic
strings required. All fields use explicit LE decoding and checked u64 extents.
Directory tables are at most 1 MiB, depth at most 32, and total entries at most
8,192. UTF-8 paths must be shorter than 512 bytes; on-disc names are Latin-1.
Cycles, repeated directory sectors/links, overlapping entry records, duplicate
case-insensitive paths, invalid names and truncated extents are rejected.
On error, an output slice can contain bytes from earlier successful partial
reads; callers must discard it and propagate the error. There is no promise of
an atomic multi-request snapshot of a file modified by another process.

Metadata and traversal storage are explicitly bounded; allow a conservative
32 MiB transient child budget. File lengths never size a whole-file allocation.
No executable mappings, asset decoding or new host privilege is introduced.

The index is not content authentication. The conformance probe's 64-byte data
witnesses use a simple non-security checksum; they do not replace SHA-256 or
pinned manifest verification. Private-image checks independently authenticate
all 716 expected files with the existing host verifier before granting the same
opened file to the child. The production launcher still needs its reviewed
identity and asset-integrity path; a caller must not treat this index or its
metadata as a certificate of a supported title.

## Boundaries and tests

`disc.rs` is game code. `probe.rs` is an ordinary checked conformance entry,
including cached lookup tests and bounded wait/wake plus revocation. Only those
two files are copied into the temporary dependency-free package. The complete
source is checked before SDK bindings; two separately optimized game objects
are audited before trusted runtime linking on Linux32/64 and Windows32/64.
An accidentally included unused unsafe/direct-filesystem reference is rejected.

`reference.rs`, `admission.rs` and `check.py` are maintainer tests, never game or
host SDK code. The five standalone tests execute on Linux32/64 and Windows32/64
under Wine, covering partial/zero/invalid replies, revocation, empty reads,
overflow/caps, exact 8,192/8,193-entry limits, 32/33-level depth and 511/512-byte
UTF-8 path boundaries. Sixteen malformed synthetic images execute in the actual
confined Linux child, alongside all seven partition layouts and a sparse image
whose file data begins beyond 4 GiB. No large image is copied or downloaded.

The supplied private Burnout ISO is read in place. In actual Linux32/64 confined
children, 716 file records and bounded data witnesses match the existing reader;
55 directory records are observed. Each run independently verifies the existing
716-file cryptographic manifest in the host. The host then revokes the exact ROM
grant and wakes a bounded child wait; subsequent reads, empty reads, size calls
and reopening the index are denied. Windows service executables are audited and
linked only; Wine standalone parser tests are not AppContainer/gameplay proof.

```sh
python3 host-integration/engines/takedawn/service-disc/check.py \
  --app /path/to/GamePKG/app \
  --app-lib /path/to/target/debug/deps/libgamepkg-HASH.rlib --wine \
  --iso /path/to/your/own.iso
```

The ISO argument is optional. Python is maintainer-only; users receive prepared
Rust, not these tools. No ISO bytes, extracted assets, saves, keys, windows,
physical controls or audible output are published or used as gameplay evidence.
`reference_inputs.json` binds the original source-kit reader and hardened app
reader consulted for the contract. This follows the repository license and
preserves the existing Takedawn/XboxRecomp notices in
`../service-native/UPSTREAM_LICENSES.txt`.

Next: load and validate the original XBE sections/header/import data through
this interface into GamePKG-owned non-executable RAM, preserve original section
ordering/overlap and bootstrap semantics, then migrate the actual kernel,
manual/media and graphics paths. Do not replace those with no-ops, accept game
crates or expose direct SDL/OS calls to make a title appear enabled.

The [checked XBE image loader](../service-image/README.md) now supplies the
next data step using the same shared SDK. Actual Linux32/64 confined loads
match the original section loop across all 64 MiB by SHA-256, including the
supported private image's 17 sections. It remains separate from stack/FS/TLS,
heap/kernel initialization, import bridging and native entrypoint execution;
no title admission or gameplay claim follows from it.
