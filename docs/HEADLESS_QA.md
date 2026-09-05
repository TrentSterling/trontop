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
```

The last command is specifically selected, not a blanket `--ignored` run. The native
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

## Coverage

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
- Eighteen original vector symbols at 16/18/20/32 px stay within their allocation
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

The explicit offscreen pass creates a GPU texture, not a window/surface. It uses the
real egui-WGPU renderer and embedded fonts, writes forty-two PNGs under `target/ui-smoke`,
and waits for dialog fade-in before capture. Fixtures use alternate presets without
changing Trent's persisted theme. These are review images, not live-telemetry captures.
The five GPU-activity additions cover compact Processes, light process tree, Users,
light selected inspector and partially available Performance counters.
Three executable-icon additions cover mixed loaded/fallback rows in light/dark and
the selected inspector. Their colored sample artwork is synthetic fixture data.
Six inventory additions cover partial/cached/unavailable/starting Startup and
cached/unavailable Services, including compact and light layouts. Generated images
are not automatically visually reviewed; checkpoint notes identify inspected cases.

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
