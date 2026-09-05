# Trontop telemetry architecture

## Current sampler

The 0.1 sampler owns one long-lived `sysinfo::System` on a dedicated thread. It
refreshes CPU, memory, and processes every second, publishes one immutable snapshot,
and requests one UI repaint. The render thread performs no operating-system queries.

`sysinfo` reports process CPU as a percentage of one logical processor, so Trontop
divides it by the logical processor count. This matches the whole-machine percentage
users expect from Task Manager. Disk byte deltas are divided by the measured interval,
not an assumed perfect one-second interval.

## Native Windows expansion

The next provider should supplement the portable sampler with these Windows sources:

1. Enumerate the `GPU Engine` PDH object and its exact instances. Extract PIDs from
   instance names and sum engine utilization by PID. Never guess instance names.
2. Prefer the Windows 11 `Process V2` counterset for advanced per-process counters.
   Its instance identity avoids the name churn that affects the older `Process`
   object when processes exit between samples.
3. Keep PDH queries long-lived and collect at least two samples before publishing a
   rate. Counter rates are deltas and do not have meaning from one observation.
4. Use `QueryFullProcessImageNameW` as a fallback when the base sampler cannot obtain
   an executable path. Access failures are normal and should remain non-fatal.
5. Add ETW only where cumulative counters cannot provide a useful rate, most likely
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

