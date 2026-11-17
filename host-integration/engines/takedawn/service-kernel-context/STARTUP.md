# Authenticated kernel-import startup owner

`startup.rs` is ordinary dependency-free checked port Rust. It composes the
existing kernel services, interrupt registry, bound context and authenticated
complete-import planner. It does not move the engine or a media bridge into the
trusted host, add an SDK operation, start a window/device, execute a title entry,
or enable an unmigrated native profile.

## Call order and ownership

The coordinator first authenticates the read-only in-place ISO and executable,
loads the image into the GamePKG-owned layout, applies the checked CPU bootstrap,
and initializes the serialized heap and kernel data. Production data uses
`Data::data_initialize_live`; the deterministic explicit-tick initializer belongs
to original-reference fixtures. RAM/root/banks, image, heap, data, native CPU,
renderer, input/audio and the actual loop remain coordinator-owned.

`Startup::startup_new(&Memory) -> Result<Startup,i32>` binds the actual creator
and root. It requires a live, uncancelled root with no service record or import
map. The idle owner allocates no records. Call
`startup_prepare(&mut self,&mut Memory,&Image,&Data,remap:&[u16],video:u32,speakers:u32)`
exactly once. It creates KSV3, ISR1 and KIC1 in that order, then uses
`image_imports::imports_image_with_context` and the existing complete publication
path. It publishes no ready state until KPL4 publication succeeds. Every other
unsupported import still fails, including in a correctly authenticated title.
There is no context-free fallback, partial import acceptance or admission waiver.

`startup_phase()` returns a fixed-width status: 0 idle, 1 preparing, 2 ready,
3 failed, 4 closing, 5 closed. `startup_error()` preserves the original setup
error. A failed or closing owner cannot prepare again or issue ready IDs.
Preflight foreign-owner/root or already-started calls reject before preparation.
Failure after preparation begins retains all successfully created owners for
explicit cleanup. Earlier accepted guest patch/protection prefixes are not
rolled back. Abort the attempted session; never retry native entry or replay
publication against that partially initialized RAM.

`startup_ids(&Memory) -> Result<[u64;4],i32>` is creator-only and ready-only. It
returns [KSV3, ISR1, KIC1, KPL4]. It checks the current map identity, its KPL4/root/
context binding and the live service/context/registry association. Invocation
still performs the complete strict map decoder. This array is **not** a new
worker context: workers retain the existing [root,map,heap inbox,heap job] format
from `Memory::thread_context`. IDs are copied metadata, not native objects,
resource leases or a source/build/admission proof.

## Worker lifetime barrier

`startup_spawn(&mut self,&Memory,entry:u32,argument:u64) -> Result<u64,i32>`
requires ready live state, creates a suspended actual SDK task and records its
join obligation before returning. Entry is an integer index in the reviewed
Plan's static task table. The caller supplies the existing reviewed copied
argument, then resumes through GamePKG. It cannot pass a function value. The
existing host task quota still applies; the owner's list is bounded to 32.

For components with their own task constructor, call
`startup_track(&mut self,&Memory,task:u64)` while the shared owners are still
retained, before allowing coordinator shutdown. Zero, creator, duplicate and
unknown/revoked jobs reject. A finished job may be tracked, but still requires a
consuming join. Every job that can use these owners must be registered; this
port owner cannot discover undeclared uses of copied IDs or replace the SDK's
runtime confinement and task ownership rules.

`startup_join_task(&mut self,&Memory,task:u64) -> Result<LocalTaskJoin,i32>`
performs the actual nonblocking consuming SDK join and removes that obligation
only on success. Busy retains it. Alternatively, share another owner's actual
SDK join evidence with `startup_accept_join(&mut self,&Memory,&LocalTaskJoin)`.
Only a currently registered exact task ID is accepted. Unrelated or repeated
evidence rejects without removing a job. The private evidence cannot be forged
by checked game code. Inspect `LocalTaskJoin::result()` and abort/report a failed
worker; releasing its lifetime obligation is not a claim that its work succeeded.

## Cleanup and errors

`startup_close(&mut self,&mut Memory)` is nonblocking and creator/root-only.
Stop and join users first. Any registered job, even finished or consumed by
another owner but not yet witnessed here, returns Busy before changing ready
state or releasing resources. Cancellation does not turn completion into join.
Join evidence and cleanup remain accepted by the actual creator after service
revocation; normal ready access continues to fail.

Once cleanup starts, it is one-way: map, context, registry, services. A same-root
view holding a different nonzero map returns Denied without clearing that map or
our owned map; the owner remains closing and may retry with the proper view.
It never becomes ready again. Context/registry closed flags, rather than an Ok
return alone, determine release progress. A liveness error reported after actual
release is retained while cleanup continues. Service cleanup explicitly releases
its owned record on dead-root/service error without unlinking somebody else's
replacement service. Other unresolved cleanup errors retain their owner state
for retry or confined-session termination. `startup_closed()` means all of this
owner's resources are gone, not that the root, heap, banks or external users have
been cleaned up. Once closed, repeated close returns the same cleanup result.
The original setup result remains available separately.

Snapshot, planning, publication and invocation are separate calls, not an atomic
transaction. A copied record does not freeze a context or prevent revocation
between checks. Serialize the coordinator's lifecycle, keep users registered and
join them before owner release. No `Drop` substitutes for this explicit ordering.

## Qualification command and scope

Run the image suite with current app dependencies and prepared call targets:

```
python3 ../service-kernel-imports/check_image.py \
  --app /path/to/GamePKG/app --app-lib /path/to/libgamepkg.rlib \
  --call-targets /path/to/reviewed/call-targets \
  --evidence /path/to/new/private/evidence --mutations
```

Fixtures are synthetic images read in place. They compose image/CPU/heap/data
startup with the production import owner, actual SDK workers and consuming joins.
No supplied title is entered. Existing image/import rejection cases still run.
All fixture sources, including unused files, are scanned before SDK injection;
each optimized game object is audited before trusted linking. An unused direct
filesystem helper must reject. Linux x64/i686 execution uses actual confined
children. Windows x64/i686 scope here is source/object/link qualification, not
Windows execution, AppContainer, actual gameplay, physical controls, audible
audio, GPU fidelity or measured frame pacing.

Remaining work includes adopting this owner in the complete native session
factory, retaining and joining actual tick/timer/IRQ/title users, migrating all
remaining imports and generated/manual/media code, connecting real handlers and
the complete timer/pacing loop, verified renderer and full builders/launchers.
The source-kit archive, launch defaults, SDK, quotas and admission gates remain
unchanged. This import-startup stage is not a playable title qualification.

## Qualified checkpoint — 2026-11-16

The current four-target fixture passes complete pre-injection source checks and
separate optimization/audit of both emitted game objects before trusted linking.
Actual confined Linux x64/i686 each pass 22 synthetic image cases (the eleven
prior cases plus modes 12–22) and two private ISO cases. The supplied ISO is
independently authenticated against all 716 manifest files before granting the
same read-only open image. Mode 23 uses this production startup owner, rejects
the still-unsupported main imports before publication, preserves the complete
bounded thunk snapshot and cleans up all retained startup owners. No title entry
is executed and no ISO data is extracted or published.

The new synthetic cases cover ready-state publication, retry denial after
failure, wrong root/data/bounds/configuration, replacement-map cleanup denial,
KPL4 magic/root/full-u64-context binding corruption, normal and revoked-service
cleanup, repeated terminal results, four actual worker consumers with shared
four-ID context and native 98 invocation, suspended/finished/unwitnessed join
barriers, wrong/repeated join evidence and failed spawn without a false job.
Record exhaustion after creating exactly one, two or three owners exercises
retained-prefix cleanup; restoration of the original record capacity catches
leaks without increasing the existing quotas.

Five broken copies pass normal source/object/link checks and then fail actual
confined Linux64 behavior: omitted outstanding-job barrier, accepting unrelated
join evidence, losing failed setup state, context-free planning and leaking a
registry after partial setup. Their sources and failure logs are preserved with
the positive receipts. An unused direct-filesystem source is separately rejected
before compilation. An initially emitted `Vec::remove` assertion import was
removed by changing checked port code to bounded shift/pop; no audit allowlist
was changed.

Windows results remain build/admission evidence only. With a bound context,
private main-import inspection reports 107 still unsupported functions versus
108 without the context (98 is conditional). This does not alter full generated
candidate closure counts or enable a title. SDK identity remains
`fe309935dd602e2cf098e50065f6035de33dac88b51d45a2bc1ba47cf026742f`.
