# Windows memory counters: alpha.30

Scope: A06/A07 data correctness, A32 graph coverage and A15/A16 layout.

## Corrected semantics

The previous COMMITTED card added used physical RAM to sysinfo's used-swap value.
Pinned sysinfo 0.38.4 on Windows calculates that swap value as
`max(CommitTotal - PhysicalTotal, 0) * PageSize`. Its total swap is similarly
derived from CommitLimit minus PhysicalTotal. Neither measures page-file occupancy,
and adding used RAM cannot recover CommitTotal. For example, a system can commit
24 GiB while physically using 32 GiB. The replacement regression uses that case.

The production sampler now calls Windows `K32GetPerformanceInfo` directly once per
cycle. Sysinfo refreshes RAM only, avoiding a duplicate per-cycle query. This stays
on the existing sampler thread, never a render callback. No driver, service, new
worker, asset or runtime DLL is installed. The API uses the existing Windows
Kernel32 component. A slow native call can delay this sampler; it is not forcibly
cancelled or claimed hard-real-time.

| Display | Windows source | Meaning |
| --- | --- | --- |
| Committed / Commit charge | CommitTotal | Committed virtual memory |
| Commit limit | CommitLimit | Current commit limit, which can change |
| Commit peak | CommitPeak | Highest commit charge since boot, not this chart window |
| Commit pressure | CommitTotal / CommitLimit | Percentage of the current limit |
| System cache | SystemCache | Standby pages plus system working set |
| Paged pool | KernelPaged | Kernel paged-pool allocation |
| Nonpaged pool | KernelNonpaged | Kernel nonpaged-pool allocation |

Page counts are multiplied by the reported PageSize with checked conversion.
Invalid page size/overflow is rejected, not wrapped or clamped. A zero reading is
valid; a zero commit limit makes pressure unavailable. System cache is explicitly
named and defined, not claimed identical to every Task Manager "Cached" label.
Actual page-file occupancy, memory speed/slots/compression and hardware-reserved
memory remain outside this slice; A06 is not full parity.

Primary references checked September 5, 2026:
[PERFORMANCE_INFORMATION](https://learn.microsoft.com/en-us/windows/win32/api/psapi/ns-psapi-performance_information),
[GetPerformanceInfo](https://learn.microsoft.com/en-us/windows/win32/api/psapi/nf-psapi-getperformanceinfo).
Pinned source: sysinfo-0.38.4/src/windows/system.rs, refresh_memory_specifics.

## Retained state and UI

The sampler keeps one optional complete counter set and separate provider health.
On failure it retains the last successful values/timestamp; About/support metadata
reports Memory / Windows commit counters. Performance and Overview label cached
or unavailable counters. Fields never collapse, and missing is `--`, not zero.

Performance has eight aligned metric boxes, reflowing from four to two columns.
The existing-app redesign skill guided baseline/spacing checks, zebra surfaces,
readable wrapping and visible missing-state labels while keeping the Tront palette.
Graphs adds a Memory filter and five timestamped series: commit charge, commit
pressure, system cache and the two kernel pools. Repeated timestamps do not append;
failed/stale samples produce gaps, not repeated cached lines. Existing history
bounds and offscreen clipping remain unchanged.

JSON export adds `system.memory_counters` with bytes, source, state and age. Legacy
`swap_used_bytes` / `swap_total_bytes` keys retain their old derived semantics with
an explicit `swap_semantics` explanation; never-connected readings are now null.
Consumers should use the new counter object and freshness. CSV process export is
unchanged. No live system data was exported during this work.

## Verification

- Five new ordinary regressions: page decoding/overflow, failure/cache/recovery,
  graph timestamps/gaps/zero, exact JSON/null/provenance, compact dark/light field
  geometry and wrapping. A first exact-f32 equality test caught rounding; pressure
  now uses an f64 intermediate before conversion to the plotting type.
- Full suite: 248 passed, 0 failed, 16 opt-in tests ignored. Strict Clippy passes.
- Read-only native probe: 20 successful calls; median 18.9 microseconds, maximum
  176.6 microseconds in one debug run. Returned commit 84,982,202,368 bytes, current
  limit 136,277,565,440 bytes, peak 274,293,256,192 bytes. A historical peak can
  exceed the current limit. This is not native FPS, close timing or a soak test.
- Six memory PNGs plus six graph-wall PNGs generated with synthetic data through
  the real offscreen egui-WGPU renderer. The six memory views plus graph-wall dark
  and compact were inspected. Initial fixture-only blank performance histories
  were fixed; actual runtime collection was not replaced with synthetic data.
- No current preview/window, global input, actual preferences, driver installation,
  source upload or release publication. A21 native close remains unmeasured.

```powershell
cargo test --offline memory_ -- --nocapture --test-threads=1
cargo test --offline native_memory_counters_read_only_probe -- --ignored --nocapture --test-threads=1
cargo test --offline render_memory_counters_visual_pass -- --ignored --nocapture --test-threads=1
cargo test --offline render_graph_wall_visual_pass -- --ignored --nocapture --test-threads=1
```

Exact optimized build identity is recorded in `CURRENT_STATE.md` after building.
