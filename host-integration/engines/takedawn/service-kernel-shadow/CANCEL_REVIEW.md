# Cancellation layout review

The pinned source kit is SHA-256
`6b37d24c0984c686c7ed6fd73cd3a5b252b5631ceefa2798e5b7a6e0adff8dc4`.
`toolkit/src/kernel/kernel_bridge.c` has SHA-256
`bf62f8787a2e78a0abdb28e10ad8f79c5267dc655491353af231067747ce446e`.
The references here are source inspection, not gameplay evidence.

`bridge_KeCancelTimer` first drops matching entries through
`xbox_kernel_cancel_timer`, then calls `xbox_KeCancelTimer(XBOX_TO_NATIVE(va))`
and ORs the results. The latter implementation in `kernel_sync.c` expects the
**host** `_XBOX_KTIMER` declared in `kernel.h`: three native pointer/handle fields,
then BOOLEAN Inserted and LONG Period. Its Inserted byte is at 12 on x86 and 24
on x64, rather than the guest dispatcher header's byte 3. If that host-layout
Inserted byte and the first native handle are nonzero, it calls a native timer
queue deletion API against those reinterpreted guest bytes.

The bridge initializer instead clears 40 guest bytes and writes Type at 0 and
Size at 2. Its set path updates the separate 32-slot polling table, then resets
an existing shadow event. It does not construct that host struct or publish a
native timer-queue handle in guest RAM. Reusing the native cancellation helper
would therefore reintroduce architecture-dependent host-pointer interpretation
and direct system access; it is not an approved migration route.

Neither examined cancellation body calls ResetEvent on the shadow event. The
comment claiming that cancellation "clears both" does not establish that
behavior. Do not add a shadow reset or clear guest SignalState merely from that
comment, and do not assume the timer set path writes guest Inserted either.

The checked firing-table cancellation is already implemented in
`../service-kernel-timers/timers.rs`. Finishing the bridge still requires an
explicit reviewed guest-field/result contract and native source witnesses for
that contract, coordinated with the complete timer/object path. Do not expose
raw timer handles, a DeleteTimerQueueTimer service or native struct casts to
preserve this unsafe mixed-layout path. This component does not enable ordinal
97 or claim complete KeCancelTimer fidelity.
