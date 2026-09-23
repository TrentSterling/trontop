# Trontop telemetry architecture

## Current sampler

The sampler owns one long-lived `sysinfo::System`, disk list, network list, user
list, and GPU PDH query on a dedicated thread. It refreshes live counters every
second, publishes one immutable snapshot, and requests one UI repaint. Startup and
Services use independent fixed workers in alpha.12, with 30-second waits after each
completed read. Service commands can request an earlier read through the next sampler
cycle. Neither inventory's native calls execute on the sampler or render thread.
The render thread performs no telemetry
operating-system queries.

Alpha.11 service Start/Stop/Restart runs on a separate single-flight command worker,
not the sampler. Alpha.12 retains typed observations independently per service;
failed inventory attempts and commands to other services cannot erase them.
`SERVICE_CONTROLS.md` describes timestamps, capacity and uncertain-result handling.
`INVENTORY_WORKERS.md` documents read coalescing, cache retention and slow-call states.

The sampler also publishes a compact CPU/memory/GPU/process-count sample directly to
the dedicated `trontop-tray` thread. A coalescing slot and native thread message wake
that thread. The tray icon, menu and native message loop are owned and destroyed there;
updates no longer depend on the main UI accepting a sample. Hovering the icon does not
request UI repaints. The 26-observation CPU history advances even if successive values
round to the same percentage. The file/taskbar icon remains the static Tront T mark.

The explicit interactive test `native_tray_updates_without_any_ui_frames` is ignored
by default; it creates a real tray icon. Do not run desktop-interactive checks on
Trent's working desktop without fresh agreement. The headless UI smoke harness is
implemented separately; see `HEADLESS_QA.md` and `AGENTS.md`.

Process CPU is a whole-machine percentage (the share users expect from Task
Manager): `100 * process CPU time delta / system CPU time delta`, where the system
delta is `GetSystemTimes` kernel (including idle) plus user time. This is exactly
sysinfo's per-process formula divided by the logical processor count, which is how
earlier builds normalized sysinfo's per-logical-processor value. Disk byte deltas are
divided by the measured interval, not an assumed perfect one-second interval.

## Process refresh stalls (fixed 2026-09-23)

Symptom: on a busy box (about 470 processes, many short-lived shells from other
tools) the sampler published roughly one sample per minute. Graphs were sparse and
the headless gauntlet warmup accepted about 3 samples in 20 s.

Root cause, measured on this box (Windows 11 26200, 24 logical processors):

- sysinfo 0.38.4 `compute_cpu_usage` (`src/windows/process.rs`) calls
  `GetSystemTimes` once **per process** on every refresh that includes CPU. The
  default `refresh_processes` kind includes CPU.
- `GetSystemTimes` is `NtQuerySystemInformation(SystemProcessorPerformanceInformation)`.
  On this box that query intermittently costs 100 to 670 ms per call (a standalone
  loop measured 0.12 ms for the first seconds, then 130 to 195 ms averages; the
  same query class called directly behaves identically). The slow state comes and
  goes system-wide and is not tied to one process or thread.
- A field bisect over repeated calls: `nothing` 0.02 to 0.10 s; `cpu` only 45 to 66 s
  per refresh. An instrumented copy of sysinfo (scratch only, never the registry)
  attributed 42.7 s, 62.3 s and 57.0 s of 42.8 s, 62.4 s and 57.1 s refreshes to
  `GetSystemTimes`; `GetProcessTimes` totaled 4 ms, and memory, I/O counters, user
  token, command line and exe together stayed under 0.12 s.
- The old app also only read user, command line and working directory at startup
  (`System::new_all`); processes that appeared later never got them.

Fix (`src/process_cpu.rs`, `src/sampler.rs`):

- sysinfo refreshes processes without CPU: memory and I/O counters every sample,
  exe/user/command line/working directory `OnlyIfNotSet` (read once per process
  instance, including processes that start later), environment never.
- Per-process CPU time comes from one `NtQuerySystemInformation(SystemProcessInformation)`
  snapshot per sample (about 15 to 65 ms for 470 processes) plus one
  `GetSystemTimes` call. Each row joins by PID and must match the snapshot's
  creation time to sysinfo's start time, so a reused PID never inherits another
  instance's CPU. A new instance reports 0 on its first sample, as before. PID 0
  (Idle) stays 0, as sysinfo reported it. A failed snapshot marks System telemetry
  Partial and clears the baseline instead of dividing across a gap.

Measurements (`native_sampler_cadence_read_only_probe`, 60 s each, same box):

| | per-sample refresh | interval between samples |
|---|---|---|
| before, `refresh_processes(All)` | calls 3+: 50.2 to 58.2 s (first two 0.08 s) | about 1 per minute |
| after, slow-kernel phase | p50 0.60 to 0.67 s, p95 0.82 to 0.86 s, max 1.0 s | p50 1.00 s |
| after, fast-kernel phase | p50 0.046 s, p95 0.048 s, max 0.98 s | p50 1.000 s, p95 1.001 s, max 1.21 s |

The "per-sample refresh" after the fix is the whole System provider step (CPU PDH,
memory, clocks, processes, disks, networks). In the slow phase its remaining cost
is not the process table alone: sysinfo's PDH CPU refresh took about 250 ms and
`Networks::refresh` about 150 ms, and the single `GetSystemTimes` about 130 ms.
The gauntlet warmup now accepts 20 samples in 20 s.

`native_process_cpu_matches_sysinfo_read_only_probe` spins one thread in the test
process and compares sysinfo's old value (divided by logical CPUs) with the native
path over the same one-second windows: worst difference 0.164 percentage points
over six rounds, accumulated CPU within 125 ms. sysinfo needs three refreshes of a
new PID before its first real delta; the native tracker needs two.

## Native Windows providers

Current providers use these Windows sources:

1. The `GPU Engine` PDH object is enumerated for exact instances. The identity includes
   PID, adapter LUID, physical adapter index, engine index and engine type. Every
   30 sampler ticks, inventory reconciliation adds/removes individual counters on
   the existing query. Retained handles keep previous samples; only new counters
   require priming. Inventory failures preserve handles and mark incomplete coverage.
2. Service inventory comes from `EnumServicesStatusExW` and includes state and PID.
3. Startup inventory reads the documented HKCU/HKLM Run keys and both Startup folders.

## GPU aggregation and missing data (alpha.8)

`gpu_activity::Usage` distinguishes Measured, Partial, Warming, Unreported and
Unavailable. API success alone is insufficient: formatted PDH values require a valid
counter status and finite, nonnegative data. Real zero is preserved. A failed query
collection breaks the interval and re-primes rates on recovery.

- A process reports its busiest measured engine across adapters, not the sum of
  engines that can operate in parallel.
- A physical engine sums its PID readings. The machine summary is the busiest
  physical engine, not the sum of all engines named 3D, or of different adapters.
  The current engine-type rows similarly show the busiest engine of that type.
- A process without a returned counter is Unreported, not proven idle. Process
  creation can wait until the next inventory cycle plus a rate-priming sample.
- Partially readable totals are labeled `>=` and explained as lower bounds. Process
  tree/account groups still sum process peaks and can exceed 100%; their tooltips
  explicitly distinguish this total from whole-GPU utilization.
- Missing values use `-- %`, including in the selected inspector. Numeric GPU values
  sort ahead of missing entries in both directions. State text does not shift
  neighboring inspector fields during refresh.
- Only fully measured machine samples extend the GPU chart. Missing/partial samples
  make gaps. Compact meters and the tray GPU tooltip conservatively report missing
  when the overall value is partial; they do not display an incomplete number as exact.

These aggregation choices follow Microsoft's documented Task Manager semantics for
busiest-engine summaries. They do not prove sample-for-sample parity: polling windows,
inventory age, unsupported counters and Task Manager's internal collection differ.
Per-adapter identification, engine mapping to NVML GPUs and per-process VRAM remain open.

The opt-in native refresh check retained 690 handles, measured 2.938 ms inventory
time and produced 690/690 valid counter readings after refresh on 2026-09-05. This
is a short read-only lifecycle check, not a whole-app performance or accuracy benchmark.

- [Microsoft: GPU metrics in Task Manager](https://devblogs.microsoft.com/directx/gpus-in-the-task-manager/)
- [Microsoft: adding/removing counters on a PDH query](https://learn.microsoft.com/en-us/windows/win32/perfctrs/creating-a-query)

Future providers should:

1. Prefer the Windows 11 `Process V2` counterset for advanced per-process counters.
   Its instance identity avoids the name churn that affects the older `Process`
   object when processes exit between samples.
2. Keep PDH queries long-lived and collect at least two samples before publishing a
   rate. Counter rates are deltas and do not have meaning from one observation.
3. Use `QueryFullProcessImageNameW` as a fallback when the base sampler cannot obtain
   an executable path. Access failures are normal and should remain non-fatal.
4. Add ETW only where cumulative counters cannot provide a useful rate, most likely
   per-process network traffic. ETW collection belongs on its own worker and must have
   a bounded handoff to the UI snapshot.

Primary references:

- [Collecting Performance Data](https://learn.microsoft.com/en-us/windows/win32/perfctrs/collecting-performance-data)
- [Process V2 counter guidance](https://learn.microsoft.com/en-us/windows/win32/perfctrs/collecting-performance-data)
- [Tool Help snapshots](https://learn.microsoft.com/en-us/windows/win32/toolhelp/snapshots-of-the-system)
- [QueryFullProcessImageNameW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-queryfullprocessimagenamew)
- [GetProcessMemoryInfo](https://learn.microsoft.com/en-us/windows/win32/api/psapi/nf-psapi-getprocessmemoryinfo)

## Product order

1. GPU total and per-process GPU columns
2. process trees with parent and child aggregation
3. priority, affinity, suspend, and resume controls
4. services and startup applications
5. per-process network rates
6. handles, threads, command lines, and signed publisher details

Every destructive process control needs an explicit confirmation or a clearly
reversible interaction. Telemetry failures should degrade one field, not the sampler.
