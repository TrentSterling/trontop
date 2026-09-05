# CODEX.md: Trontop handoff

Trontop is an active hand-driven Codex project. The overnight coordination loop
must not modify this repository.

## Current checkpoint

Latest local version: 0.3.0-alpha.3. Read the latest section of `docs/CURRENT_STATE.md`
and `docs/HEADLESS_QA.md`. The hover/rounded-controls slice and safe headless harness
are now implemented. A separate optimized review EXE exists under
`target/review-build/release/trontop.exe`; do not confuse it with the still-running
older `target/release` copy. Fifteen test-fixture PNGs were generated without opening
any app window. No native drag fix or published release is verified.

Trent reports a clear subjective drag improvement; there is no measured 60 FPS claim.
Alpha.3 integrates real NVIDIA NVML temperature/power/clocks/fan/VRAM on the sampler,
with a dedicated GPU Sensors page and bounded, gap-aware histories. Native read-only
probe and headless checks passed. Details and remaining CPU/storage/vendor work are
in `docs/SENSORS_PLAN.md`. No app windows were launched or manipulated for this slice.

## Earlier checkpoint (historical)

Version 0.3.0-alpha.1 builds on the seven-page native system control deck with a real
process hierarchy, guarded scheduler controls, theme-derived zebra tables, and an
embedded multi-resolution Windows icon and version resource.
Read `docs/CURRENT_STATE.md` for the full handoff and `TASK_BOARD.md` before choosing
new work. The private repository is connected at `TrentSterling/trontop`; authentication
and the first Windows CI run succeeded. The private alpha release has NOT been published.

Trent parked this session to work on another project. Read `docs/DRAG_INVESTIGATION.md`
first when resuming. Window dragging is still visibly laggy compared with Terminal and
Explorer. There is no validated drag fix to port to Boxel or other egui projects.
Trent's latest global zebra/rounded-controls/branding request is saved in
`docs/UI_POLISH_BRIEF.md`; it is not limited to the process table.

The latest local slice adds a dedicated native tray worker with a full-width CPU meter
and scrolling history, column bands, consistent table cells, a rebuilt Users resource
table, and scrollable inspector content. Branding/icon polish and a proper headless UI
smoke harness remain unfinished. Do not mistake the passive drag recorder for that harness.

Two small headless widget tests now check left-aligned labels, right-aligned numbers,
vertical centers, and full-cell clicks. The final table-label alignment fix is tested
in source but not release-built or visually checked; the running EXE is older.

No global mouse/keyboard injection, focus stealing, or automatic minimize/restore/move
tests on Trent's working desktop. Earlier automation almost closed a day-job Claude
session. Use headless tests. The old temporary desktop-control helper is now guarded
against those actions. Other day-job windows are off-limits.

GPU Engine instances are enumerated through PDH and aggregated by PID. Do not
synthesize counter paths or display estimated GPU values. All slow inventory and
live telemetry remain outside the render thread.

## Verification

```powershell
cargo fmt --all --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```
