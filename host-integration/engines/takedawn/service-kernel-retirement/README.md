# Creator-cancellation retirement regression

The checked EVD2 and SSD1 drain paths now use GamePKG's typed local retirement
(op 49) directly for an already journal-owned retiring ID. They no longer use a
cancel-sensitive state query to infer that a native object is absent. Wrong
actual kinds and other errors retain the retiring row; successful retirement or
an absent native entry lets existing alias/row cleanup proceed. Known revocation
still remains the domain's observable close error. There is no record/schema,
quota, alias, ACTIVE-wait or creator-ownership change.

This fixes a reproduced lifecycle gap: an actual creator task can be cancelled
after its cleanup table guard is acquired, then a state query returns Denied for
a still-live event/semaphore. The old drain treated that denial as absence,
removed aliases/ledger ownership and reported a closed domain while the native
object remained. The typed retirement operation deliberately remains available
for cancelled-task cleanup and validates actual kind without acquiring anything.

Run `check.py --app APP --wine --evidence NEW_DIR`. The maintainer-only provider
interferes at the query boundary and calls the actual public SDK cancellation
operation on the actual owning task. It forges neither cancellation bits nor
native objects. Each domain closes with the observed Denied, the actual worker
returns and is joined, and the uncancelled bootstrap verifies the native entry
returns precisely Denied. Restoring either old query/skip path compiles and must
fail that final native observation. Linux32/64 run directly; Windows32/64
components use Wine. Existing source/object/confinement fixtures separately
compile the real changed owner code; this provider itself is rejected even if
packaged as unused source.

The checkpoint proves **domain/native retirement**. It does not pretend the
cancelled creator can reacquire its table mutex and complete every root/table/KSV
obligation: the outer runtime/process teardown owns those remaining obligations.
It does not enable an NT import/title, prove original rendering, physical Windows
controls, audible audio or frame pacing. SDK
`49a313ff062e83e3bcbaadb125f4ace190847b0321cc25877e45f92023f0234a`
is required; older SDK Unsupported is not silently ignored. Maintainer Python is
not an end-user dependency.
