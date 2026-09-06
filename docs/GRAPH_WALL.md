# Graph wall: alpha.27, memory correction in alpha.30

Alpha.30 replaces the incorrectly labelled page-file graph with real Windows
commit charge/pressure, system cache and kernel-pool charts, with a Memory filter.
See `MEMORY_COUNTERS.md` for source, cache/gap semantics and current verification.
The alpha.27 build/test records below remain historical.

Scope: A32, explicitly requested after Trent's alpha.26 screenshots on 2026-09-05.
One scrollable Graphs page, mostly time-series plots. Overview is unchanged.
Sidebar entry plus Ctrl+9; Lines/Bars toggle, Everything and category filters.
Bars are readings over time, not a statistical frequency histogram.

## Included data

- Whole-machine CPU, physical memory percentage and GPU activity.
- Windows commit charge/pressure, system cache and paged/nonpaged pools (alpha.30).
- NVIDIA GPU temperature and board power, graphics/memory clocks, fan target
  (not RPM), VRAM used. Device identity is the provider's UUID.
- Every reported drive temperature channel, using opaque device ID plus sensor
  index. Sensor 0 is not assumed to be a CPU core or a specific physical location.
- Physical-disk active time, response time, queue depth, read/write throughput.
  These are device counters, not duplicated sums across mounted volumes.
- Separate receive/send traffic for each reported network interface.

The original alpha.27 added no OS queries, drivers, services, dependencies, input
hooks or asset files. Alpha.30 reads the underlying Windows memory counters in
place of sysinfo's derived swap query; no new runtime component is installed.
The existing sampler feeds a bounded history on accepted snapshots, even while
another page is selected. Repaints do not collect telemetry or reformat processes.
Maximum 512 series and 128 points per series, aged to a 120-second time window.
The UI reports omitted fields if that series limit is reached.

## Data and visual behavior

- Sample timestamps determine the horizontal position, not UI frame rate.
  A retained 5-second drive sample is not repeated as five new observations.
- Missing/cached/partial samples are gaps, not zero or fabricated interpolation.
  Current partial GPU values have a >= prefix and a graph-gap status.
- Last usable readouts remain labeled Cached if a field goes missing. Removed
  devices retain their charts for the two-minute window, then expire.
- Missing GPU UUID means no historical stitching across unidentified adapters.
- Graph units/scales are visible. Real zero and negative temperature readings are
  allowed. Hover shows the nearest sample's value and age.
- Responsive 1-4 column grid, continuous across categories without section holes.
  Shared theme tokens, alternating card surfaces, hover frames, clipped long
  labels with full-text tooltips. Offscreen plots emit no chart geometry.
- CPU temperature has a compact explanatory status, not a simulated graph.
- Corrected the old Sensors footer which incorrectly said storage was unconnected.

## Verification

- `cargo test --offline --quiet`: **233 passed, 0 failed, 14 ignored**, 30.18 s.
  The ordinary page matrix now covers **480** page/size/theme/data cases.
- `cargo clippy --offline --all-targets -- -D warnings`: PASS, 5.13 s.
- Seven new history tests: deduplication/cache gaps, missing field retention,
  partial GPU semantics, UUID reorder/unknown identity, drive cadence/negative
  readings, history/series limits and empty snapshots/responsive columns.
- Local-input production-UI test exercises Bars, category filtering and return
  to Everything. The existing pending-action navigation test now scrolls the
  actual sidebar when the action footer reduces minimum-window space.
- `cargo test --offline render_graph_wall_visual_pass -- --ignored --nocapture`:
  PASS, 5.79 s, **six** PNGs under `target/ui-smoke/graphs-*.png` using the real
  egui-WGPU offscreen renderer (RTX 5070 Ti, Vulkan). Synthetic TEST DATA only.
  Reviewed all six: dark, light, Bars, thermal filter, compact and empty. A first visual pass found
  section whitespace and min-size navigation clipping; the final pass removes
  those holes and tightens navigation spacing without shrinking click targets.
- Native desktop: no windows/input touched. This is not native FPS, drag/close
  latency or mixed-load soak evidence. It does not supersede those release gates.

## Exact review executable

`C:/trontstack/trontop/target/review/alpha27-graphs/trontop.exe`

Optimized release, **0.3.0-alpha.27**, **13,639,168 bytes**, built in **59.62 s**
at **2026-09-06 00:11:45.802 UTC** (September 5 local), from modified afc6818.
SHA-256: `B6ACDA8C1F9B4D3B9B4AFA6C6D165E25C884CDE6F4C0F565930B1C243D16DA10`.
The retained copy was hash-verified. It was not launched; current user windows
were left untouched. Static-CRT configuration and embedded assets are unchanged;
this is not a new clean-machine portability certification. No push/release.

## CPU temperature: why it is still missing

Trontop has NVML GPU sensors and the Windows storage temperature query, but no
CPU package/core sensor backend. Prior read-only probes on this machine found no
LHM/OHM WMI namespace and unsupported ACPI thermal-zone readings. Those are prior
probe results, not fresh discovery performed during this UI slice.

Primary sources rechecked September 5:

- [Microsoft Win32_TemperatureProbe](https://learn.microsoft.com/en-us/windows/win32/cimwin32prov/win32-temperatureprobe)
  documents that CurrentReading is not populated by current WMI implementations.
- [LibreHardwareMonitor IntelMsr](https://github.com/LibreHardwareMonitor/LibreHardwareMonitor/blob/master/LibreHardwareMonitorLib/PawnIo/IntelMsr.cs)
  reads Intel MSRs through PawnIO. This is low-level hardware access, not a Rust
  limitation or evidence that the CPU lacks sensors.
- [RIGStats](https://github.com/dvalfrid/rigstats#architecture), another Rust/egui
  monitor, uses a .NET LHM sensor sidecar and bundled signed PawnIO driver.

A10/D01 remain open: agree on an optional compatible provider, or an approved
hardware-access integration. No driver/service installation or security changes
were performed. Do not relabel ACPI zones as CPU package temperatures.

## Slow close: bounded diagnosis, no unverified fix claim

Trent asked why closing lags and suggested concurrent Unity builds. Source review
of the currently shown alpha.26 close path finds:

1. `app/preferences.rs::request_close` captures settings and defers viewport Close
   until the background settings controller has no pending write. This does not
   block the event loop, but the visible window can remain open awaiting storage.
2. After 250 ms the pending-close dialog exposes Keep open/Retry/Close anyway.
3. Sampler shutdown has a zero wait budget; the preference controller does not
   join. Tray teardown permits a 100 ms wait budget before detaching.

Heavy disk activity or scheduler pressure could stretch the save. Source review
does not establish that this caused Trent's particular delay. No native close
timing was collected and no new close fix was implemented in alpha.27. A21 stays
open. If resumed, instrument close-request, save completion, viewport destruction
and process exit on an explicitly owned isolated preview; do not use global input
or disturb the user's other work.
