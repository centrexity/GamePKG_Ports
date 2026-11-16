# Remaining native IRQ integration

The checked queue/epilogue boundary is implemented and qualified for the scopes
in README.md. This is not a playable port, title admission or production device
qualification. Do not use fixture routes/observer/clock as real game adapters.

Remaining required work:

1. Supply actual finite generated native vblank/APU/OHCI handlers in checked
   `irq_routes`. Retain original device state, IRQ acknowledgements, MMIO rules,
   native call targets and visual/audio behavior. Never promote MMIO traps to
   safe points or move an engine/media bridge into trusted host code.
2. Finish original production diagnostics/holder attribution and configuration
   reload/handler-registration adaptation. Constructor options are preserved;
   fixture diagnostic entry counters are not a replacement for those features.
3. Wire `irq_observer` to the original spin wake and timer/DPC consumer roles,
   including DPC prompt behavior. Integrate the existing DPC queue, pacing,
   host-owned wakeable sleeper, timer/event shadows and per-thread TIB. Never
   drain DPCs inline on an arbitrary title thread. Keep the timer's guest stack
   lease for its whole worker lifetime and release on every exit/error.
   The checked queued/release notification adapter is now available in
   `../service-kernel-dpc/notify.rs`; see `NOTIFICATIONS.md` there for exact
   bindings, readiness and partial-error semantics. It does not connect the
   production observer or supply a callback drain automatically.
4. Initialize/share one IQL/stack/IRQ instance per title, attach every fresh Task
   view, and connect the full native thunk/context/bridge and entry paths. The
   existing four-ID native context has not automatically expanded to carry this
   composition. Coordinate the reviewed source-format/context change before
   updating the full candidate's provenance and cache receipts.
5. Complete renderer/media/bootstrap/launch mediation and re-run the complete
   migrated source/object/native execution gates. Current full-candidate/import
   counts remain unchanged by these library components. No profile, source pin,
   creator signature, cache entry or environment setting is an exemption.
6. Verify actual Linux title gameplay, hardware visuals, physical controls,
   audible audio and measured frame pacing. Qualify real Windows policy/graphics/
   controls/audio separately; Wine component success does not prove them.

The tests also found and fixed event-dependent cleanup under full sync quota and
forced stale CAS. Generic SDK events do not categorically deny cancellation;
cleanup now needs no newly allocated event. Long-wait diagnostics explicitly
follow original signed32 Windows/Xbox semantics across targets and do not impose
a gameplay duration limit. See README.md for the exact numeric and test scopes.

Private receipts distinguish unmodified default differential sources from
instrumented checked race/fault copies. Original draft plans and failed runs
remain in private workspace evidence; do not publish them, ROMs or private data.
