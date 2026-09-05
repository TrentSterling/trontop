# Trontop telemetry architecture

## Current sampler

The sampler owns one long-lived `sysinfo::System`, disk list, network list, user
list, and GPU PDH query on a dedicated thread. It refreshes live counters every
second, publishes one immutable snapshot, and requests one UI repaint. Startup and
service inventory refresh every 30 seconds. The render thread performs no
operating-system queries.

The sampler also publishes a compact CPU/memory/GPU/process-count sample directly to
the dedicated `trontop-tray` thread. A coalescing slot and native thread message wake
that thread. The tray icon, menu and native message loop are owned and destroyed there;
updates no longer depend on the main UI accepting a sample. Hovering the icon does not
request UI repaints. The 26-observation CPU history advances even if successive values
round to the same percentage. The file/taskbar icon remains the static Tront T mark.

The explicit interactive test `native_tray_updates_without_any_ui_frames` is ignored
by default; it creates a real tray icon. Do not run desktop-interactive checks on
Trent's working desktop without fresh agreement. The larger headless UI smoke harness
is still pending; see `TASK_BOARD.md` and `AGENTS.md`.

`sysinfo` reports process CPU as a percentage of one logical processor, so Trontop
divides it by the logical processor count. This matches the whole-machine percentage
users expect from Task Manager. Disk byte deltas are divided by the measured interval,
not an assumed perfect one-second interval.

## Native Windows providers

Current providers use these Windows sources:

1. The `GPU Engine` PDH object is enumerated for exact instances. PIDs and engine
   types are parsed from returned instance names and aggregated. Query counters are
   rebuilt periodically as engines appear and disappear.
2. Service inventory comes from `EnumServicesStatusExW` and includes state and PID.
3. Startup inventory reads the documented HKCU/HKLM Run keys and both Startup folders.

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
