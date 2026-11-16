# One actual join across shared owners

A timer/DPC worker can also own pacing state. Joining separately through each
controller consumes the same runtime job twice. Use the SDK private-field
`LocalTaskJoin`, minted only by a successful `gamepkg::local_task_join(task)`.
Its task ID and return value are copied u64 values, not native handles or leases.
The return value still needs application-level interpretation.

- `Queue::queue_join_task(&mut self, &Memory)` and
  `Schedule::schedule_join_task(&mut self, &Memory)` perform the one actual join
  and return the typed receipt. Busy retains the controller's state.
- `queue_accept_join(&mut self, &Memory, &gamepkg::LocalTaskJoin)` and
  `schedule_accept_join(&mut self, &Memory, &gamepkg::LocalTaskJoin)` adopt that
  receipt only for the assigned task, creating task and memory root. Borrowed
  controllers reject. Wrong-job receipts return Denied. Unassigned/already joined
  controllers return Invalid. All owners must adopt before their normal close.
- Legacy `queue_join` / `schedule_join` return only the scalar result. They remain
  suitable for standalone owners; they discard the typed proof and cannot be used
  to coordinate a later shared join. A finished snapshot cannot authorize close.
- Queue's existing `queue_join_witness` can be obtained after either successful
  queue join or receipt acceptance. It retains the private queue/root/task checks
  required by timer-table close, including after queue close. It does not replace
  the SDK receipt needed by other controller types.
- Stop producers, release the worker's retained resources in their required order,
  join exactly once, adopt evidence into owners, then close. Receipt adoption is
  allowed after configuration revocation for cleanup, but does not grant live
  reporting or extend a record/stack lifetime. Revoked close cleans then reports
  Denied. Failure to stop/join still requires confined process teardown.

No shared record schema, syscall, quota, dependency or import exemption changes.
Full SDK: `fe309935dd602e2cf098e50065f6035de33dac88b51d45a2bc1ba47cf026742f`.

Run `check_join.py --app APP --app-lib APP_RLIB --wine --evidence DIRECTORY`.
Three real join origins coordinate two queues, a schedule and a timer table.
Tests reject wrong jobs, borrowed/foreign roots, premature close and duplicate
joins; cleanup after revocation is checked. Three weakened implementations must
fail. Four-target vectors match; every one of eight optimized game objects per
target is audited after complete source scanning and before trusted linking.
Linux x64/i686 run confined. Windows component execution uses Wine. Pacing,
queue and timer-firing regressions also pass with this SDK; pacing emits two
objects, queue/firing eight, and all emitted objects are audited.

This is worker-lifetime composition, not the production timer loop, IRQ/profiler,
TIB/bootstrap, title source closure, launcher integration or gameplay. Both title
profiles remain blocked. Do not enable a legacy direct-device path.
