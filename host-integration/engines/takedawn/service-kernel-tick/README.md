# Checked periodic kernel tick

This dependency-free game component supplies the normal periodic KeTickCount
word update using GamePKG-owned clocks, numeric reviewed tasks, events and RAM.
It is native Rust, not an instruction decoder or a trusted host engine. It does
not migrate the rest of the original NV2A ack worker, DSP/fences/MMIO, fast-kick
polling, timer/DPC/IRQ scheduling, or enable title admission.

## Original behavior and clock selection

`reference_inputs.json` pins the complete public source kit, original
`nv2a_ack_thread` byte range and its exact volatile tick assignment. The original
word value is the wrapping u32 sum of GetTickCount() and the low u32 of
`guest_skew_ns / 1_000_000`. `tick_value(uptime_ms, skew_ns) -> u32` preserves
that integer division, truncation and addition wrap for all u64 inputs. The
maintainer C oracle compiles the unchanged assignment; its inert 64-byte backing
only qualifies this arithmetic, not the surrounding device worker.

Initial kernel data has a different rule: **raw uptime without guest skew**.
`Data::data_initialize_live(&mut Memory, region, video, command, hd)` selects an
actual child-local sample at the original tick-field construction point. The
existing `data_initialize(..., tick, ...)` retains its explicit-value contract
for deterministic original-C comparisons. Both share the same initializer;
there is no extra zero-tick publication. Existing non-atomic prefix/error
semantics, keys, launch data and exports are unchanged.

All live reads use app `87b8ffb`'s `local_performance_sample()` / local operation
38, SDK `d53f61beb0f6a40706b0c1c4bd09a912694ae1f1e5405efac9a88d3eda7c7573`,
composite recipe `ae6723644c2a82dc`. No calendar anchor, TSC origin or broker
connection lock is acquired. Clock errors propagate without a remote, native-OS
or guessed-clock fallback. The worker samples uptime before reading shared skew,
as in the original assignment.

## Ownership and API

- `Memory::memory_writable_word_backing(address) -> Result<[u64;2], i32>` validates
  the live root, complete four-byte guest range, writable view, guest/backing
  alignment and committed read/write host page, then copies window ID/offset.
  No native address or pointer escapes. Subsequent SDK writes recheck current
  ownership, complete bounds and page protections; the tuple is not a lease.
- `Data::data_tick_binding(&Memory) -> Result<[u64;3], i32>` additionally validates
  that data belongs to this root, selecting exactly its base+0x20 word. It copies
  root/window/offset, not private keys, a Data owner or a memory-view object.
- `Tick::tick_new()` creates an empty owner; `tick_closed()` initially returns true.
- `tick_start(&mut self, &Memory, &Data, entry:u32)` requires a valid configured
  root and target. The bootstrap's reviewed integer entry must call
  `tick_worker(record_id)`. The source is scanned before that entry/bindings are
  injected; no arbitrary callback is accepted. Create the manual stop event and
  record, create the task **suspended**, publish its exact task ID by CAS, then
  resume. Success means startup was accepted, not that the first tick is ready.
  The worker verifies its reviewed task identity before writing.
- A failed start retains any created resources in that same owner. Close it
  before dropping/reinitializing data or the root. A second start while owned
  is Invalid. No Drop implementation blocks implicitly or hides cleanup errors.
- `tick_stop(&Memory)` signals the manual event; a write already accepted may
  complete. `tick_finished(&Memory)` snapshots task completion, not cleanup.
- `tick_close(&mut self, &Memory)` requests stop and polls/consumes the join.
  Busy retains live resources; retry off the window owner thread. An unpublished
  suspended task is cancelled before joining. Only after join are the metadata
  record and stop event released. A terminal worker error is returned after
  cleanup; transport/revocation/cleanup errors can retain ownership. Inspect
  `tick_closed()` rather than infer full cleanup from a task result. Runtime
  teardown remains the final owner when the entire local service is revoked.
  Repeated close of an empty owner succeeds. Foreign-root operations on a live
  owner are Denied and leave it intact.

The active worker revalidates its immutable metadata, root and pinned KSV3 owner
on each iteration. It checks stop/cancellation before sampling and stop again
before writing. Its requested ordinary wait is 1 ms, through local op 23 on the
child worker, never the window owner. This is not a real-time scheduling or
Windows high-resolution wake guarantee; actual Windows cadence still needs
hardware qualification. GPU fast-kick ordering remains separate required work.
Stop/join this owner before closing kernel services, data, RAM or the root.

The transient record is exactly 64 bytes, all LE u64:

| Offset | Value |
| --- | --- |
| 0 | magic `0x314b4354`, high u32 reserved zero |
| 8 | owning root ID |
| 16 | owned RAM window ID |
| 24 | aligned word byte offset |
| 32 | manual stop event ID |
| 40 | reviewed task ID, published before resume |
| 48 | pinned kernel-services record ID |
| 56 | reserved zero |

IDs must be nonzero; offset+4 must not overflow. Wrong shapes/reserved fields
are Invalid (-1); Busy (-2), Failed (-3), Denied (-4) and Unsupported (-5)
propagate from the SDK. Worker completion returns 0 or the positive magnitude
of its error; close validates that result. The schema is private metadata, not
a host ABI/save format. The referenced service record is now KSV3; the tick record/root/CPU schemas and
all host quotas are unchanged. Warning flags do not enter the tick metadata.

## Qualification

Run `check.py --app APP --app-lib EXACT_DEBUG_RLIB --wine` as maintainer tooling.
Players do not need Python or the original C oracle. Tests include:

- 2,048 exact original assignment vectors on Linux x64/i686 and Windows
  x64/i686 under Wine, including division/truncation and u32/u64 extremes.
- Actual local workers, raw-uptime initialization while skew is nonzero, skew
  advancement without TSC/calendar initialization, and a full 64 KiB RAM
  no-scribble comparison outside the one word.
- Read-only guest views and host pages, alignment/unmapped targets, foreign roots,
  duplicate start, stale/malformed metadata, revoked services and revoked roots.
- 132 successful start/stop plus partial-start cleanup cycles, and actual
  record/event/task exhaustion followed by cleanup and successful reuse.
- Complete source scanning before generated bindings and separate optimized
  game-object audits before trusted linking on all four targets. No engine,
  worker, memory or kernel-data source exception is used.
- Actual confined Linux x64/i686 children continue ticks while the parent withholds
  a **queued** fullscreen request for at least 60 ms, holding the remote client
  mutex. The parent waits for the broker's queue wake before starting that hold;
  no production window thread is blocked. The silent test backend returns
  Unsupported; no real window or audio device is opened.

The existing kernel-data matrix also requalifies 24 original initializations,
400 ordinal lookups, every 64 MiB RAM byte/heap counters and 1,024 certificate
signatures. Wine and cross-builds do not prove actual Windows AppContainer or
hardware input/audio/graphics. These are component/lifetime checks, not complete
title entry, frame pacing, visual fidelity or playable Burnout. The private main
image still has 111 unresolved imports; remaining timers/scheduler/device/
renderer/media/build/launch migration and gameplay gates remain required.
