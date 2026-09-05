# Independent inventory workers (alpha.12)

Startup and Services no longer enumerate on the live system sampler, either at
startup or during later refreshes. `src/inventory.rs` owns exactly one worker for
each inventory. This removes those two native-call paths from the one-second
CPU/process/GPU/tray publication cycle; it is not proof that every other provider
can never stall.

## Publication and scheduling

- Each worker starts one read immediately, then waits 30 seconds after completion
  before its next automatic read. Services can be refreshed early through the
  existing Refresh list action or after a service command finishes.
- Each worker permits one in-flight read and one queued follow-up. Repeated early
  refresh requests coalesce. A request during a read does not start another thread;
  the follow-up receives a new start timestamp only after the prior call returns.
- Native enumeration, Startup cache merges and alphabetical ordering run outside
  the publication lock. Completed data uses immutable Arc snapshots and Health.
  Reading uses try_lock and keeps the previous local snapshot on contention.
- The sampler consumes the latest inventory snapshots during its normal cycle.
  Worker completion does not introduce a new UI animation/repaint loop. Other
  provider work can still delay the sampler consuming that completed result.
- Startup's five sources retain the alpha.10 merge/completeness rules. They share
  one Startup worker; one blocked Startup source can still delay its siblings,
  but it cannot hold up the separate Services worker or directly block the sampler.

## Slow calls, cache and shutdown

After five seconds in flight, the consumer reports an inventory-timeout issue and
preserves previous fields. With previous successful data this is stale/cached, not
an empty successful result. With no prior data it becomes unavailable. Startup's
source/row flags become cached without deleting entries; its failed view is built
once for that attempt and reused, not cloned/sorted every tick. That one-time
bounded snapshot invalidation still executes on the consumer.

A pending/failed-worker state has no completed query-duration value. Its elapsed
wait is not represented as a finished native-call measurement. A returned result
restores normal health/completeness while retaining the original read-start time;
an old slow result cannot masquerade as a newly started read.

Five seconds is a reporting deadline, not native cancellation. A stuck worker is
not replaced, so repeated refreshes cannot accumulate blocked threads. A worker
that exits or fails to start remains unavailable until app restart; there is no
automatic respawn loop. Shutdown sets a stop flag, disconnects the wake channel and
does not join a stuck call. The closure retains its own buffers until it returns
or the process exits. No thread is force-terminated.

Raw native failure messages never enter the default support report. Closed issue
codes distinguish source-read failure, timeout and worker unavailability. Cached
fields remain in their existing rounded/zebra layout with explanatory hover detail.

## Verification

Five injected-worker tests cover independent updates with another worker blocked,
refresh coalescing, original versus follow-up timestamps, cached Startup rows,
failed-view reuse/recovery, nonblocking publication reads, worker unavailability,
bounded drop and cancellation of a queued refresh on shutdown. The prior service
cache failure/recovery regression moved from sampler tests into this module.

The specifically selected read-only native worker probe passed on 2026-09-05:

```powershell
cargo test --offline native_inventory_workers_publish_read_only_snapshots -- --ignored --nocapture
```

It returned 13 Startup entries and 303 services. Startup collection measured
0.7659 ms, Services 1.6804 ms; 1,000 snapshot-pair reads took 211.1 microseconds.
Both results were observed in 10.4759 ms including the probe's polling wait. This
is one short debug-build worker check, not a whole-app frame-rate, startup-time,
close-latency or soak benchmark. It sends no service commands and opens no windows.

The native probe is ignored by the ordinary suite and selected explicitly. Never
run blanket ignored tests on Trent's working desktop; one older test creates a
real tray icon. See `HEADLESS_QA.md` for the separate offscreen visual coverage.

Remaining performance work includes potentially slow sysinfo/PDH/NVML/process-control
queries, snapshot copying and process view sorting, plus separately authorized
native drag/close and sustained-use measurements. No driver or input hook is installed.
