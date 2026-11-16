# Interrupt integration handoff

The checked native register prerequisites are in `registers.rs`. This file
coordinates the remaining port adapter; it does not enable interrupt delivery or
declare an engine bridge trusted. All implementation belongs in dependency-free
checked game source using the GamePKG SDK. The app's generic local services and
confinement remain unchanged. Do not promote the legacy direct-device executable.

## Implemented native function state

Private CPU format **v5 is exactly 316 bytes**, all little endian:

| Offset | Value |
| --- | --- |
| 0..35 | eax, ebx, ecx, edx, esi, edi, ebp, esp, seh_ebp: nine u32 |
| 36 / 40 | df / fp_top: i32; fp_top must be 0..7 |
| 44 / 46 | fp_cc / fp_control_word: u16 |
| 48..111 | eight f64 stack values, exact IEEE bits |
| 112..239 | xmm0..xmm7, four u32 per register |
| 240 | fs_base: guest u32 TIB/PCR address |
| 244 | eflags: u32, default 0x202 as in xbox_memory_layout.c |
| 248 | fp_cmp: i32, default zero |
| 252..315 | mm0..mm7: two u32 per register, low then high word |

The old 244-byte v4 record rejects Invalid before mutation. Encode/decode preserve
all flags/comparison/MMX/floating bits; they do not normalize NaNs or mask flags.
The original 244-byte prefix and all original field offsets are retained.
`decode(&mut self, bytes)` accepts only v5 and validates fp_top before any write.
`irq_prepare(&mut self, stack_top: u32)` performs the original worker-stack setup:
set guest ESP, zero EAX/ECX/EDX/EBX/ESI/EDI, preserve everything else. The supplied
guest stack must come from the port's checked RAM stack allocator; this is not a
native stack switch or memory permission grant.

`irq_restore(&mut self, saved: &[u8]) -> Result<(), i32>` restores arithmetic/
function state from that exact record but leaves the **current** fs_base untouched,
as the original `irq_regs_restore` does. A callback that changed its FS value is
not silently reset by this helper. Size/top errors preserve every current field.
These are value operations, not a CPU interpreter, host-register API or an IRQ
handler selector. MMX state preservation does not implement MMX instructions.

The maintainer converter recognizes only the exact pinned g_eflags/g_fp_cmp TLS
accessor shapes and lowers them to these value fields. Forged/aliased/qualified
accessors and MMX accessors still reject. MMX operations require separately
reviewed lowering and original-reference tests. Do not whitelist accessors or
native library calls in admitted game source or object imports.

Private vectors now use **TKNA**, not TKN9: the new CPU length and ninth captured
RAM span at 0x36c0e0 are not compatible with old fixture bytes. Batch limits remain
1..16 batches of at most 4096 cases. Source receipts must bind the current
register source/hash and v5/316 metadata; stale version, length and helper hash
reject. Root288, KSV3/64 and four opaque context IDs remain unchanged. Existing
SDK identity, memory/task/record quotas and compiler/confinement limits remain.

## Checked IRQL component available

`../service-kernel-irql/README.md` now defines the IQL1 controller/task contract,
original PCR/release ordering, separate raised/CLI holds, literal native hook
signatures and explicit partial-error/abort behavior. The controlled differential
and reviewed-worker fixtures pass source/object admission, but are not production
IRQ/DPC handlers. Do not reuse fixture/no-op hooks as a runnable title. Supply the
real finite native callbacks and integrate pending delivery, safe/release races,
worker stacks and timer/DPC roles before enabling any kernel import. Original
holder/profiling diagnostics and STI call-site shadow still need wiring. The
controller has not been imported into the full candidate; unresolved counts and
engine admission remain unchanged.

## Required dispatch-gate semantics

Use a GamePKG-owned actual recursive mutex (local21/create, typed26/acquire,
24/unlock,25/release), real reviewed task identity (14) and bounded copied state
(34..37). Game code must never call native critical sections, TLS accessors,
Interlocked, SDL, OS threads or raw memory. Keep per-task IRQL and shared counters
in port-owned values/records. A copied task/record ID is not lock ownership.

Preserve the original `kernel_hal.c` ordering, including intermediate effects:

- A transition to DISPATCH(2)+ enters the one-CPU recursive gate, once per task's
  counted raise; repeated raises do not increment the shared count twice. ISR
  entry above DISPATCH already holds a delivery gate and does not acquire an
  additional raised-level gate. Raw gate depth and the raised-level-held flag
  are distinct. Track actual logical owner, not truncated native thread IDs.
- Lowering through the boundary first runs a safe point while still at the old
  level and holding the gate. It then drops the counted hold and raw gate,
  defers the release point, writes current IRQL and guest PCR byte fs:[0x24],
  and only then processes the release point. This closes a real post/release
  race and must not be reduced to a final-state-only numeric update.
- The original erroneous KfLowerIrql-to-a-higher-level path sets/publishes the
  requested value without introducing a new counted raise. Preserve its
  diagnostics and pair semantics; an already counted task remains counted
  until it really lowers.
- CLI's first disable acquires a separate recursive gate depth and increments
  shared masking once, clears IF; repeated CLI does not add another depth.
  STI enables only **after its following-instruction shadow**, sets IF, drops
  that mask/depth and runs the original safe/release points. Do not enable at
  the STI instruction or conflate masking with raised IRQL.
- DPC wake-on-insert is conditional on being outside the gate and the original
  prompt setting. Gate/IRQL release wakes only below DISPATCH, outside ISR and
  with pending work. Never run a DPC inline on an arbitrary title thread:
  the original vblank acknowledgement model requires the timer consumer.

Keep bounded service waits off the app window owner. Timeout/Busy is an explicit
incomplete acquisition, not permission to proceed. Cancellation cannot abandon
held recursive depths; coordinate owner cleanup and actual joins. KSV3/root
revocation must deny new work without fabricating callbacks or task completion.

## Required interrupt and callback integration

The original three lines (vblank, APU, OHCI) coalesce pending bits. A poster tries
the gate, posts on contention, then tries once more to close the release race.
Safe points run only on the actual gate holder, below device IRQL, outside ISR,
with native IRQ masking clear. Do not run MMIO signal/trap handlers as safe points.

A safe-point pass takes pending bits and processes lines in their original order.
On guest worker-stack exhaustion, put that line and all remaining bits back; do
not drop them or spin as if delivery completed. A released gate takes pending
work back once to catch posts between its prior safe point and unlock. Preserve
original diagnostics/stats, options and unsupported cases explicitly.

For each delivered ISR: allocate the original checked guest worker stack, snapshot
the complete register values above, prepare the stack/GPRs, mark ISR, enter its
original device IRQL, invoke only a finite reviewed native handler route, restore
IRQL, clear ISR, restore arithmetic values while retaining FS, then free stack.
No function pointer or arbitrary host callback enters the game-facing ABI.
Native handler dispatch, device state and Xbox-specific timing remain port code.

For DPC execution, retain the original KDPC routine/context reads, arg1/arg2
writes at +20/+24, guest stack argument/zero-return setup and IRQL2 bracket.
Use the generated finite direct native dispatcher; unsupported/missing targets
remain explicit failures. Connect `service-kernel-dpc` queue, the timer consumer,
`service-kernel-pacing`, wakeable sleeper, timer/event shadow records, guest TIB
and shutdown. Restore interrupted state on callback completion, including when
callbacks changed IRQL. No engine/media bridge exemption is permitted.

## Qualification boundary

`check_irq_state.py --wine` pins the original state type/save/restore and prepare
prefix, compares 4096 full-state transitions per target, and requires a corrupt
FS restore to fail. `registers_tests.rs` tests 65536 arbitrary codec records and
atomic rejection. `irq_state_checks.rs` runs in the actual checked game fixture.
The full native harness adds original sub_002490E1 comparison behavior and the
complete source/object build matrix; see README and the app validation handoff.

These tests do not deliver an interrupt, execute a DPC or establish correct
thread/gate ordering, device behavior, graphics, audible audio or gameplay.
The remaining gate/callback adapter needs original-C ordering/race fixtures,
real reviewed-task concurrency, teardown/protection controls, all-object builds
and actual Linux gameplay. Wine components do not qualify real Windows policy.
