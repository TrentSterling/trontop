# Trontop current state

Last updated: 2026-09-05

## Latest source: alpha.10 stable startup and service inventories

Branch `feat/provider-diagnostics`. Startup now retains results independently for
three Run keys and two Startup folders. Failed enumeration preserves the last known
entries for that source; a complete read can authoritatively remove entries. New
rows from an incomplete read are marked Observed, older rows Cached. Source cards
distinguish starting, live, empty, absent, partial, cached and unavailable results.
Services keeps its last complete list and explicitly labels stale reported states.
No startup enable/disable or service action support is implied.

The redesign skill's state/alignment audit guided fixed-height status surfaces and
stable source fields/table headers, retaining Tront zebra, hover and rounded styling.
Startup retention is bounded per source. Alphabetical indices rebuild on inventory
refresh; both tables format only visible rows. Full behavior and limitations:
`STARTUP_INVENTORY.md`. Slow native inventory calls still share the sampler; this is
not a claim that every provider stall or refresh issue is solved.

Final local gate: formatting PASS; **84 passed, 0 failed, 6 opt-in tests ignored**
(27.99 s); strict Clippy PASS; optimized release PASS (53.98 s). The explicit
offscreen test produced **42 PNGs** in 28.13 s on RTX 5070 Ti/Vulkan. Selected visual
reviews covered partial Startup, compact starting/unavailable Startup, light cached
Startup, cached Services and light unavailable Services. Not all 42 were inspected.
The new geometry test covers 32 inventory page/theme/state combinations, and a
20,000-entry test verifies that one viewport formats fewer than 100 rows.
No native GUI, desktop input, focus or window manipulation was used.

Separate review EXE: `target/review-build/release/trontop.exe`, **13,041,664 bytes**,
PE version **0.3.0-alpha.10**, SHA-256:
`F2148ADF82876419CBDD8D9DAFF8FE05729562DDDB0748816954678F00ECDF1A`.
Built from modified 018a17a source before checkpoint. Dependency inspection shows
Windows-only imports, no dynamic MSVC runtime or required vendor DLL/assets folder.
It has not been launched or copied over the deployed alpha.5 EXE. A read-only process
check at 09:22 UTC on 2026-09-05 found no running Trontop process; prior PID 255824
is historical, not current runtime verification.

Alpha.9 private Windows CI
[33956631086](https://github.com/TrentSterling/trontop/actions/runs/33956631086)
passed formatting/tests/strict Clippy/release/artifact for
018a17aa7646def8b2601510f049dd2ce67b243d at 09:14:03 UTC on 2026-09-05.
Alpha.10's remote gate is pending. No tag or alpha release is published.

Still open: CPU/motherboard sensors, broader vendor coverage, suspend/resume,
service actions, startup enable/disable, export, real close/drag measurements,
soak/clean-machine/release gates. Ctrl+Shift+Esc remains a proposal only. Do not
install drivers/hooks, replace the deployed EXE or manipulate desktop windows
without the appropriate fresh permission. This is not full Task Manager parity.

## Previous: alpha.9 background executable icons

Branch `feat/provider-diagnostics`. Processes/Details rows and the inspector now
request real embedded executable artwork through one bounded background worker.
No native file queries happen on the render thread. The in-memory cache has at most
256 entries, 32 outstanding requests and eight texture uploads per frame. Failed
lookups back off; a failed refresh keeps prior artwork. Fixed-size vector fallbacks
keep names aligned before/after loading. The inspector icon is hover-only, not a
dead button. The redesign skill's state/alignment audit guided this integration;
Trent's gradient/zebra theme and existing vector vocabulary are preserved.

Only local fixed-drive executable paths pass the native provider's best-effort
ancestor checks. Remote/removable/reparse/cloud-placeholder paths fall back; no
shell extension handlers, target execution, downloads or new runtime assets.
Artwork is never used as process identity or publisher trust. Full architecture,
limits and primary API references: `PROCESS_ICONS.md`.

Final local gate: formatting PASS; **74 passed, 0 failed, 6 opt-in tests ignored**
(23.16 s); strict Clippy PASS; optimized release build PASS (43.91 s). Explicit
offscreen pass PASS, **36 PNGs** in 22.52 s on RTX 5070 Ti/Vulkan. Reviewed mixed
loaded/fallback light/dark rows, selected inspector and compact copper Processes.
The compact table remains horizontally scrollable; this is not universal layout or
native DPI verification. No native window/input/clipboard commands were executed.

The separately selected read-only icon probe extracted this test EXE's own embedded
icon in 2.4387 ms, then ran 40 repeats: GDI/USER resources (4, 2) to (4, 2). An earlier
run was 2.9378 ms with identical counts. This is a bounded resource-lifetime check,
not total app overhead or end-to-end closing/dragging performance.

Separate optimized review EXE: `target/review-build/release/trontop.exe`,
12,960,256 bytes, PE version 0.3.0-alpha.9, SHA-256:
`D17075C24E012E7465355BCDD8D494AE9548FDCA4DF34AFC3541683AC9C804B1`.
Built from modified 2282469 source before checkpoint. `dumpbin /dependents` shows
Windows-only imports, no dynamic MSVC runtime or required vendor DLL. It was not
launched. The separately deployed alpha.5 EXE was not replaced/restarted.

Alpha.8 private Windows CI
[33955277229](https://github.com/TrentSterling/trontop/actions/runs/33955277229)
passed formatting/tests/strict Clippy/release/artifact for
2282469d6df0c7da2f16b1a4f6044e1574d5a947 at 08:47:16 UTC on 2026-09-05.
That run is not verification of alpha.9, whose remote gate is pending. No tag or
alpha release has been published.

Next: startup still replaces its whole row cache after partial source failures.
Preserve rows and freshness independently per source and show their state on the
Startup page; service failures also need a visible cached-state explanation there.
CPU/motherboard sensors, native performance measurements and all remaining release
gates stay open. Ctrl+Shift+Esc remains a proposal only. No desktop automation.

## Previous: alpha.8 GPU lifecycle and truthful activity states

Branch `feat/provider-diagnostics`. GPU inventory reconciliation now preserves
existing PDH handles and their rate samples. Only newly added counters warm up.
Failed enumeration leaves existing handles owned and explicitly marks incomplete
coverage. A failed collection re-primes rates instead of bridging the failed interval.

- Engine identity includes adapter LUID, physical adapter and engine index. Per-PID
  utilization uses the busiest engine. Physical-engine load sums its PID readings;
  the machine summary is the maximum engine load. It no longer sums parallel engines
  or unrelated adapters into a misleading engine-type total.
- Missing GPU data is distinct from zero through process/Details lists, process-tree
  and account totals, inspector, summary cards, meters and sorting. Partial totals
  use `>=` with explanations. Unknown readings sort last in both directions.
- Group GPU totals remain sums of process peaks, explicitly described on hover,
  not whole-GPU utilization. Partial/unavailable machine samples make history gaps;
  compact meters and the tray GPU tooltip conservatively omit partial values.
- The redesign skill's state/alignment audit led to fixed-height short inspector
  status text with the full reason on hover, main-token PID/account contrast, and
  five new GPU-state visual fixtures. Existing rounded zebra/hover surfaces remain.

Verification: formatting PASS; 64 tests passed, 0 failed, 5 opt-in tests ignored
(22.48 s); strict Clippy PASS. Explicit offscreen pass PASS, 33 fixture PNGs in
20.05 s on RTX 5070 Ti/Vulkan. Reviewed compact Processes, light tree/inspector,
Users, partial GPU Performance and Overview. The extra inspector regression requires
identical neighboring-field geometry through five live/missing states in both modes.
Headless UI checks do not execute viewport or clipboard commands.

The separately selected native PDH probe retained 690 handles, refreshed inventory
in 2.938 ms, and produced 690/690 valid rates and 39 PID readings after refresh.
Earlier probe: 690 retained, 5.351 ms. These are short read-only lifecycle checks,
not benchmarks of total app overhead, dragging or exact Task Manager parity.

Separate optimized review EXE: `target/review-build/release/trontop.exe`,
12,910,080 bytes, PE version 0.3.0-alpha.8, SHA-256:
`E25B63533890B395EC3AFE9E7E6FABAB9FF8BA65A3B5E8F9119334133584B53D`.
Build PASS (39.62 s), from modified e6db3e7 source before checkpoint. `dumpbin`
shows Windows-only imports, no bundled vendor library/dynamic MSVC runtime or runtime
asset folder. The EXE was not launched. No deployed alpha.5 replacement/restart,
desktop input, focus change, native window manipulation or shortcut installation.

Alpha.7's private Windows CI run
[33953608787](https://github.com/TrentSterling/trontop/actions/runs/33953608787)
passed all steps, including artifact upload, for e6db3e734bcb5a8eed967f85b62b66330fbb9719
at 08:10:14 UTC on 2026-09-05. That run does not verify alpha.8; its remote gate is
still pending. No release/tag is published.

Next: finish per-source startup status, CPU/motherboard coverage, executable icons
and release gates. GPU process creation may await the 30-tick inventory cycle plus
rate priming. Per-adapter GPU pages/VRAM mapping remain open. The Ctrl+Shift+Esc
proposal remains research only, with no hook or system-setting changes.

## Previous: alpha.7 isolated drive temperatures

Branch `feat/provider-diagnostics`. Added a native read-only storage provider and
slim zebra/hover sensor rows with fixed numeric alignment, guided by the supplied
component studies and the redesign skill. GPU, drive and CPU provider labels are
explicitly scoped; a live GPU does not imply complete sensor coverage.

- SetupAPI enumerates opaque disk interfaces. Access-0 handles query only
  `StorageDeviceTemperatureProperty`; no sectors, SMART pass-through, writes,
  threshold changes, additional drivers, services or elevation.
- One storage coordinator and at most 32 drive workers, one request per worker.
  Successful requests repeat after 5 s; failed/slow completed requests back off
  60 s. At 1 s, cancellation is requested on that owned worker only. A stuck call
  retains its slot/resources until completion, including across unplug/replug;
  it cannot create replacement-thread growth or block the regular sampler/UI.
- Failures keep the last usable readings and original timestamp marked Cached.
  Unsupported data remains unavailable. Per-drive query costs and provider-health
  coverage are visible. Overview reports each drive's hottest available sensor.
- Descriptor bounds/indices are validated. The SSD's unset -274 C threshold and
  SDK missing sentinel are unavailable, never presented as valid limits.

Verification: 55 tests passed, 0 failed, 4 opt-in tests ignored (29.45 s); strict
Clippy PASS; optimized build PASS (48.18 s). Explicit offscreen pass PASS with
28 PNGs (20.77 s); reviewed compact/light/cached/unavailable Sensors and Overview.
New tests cover parser bounds, stale cache, worker limits, failed-query backoff,
lost worker state, blocked-query isolation, hotplug duplicate prevention, bounded
drop and stable sensor text geometry. UI checks use fixture data and no OS input.

The explicitly selected native runtime probe queried ONLY the previously identified
TEAM TM8FP6002T SSD: 45/45/43 C, query 7.2355 ms, warning 90 C, critical 95 C.
The earlier run was 44/44/41 C at 7.2946 ms. No HDD query was repeated this slice.
These are short read-only measurements, not a sustained runtime/close benchmark.

Separate review EXE: `target/review-build/release/trontop.exe`, 12,900,864 bytes,
PE version 0.3.0-alpha.7, SHA-256:
`94B0396710AA005A3F5C169FCAAF72A6282479F65EA46CE2D6D5C4883C39537D`.
Built from modified b157f7e source before checkpoint. Windows-only imports including
SetupAPI; no bundled vendor DLL, dynamic MSVC runtime or runtime asset directory.
Not launched. The separately deployed alpha.5 copy was not replaced/restarted.
No native window manipulation, global input or unrelated process changes.

CPU/motherboard, unsupported storage-controller coverage, wear/error counters and
the alpha release gates remain open. Cancellation is a request, not a guaranteed
driver completion; coordinator inventory itself can stall only the storage provider.
No new end-to-end closing/dragging measurement. Remote CI for alpha.7 is pending;
no release/tag has been published. See `SENSORS_PLAN.md` for sources and exact limits.

## Previous: alpha.6 diagnostics, overview and vector controls

Branch `feat/provider-diagnostics`. The existing running alpha.5 preview was NOT
closed, replaced, moved or relaunched for this work. The separate optimized alpha.6
review EXE is `target/review-build/release/trontop.exe`, 12,839,936 bytes, SHA-256:
`91BDB02B4A0AB2744F4E8B3093F27FC1A8693E94FF16BBBC155E8B2DEB3E19E6`.
PE version 0.3.0-alpha.6. Built from the modified bfe8689 worktree before checkpoint;
About honestly records that modified build identity. It has NOT been launched.
`dumpbin /dependents` shows only Windows imports, no bundled NVML or dynamic MSVC CRT.
No installer or runtime image directory was introduced. No alpha.6 remote CI/release.

Implemented and locally verified:

- Overview dashboard, Hardware sensors navigation and About/provider-health dialog.
  Reports contain only build/provider status, timing, coverage and static reason
  strings. Copy is explicit, never uploaded; headless tests do not access clipboard.
- Stable sensor fields during startup/failure; the last complete NVIDIA snapshot is
  labeled Cached with original freshness. Cached values do not extend live graphs.
  GPU Engine fields remain present with dashes, and absent history samples form gaps.
- Native read-only startup queries, service cache retention/failure metadata, periodic
  CPU frequency refresh, unknown-account labeling and validated PDH counter status.
- Worker waiting is bounded at shutdown: sampler detaches after signaling stop, tray
  posts quit with at most 100 ms join waiting. Threads retain their own query/resource
  ownership. A blocked-worker regression test passes; whole-app close latency has NOT
  been measured, and no claim covers renderer/settings-save delays.
- Original 18-symbol vector vocabulary for navigation, window controls, action buttons
  and tree chevrons. No emoji or icon-font dependency. Accessible names/tooltips remain;
  disabled actions cannot click or shift. Selected navigation has a thin accent marker.
- Four supplied inspiration boards and a generated Signal icon candidate saved under
  `docs/inspiration`. PNG inspection confirmed ARGB with transparent corner pixels.
  Candidate is not embedded/shipped; existing theme-aware Tront mark remains in use.

Verification: formatting and diff whitespace checks PASS; 48 tests passed, 0 failed,
3 opt-in tests ignored (26.50 s); strict Clippy PASS; optimized build PASS (54.44 s).
Explicit offscreen rendering PASS, 25 PNGs using RTX 5070 Ti/Vulkan. Inspected compact
Processes, Overview, Hardware sensors and About, plus light/cached variants. The
headless page matrix covers 432 page/size/theme/data combinations. No native window,
tray test, global input, focus changes or user process mutations were used for UI QA.

Still open: individual startup-source status retention/display, per-process GPU
missing-value semantics, full Task Manager parity and release gates. CPU/motherboard
and drive temperature fields are explicit placeholders, NOT connected sensors. The
SSD probe proved one supported native path but the HDD query stalled for 4.25 seconds;
integrate storage on its own bounded slow worker. GitHub sensor research is saved in
`SENSORS_PLAN.md`. No approval exists to install drivers/services or elevate Trontop.

## Previous: alpha.5 preview refreshed at Trent's request

On 2026-09-05 at approximately 01:24 CDT, replaced the old alpha.1 EXE with a new
optimized build from the uncommitted `feat/provider-diagnostics` worktree. Trent
explicitly requested killing/replacing his running instance. The old PID 62220
exited before replacement; the guarded stop command refused its changed process
set and terminated nothing. Confirmed no Trontop remained before deployment.

The previous EXE is recoverable at
`target/replaced-builds/alpha1-20260905-0124/trontop.exe`. Deployment retained the
existing `target/release/trontop.exe` path and did not edit settings. New PID 255824
was observed with a responsive native window titled Trontop and PE version
0.3.0-alpha.5. No desktop input, focus manipulation or other app changes were used.
This one refresh does not authorize automatic restarts during further iteration.

New EXE: 12,785,152 bytes; SHA-256:
`A43DCD9DE277B4717EA4376B1D5EB06ECCC7D2597454A7CFD948A81B9BD6E778`.
At this historical checkpoint the separate review EXE matched this hash. Imports
remain Windows-only, with no NVML or dynamic MSVC runtime dependency.

Local source checks: formatting PASS; 40 tests passed, 0 failed, 3 ignored;
optimized build PASS (48.95 seconds). Eight unused diagnostics warnings remain
because the About/provider-health UI has not yet been connected. Strict Clippy,
new diagnostics offscreen fixtures, and remote CI are NOT verified for alpha.5.
The launched build includes the preceding hover/sensors/action-safety features and
the in-progress sampler corrections, but is not a completed release milestone.

At the alpha.5 checkpoint the next work was to connect the About dialog and report
action and expose independent
provider freshness and stale inventories; preserve startup-source status detail;
finish GPU unavailable-versus-zero presentation; test failure/recovery and report
copy without accessing the real clipboard; then run the full gate and visual pass.
See `DIAGNOSTICS_PLAN.md` and the newer alpha.6 entry above for the completed portion.

## Previous: alpha.4 process-action safety

Version 0.3.0-alpha.4 binds End Task, priority and affinity actions to the exact native
process creation FILETIME. Each operation verifies identity and Windows-critical
status on the same handle used for the action. Missing/changed identity refuses the
action. Pending End Task names/targets do not follow reused PIDs; stale selections
clear. New process rows receive identity queries on their first sample.
See `PROCESS_ACTION_SAFETY.md` for behavior, native/headless tests and limits.

The offscreen pass exposed an action-button baseline offset next to Cancel. The
shared helper now scopes visual colors without nesting layout allocations. A new
headless test checks enabled/disabled geometry and visual-state restoration; both
confirmation PNGs were inspected again after the fix.

Local gate: formatting PASS; 35 tests passed, 0 failed, 3 opt-in tests ignored;
strict Clippy PASS; optimized release build PASS. The explicitly selected offscreen
visual pass passed with 17 PNGs, including valid/stale confirmations. Native mutations
were restricted to harness-owned hidden child processes. PID 62220 was left untouched.

Review EXE remains `target/review-build/release/trontop.exe`; 12,784,128 bytes,
PE version 0.3.0-alpha.4; SHA-256:
`D4FCD918DBC411A2967A3088F53023F3418F8FA808CFFEC205D1FAB91A34CCC3`.
`dumpbin /dependents` shows only Windows imports, no NVML or dynamic MSVC runtime.
It has not been launched. Code checkpoint `f2f725c` is pushed to the private repo and
passed Windows CI run `33946298909`: formatting, 35 tests, strict Clippy, optimized
build and artifact upload. See `CI_ALPHA4.md` for the separate CI EXE, checksum and
scope. Neither review EXE was launched. No release/tag is published.

The native NVML probe passed again after this checkpoint: 0.327-0.354 ms warm query
times in a short five-sample run. A separate read-only storage capability probe
returned three temperatures from the TEAM SSD, but the HDD attempt took 4.25 seconds.
Storage sensors are NOT integrated; a dedicated slow worker is needed. Exact evidence,
permission context and remaining work are in `SENSORS_PLAN.md`.

The release audit found substantive alpha gates still open: suspend/resume, service
actions, About/provider diagnostics, JSON/CSV export, crash logs, administrator paths,
and the mixed-load soak. Do not weaken those gates or call the product complete.
The next bounded About/provider-health slice has code-audit findings and a privacy-safe
support-report plan in `DIAGNOSTICS_PLAN.md`.

## Earlier: alpha.3 GPU sensors

Version 0.3.0-alpha.3 adds optional read-only NVIDIA sensors on the background
sampler. Performance > GPU Sensors shows per-adapter temperature, board power,
graphics/memory clocks, fan target and VRAM. Rounded theme-aware cards include
two-minute temperature/power histories with rolling peaks and explicit missing data.
UUID-keyed histories survive enumeration reorder and expire on disconnect.
Existing PDH GPU Engine/per-process usage is unchanged. Read `SENSORS_PLAN.md` for
provider scope, safe loading, tests, measured query timings and future CPU/storage work.

Final verification: formatting PASS; 30 unit/headless tests passed, 0 failed,
3 opt-in tests ignored; strict Clippy PASS; optimized release build PASS. Explicit
native read-only NVML probe PASS; explicit offscreen visual pass PASS (15 PNGs).
No interactive tray test or desktop input automation was used. New dark/light,
compact and unavailable sensor images were inspected. These images use fixture data.

Review EXE: `target/review-build/release/trontop.exe`, 12,783,104 bytes,
SHA-256 `C957FA769F89CE73D7A2B6967B8433D355B47951E23AA4E16F5EFCF7A4387A2A`.
PE version is 0.3.0-alpha.3. `dumpbin /dependents` shows only Windows imports, no
NVML or dynamic MSVC runtime dependency. Sensor support dynamically uses the installed
driver, but no vendor DLL is required to launch the app. Clean-machine QA remains open.

This build has NOT been launched. PID 62220 still runs the older alpha.1 release-path
EXE and was left untouched. No new drag measurement, cross-project patch, push, or
GitHub release was performed in this slice. The broad product goal remains unfinished.

## Earlier: alpha.2 hover and safe UI verification

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
