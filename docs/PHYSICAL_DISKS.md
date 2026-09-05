# Physical disk activity

Alpha.18 adds Performance > Physical disks. Mounted-volume pages remain separate
and are labeled Volume. PDH disk numbers and instance names do not establish a
physical-device serial, volume extent mapping, controller model or sensor identity.

## Readings

| Field | Windows PhysicalDisk counter | Interpretation |
|---|---|---|
| Active time | `% Idle Time` | `100 - idle`, clamped to 0-100 after validity checks |
| Response time | `Avg. Disk sec/Transfer` | Seconds converted to milliseconds per completed transfer |
| Queue depth | `Current Disk Queue Length` | Outstanding requests, including requests in service, at collection |
| Read speed | `Disk Read Bytes/sec` | Physical-device read throughput |
| Write speed | `Disk Write Bytes/sec` | Physical-device write throughput |

Active time is deliberately not queue-weighted `% Disk Time`. Queue/rate/latency
values are not capped at 100. Valid zero is retained; NaN, infinity, negative values,
failed API results and invalid counter statuses never become live measurements.

An instantaneous queue sample supplies initial instance labels while rate counters
collect their baseline. Every field remains present. Last successful values carry
their own timestamp and are labeled Cached when the new sample cannot supply them.
Values older than three seconds cease to be live. Complete array reads confirm
removal; partial reads cannot silently remove retained devices. Selection follows
the PDH instance string, not enumeration position. A removed selection does not
silently switch to another disk. Disk numbers/instance names can be reused after
hotplug, so this is presentation identity, not persistent hardware identity.

Activity, response and queue charts hold 120 display samples, nominally one second
apart. Their axis says **120 samples**, not an exact elapsed duration. Cached,
duplicate-provider and unavailable samples leave gaps rather than extending a line.
History scales use the declared sample window, not VecDeque allocation capacity.

## Threading and bounds

One independent `trontop-disk-activity` worker owns its query and five wildcard
handles. No PDH call occurs on the renderer or main system sampler. A one-slot
mailbox overwrites older pending snapshots with the newest completion. Consumers
use a non-waiting lock and retain their previous immutable snapshot on contention.
No native operation runs under the publication lock. A stuck call does not spawn
a replacement worker or block monitor drop. Normal idle waiting uses park/unpark.

PDH buffer sizing starts from zero, retries size races at most three times and
caps each array at 1 MiB and 129 entries (128 devices plus the total). An aligned
allocation holds native items and trailing UTF-16. Count, pointer offset, string
region, terminator and encoding are validated before names are used. Names are
bounded and aggregate/non-numbered instances excluded. Duplicate identities in a
column are unavailable, not summed. Retention is limited to 128 devices.

The query closes its own handles on normal worker exit. An OS call that never
returns can keep that one detached worker/query alive until process exit. This
is bounded shutdown waiting, not cancellation of arbitrary Windows calls.
Failed handle setup retries on a 30-second backoff. Existing handles retain their
sampling state; a failed collection requires a new rate baseline.

Provider diagnostics report freshness, query duration and readable-field coverage.
JSON export preserves per-field value/state/age. Raw PDH instance text is excluded
unless private details are explicitly enabled; disk number and metrics remain.
Default support reports include only the closed provider issue vocabulary, not
instance names. The new worker role is allowlisted in local failure reports.

## Evidence and limits

Ordinary tests cover unit conversion, zero/invalid/duplicate counters, malformed
native arrays, partial/failure/stall/recovery, removal and retention bounds,
non-waiting mailbox/drop, cached/missing JSON export and identity-based selection.
Headless UI checks cover dark/light at 1040x640 and 1280x760, stable metric baselines,
missing-state geometry, chart gaps and three graphs fitting the larger viewport.
The visual pass adds three production-UI fixtures, not runtime fake telemetry.

Run only the explicitly selected native probe:

```powershell
cargo test --offline native_physical_disk_pdh_probe -- --ignored --nocapture --test-threads=1
```

The final read-only probe on 2026-09-05 returned five valid readings on each of
three physical disks after warmup. Initial setup/collection took 136.823 ms;
three warm collections took 0.210, 0.168 and 0.188 ms. An earlier run also passed.
These are short debug-provider observations, not whole-app overhead/FPS benchmarks.
No file workload was generated, no disk setting changed and no native app window,
tray test, input injection, focus change or elevated disk access was involved.

Still unverified: non-English Windows, live unplug/replug, disabled/damaged PDH
categories, RAID/Storage Spaces/controller mappings, long stalls on real devices,
and a native end-to-end UI/close/soak test. No claim of full Task Manager parity.

## References

- [Microsoft: PdhAddEnglishCounterW](https://learn.microsoft.com/en-us/windows/win32/api/pdh/nf-pdh-pdhaddenglishcounterw): language-neutral counter paths. Adding a handle alone does not prove an instance has usable data.
- [Microsoft: formatted wildcard counter arrays](https://learn.microsoft.com/en-us/windows/win32/api/pdh/nf-pdh-pdhgetformattedcounterarrayw): buffer sizing, status handling and baseline collection. This implementation formats wildcard arrays directly rather than expanding individual handles.
- [Zabbix Windows agent template](https://github.com/zabbix/zabbix/blob/master/templates/os/windows_agent_active/README.md): independent monitoring reference for busy time derived from idle time. No third-party implementation code was copied.
