# Snapshot-indexed process views (alpha.14)

The old Processes/Details paint path cloned the entire displayed process collection
on every frame, including account, command-line and path storage. Flat/Details also
constructed owned tree-shaped records for every process before drawing visible rows.
History cloned and sorted all filtered processes each frame to display twelve entries.
The refresh path separately owned copied flat and tree records.

Alpha.14 replaces those copies with snapshot-scoped indices. Tree metadata is `Copy`
and contains an index, depth/expansion state and aggregate counters, not a process
record. Processes and Details borrow only the records being painted. History caches
its twelve indices whenever the view rebuilds. The snapshot remains authoritative.
Name/account comparisons retain the previous ASCII-folded UTF-8 order using byte
iterators instead of allocating two lowercased strings on each comparison.

Indices are NOT identities across samples. `accept_sample` validates the selected
PID/creation time, swaps the snapshot and rebuilds the views before another UI frame.
Sorting, filtering and tree expansion also rebuild them. Process action identity
guards are unchanged. Private test fixtures replacing a snapshot must use this same
entry point, not bypass invalidation by assigning new records directly.

## Measurement

Run the specifically selected headless probe, never a blanket ignored-test run:

```powershell
cargo test --offline --release process_view_timing_probe -- --ignored --nocapture --test-threads=1
```

The probe uses the production egui UI with synthetic 500/5,000-process snapshots,
all roots, roughly 2 KiB command lines per process, 1280x760 logical size and no
sampler/tray/native window. Each page warms five frames, then measures 60 frames.
Refresh averages 20 view rebuilds. No live process records are copied into test files.
This measures CPU-side headless egui frame production, not GPU submission/present,
window movement, monitor FPS, total app overhead or sampler latency.

On 2026-09-05, after compilation finished, three sequential baseline/indexed pairs
were run on the same working machine. This was not an idle-machine lab benchmark.
The table gives the median of each variant's three per-run medians, in microseconds.
Refresh gives the median of three per-run means, not an individual-rebuild median.

| Processes | Work | Alpha.13 | Alpha.14 |
|---:|---|---:|---:|
| 500 | Tree frame | 307.6 | 211.2 |
| 500 | Flat frame | 307.8 | 207.1 |
| 500 | Details frame | 405.8 | 299.3 |
| 500 | History frame | 197.0 | 79.7 |
| 500 | Refresh mean | 571.3 | 232.8 |
| 5,000 | Tree frame | 3,888.1 | 206.9 |
| 5,000 | Flat frame | 4,106.8 | 205.7 |
| 5,000 | Details frame | 4,389.5 | 299.0 |
| 5,000 | History frame | 4,850.4 | 79.8 |
| 5,000 | Refresh mean | 14,010.5 | 2,555.0 |

Across the three 5,000-process runs, tree medians ranged 3,815.2-4,123.0 us before
and 206.2-208.9 us after. Tree p95 ranged 4,021.0-4,484.2 us before and
215.2-250.2 us after. The representative tree ratio is about 18.8x for this stress
fixture, not a claim that the whole application or real dragging is 18.8x faster.
The 500-process tree improvement is about 1.46x.

Baseline test binary: source 3142016 plus the same timing probe, retained only under
ignored `target/perf-baseline/alpha13/view-timing.exe`.
SHA-256 `EF3D4C23F1049C33ABFE08DB85134EC7701042AFB60927622E09F4118B38366A`.
Indexed test binary: `target/release/deps/trontop-8c73dd02fc3e56a3.exe`.
SHA-256 `26E0896231DB2A38EBC842B84E14CC5BBAB428FA23FC13682BCE81ECACDAEF16`.
These are test programs, not application preview builds. Direct PowerShell GUI-EXE
capture closed stdout early on the first repeat attempt; that failed attempt was
excluded. The successful pairs used owned `Process` handles with redirected stdout/
stderr and waited for each test's exit before starting the next, with no OS input.

## Correctness and visual evidence

Four new ordinary tests cover source-index sorting across all ten columns and both
directions, equivalent ASCII/Unicode ordering, view refresh after reordered/empty
snapshots, search and History invalidation, reused-PID selection expiry, and unchanged
index storage across headless repaints. Existing tree aggregation/ancestor search,
GPU missing/partial values, action safety, row selection and layout tests still pass.
Index storage checks do not count every allocation; the source change and timing
probe establish removal of the former whole-list paint copies.

Local gate: 125 passed, 0 failed, 8 opt-in ignored; strict Clippy and optimized
release pass. The selected offscreen pass produced 53 PNGs. Processes, Details,
History and the partial-GPU light tree were visually reviewed. No UI geometry change
was intended; this is not a fresh app-wide design/accessibility audit.

## Still open

Search currently folds fields on view rebuild, and hierarchy construction still uses
maps, allocations and recursive traversal. Deep/cyclic/adversarial hierarchies need
broader stress tests; this all-root timing fixture does not validate them. Inspector
cloning, actual sampling costs, visible-frame rendering, real drag/close latency,
memory/handle soaks and CPU/GPU present behavior are separate measurements. No native
drag fix is validated for Trontop or other egui projects by this work.
