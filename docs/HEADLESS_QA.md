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

## Coverage

- 336 page/size/preset/mode/data cases: seven pages, 1040x640 / 1280x760 /
  1920x1080, four presets, light/dark, populated/empty fixtures.
- Actual text geometry: visible page titles, finite bounds, single-line table names.
- Existing widget tests verify left-aligned labels, right-aligned numbers, vertical
  centering, and full-cell clicks.
- Local egui pointer events exercise all navigation items and process selection.
- Compact sidebar checks keep GPU text visible above the footer. Local wheel input
  must reveal the bottom CPU detail row in the separately scrolled Performance pane.
- Search ancestor context, selected inspector, all Performance device types, missing
  device indices, and five dialogs are rendered without native services.
- Sixteen GPU sensor size/mode/state cases cover available, partially unsupported,
  missing-driver and multiple-adapter fixtures. Additional tests verify history
  identity, missing samples and removal after expiration.
- Thirteen shared surface variants in both modes must change background under the
  pointer, restore it after exit, and preserve geometry. Includes selected/unselected
  navigation/devices, cards, metrics, detail rows, badges, meters, action buttons,
  control rows and charts.

The explicit offscreen pass creates a GPU texture, not a window/surface. It uses the
real egui-WGPU renderer and embedded fonts, writes fifteen PNGs under `target/ui-smoke`,
and waits for dialog fade-in before capture. Fixtures use alternate presets without
changing Trent's persisted theme. These are review images, not live-telemetry captures.

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
