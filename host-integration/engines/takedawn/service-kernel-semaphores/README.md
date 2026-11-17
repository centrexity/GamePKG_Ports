# Checked semaphore ownership

This is dependency-free, checked game code. Only the generic counted primitive
belongs to GamePKG's audited host SDK. This component grants no direct OS, SDL,
thread, filesystem or device access. It does not enable a native title profile.

`Semaphores::semaphores_new(&Memory,&Handles)` requires the actual table creator;
`semaphores_borrow(&Memory,&Handles,u64)` validates a live non-owning view.
A borrow cannot create or close an owning domain. IDs retain all 64 bits. The
shared tagged table uses kind 3 for semaphores; kind 1 remains events. Internal
kind 2 is not a semaphore. Guest handles stay four-byte first-free tags, including
original alias rules and all 16,383 slots. The host retains its unchanged shared
128 synchronization object quota and 64-record/64 KiB quota.

`semaphores_begin` returns a `SemaphoreCreation` holding the table guard and a
reserved ledger row. `semaphore_create(&Memory,initial:u32,maximum:u32)` accepts
one creation attempt. `semaphore_publish(&mut Memory,output:u32)` accepts one
publication attempt: transfer ownership, insert the tag, then copy four LE bytes
to guest RAM. Null output transfers an orphan without a tag. The five observable
phases are reserved, native accepted, domain accepted, tag accepted and guest
write/null output complete (0..4). Errors preserve every accepted prefix.
`semaphore_creation_close` is mandatory on both success and failure. It releases
untransferred private objects through SDK op 25, including after cancellation;
transferred objects remain the domain's obligation. Stale metadata updates retain
retryable release/clear state. A copied ID or typed state is never a lease.

`semaphores_state` returns `[count,maximum]`; `semaphores_release` returns the
previous count. `semaphores_alias` and `semaphores_remove` use the same typed table
and ledger, never guessed SDK ownership. Release is atomic at the SDK. Metadata,
service or later guest-write failures do not roll back an accepted count change.
All operations recheck observed KSV liveness. A known revocation is never normal
success; callers must stop/report accepted prefixes rather than blindly retry.

`semaphores_wait_begin` returns a task-bound `SemaphoreWait`. Its one typed wait
attempt uses 0..1000 ms off the window owner thread and consumes one permit only
on success. It publishes the actual outcome/completion before returning. Close
and last-alias removal refuse active/unknown waits. Completed wait records can be
reaped by the creator after cancellation without reacquiring a cancelled guard.
The domain has 32 wait slots; SDK task/sync/record quotas still apply independently.
`semaphores_close` requires the actual domain and table creator. It journals
retirement, drains native objects and aliases and releases completed wait records
before releasing its ledger. An incomplete close retains its obligation; a
completed revocation error remains observable on repeated close.

## Exact records

All fields are little endian u64, exact lengths, strict reserved zeros, full IDs.
SSD1 is 2624 bytes: magic `0x31445353`, root, KSV, HTR2, creator task, closing
at offsets 0/8/16/24/32/40; 48/56 reserved. 128 rows begin at 64, each native ID
and state (0 free, 1 reserved, 2 live, 3 retiring). 32 wait slots begin at 2112,
each SWL1 ID and state (0 empty, 1 active, 2 retiring). Duplicate IDs, malformed
states, wrong root/services/table/task and event-domain substitutions deny.
SWL1 is 80 bytes: magic `0x314c5753`, root, KSV, HTR2, SSD1, native ID, actual
task, completed at 0..56; sign-extended i32 outcome at 64, reserved zero at 72.
Malformed/truncated/resized records never establish typed ownership.

## Compatibility and checks

`check.py --app <app> --app-lib <libgamepkg.rlib> --wine --evidence <new-dir>`
compares 1344 bytes with ten unchanged, byte-pinned original C function bodies,
runs real SDK fault cases and required-failing copies, then admits, optimizes and
audits all eight game objects before trusted linking. Linux32/64 fixtures run
inside confinement. Windows32/64 component execution is Wine; signed Windows
artifacts are build-only. The C and altered SDK test shells cannot be packaged,
even unused. No Python or C test provider is an end-user dependency.

The provider around those original functions deliberately models the documented
Windows semaphore contract. It is **not** the upstream POSIX shim: the pinned
`toolkit/src/platform/win32_compat.c` accepts invalid creation and clamps excessive
releases, whereas Windows rejects invalid counts/over-release without changing
state. GamePKG consistently follows the documented Windows rule on both targets.
This is an explicit invalid-input correction, not a claim of byte-equivalence to
that POSIX shim. Source hashes and this comparison scope are pinned in
`reference_inputs.json`. See Microsoft's [creation contract](https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-createsemaphorew)
and [release contract](https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-releasesemaphore).
These tests establish neither original gameplay nor physical Windows controls,
audible audio, visual equivalence or measured frame pacing.
