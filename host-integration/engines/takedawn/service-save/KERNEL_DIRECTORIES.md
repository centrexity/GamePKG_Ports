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

The authenticated title dispatcher still needs to select this adapter
alongside the optical and regular save adapters. Regular files must route
through the save worker and retain its guarded close, flush and replace.
The default title admission gate continues to refuse unsupported imports.
