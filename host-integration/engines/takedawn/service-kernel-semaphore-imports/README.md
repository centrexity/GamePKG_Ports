# Actual NT semaphore imports

This dependency-free game module uses shared GamePKG services only. It enables
classification/readiness for ordinal 193 (`NtCreateSemaphore`, 16 argument bytes)
and 222 (`NtReleaseSemaphore`, 12 argument bytes) **only with live KIC4**. Earlier
KIC1/2/3 contexts deny readiness before publication or invocation. The native title
profile remains blocked; implementing two imports does not enable title entry.

193 reads `(output, ObjectAttributes, initial LONG, maximum LONG)` from the checked
complete caller frame. ObjectAttributes stays ignored, as in the pinned source.
Invalid signed/count inputs or actual SDK capacity failure return
`STATUS_INSUFFICIENT_RESOURCES` (0xc000009a), leaving output untouched. Success
uses the retained creation transaction and copies a four-byte guest tag. Null
output retains an owned orphan. Tag insertion and guest write failures are fatal
accepted prefixes, never invented NT success. Normal completion sets only EAX and
advances ESP by 20, preserving every other CPU byte.

222 reads `(tag, release LONG, PreviousCount)` and validates full typed ownership
under the tagged table guard. Ordinary invalid tags, kinds, unowned/stale native
objects or invalid/excessive counts return `STATUS_INVALID_HANDLE` (0xc0000008)
without writing PreviousCount. Corrupt/protected metadata, root/services failures
and internal errors propagate rather than being disguised as invalid handles.
On success the count changes atomically first; optional PreviousCount then gets
four LE bytes. Later protected/unmapped writes or observed revocation are fatal:
the accepted count remains, CPU stays unchanged, and callers must stop/report.
Normal completion changes only EAX and ESP (+16). All frame/argument reads precede
normal mutations; success is revalidated after accepted changes and guest copies.

The C witness contains ten unchanged byte-pinned original function bodies. Its
provider models the documented Windows count/error rules on both platforms;
invalid-input behavior intentionally corrects the upstream POSIX clamp/unchecked
creation shim. See `../service-kernel-semaphores/README.md` and the pinned
`reference_inputs.json` scope. It does not claim equivalence to that buggy shim.

`check.py --app <app> --app-lib <libgamepkg.rlib> --wine --evidence <new-dir>`
requires a 3100-byte original-C/actual-SDK comparison on all four targets, seven
SDK fault scenarios per target and thirteen functionally failing altered copies.
It also scans all complete game modules, rejects packaged provider/runtime code,
optimizes and audits eight game objects separately before trusted linking and
runs confined Linux32/64 children. The manual KPL4 setup here is synthetic;
authenticated image startup is checked separately by
`../service-kernel-imports/check_sync.py --mutations`.
Windows components run through Wine; signed Windows images are build-only.
These tests do not establish actual title entry, graphics, audible sound, physical
controls, real Windows confinement or measured frame pacing. Generic NT close,
single/multiple waits, file/thread adapters and all remaining imports need their
own typed ownership, complete semantics, integration and qualification.
