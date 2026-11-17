# Typed ROM handles

`RomFiles` binds opened ROM extents to the existing HTR2 guest token table
as kind 4. It uses the unchanged shared SDK and borrows the already
authenticated and indexed disc. A guest token is a table slot, with the
same reuse policy as existing handles. The underlying ROM range and
child-local record IDs never reuse. A token is scoped to its table.

- `rom_open` looks up a relative path in the disc index, opens the read-only
  host range and publishes one guest token. A nonzero output means it was
  published, including if guard release subsequently failed. Keep that
  token's close obligation. A failure before publication closes private
  resources. The owner retains failed retirements for `rom_cleanup`.
- `rom_read_begin` reserves a file for one read and drops the table guard.
  A populated output retains the reservation even on guard-release failure.
  Read and close on the same reserved file return Busy. Other files and
  object kinds can use the table. The reservation belongs to the calling
  task and memory root.
- `RomRead::rom_read_into` accepts up to 64 KiB and transfers at most the
  remaining extent. `None` uses the saved cursor. An explicit offset never
  changes it. EOF, even for a zero-length request, still validates the
  underlying handle. A short host reply is continued and unexpected
  physical EOF fails. On failure the caller must discard the output.
  `rom_read_finish(ram, publish)` ends the reservation. Pass true after the
  caller has accepted the output to publish the successful cursor update.
  False discards that update. Unconsumed or failed reads never advance.
  Retry a failed finish with the same choice. Always finish, even after
  cancellation. Never replay a native read
  to recover from a completion-record error.
- `rom_wait_begin` captures the exact ROM record and read-only range for
  the calling task and memory root. `rom_wait(ram, millis)` returns true
  when the file is idle and its range is still live, or false on timeout.
  `Some(0)` polls once and `None` waits until ready or cancelled. While a
  read is reserved, the waiter rechecks using millisecond monotonic waits
  on its game worker. It holds no table guard and consumes no completion.
  A closed or reused token cannot redirect an existing wait.
- `rom_close` retires the host range before removing the typed token and
  local record. A Denied close of a known owned range also covers host
  revocation. The session owns final teardown if its transport has failed.
  `rom_drain` uses the creator's cleanup guard and removes only ROM entries.
  Call it after all producers finish their reservations and join, before
  closing HTR2.

Each owned file uses a 56-byte record of seven LE u64 values. The fields
are the RMF2 marker, table ID, host range ID, extent length, cursor, busy
flag, and asynchronous return mode. A record must have the right shape, table and kind before it can be
used. Compare/exchange serializes reservation and finish against the same
record version. This is checked port metadata over the child's owned
records. The actual host range remains separately revocable and read-only.

Run `python3 check.py --app <app> --app-lib <current-rlib> --evidence <new-dir>`.
The signed fixture opens the same synthetic file twice at an image offset
beyond 4 GiB. It checks typed dispatch, cursor independence, close/read
contention, discarded cursor updates, wrong-root finish, host revocation,
zero and finite wait timeouts, completion across two actual workers, slot
reuse, and cleanup while an event remains live. Linux32/64 execute confined children. Windows32/64 checks
build and audit the same source. Fixture media are temporary.

`RomRead::rom_read_guest` preflights the whole requested output and the
eight-byte status block, reads into private scratch, and publishes the data
and status with one checked local scatter write. The status is written last.
Success returns zero. A nonempty request at EOF returns 0xC0000011 and writes
a zero count. Rejected destinations and transport errors return a service
error. Backing permissions are checked again under the write lock, and a
failed publication discards the proposed cursor change.

Guest APC delivery, general waits, saves, normal title launch and
gameplay have separate integration requirements.

## Scoped disc import dispatcher

`RomKernel::rom_kernel_invoke` accepts a canonical ordinal from its caller.
It reads the complete 32-bit guest frame before changing registers. On a
completed call it records the return address and updates EAX and ESP, leaving
the other registers intact. Driver errors propagate without changing the
registers. If a native or guest action has already been accepted, the caller
must handle the error as a failed session operation and avoid replay.

| Ordinal | Supported scope |
| --- | --- |
| 202 NtOpenFile | Counted ASCII path under D:, the DOS device D: alias, or the CdRom0 device prefix. Read-only access, shared readers, regular files only. Keeps synchronous return semantics. Relative root handles, other drives and write access are refused. A missing indexed name returns STATUS_OBJECT_NAME_NOT_FOUND. |
| 190 NtCreateFile | Read-only FILE_OPEN for the same optical paths and sharing rules. Without a synchronous flag, records the asynchronous return mode. Other dispositions and writes are declined. |
| 219 NtReadFile | Null or negative byte offset chooses the cursor. A nonnegative offset is positional. A supplied event must be a typed event token. APCs are declined. Deferred reads can complete without an event. |
| 233 NtWaitForSingleObject / 234 NtWaitForSingleObjectEx | A non-alertable wait on one owned ROM file returns success when idle or STATUS_TIMEOUT when still busy after the timeout. Null waits indefinitely, zero polls, negative time is relative and positive time is absolute in guest 100 ns units. The Ex mode argument is ignored, as in the reference bridge. Other handle types and alertable waits are out of scope. |
| 187 NtClose | Retires an owned ROM file token. Other handle kinds remain with their respective owners. |

An event completion retains the exact typed event with the same lifetime
reservation used for waits. Data and the I/O status are published together,
then the cursor is committed, then the event is signaled once. Synchronous
reads return the final status even with an event. An asynchronous create-open
returns STATUS_PENDING after completion, including when the status block
contains EOF. The I/O itself runs inline on the calling game worker. This
matches the reference bridge's immediate-completion shape when its pending
return option is enabled. An eventless read can be followed by a scoped wait on its file token.

The standalone signed fixture executes these frames with a sparse synthetic
disc. The general kernel import planner still rejects these ordinals without
a broader file context. APCs, general object waits, save paths and other
file types remain separate integration requirements.

## Authenticated startup routing

`Startup::startup_prepare_with_rom` opts in after creating the KIC5 object
context. The image planner can then publish main-table markers for 187,
190, 202, 219, 233 and 234. The planner still checks every other import and rejects
unsupported ordinals before publishing any markers. The default planner
and generic kernel dispatcher continue to decline these file ordinals.
Lookbehind file imports remain unsupported.

`StartupFiles` borrows the ready startup and authenticated image. Each
call checks the ROM grant, image digest, live context, marker alignment,
main-table bounds and the marker still stored at that slot. The adapter
reads only the disc that supplied the authenticated image. The startup
creator must drain its file handles after file users have joined and before
closing the kernel startup owner.

The image checks exercise this route under Linux32 and Linux64 with a
synthetic authenticated XBE. Mode 86 opens, reads and closes a file through
the published markers. Mode 87 verifies that ordinary KIC5 rejects the
same file imports. Mode 88 includes an unsupported set-information import and
verifies that the explicit optical path also rejects the entire table.
Mode 89 exercises NtCreateFile, both event-backed and eventless Pending
returns, and the two single-object ROM waits through the published markers.
The private image is still checked through the ordinary gate.

## Scoped DOS drive links

The authenticated regular-save profile supports ordinals 67 and 69. The
arguments are pointers to Xbox counted ANSI names with little-endian length,
capacity, and buffer address. Create reads both complete names before
publishing anything. Delete reads the complete alias before changing it.
Accepted aliases have the form \??\X: for an ASCII letter and are compared
case-insensitively. Each file router owns 26 independent slots and clears them
at teardown. Create returns STATUS_OBJECT_NAME_COLLISION for a duplicate.
Delete returns STATUS_OBJECT_NAME_NOT_FOUND for an absent entry.

Targets are the disc device, private partition, and the approved TDATA and UDATA
directories and title roots. Relative targets, other Xbox partitions, other
title IDs, link chains and host paths return the explicit unsupported result.
The router expands X:\... or \??\X:\... for file open/create and private
path metadata queries. The expanded counted name is capped at 528 bytes and
undergoes the existing destination service's checks. Unbound and relative
drive forms return the unsupported result in this profile. Existing file
handles remain valid after deletion. Legacy optical-only and directory-only
profiles retain their existing path contracts. No native symbolic link,
new storage capability, or guest-memory rewriting is involved.

The synthetic startup fixture checks malformed descriptors, denied targets,
case-insensitive collisions and deletion, optical reads after deletion, and
regular save reopen and metadata through a drive alias. It verifies continued
rejection of an unmigrated startup profile. Real-media validation prepares the
import table in place. Original title execution and symbolic-link object
open/query are outside this checkpoint.
