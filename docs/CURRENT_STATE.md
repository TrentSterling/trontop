# Trontop current state

Last updated: 2026-09-04

## Latest: alpha.2 hover and safe UI verification

Work resumed on the UI after the parked checkpoint below. Version 0.3.0-alpha.2
adds shared hover backgrounds for navigation, device tiles, metrics, detail rows,
stat cards, badges, graphs, table cells, passive labels and custom action buttons.
Selected items retain a distinct state. Theme Studio uses aligned, rounded zebra
control rows and scrolls on short windows; slider tracks now contrast with cards.
Performance rail/content scroll independently. The sidebar footer has reserved
space and cannot overlap its GPU meter at the tested minimum size.

`docs/HEADLESS_QA.md` documents the new harness, local-pointer navigation/selection
and scrolling tests, 336 page/size/preset/mode/data cases, and eleven actual egui-WGPU
offscreen PNGs. No native app window or desktop input is used. This supersedes the
older statement below that only two cell tests exist. The wider branding/icon and
app-wide zebra/detail polish are still incomplete; do not call the whole UI done.

Review EXE: `target/review-build/release/trontop.exe`, 12,756,992 bytes,
SHA-256 `42B4AB4D8D16DAF19D8CC71FA47DC39674961031B4842992FFD60CA5A2DD1FF8`.
The optimized build succeeds and PE metadata reports Tront / Trontop /
0.3.0-alpha.2. It has NOT been launched on Trent's desktop. The running PID 62220
was confirmed to be the older release-path EXE, not debug; it was left untouched.
No new native dragging measurement was performed. Trent's later feedback is that
dragging now feels noticeably smoother; preserve that observation without claiming
a verified 60 FPS fix. No cross-project patch or GitHub release is published.

Final alpha.2 verification: formatting PASS, 23 unit/headless tests passed with
2 opt-in tests ignored by default, strict Clippy PASS, optimized release build PASS.
The offscreen GPU test was explicitly selected and passed separately (11 PNGs).
The native tray test was not run in this continuation. The UI images use fixtures,
not live system samples; native drag performance and real sensor integration are
outside this gate.

Sensor expansion is planned in `SENSORS_PLAN.md`. A read-only 5070 Ti query confirmed
temperature/power/clocks/fan/VRAM availability on this machine. Those fields are not
yet integrated into Trontop; CPU temperature needs separate provider research.

## Earlier parked checkpoint (historical)

Read `DRAG_INVESTIGATION.md` for unresolved window-movement lag and the explicit
ban on global desktop input automation. No dragging fix was validated or ported.

The newest local checkpoint adds a dedicated native tray thread fed directly by the
sampler, a full-area CPU level with scrolling history, alternating gradient column
bands, padded/vertically aligned table cells, corrected footer space, a rebuilt Users
resource table, wider inventory tables, and scrollable inspector content. Ordinary
selected labels now use the high-contrast text token. A final source follow-up fixes
centered table labels caused by add_sized forcing a centered widget layout. Two
headless tests check actual text geometry and full-cell input. This follow-up is NOT
release-built or visually checked in the running app; the visual pass is not finished.

Final source verification: formatting PASS; unit tests 18 passed, 0 failed,
1 interactive tray test ignored; strict Clippy PASS. No desktop interaction was used
for the alignment tests, and the running executable was not restarted.

Pre-alignment checkpoint verification: formatting PASS; unit tests 16 passed, 0 failed,
1 interactive tray test ignored; strict Clippy PASS; release build PASS. Earlier in
this session the ignored native tray test was explicitly run and passed without any
application UI frames. Four real tray captures also differed while the window was
hidden. A repeated hidden/hover sample measured about 0.228% whole-machine CPU,
213.1 MiB working set, 1,163 handles and 46 threads on this machine. These are limited
smoke observations, not a sustained soak or evidence that dragging is fixed.

The private GitHub baseline CI succeeded at run 33940457527. The newer local checkpoint
still needs its own CI run. No alpha tag or release is published. No full headless UI
harness or new branding/icon system is implemented yet. See `TASK_BOARD.md`.

## Product identity

Trontop is Trent Sterling's native Windows system control deck. It exists because
Windows 11 Task Manager traded too much density and immediacy for generic shell UI.
The target is a portable tool that feels at home beside TrontSnap, SpaceView,
TrontEQ, Photochop, and the rest of the TrontStack native apps.

The visual direction is dark, technical, and readable. Purple and teal are the
default TrontStack signal colors. Demigod orange, Monke Portal cyan/violet, and the
original copper theme are included as presets. The UI should carry Trent's actual
identity: high-performance Unity and VR work, multiplayer systems, procedural tools,
and practical utilities built for demanding machines.

## Version 0.3 alpha checkpoint

Implemented:

- custom frameless window with branded drag region and window controls
- Processes, Performance, History, Startup, Users, Details, and Services pages
- process search by name, user, PID, path, and command line
- real process-tree ordering with expand/collapse, automatic ancestor context during
  search, descendant counts, and clearly labeled subtree CPU/GPU/memory/I/O totals
- live process CPU, GPU, memory, disk rates, cumulative I/O, state, user, parent,
  executable, command line, working directory, start time, and accumulated CPU time
- exact Windows GPU Engine PDH enumeration with machine, engine, and PID aggregation
- CPU, memory, disk, network, and GPU performance device views
- Windows Service Control Manager inventory
- HKCU/HKLM Run key and Startup folder inventory
- guarded End task flow and Run task launcher
- current priority class and CPU affinity sampled outside the UI thread every five
  seconds; guarded priority and affinity editors validate PID start identity and require
  explicit confirmation before applying a change
- live Theme Studio with dark/light modes, primary and secondary colors, gradients,
  frost, corner control, presets, and persistence
- theme-derived zebra rows across dense grids, stronger two-signal hover/selection
  states, and a branded process-inspector empty state
- globally non-selectable display labels; explicit paths and command lines remain
  selectable for copying
- live tray icon whose meter and color track CPU load, plus a CPU/memory/GPU/process
  tooltip and Show/Quit actions
- generated multi-resolution Trontop PE icon and embedded Windows company, product,
  description, filename, and prerelease version metadata
- one-second background snapshots; no OS query runs on the egui render thread
- checked-in Windows GitHub Actions verification and portable-executable artifact upload

## Verification baseline

The expected gate is:

```powershell
cargo fmt --all --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```

The v0.3 alpha visual validation used a real Windows 11 machine with an RTX 5070 Ti. PDH
reported real machine and per-process GPU utilization. Visual captures live under
`C:/trontstack/tmp/` during the active session and are not repository assets.

The process-control integration test launches a hidden disposable child, reads its
native priority and affinity, changes both, verifies the new values, restores them, and
terminates the child. No production workload is modified by that test.

The final release runtime sample was responsive at 0.3125% whole-machine CPU over
10 seconds and 210.6 MiB working set while sampling roughly 450 processes. The main
window opened centered at 1280 by 760 and the native tray host was present. Treat
these numbers as a comparison baseline, not a machine-independent budget.

The earlier 0.3.0-alpha.1 process-tree baseline was responsive at 0.1947% whole-machine
CPU over 10 seconds, 211.7 MiB working set, 1,181 handles, and 47 threads while
sampling roughly 445 processes. Its portable executable is 12,623,872 bytes with
SHA-256 `04A2C7C478AD506F33E7A5ACA524D12E2DF5DCFF32B63EB1652818CA053402CD`.

## Architecture map

- `src/sampler.rs`: one long-lived background telemetry worker and immutable snapshots
- `src/windows_metrics.rs`: GPU PDH, Service Control Manager, and startup inventory
- `src/model.rs`: snapshot and table models
- `src/app.rs`: navigation, pages, actions, persistence, and window shell
- `src/widgets.rs`: Tront visual primitives, charts, meters, tables, and marks
- `src/theme.rs`: derived tokens, gradient painter, presets, and serialization
- `src/tray.rs`: native live tray icon and tray actions
- `src/platform.rs`: guarded process and shell actions

Read `docs/TELEMETRY.md` before changing providers. Read `TASK_BOARD.md` and
`RELEASE_PLAN.md` before choosing the next slice.
