# Alpha.33: cores, dense Overview and ColorMagic

Bounded slice: A06, A13, A22, A32. Requested September 6 after the alpha.32
review. This is not a claim of Task Manager parity or final release acceptance.

## Visible changes

- Performance / CPU defaults to **All cores**, with a **Total CPU** switch.
  Each tile uses its own logical-processor busy-time history, stable indices,
  a fixed 0-100% scale and missing-sample gaps. Up to 256 processors fit within
  the shared 512-series budget; higher counts get an explicit limit message.
- Overview becomes a dense graph wall: machine signals, all logical processors,
  then memory, GPU engines, storage and sensors. Adds process-count and available
  RAM histories. Lines/Bars and retained-value semantics are shared with Graphs;
  no second polling loop or history store was added.
- Theme Studio has **Randomize**, six ColorMagic families plus **Surprise me**,
  and a 12-roll undo history. Four related pegs, matching accents and subtle neutral
  tints replace unrelated RGB rolls. Mode, peg positions, gradient intensity and
  layout stay intact. Nothing randomizes on launch.
- The misleading CPU **Speed** field is now **Power clock**, with an explicit
  Windows CurrentMhz / CPU 0 explanation. This does **not** implement live boost
  frequency. See [hardware findings](HARDWARE_RESEARCH.md).

The redesign skill informed targeted density, alignment and readability changes
within egui. Trent's multi-color gradients and dense dashboard brief override its
generic single-accent website advice. No new runtime dependencies or bitmap assets.

## Verification

Final ordinary suite: **263 passed, 0 failed, 21 ignored** in 55.66 s.
Strict Clippy passed in 4.04 s. Formatting and diff checks passed.

New checks cover distinct logical histories, missing values, the many-core budget,
production All cores/Total CPU and Randomize/Undo interactions, palette undo and
contrast, and clipped-versus-full layout at 256 cores and fractional scale.
512 seeds across seven choices and both modes verify normalized, reproducible,
diverse palettes and at least 4.5:1 tested text/surface contrast. They do not
replace Trent's aesthetic approval or the whole-app native acceptance gate.

Six final offscreen PNGs generated in 6.53 s and inspected under `target/ui-smoke/`:
`alpha33-cores-24.png`, `alpha33-cores-compact.png`, `alpha33-overview-dense.png`,
`alpha33-overview-light.png`, `alpha33-magic-dark.png`, `alpha33-magic-light.png`.
These are synthetic fixtures, not screenshots of a newly launched preview.
The read-only native CPU probe separately confirmed 24 distinct logical readings;
details and repeat command are in HARDWARE_RESEARCH.md.

### Optimized CPU-only UI probe

One optimized test-binary run, 20 warm-up and 100 measured frames per case.
Times include egui layout and tessellation, not GPU execution, sampling, native
movement, close latency or FPS. The cases are not an A/B comparison; cache and
other machine activity can affect their ordering.

| Page | Logical CPUs | Size | Median | p95 | Max |
| --- | ---: | --- | ---: | ---: | ---: |
| Performance | 24 | 1040x640 | 1.042 ms | 1.240 ms | 1.707 ms |
| Performance | 24 | 1920x1080 | 1.602 ms | 2.188 ms | 2.408 ms |
| Overview | 24 | 1040x640 | 0.327 ms | 0.402 ms | 0.494 ms |
| Overview | 24 | 1920x1080 | 1.257 ms | 1.959 ms | 2.273 ms |
| Performance | 256 | 1040x640 | 0.467 ms | 0.779 ms | 0.808 ms |
| Performance | 256 | 1920x1080 | 2.870 ms | 3.441 ms | 3.692 ms |
| Overview | 256 | 1040x640 | 0.324 ms | 0.407 ms | 0.479 ms |
| Overview | 256 | 1920x1080 | 1.188 ms | 1.781 ms | 1.950 ms |

```powershell
cargo test --offline
cargo test --offline render_cores_magic_visual_pass -- --ignored --nocapture
cargo test --release --offline cores_overview_cpu_timing_probe -- --ignored --nocapture
```

Only opt into named isolated probes, not all ignored tests. Production executable
identity belongs in [CURRENT_STATE.md](CURRENT_STATE.md).

## Stop line and next product discussion

This slice ends at verified build and review handoff. CPU temperatures, live boost
semantics, native drag/close/tray/soak and the parity sheet remain open. No desktop
input, focus/window changes, installed drivers or uploads. Existing previews and
personal settings are not replaced by this pass.

Two high-value **proposals**, not new required asks:

1. **Shared history cursor:** point at one instant and read that instant across
   CPU, watts, temperature, memory and I/O. Correlate a hitch instead of only
   watching lines move. Test timestamps/gaps before adding longer history or
   automatic diagnosis.
2. **Developer project grouping:** distinguish Trent's Unity editors, builds and
   compiler children using ancestry and known command-line paths, then show totals
   per project. Keep uncertain matches explicit; never guess ownership or turn a
   group into an unconfirmed kill target.

Prefer a reviewed vertical slice after the core reliability/measurement gaps to
more isolated border/icon rearrangement. New concepts need Trent's promotion
before implementation; they do not silently expand the required ledger.
