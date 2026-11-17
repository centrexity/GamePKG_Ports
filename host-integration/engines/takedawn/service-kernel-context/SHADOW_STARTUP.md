# Authenticated startup owns the timer shadow map

`Startup::startup_prepare_with_shadow(&mut self, &mut Memory, &Image, &Data,
remap: &[u16], video: u32, speakers: u32) -> Result<(), i32>` is the explicit
KIC2 startup path. It derives imports from the authenticated in-place image,
creates KSV3 then ISR1 then one Shadow then KIC2, and publishes the complete KPL4
map only after full planning. The existing `startup_prepare` remains KIC1 with
its previous initial allocation requirements. Neither path admits unsupported
imports, changes source/object policy, or enables a blocked builtin profile.

Idle owns no records. Preparation remains once-only: any partial setup retains
its owners and error in failed phase; callers close rather than retry. KIC2 adds
one initial shadow record and one native SDK mutex, within the existing 64-record
and 128-sync quotas. Mapping pages and native events consume the same bounded
budgets when created; startup does not reserve unlimited future capacity.
Failures at each service/registry/shadow/context/map stage stay explicit.

`Startup::startup_shadow(&self, &Memory) -> Result<kernel_pool::Shadow, i32>`
requires the ready live creator-owned startup and returns a borrow, never an
owner. `startup_ids` still returns four IDs `[KSV3, ISR1, KIC1-or-KIC2, KPL4]`.
The worker context stays `[root, KPL4, heap inbox, heap job]`; workers decode the
KIC2 from their map and borrow its shadow. No extra worker ID or host SDK call is
introduced. Startup checks that the context's full-u64 shadow identity is the
one it actually owns. A different live map with the same root/KSV cannot replace
it, and a KIC1 owner cannot silently acquire an unowned KIC2 map.

All subsystem jobs must remain tracked until an actual consuming join is shared
back to Startup. `startup_close` returns Busy before any state change while even
one tracked job remains; a finished snapshot is insufficient. After all joins,
cleanup proceeds one way: KPL4, KIC2, all shadow events, shadow pages/record/native
mutex, ISR1, KSV3. Borrowers must stop using all of these first. Event cleanup is
not merely dropping the map's record: native events must be revoked too.

Non-Denied event-drain errors retain the map for explicit cleanup. Denied is
remembered as the stable close error, and cleanup still requires actual shadow
closure before releasing the remaining owners. A false closed flag, copied ID
or completed task cannot substitute for resource release or join proof.
Revoked-KSV cleanup releases the owned resources and preserves Denied on repeated
completed close. A cancelled creator can still encounter the underlying Shadow
owner restrictions; errors do not imply completed closure. Inspect
`startup_closed()` and retain the owning value whenever cleanup is incomplete.

The retained timer composition now has a KIC2 mode using this very same map.
Its primary timer goes through native import 113 before asynchronous producers
start. An actual assigned timer worker with retained guest stack/TIB invokes 113
for a second timer, checks all 316 CPU bytes (only EAX/ESP change), and proves the
import map identity equals Dispatch's event map. Kind 2 creates an auto-reset
event despite its even value. The original ordered DPC/timer paths use controlled
finite callbacks and inert IRQ/profiler providers; this is not the original loop.
Startup closes both timer events only after the actual timer and periodic Tick
jobs have been joined. The fixture verifies both events/map are gone and all 128
native sync slots can be allocated again, including after KSV unlink.

Qualification covers 48 synthetic startup cases plus three independently
host-authenticated private ISO checks per Linux architecture (102 actual confined
sessions total), all four source/optimized-object/link targets and 21 required-
failing compiled copies. New controls reject an omitted KIC2 binding, skipped
native-event drain, accepted same-root substitute map and leaked owned map.
Four record-quota prefixes and native-mutex quota exhaustion recover their full
budgets. The initial attempt correctly failed a fixture's missing expected
native-quota error; a later deliberate broken-copy test had a compile mistake.
Neither attempt is relabeled as the final qualification: bad copies must compile
and pass source/object checks before their runtime rejection counts.

The private main import gap is 106 with KIC2, 107 with KIC1 and 108 without a
context. All three paths still reject the full title before native entry. No full
generated-source closure count is requalified here. Windows image scope remains
source/object/link only; older component Wine results are separate. These tests
do not establish real Windows confinement, original gameplay, physical controls,
audible audio, GPU fidelity, original handlers or measured frame pacing.

SDK `fe309935dd602e2cf098e50065f6035de33dac88b51d45a2bc1ba47cf026742f`,
root/KSV/CPU/worker codecs, quotas, the public source archive, pinned submodule,
catalog, defaults and blocked title profiles stay unchanged. Checked source-kit
and exact optimized-object identities must rebuild. The installed matching host
SDK needs no binary update. Complete native-loop/handler/import/source/renderer/
media/build/launcher migration and actual gameplay qualification remain required.
