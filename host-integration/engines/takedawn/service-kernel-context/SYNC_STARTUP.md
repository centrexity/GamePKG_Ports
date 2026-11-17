# KIC4 semaphore-owned startup

`Startup::startup_prepare_with_sync_objects(&mut Memory,&Image,&Data,remap:&[u16],
video:u32,speakers:u32,scratch:u64,offset:u64)` explicitly adds a semaphore owner
to the existing shadow/table/event graph. KIC1, KIC2 and KIC3 constructors keep
exact legacy sizes/requirements. Each accepted owner is retained immediately,
including on allocation, context creation or import publication failure. A failed
startup cannot be retried or entered as a native title.

KIC4 is exactly 88 little-endian bytes. Its u64 fields are magic 0x3443494b at 0,
root 8, KSV 16, connections 24, shadow 32, HTR2 tagged table 40, EVD2 event domain
48, SSD1 semaphore domain 56, and zero reserves at 64/72/80. KIC1/2/3 stay exactly
64/72/80 bytes. Noncanonical lengths, reserved fields and cross-root/services/
table bindings deny. `KernelContext::kernel_context_sync_objects(&Memory)`
returns the copied `[table,event_domain,semaphore_domain]` structural bindings;
these are not ownership capabilities, resource leases or atomic snapshots.
Every typed consumer fully reborrows its ledger and checks actual SDK kind and
liveness before use. A well-formed same-root copied ledger can be structurally
borrowed; it cannot replace the exact retained owner identity in Startup.

`Startup::startup_sync_objects(&Memory)` returns borrowed
`(Handles,Events,Semaphores)` only after validating the exact owned triple and
full typed ledgers. `startup_ids` and therefore spawn/track admission check this
identity. `kernel_context_ready` and actual invocation revalidate KIC4 and full
SSD1 before admitting 193/222. Legacy contexts remain Unsupported for them.

Every actual worker join must be consumed before shutdown. Merely finished or
copied task IDs are insufficient, and replayed join evidence denies. Shutdown
revokes the import plan/context, drains SSD1, drains EVD2, clears tagged scratch,
then closes shadow, connections and services. It never closes backing storage
before the typed domains using it. Busy/incomplete cleanup retains the owner and
its obligation for retry; completed revocation errors persist on repeated close.
An active creation/table guard or wait prevents premature domain destruction.

`service-kernel-imports/check_sync.py --app <app> --app-lib <libgamepkg.rlib>
--call-targets <reviewed-call-targets> --iso <private-image> --mutations --evidence
<new-dir>` checks synthetic modes 69..78/80 and private-image mode 79. They cover
strict codecs, copied identity substitution, malformed typed ledgers, legacy
readiness, real suspended/resumed/consumed-join workers, retained orphans, partial
setup at three quota boundaries, incomplete close and completed revocation.
Mode 70 routes actual imports 193/222 and checks the exact 3100-byte C witness.
Private mode 79 authenticates and reads the supplied image in place and refuses
its remaining 102 unsupported main ordinals before entry. KIC3 remains 104,
KIC2 106, KIC1 107 and no-context 108. No asset is extracted or published.

Seven altered Startup copies must pass source admission, separate optimization
and every object audit, then fail actual contained Linux64 behavior. Positive
Linux32/64 cases execute inside confinement; Windows32/64 cases only build/audit/
link. No source gate, object audit, SDK budget, confinement or profile gate is
relaxed. No renderer/media/handler/main-loop integration or full-title, physical
control, audible output, visual fidelity or pacing claim follows from startup.
