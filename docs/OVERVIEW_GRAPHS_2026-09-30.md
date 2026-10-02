# Alpha.47 Overview and graph truthfulness

Continued complete coverage advances A07/A16/A25/A32. The requested End process
tree, all-instance termination and Startup controls remain included.

Overview and the shared System live resolver now require an available bridge as
well as a fresh poll. Retained package temperature and power cannot appear live
after the provider becomes unavailable. Overview also applies the temperature
representation guard introduced in alpha.46. The shared CPU gap predicate uses
provider status too, so a retained reading cannot suppress the missing-data message.

Overview's native VRAM caption marks a retained memory value with `~` in both its
full and compact forms, and explains the marker on hover. A subsequent valid read
removes it. Software adapters cannot supply this hardware caption. The optional
NVML fallback requires a successful read within three seconds and an uncached
snapshot. Partial GPU activity and disk throughput retain their `+` and Partial
labels; disk throughput remains a lower bound when a counter is missing.

Graph tooltips resolve each series within half its sampling cadence of the
hovered sample. They reject future points, points outside the visible two-minute
window and remote values across missing intervals. An explicit missing point
remains `No exact measurement`, even when an adjacent value exists. Partial
points retain `+ (lower bound)` in the tooltip. Actual samples are formatted;
the tooltip does not interpolate or reuse the card's retained current value.

Eleven new ordinary regressions cover seven Overview flows and four plot flows:

- Bridge unavailable/overflow/expiry/recovery through Overview and System,
  including the shared CPU gap message.
- Sensor details, All processes, All cores and degraded-source Details links;
  grouped-process selection opens the heaviest instance without executing actions.
- Partial GPU/disk totals, live/retained/recovered VRAM, software-adapter refusal,
  fresh/cached/expired NVML fallback and multi-GPU identity/cache/provider gaps.
- Core hover readings, missing/nonfinite values and measured zero.
- Lines/Bars tooltip timing, missing samples, slower cadence and lower bounds.
  Line/mesh geometry stops at explicit and long gaps; partial points remain hollow.

The bridge retention, overflow, tooltip timing/partial and cached VRAM assertions
failed before their fixes. The first offscreen review exposed the suppressed CPU
gap; the extended ordinary assertion reproduced it before its fix. A multi-GPU
fixture initially advanced the provider timestamp without advancing the history's
clock. The fixture now drives the production history at that same virtual instant;
production continues to refuse future provider timestamps.

```powershell
cargo test --offline app::ui_smoke::overview -- --nocapture
cargo test --offline app::graphs::interaction_tests -- --nocapture
cargo test --offline specs::live -- --nocapture
cargo test --offline app::ui_smoke::overview::render_overview_truthfulness_review -- --ignored --exact --nocapture
cargo fmt --check
cargo test --offline -- --quiet --test-threads=4
cargo clippy --offline --all-targets -- -D warnings
cargo build --offline --release
./scripts/measure-coverage.ps1 -ReadOnlyProbes -TestThreads 4
```

The selected offscreen check passes. Three final 1200x900 PNGs were inspected in
`target/ui-smoke/overview-state-alpha47/`: fresh dark, retained dark and retained
light. The retained views show the VRAM marker and explicit CPU gap, with the top
process lists and core strip inside the viewport. The worker-free fixture labels
its CPU gap `Not checked yet`; this is synthetic test data, not live-provider
acceptance or marketing media. Full final evidence is in `CURRENT_STATE.md` and
`COVERAGE.md`.

Final ordinary suite: **519 passed, 0 failed, 56 ignored**. Formatting, strict
all-target Clippy and the optimized release build pass. Review EXE:
`target/review/alpha47-overview-graphs/trontop.exe`, version `0.3.0-alpha.47`,
**17,791,488 bytes**, SHA-256
`F954B803347829E860520C76F05FB40BBCE7D1F314286C206EBEA9D89EC33C89`.
It matches the final optimized build. The candidate was not launched or installed.

No native queries were added to the render thread. D01, field parity, isolated
native interaction, clean-machine and exact-build release acceptance remain open
in `ASK_LEDGER.md`. The active completion goal is not satisfied by line coverage.
