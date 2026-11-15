# Checked kernel data exports

`kernel_data.rs` ports the memory effects of the pinned `kernel_data_init` and
`kernel_data_va_for_ordinal` functions. `signature.rs` derives the original
16-byte certificate signing key through fixed HMAC-SHA1 over the original public
master constant. Both are dependency-free ordinary Rust game source. All RAM,
allocation, queues and task operations use existing GamePKG-owned services.
No environment, OS clock, console, file, device or host pointer is exposed.

This component is **not connected to the title's complete kernel resolver or
native launcher**. The existing nine-function resolver still rejects data
imports; do not truncate the real table to pass it. Complete remap/count/
lookbehind handling, kernel functions, IRQ/thread startup and title admission
remain required. This component cannot enable a legacy build/play/cache path.

## API and ordering

`Data::data_initialize(&mut Memory, region: u32, tick: u32, video: u32,
command: &[u8], hd: Option<[u8; 16]>) -> Result<Data, i32>` takes the reviewed
bootstrap's region base and copied setup inputs. Region must be at least
`0x700000`, aligned to 64 KiB, with `region + 0x41000 <= 64 MiB`. The data page
is `region + 0x40000`, as in the original. Invalid bounds and command inputs
longer than 65,536 bytes return Invalid (-1). A live valid root is required.
Initialize before other callers write this data page; a whole-page read/modify/
write preserves original untouched bytes but is not concurrent publication.
The coordinator is responsible for using its authenticated bootstrap region.

Tick and video are exact u32 copied values. The coordinator must supply time
from GamePKG's monotonic service, then periodically call `data_tick`; this
component does not start a clock thread or qualify tick scheduling. Video setup
must supply the original selected boot video value. No environment query or
unreviewed timer/device implementation runs here.

The copied command is a byte string: stop at its first NUL, preserve bytes and
truncate payload to 3,071 bytes. An empty/leading-NUL command requires no heap
client and publishes a zero LaunchDataPage pointer. Otherwise the existing
exclusive per-caller HeapClient requests 7,168 bytes aligned to 4,096, preserving
the original allocation and zeroing. The first page receives type 3, title ID
`0x45410091` and the command at offset `0x400` with a trailing NUL. Ordinary
allocator exhaustion publishes zero; denial, missing service, poison and other
errors propagate. The client is restored before propagating allocation errors.
There is no OS allocator or automatic retry of an accepted allocation.

`hd` is an optional copied original-console HDD save key, **not a package creator
signing key**. The optional caller must load/parse any configured key or EEPROM
through approved private file grants; environment/file parsing is separate work.
None preserves the original zero-key default without reading the image header.
Some checks the original `XBEH` header and certificate address in
`[0x10000, 0x800000)`, then reads its 16 certificate bytes at `certificate + 0xC0`
through checked RAM. The original test constrains the certificate start, not
its complete extent; a certificate at `0x7FFFFF` is preserved when the checked
read succeeds. Invalid header/address leaves both keys zero. Inaccessible reads
propagate. A valid certificate copies the supplied HDD key and derives the
signature key exactly as the original. Neither key is printed or persisted by
this module. Its returned state supplies those same selected keys to future
port-owned crypto wrappers, rather than inventing a second global key source.

Launch allocation/publication precedes key reads, preserving original ordering.
The export page itself is published in one checked transfer after all data is
prepared. A denied write leaves that page unchanged. This is not a transaction
with the shared allocator or launch-page writes: a later key-read/export-page
failure may follow a successful allocation. Abort initialization and clean up
workers/runtime; do not replay or claim rollback of that allocation.

The page preserves every original field and hole: hardware/version, tick,
launch pointer, six distinct object-type values, original zero IO types,
1 ms increment, boot video, empty circular Idex list in separate 512-byte
storage, three original ANSI_STRING descriptors and strings, cache partition
count 3, selected HD/signature keys, and zero LAN/alternate/public-key arrays.
Original diagnostics/status messages are separate coordinator reporting; these
memory-effect comparisons do not qualify the entire logging wrapper.

`data_base`, `data_launch`, `data_keys`, `data_address` and `data_tick` validate
the same live root before operating. Wrong/revoked roots return Denied (-4).
`data_address` returns the exact original data address for these 24 canonical
ordinals and zero for all other ordinals; zero is classification, not a successful
function implementation:

`16, 22, 30, 31, 40, 41, 42, 64, 70, 71, 156, 157, 164, 259, 322, 323, 324,
325, 326, 353, 354, 355, 356, 357`.

`data_tick(&mut Memory, tick)` writes only the tick word through existing checked
RAM. u32 wrap is preserved. Root revocation prevents further Data operations;
actual RAM remains governed by host page protections/runtime lifetime, as in the
[guest-layout contract](../service-memory/README.md). Returned copied keys and
addresses do not grant host capabilities. Caller shutdown must close/unregister
clients, stop/join owners, release shared records and end the confined runtime.

## Reproducible validation

`reference_inputs.json` pins the source kit, complete kernel/crypto/console-key
sources, layout header and exact original functions. The C oracle uses the exact
original initialization, data classification, SHA1/HMAC/derivation and Heap
bodies. Time, video, command and optional keys are deterministic synthetic
inputs. No original environment/file loader or hardware/device implementation
is called. C function pointers/unsafe pointers exist only in maintainer test
code and are never linked into admitted game/runtime objects.

```sh
python3 host-integration/engines/takedawn/service-kernel-data/check.py \
  --app /path/to/GamePKG/app --app-lib /path/to/libgamepkg.rlib --wine
```

Native Linux32/64 and Windows32/64 under Wine each compare 24 initializations:
region/tick/video edges, absent/empty/embedded-NUL/long/truncated commands, heap
exhaustion, optional keys, invalid headers/certificates and valid boundary
certificates. All data-page/launch bytes, selected keys, all 400 ordinal probes,
Heap counters and every byte of 64 MiB RAM match the original. The final RAM
comparison includes the separately tested explicit tick-word update. Each target
also compares 1,024 signature vectors and a zero-certificate value independently
computed with maintainer Python's standard HMAC. Only synthetic keys are used.

The admitted Rust fixture stages the complete ordinary source before bindings,
optimizes separate game objects, and audits every object before trusted linking
on all four targets. Actual confined Linux32/64 runs compare exact original-code
witnesses for the export page, allocated launch block, keys, counters and all
400 addresses. They do not compare the entire 64 MiB through the pipe. Every
main mirror observes the data page and tick update. Controls require exact error
codes for invalid bounds, oversized commands, missing owner, denied page/cert
reads/writes, wrong/revoked roots, denied allocator clears and errors after an
accepted allocation. The latter prove one allocation, retained client ownership,
unchanged export page and explicit abort/cleanup without replay.

Synthetic cases explicitly decommit before recommitting their exclusively owned
backing after the previous actor stops/joins; commit continues to reject live
pages. The i686 C oracle links its stack-protection support library instead of
disabling stack checks. Windows oracle calls preserve the compiler's original
calling convention. Neither accommodation changes game-source/object policy.
All three banks remain charged; the child stays 256 MiB and signed fixture limit
384 MiB. Python/C are maintainer-only; users receive Rust/native tools.

No actual ISO, key file, game window, graphics, physical input or audio is used.
Windows AppContainer operation, complete kernel initialization, gameplay,
rendering fidelity, audible output and measured frame pacing remain unqualified.
The installed shared SDK is unchanged. See `service-native/UPSTREAM_LICENSES.txt`
for the pinned upstream notices; these tests do not publish private game assets.
