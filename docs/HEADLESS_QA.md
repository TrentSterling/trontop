# Safe UI verification

The UI harness runs the production `eframe::App::ui` with an egui Context and a
test Frame. `TrontopApp::with_services` supplies no sampler and no tray. It does not
create a native window, inject OS input, change focus, or execute viewport commands.
Fixtures are synthetic TEST DATA and must never become a runtime telemetry fallback.

## Commands

```powershell
cargo test app::ui_smoke -- --nocapture
cargo test shared_surfaces -- --nocapture
cargo test render_offscreen_visual_pass -- --ignored --nocapture
cargo test render_graph_wall_visual_pass -- --ignored --nocapture
cargo test --release process_view_timing_probe -- --ignored --nocapture --test-threads=1
cargo test --release process_tree_timing_probe -- --ignored --nocapture --test-threads=1
```

The ignored-test commands are specifically selected, not a blanket `--ignored` run. The native
tray ignored test creates a real icon and must not run on the working desktop. A
separate, read-only `native_nvml_read_only_probe` opt-in test queries the installed
NVIDIA driver without any app window or input. See `SENSORS_PLAN.md`.
The `native_storage_ssd_probe` opt-in test is additionally restricted to the exact
previously identified TEAM SSD. It is not run by generic CI or the visual harness.
The separately selected `native_gpu_refresh_preserves_warm_counters` test opens its
own read-only PDH query, primes rates, refreshes inventory, and verifies retained
handles keep their samples. It never creates windows or installs input hooks.
The separately selected `native_executable_icon_read_only_probe` extracts the test
EXE's own resource and checks GDI/USER counts across 40 repeats, with no windows or
shell execution. Do not broaden it to arbitrary processes or the native tray test.
The separately selected `native_inventory_workers_publish_read_only_snapshots`
probe starts only the read-only Startup and Services workers and measures their
publication path. It sends no service commands. See `INVENTORY_WORKERS.md`.

## Coverage

- Alpha.26 adds twelve settings regressions covering bounded workers, legacy
  migration, staged replacement, invalid/read-only/conflicting files, coalescing,
  retained failures/retry, and blocked read/write drop. Production lifecycle tests
  call raw_input_hook, logic and UI, check local close/cancel commands without
  executing them, and verify loading navigation, Revert baseline and all close
  choices. A real app/worker/file/fresh-app round trip restores theme, named
  palette and zoom in an exclusively owned fixture directory. No user settings.
  Final ordinary gate: 225 pass, 13 ignored; strict Clippy/format/release PASS.
  Final matrix: 101 PNGs in 60.26 s; all eight new compact settings states and
  normal dark/light Theme Studio reviewed. This caught and fixed disabled-window
  translucency and tight warning padding. See `SETTINGS_PERSISTENCE.md`.
  Native restart, close timing, drag and soak remain separate unresolved gates.

- Alpha.25 adds four regressions for bounded background graphics diagnostics and
  non-waiting service-result polling. The actual renderer callback is exercised
  with a saturated queue and blocked writer. Final ordinary suite: 213 pass,
  13 ignored; strict Clippy/format/release PASS. Fresh optimized offscreen tests
  recover identical pixels in 3/3 device-loss cycles and keep an injected stalled
  recovery worker non-blocking. See `FAILURE_REPORTS.md` and `CURRENT_STATE.md`.
  No visual/layout code changed; alpha.24's 93 PNGs were not regenerated or
  claimed as fresh alpha.25 visual evidence. No native window or input was used.

- Alpha.24 adds seven compact-control regressions: six dialog types across two
  sizes/four UI scales, stable name/action geometry, full-name hover, affinity
  scrolling/high-bit/zero-mask/review behavior, numeric History alignment and
  maximum PID/counter text, plus CPU range formatting. All use synthetic data and
  local egui input only. Final ordinary gate: 209 pass, 13 ignored. Fourteen new
  dark/light compact images bring the offscreen matrix to 93 PNGs; all 14 new
  views were reviewed across the pass. See `COMPACT_CONTROLS.md` for exact limits.

- Alpha.23 adds six contrast regressions covering 4,096 RGB samples per mode,
  reference failures, actual action/icon state ink, hover badges/device labels,
  stable geometry, installed text-bearing surfaces and heat tiles. Final ordinary
  suite: 202 pass, 13 ignored; strict Clippy/formatting PASS. The offscreen pass
  produces 79 PNGs, including six extreme palette fixtures. Read
  `THEME_CONTRAST.md` for inspected scope and remaining acceptance limits.

- Alpha.22 moves native process/shell actions off the UI thread. Six worker tests
  plus four production-UI tests cover bounded dispatch/drop, exact frozen targets,
  stalled navigation, duplicates, stale confirmations, Enter/recovery gating and
  fixed-height pending/error messages. Default fixture apps cannot execute native
  actions. One explicitly owned hidden child verifies same-handle lifetime safety;
  shell/Explorer actions use injected backends only. See `PROCESS_ACTION_SAFETY.md`.
  Ordinary gate: 196 pass, 13 ignored; strict Clippy PASS. Three action-state PNG
  fixtures bring the offscreen suite to 73 PNGs (49.35 s), with pending compact,
  slow light and error compact reviewed after final tint/padding adjustments.
  No native desktop is involved.

- Alpha.21 adds four compact-layout regressions: all default process headers and
  toolbar actions fit without an inspector; selected compact metrics retain full
  values above the table; local inspector toggles retain selection/filter; long
  names/accounts do not change the identity block height. Tests use both modes,
  1040x640/1280x760 logical points and egui scale factors 1/1.25/1.5/2 for the
  header test. No native DPI transition is exercised. The offscreen suite now
  generates 70 PNGs. Compact/normal inspectors, hidden inspector, Processes and
  wide light Processes were inspected. Final ordinary gate: 186 pass, 13 ignored.
  The exact build and selected recovery/CPU retests are in `CURRENT_STATE.md`.

- Alpha.20 adds non-blocking snapshot transfer, renderer replay/state-gating and
  recovery-log tests. Separately selected offscreen device-loss and stalled-setup
  probes run the production recovery helper with no native windows or adapter reset.
  A full-UI CPU probe includes tessellation. See `RENDERER_RECOVERY.md` for numbers
  and the explicitly unverified native surface/drag/close/soak boundaries.

- Alpha.19 adds four-stop math/mesh, strict theme migration/import, bounded named
  library, backdrop contrast and local ramp drag/keyboard checks. All four Studio
  tabs keep their footer visible in dark/light at compact/normal sizes. The
  offscreen suite produces 67 PNGs; see `THEME_STUDIO.md` for reviewed scope and
  remaining native persistence/full-app contrast checks.

- Alpha.18 adds physical-disk provider/unit/native-buffer, latest-mailbox/stall,
  export freshness/privacy, identity selection and metric-geometry regressions.
  Dark/light and compact missing-data fixtures add three PNGs. The specifically
  selected `native_physical_disk_pdh_probe` is read-only, creates no app window
  and makes no disk writes; never replace its filter with a blanket ignored run.
  See `PHYSICAL_DISKS.md` for the native observations and remaining coverage.

- Alpha.17 adds allowlisted bounded failure-log checks and owned hidden child panic
  probes (no GUI, tray or sampler). About geometry/explicit-location-copy checks
  run at 1040x640 and 1280x900 in both themes; commands never reach the OS clipboard.
  About dark/light/compact PNGs were inspected. See `FAILURE_REPORTS.md` for exact
  panic coverage and limits; this is not a native crash/drag/close smoke test.
- Alpha.16 adds nine ordinary tests for displayed group sorting, GPU missing/partial
  order, identity-retained expansion and page-switch sort visibility. Local egui
  header/Tree/List/search interactions inspect the actual displayed row order.
  Two new dark/light grouped-sort fixtures bring the offscreen pass to 57 PNGs.
  See `PROCESS_SORTING.md`; no native window or input is involved in these tests.
- Alpha.15 adds eight hierarchy regressions and one deep-name production-UI test:
  50,000-node chain/cycle on 256 KiB stack, 128 arbitrary graphs, 2,880 small valid
  reference comparisons, creation-time guards and compact light/dark hover/selection.
  Its opt-in timing probe measures only tree building. See `PROCESS_TREE.md`.
- Alpha.14 adds four ordinary tests for source-index sorting, snapshot/History/search
  invalidation, selection identity and unchanged view storage across paints. Its
  opt-in headless timing probe compares 500/5,000 synthetic process workloads without
  a window, sampler or tray. See `PROCESS_VIEW_PERFORMANCE.md`; not an FPS/drag test.
- Alpha.13 export uses injected file-picker results only. Ten new encoding/worker/
  owned-file/UI tests cover privacy, Unicode, CSV parsing, unavailable/partial GPU,
  failed replacement, cancellation, one-job/drop behavior and explicit-only Save.
  Save/Close/status text must fit both clip rect and screen at 1040x640, with stable
  positions across outcomes in both themes. Native Save As is a separate gate.
- The offscreen pass now produces 57 PNGs, including two deep-tree layouts.
  The three export variants are fixture
  data and do not open a file picker or write an exported process snapshot.
- 432 page/size/preset/mode/data cases: nine pages, 1040x640 / 1280x760 /
  1920x1080, four presets, light/dark, populated/empty fixtures.
- Actual text geometry: visible page titles, finite bounds, single-line table names.
- Existing widget tests verify left-aligned labels, right-aligned numbers, vertical
  centering, and full-cell clicks.
- Shared action-button tests require the same baseline and height as a neighboring
  plain button in enabled/disabled states, and restore the surrounding visual colors.
- Local egui pointer events exercise all navigation items and process selection.
- Compact sidebar checks keep GPU text visible above the footer. Local wheel input
  must reveal the bottom CPU detail row in the separately scrolled Performance pane.
- Search ancestor context, selected inspector, all Performance device types, missing
  device indices, and five dialogs are rendered without native services.
- Sixteen GPU sensor size/mode/state cases cover available, partially unsupported,
  missing-driver and multiple-adapter fixtures. Additional tests verify history
  identity, missing samples and removal after expiration.
- Confirmation fixtures simulate PID reuse and require the original target/name,
  stale warning and expired selection. Local pointer input only cancels the dialog;
  native mutation tests exclusively use their own disposable hidden children.
- Fifteen shared surface variants in both modes must change background under the
  pointer, restore it after exit, and preserve geometry. Includes selected/unselected
  navigation/devices, cards, metrics, detail rows, badges, meters, action buttons,
  control rows and charts.
- Nineteen original vector symbols at 16/18/20/32 px stay within their allocation
  and produce no font text. Disabled icon actions cannot click or shift their bounds.
- About support-report copy only emits a command after explicit local click; report
  checks reject private fixture fields. The harness never executes clipboard commands.
- Six sensor fields keep identical geometry across live, cached and unavailable
  snapshots. Cached snapshots cannot extend the live history. Missing GPU graph
  samples produce gaps, not zero values or connecting traces.
- Three drive sensor rows retain identical label geometry, visible single-line
  readings and explicit state across live/cached/unavailable/disconnected snapshots
  in both modes. Opaque device-interface IDs never enter the rendered text.
- Fake storage backends exercise stuck I/O, independent fast-drive updates, timeout
  cancellation, retry deadlines, worker limits, disconnect/reconnect and bounded
  monitor drop. These tests do not query actual drives or manipulate the desktop.
- Six GPU activity states keep right-aligned, single-line numeric cells in both
  modes. Processes, Details and Users render measured zero, partial lower bounds
  and missing data without clipping. Tree aggregation preserves incomplete coverage.
- Inspector GPU status uses a short line with the full reason on hover; tests require
  unchanged Working set geometry through measured/partial/warming/unreported/failed
  states in compact dark/light layouts. PID/account text uses the main text token.
- Partial/unavailable GPU samples leave history gaps, preserve known engine rows,
  and recover to an explicit measured zero. Unknown values sort last in both
  directions; process-tree and user totals cannot silently treat them as zeros.
- Executable icons retain identical process-name geometry through fallback/artwork
  states in both themes, and local icon clicks select the corresponding PID. The
  headless app has no native icon worker; fixture paths cannot trigger OS reads.
- Icon cache tests cover native-path rejection using injected callbacks, duplicate
  requests, malformed/missing results, refresh failure retention, cache/queue/upload
  limits, eviction, disconnected workers and a blocked-loader drop. Pixel tests
  preserve transparent, antialiased and opaque-black coverage.
- Thirty-two inventory page/theme/state cases require fixed source fields and table
  headers through starting, live, partial, cached, unavailable, aged, recovered and
  empty snapshots. Synthetic failures never invoke native inventory providers.
- Startup cache tests cover independent failure/recovery, complete-read removals,
  same-display-name identities, same-timestamp attempts, and entry/text retention
  limits. Global alphabetical indices rebuild on inventory refresh.
- A 20,000-entry inventory fixture requires fewer than 100 row-format callbacks
  for one viewport. This checks virtualization, not whole-app frame rate.
- Service controls use an injected command backend, never real service commands.
  Local egui input selects a row, stages/cancels/confirms actions and verifies that
  expired or changed targets cannot be confirmed. Fixed header geometry is checked
  through five command states in both themes. Unknown outcomes disable retry and
  mark older rows Pre-command. Failed inventory cannot erase newer command state.
- Ten injected service-controller tests cover state/PID preflight, command ordering,
  single-flight behavior, non-atomic restart errors, timeouts, cancellation and
  blocked-worker drop. Native mapping/error tests and a bounded read-only SCM query
  test send no Start/Stop commands. Real command validation is a separate gate in
  `SERVICE_CONTROLS.md`, not evidence supplied by this harness.

The explicit offscreen pass creates a GPU texture, not a window/surface. It uses the
real egui-WGPU renderer and embedded fonts, writes PNGs under `target/ui-smoke`,
and waits for dialog fade-in before capture. Fixtures use alternate presets without
changing Trent's persisted theme. These are review images, not live-telemetry captures.
The five GPU-activity additions cover compact Processes, light process tree, Users,
light selected inspector and partially available Performance counters.
Three executable-icon additions cover mixed loaded/fallback rows in light/dark and
the selected inspector. Their colored sample artwork is synthetic fixture data.
Six inventory additions cover partial/cached/unavailable/starting Startup and
cached/unavailable Services, including compact and light layouts. Generated images
are not automatically visually reviewed; checkpoint notes identify inspected cases.
Five service-control additions cover normal, light and compact layouts, Restart
confirmation and command-error/uncertain state. The UI fixtures have no sampler;
Refresh list is intentionally disabled there, not evidence of a broken runtime control.
Three alpha.12 additions show retained results for multiple services and cached
Startup/Services after timeouts. Three new UI tests require per-service freshness,
disabled actions when tracking is full, retained rows and fixed headers after timeout.
Five injected inventory-worker tests never call native enumeration and check stalled
providers, coalesced refreshes, cached recovery, publication contention and shutdown.

## Limits

This does not measure native dragging, DWM presentation, GPU/CPU sensor accuracy,
multi-monitor DPI transitions, accessibility completeness, or sustained app resource
usage. It is not a universal no-overlap proof. Inspect the PNGs as well: the first
visual pass caught a sidebar footer overlap despite passing the initial tests.

## Build while the old EXE is running

```powershell
cargo build --release --offline --target-dir target/review-build --jobs 4
```

The review binary is `target/review-build/release/trontop.exe`. This keeps the running
`target/release/trontop.exe` untouched. Launching it on Trent's desktop is a separate
manual action; do not automate window closure, focus, or replacement.
