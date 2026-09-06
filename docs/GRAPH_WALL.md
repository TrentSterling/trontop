# Graph wall

## Alpha.33: All cores and dense Overview (A06/A32)

Per-logical-processor history, the CPU grid, graph-heavy Overview, process count
and available RAM share this bounded history store. Clipped rows reserve space
without laying out their contents. Scope, final tests, six visual fixtures and
optimized CPU-only timing: [ALPHA33_REVIEW.md](ALPHA33_REVIEW.md).

## Alpha.31: bounded offscreen layout (A22/A32)

The graph wall previously built every card's labels, frames and child UI on every
paint, even though plot geometry was clipped. Alpha.31 measures one real row per
frame and reserves space for offscreen rows without building their contents.
Histories, available metrics, chart precision and sampling frequency are unchanged.
The first row is intentionally still laid out when scrolled away; its measured
height accounts for current fonts, scale and theme margins without a guessed cache.

Each row uses a shared width, preventing cumulative fractional-scale rounding
from shifting later columns. Skipped rows consume the same single parent auto ID
as a real row scope, preserving child hover identity. Category changes return to
the first row; switching Lines/Bars preserves the current scroll position.

The existing-app redesign skill guided geometry/hover checks while preserving
Tront's requested gradient, rounded zebra grid and dense graph layout. No visual
rebrand, additional telemetry query, worker, runtime asset or dependency was added.
Pinned egui source confirms the allocation/ID rules used here; see its
[ScrollArea documentation](https://docs.rs/egui/0.35.0/egui/containers/scroll_area/struct.ScrollArea.html)
and `egui-0.35.0/src/ui.rs` (`scope_dyn`, `allocate_space`, `columns_dyn`).

A new ordinary regression compares the optimized wall to a test-only full-layout
reference at the 512-chart limit. Visible text and bounds match within 0.1 logical
point through top/middle/bottom/return scrolling, dark/light, and 1/1.5/2 UI scales.
At most 24 cards are laid out in those cases. A separate local-input test proves
deep scrolling reaches the network tail, then Memory and Everything restart at
their first graphs. Initial tests exposed the fractional-width drift; fixing row
width restored strict geometry comparison. Fixture freshness was frozen to prevent
elapsed test execution from changing one side's status labels.

Full ordinary suite: **251 passed, 0 failed, 17 ignored**, 55.74 s. Strict Clippy
passed (1.98 s). Native dragging, presentation, close, GPU submission, live-provider
acceptance and the 60-minute soak are not covered by these CPU/layout checks.

Optimized same-binary comparison: 20 warm-up and 120 measured hover-changing frames
per case, including the production chrome/layout and CPU tessellation. Reference
uses the same row-width/ID corrections but lays out every card. These are synthetic
120-second histories, not telemetry collected from the working desktop.

| Charts | Logical window | Reference p95 (ms) | Visible rows p95 (ms) | Cards laid out |
| --- | --- | --- | --- | --- |
| 25 | 1040x640 | 0.6550 | 0.4433 | 4 |
| 25 | 1920x1080 | 0.9745 | 0.9226 | 20 |
| 153 | 1040x640 | 0.9785 | 0.4251 | 4 |
| 153 | 1920x1080 | 1.4937 | 0.9248 | 20 |
| 512 | 1040x640 | 2.4778 | 0.2812 | 4 |
| 512 | 1920x1080 | 3.1242 | 0.8755 | 20 |

At 512 charts, median CPU time changed from 2.4251 to 0.2157 ms compact and
2.6935 to 0.5680 ms wide. The earlier unmodified-row baseline was 2.7232/3.0529 ms
p95 respectively. This is one local run, not a promise every frame gets faster:
the 25-chart wide case's maximum was 1.9275 ms optimized versus 1.1619 ms reference.
There is no claim this identifies the user's native drag/close delay.

The offscreen visual pass exposed missing scale glyphs after a category click.
The screenshot helper discarded intermediate click frames and their font-texture
updates. Both graph and memory visual passes now retain those deltas. A third new
ordinary regression reconstructs the font atlas from captured updates and matches
it pixel-for-pixel to egui's atlas; discarding the click frames fails that comparison.
The regenerated thermal PNG has the full numeric scale and W unit. This is a
test-harness repair, not a production renderer change.

Graph and memory offscreen tests generate twelve PNGs with the actual egui-WGPU
renderer on RTX 5070 Ti/Vulkan. All six graph views plus both filtered memory
graphs were inspected: dark, compact, light, Bars, thermal, empty and cached.
Text alignment, rounded zebra surfaces, chart fills and dense spacing are retained.
No native window, input, tray or personal settings were touched. Exact executable
identity is recorded in `CURRENT_STATE.md`; no source upload or release occurred.

```powershell
cargo test --offline graph_wall_ -- --nocapture --test-threads=1
cargo test --offline --release graph_wall_cpu_timing_probe -- --ignored --nocapture --test-threads=1
cargo test --offline render_graph_wall_visual_pass -- --ignored --nocapture --test-threads=1
```

## Earlier graph data additions

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
