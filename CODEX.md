# CODEX.md: Trontop handoff

Trontop is an active hand-driven Codex project. The overnight coordination loop
must not modify this repository.

## Current checkpoint

Working branch: `feat/provider-diagnostics`, alpha.7 source. Read the newest
`docs/CURRENT_STATE.md` entry for verification and review-EXE identity.

Implemented: Overview and Hardware sensors navigation, About/provider diagnostics
and explicit privacy-safe report copy, stable missing/cached GPU sensor fields,
gap-aware GPU activity charts, bounded worker-shutdown waiting, and original vector
navigation/window/action/tree icons. Four user mockup boards and a generated
transparent Signal icon candidate are saved in `docs/inspiration`; the candidate
is not installed as the application icon. Alpha.7 integrates read-only Windows drive
temperatures using isolated, bounded workers with timeout/backoff/cache states.
The TEAM SSD returned three real readings in the opt-in native test. CPU/motherboard
providers remain unconnected. No full Task Manager parity.

Local tests: 55 passed, 0 failed, 4 opt-in tests ignored. Strict Clippy passes.
Offscreen QA produced 28 PNGs without native windows/input. Native end-to-end close
latency and dragging performance are NOT measured. CPU provider research and the
slow SSD/HDD probe findings are in `docs/SENSORS_PLAN.md`; no driver install authority.

Running instance is still the separately deployed alpha.5 preview. On
2026-09-05 Trent explicitly requested replacing/reopening his old running build.
The old alpha.1 process exited before replacement; no process was terminated.
The freshly optimized alpha.5 preview now runs as PID 255824 from
`target/release/trontop.exe`. Its native window was observed responsive with title
Trontop. The previous EXE is backed up at
`target/replaced-builds/alpha1-20260905-0124/trontop.exe`. No other windows were touched.
This is a development preview, not a published alpha release. Do not automatically
restart it again for subsequent edits. Read the newest `docs/CURRENT_STATE.md` section.

Next: preserve/display individual startup-source results, finish per-process GPU
missing-value semantics, investigate remaining CPU/motherboard sensor coverage, then continue
slim components and real executable icons with a bounded background cache. Complete
the release gates in `RELEASE_PLAN.md`; alpha.4 CI is not verification of alpha.7.

## Previous verified checkpoint

Latest local version: 0.3.0-alpha.4. Read the latest section of `docs/CURRENT_STATE.md`
and `docs/HEADLESS_QA.md`. The hover/rounded-controls slice and safe headless harness
are now implemented. A separate optimized review EXE exists under
`target/review-build/release/trontop.exe`; do not confuse it with the still-running
older `target/release` copy. Seventeen test-fixture PNGs were generated without opening
any app window. No native drag fix or published release is verified.

Trent reports a clear subjective drag improvement; there is no measured 60 FPS claim.
Alpha.3 integrates real NVIDIA NVML temperature/power/clocks/fan/VRAM on the sampler,
with a dedicated GPU Sensors page and bounded, gap-aware histories. Native read-only
probe and headless checks passed. Details and remaining CPU/storage/vendor work are
in `docs/SENSORS_PLAN.md`. No app windows were launched or manipulated for this slice.

Alpha.4 adds exact native process-creation identity checks on the same handle used
for End Task/priority/affinity, Windows-critical-process refusals, and stale-confirmation
protection. Read `docs/PROCESS_ACTION_SAFETY.md`. Local gate: 35 tests passed, strict
Clippy and optimized build pass. The shared action-button helper also fixes the
confirmation baseline offset, verified by geometry tests and offscreen images.
The larger alpha release gates are still incomplete. Code checkpoint `f2f725c` is
pushed privately and Windows CI run `33946298909` passed all its gates. Read
`docs/CI_ALPHA4.md` for the CI artifact/hash and `docs/DIAGNOSTICS_PLAN.md` for the
next bounded slice. Storage temperature probing succeeded on the TEAM SSD but is
not integrated; HDD queries were slow, so do not put them on the system sampler.

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
