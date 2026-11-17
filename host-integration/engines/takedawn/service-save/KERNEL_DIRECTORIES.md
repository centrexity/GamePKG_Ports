# Private save directories

`kernel_directories.rs` handles the checked hard disk calls captured in
Burnout's original startup. It requires the GamePKG SDK with file broker
operations 23 and 24. Rust game sources use the SDK's `save_volume` and
`save_directory` helpers. The game never sees the host save path.

Construct `SaveDirectories::save_directories_new(ram, table, grant)` on
the creator of the HTR2 table. The grant must be the session's private
writable save grant. The adapter binds to that task, memory root and
table. It owns up to 64 kind-5 directory tokens. Tokens represent the
observed entries in this virtual volume and carry no directory-relative
path authority. Finish all calls and drain the adapter before closing the
table. Recoverable guard and local-record release failures retain their
cleanup obligation.

The guest frames use the Xbox 32-bit little-endian ABI. On a completed
call, EAX holds the returned status and ESP consumes the return address
and arguments. The adapter records the return address and preserves other
registers. Service failures leave the registers unchanged. The caller
must stop or settle a failed session operation whose host effect was
already accepted. A directory that was actually created remains present
if publishing its guest result fails.

| Ordinal | Admitted operation |
| --- | --- |
| 202 NtOpenFile | Observe the root or a reviewed directory. Directory access 0x100001, share 3, options 0x800021. Publishes an opened token and information 1. An absent name returns STATUS_OBJECT_NAME_NOT_FOUND without writing the token. |
| 190 NtCreateFile | Open or create one reviewed directory. Same access and sharing, normal attributes, null allocation size, disposition 3, options 0x4021. Returns information 1 for opened or 2 for created. |
| 218 NtQueryVolumeInformationFile | Class 3 on an owned directory token. Writes a 24-byte record and an eight-byte status block. Total and available clusters are the actual private quota and remaining logical bytes rounded down to 16 KiB units. Geometry is 32 sectors per unit and 512 bytes per sector. |
| 187 NtClose | Closes one owned directory token. Shared-table records with other types belong to their respective owners. |

The namespace is the counted ASCII path
`\Device\Harddisk0\partition1\` and its `UDATA`, `TDATA`,
`UDATA\4541005b` and `TDATA\4541005b` directories. Names are matched
ASCII case-insensitively and mapped to canonical portable strings.
The native directory service applies its own containment and quota checks.
The bridge rejects relative roots, alternate volumes, regular files,
unsupported options and other paths.

The signed fixture in `../service-rom-handles/check.py` exercises real
directory creation, reopen, missing entries, incorrect grants, traversal,
bad output and exact volume bytes with temporary per-title save roots.
It runs under confinement on Linux32 and Linux64. Windows32 and Windows64
compile and audit the same fixture. These are synthetic CPU frames, with
no original title entry, saves, graphics, physical controls or audio.

## Authenticated startup

The reviewed coordinator calls
`Startup::startup_prepare_with_private_files(..., [scratch, offset, grant])`
with a live private save grant. It verifies the grant before planning any
markers and binds the selected grant to the same startup owner as the
authenticated optical image. The ordinary and optical-only profiles do
not admit the volume query.

`StartupFiles::startup_files_call` checks the live startup context and the
marker in its original main-table slot. Opens for the reviewed hard disk
partition route by the counted name. A close routes by the directory
owner's typed token, and the volume query accepts an owned directory token.
The optical route uses the original ROM grant, and its read and wait
operations refuse directory tokens. Close and drain both adapters before
closing the startup owner. Regular save files still require the save
worker route with guarded close, flush and replace.

Run `service-kernel-imports/check_sync.py` to check the combined path.
Mode 90 opens the root, queries quota geometry and creates and reopens both
title trees while also exercising the optical path. Mode 91 checks rejection
of the volume import under the optical-only profile, even with a private
save grant present. Mode 92 checks that an unsupported import prevents any
marker publication with both adapters selected. Optional mode 93 checks the
actual image in place with both adapters selected and verifies that all
remaining unsupported imports still prevent launch.
