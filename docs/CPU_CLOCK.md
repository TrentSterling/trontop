# CPU clocks: alpha.34

Scope: A06 frequency discrepancy, A07 retained data, A22 bounded work and A32
graphs. This is not Task Manager parity, a CPU temperature provider, or native
drag/close verification.

## What changed

Performance / CPU now shows an interval-average clock, fastest reporting processor
and slowest reporting processor. The source expander lists group-local processor
readings and their nominal clocks. That list uses a bounded scrolling viewport.
Overview and Graphs share two new timestamped histories (average and fastest MHz).
Wide Overview layouts fit five chart columns so these signals do not unnecessarily
push the core grid down. Compact layouts keep their existing column count.

The old `CurrentMhz` / CPU 0 power-clock reading remains in the source expander and
the legacy JSON key, with explicit provenance. It is never used as the missing-data
fallback for a dynamic clock. JSON additionally includes interval duration, group
and processor identity, nominal clocks, nullable individual values and provider age.

## Source and formula

Primary source audit pinned to System Informer / phnt revision
`d291af7dbb44c93a3a47e435956a4f8fa64f3fa6`:

- [CPU implementation](https://github.com/winsiderss/systeminformer/blob/d291af7dbb44c93a3a47e435956a4f8fa64f3fa6/SystemInformer/syssccpu.c)
- [Native counter declarations](https://github.com/winsiderss/systeminformer/blob/d291af7dbb44c93a3a47e435956a4f8fa64f3fa6/phnt/include/ntexapi.h)
- [Power declarations](https://github.com/winsiderss/systeminformer/blob/d291af7dbb44c93a3a47e435956a4f8fa64f3fa6/phnt/include/ntpoapi.h)

The Rust collector uses query-only `NtQuerySystemInformationEx`, class 100, for
each active processor group. `NtPowerInformation` level 87 / internal selector 43,
version 1 supplies each processor's branded nominal frequency. No new driver,
service, elevation prompt, affinity change or power-policy write is performed.
These are native/private interfaces, not a promise of support on every Windows
version or CPU. Unsupported calls and unknown layouts fail closed.

For each processor and state bucket, calculate the difference in cumulative hits
between two valid samples. Multiply each delta by that bucket's percent frequency
and **that processor's nominal MHz**, then divide the sum by total hits and 100.
The overall number uses the same hit-weighted aggregation over processors. Fastest
and slowest are extrema of the measured processor interval averages, not a peak
instant captured inside the interval. Zero new hits means no individual reading,
not proof of zero MHz. A bucket that actually reports zero percent is preserved.

System Informer's inspected implementation instead scales all processor buckets
by one `CpuMaxMhz` reference. Trontop deliberately uses per-processor nominal
references because this hybrid machine returns both 3700 and 3200 MHz. Therefore
the aggregate is not represented as System Informer's or Task Manager's exact
number. There is no screenshot-targeted multiplier, UI smoothing or invented boost.
This is an OS performance-state interval, not a direct APERF/MPERF hardware probe
or an independently verified wall-time-effective clock.

## Bounds, identity and failure behavior

- Windows 8.1+ hit-count layout: 8-byte-aligned entries and 16-byte buckets.
  Validate count, offset table, every region and non-overlap before reading fields.
- Limit 64 groups, 64 processors/group, 256 buckets/processor, four allocation
  attempts and 1 MiB returned buffer/group. Use `u128` for bounded weighted sums.
- The local Windows 11 result repeated an embedded `ProcessorNumber` (often 13)
  across multiple distinct counter entries. Index by requested group and offset
  table order, as the reference consumer does, not that unreliable embedded field.
  This must not be confused with PDH's NUMA-node/index instance naming.
- Nominal, topology, bucket order/percent changes or decreasing counters reset the
  baseline. A failure also clears it. Recovery requires two fresh valid samples.
- Accepted intervals span 0.1 to 5 seconds. A long provider pause/sleep resets the
  baseline rather than drawing a fresh-looking point averaged over the whole gap.
- Failed native queries back off for 30 seconds. Keep prior values, last usable
  timestamp and explicit stale status; never append those values as fresh history.
- Queries run on the existing sampler, not egui. There is no new timer/thread.
  A synchronous OS call cannot be cancelled here; the short probe below is not
  a worst-case latency guarantee. Existing non-waiting UI snapshot/close paths stay.

The MIT reference notice is preserved in `SYSTEM_INFORMER_NOTICE.txt` and embedded
in About so the single portable EXE carries it. No dependency or external asset
directory was added.

## Local evidence, September 6

The production collector's four-observation read-only probe passed. First sample
was correctly unprimed. Three later samples had 24/24 contributing processors:

| Sample | Average MHz | Fastest MHz | Slowest MHz | Query ms |
| --- | ---: | ---: | ---: | ---: |
| 0 (baseline) | unavailable | unavailable | unavailable | 0.6100 |
| 1 | 5165.58 | 5377.22 | 3637.98 | 0.1890 |
| 2 | 5147.64 | 5372.79 | 3616.37 | 0.1421 |
| 3 | 5123.62 | 5374.27 | 3878.84 | 0.1427 |

CPU 0 nominal was 3700 MHz; processor 8 nominal was 3200 MHz. Their dynamic values
were distinct (sample 1: 5346.83 and 3680.08 MHz). A separate earlier PDH read had
valid status 0 and CPU 0 performance of 144.881 to 145.031 percent while its
frequency counter stayed at 3700 MHz. This corroborates why the plain frequency
field is insufficient. These were not synchronized windows or a same-instant
cross-monitor accuracy benchmark. No Task Manager window was manipulated.

The timing is a short debug-provider probe on this machine, not release UI FPS,
whole-sampler overhead or a soak result. Ordinary tests cover heterogeneous/group
weighting, no-hit values, integer bounds, malformed/overlapping buffers, repeated
embedded processor numbers, resets, recovery, graph gaps, JSON and UI states.
Synthetic offscreen views cover Performance live/cached/missing and dense Overview.
Final build identity and complete verification results belong in CURRENT_STATE.md.

```powershell
cargo test --offline cpu_clock
cargo test --offline native_cpu_distribution_read_only_probe -- --ignored --nocapture
cargo test --offline render_cpu_clock_visual_pass -- --ignored --nocapture
```

No preview was replaced, no native window/input was used, and nothing was uploaded.
CPU temperatures, the wider A06 field comparison, cross-hardware validation and
the release-wide native gates remain open.
