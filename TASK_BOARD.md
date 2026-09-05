# Trontop task board

This is the durable resume board. Keep it honest and update it when a slice lands.

## Current: alpha.11 confirmed service controls, stable inventories and sensors

- [x] Provider-health model, safe report formatter and background sampler metadata
- [x] Native read-only startup inventory with distinct missing/failed source results
- [x] Validate both PDH API/status results and preserve valid zero counter readings
- [x] Periodically refresh CPU frequency; label missing process accounts as unknown
- [x] Refresh Trent's old alpha.1 EXE with the optimized alpha.5 development preview
  on explicit request; PID 255824, backup and checksum in `docs/CURRENT_STATE.md`
- [x] Connect About/status UI and explicit privacy-safe report copy
- [x] Overview dashboard and Hardware sensors page with honest missing fields
- [x] Preserve last complete NVIDIA snapshot as explicitly cached after provider failure
- [x] Stable GPU Engine fields and graph gaps instead of invented missing zeros
- [x] Bound sampler/tray worker waiting during shutdown; blocked-worker regression test
- [x] Original vector navigation/window/action/tree icons, including accessible names
  and disabled-action/hover/geometry tests; no emoji/font-icon dependency
- [x] Save four supplied mockup boards and transparent Signal identity candidate
  under `docs/inspiration`; candidate is not the shipped icon
- [x] Preserve Startup entries independently per source; show source/row freshness
  and cached Services state without shifting table headers: `docs/STARTUP_INVENTORY.md`
- [x] Bound retained startup entries/text; cache alphabetical indices on refresh
  and format only visible inventory rows, tested with 20,000 entries
- [x] Preserve warmed PDH handles during inventory refresh and aggregate by real
  physical-engine identity, not summed engine types or per-process parallel engines
- [x] Complete GPU unavailable/warming/unreported/zero/partial presentation, sorting,
  tree/account totals and stable inspector status; partial graphs remain gaps
- [x] Failure/recovery, report-copy, disabled vector controls and missing sensor
  geometry coverage: 101 tests pass, strict Clippy clean, 47 offscreen review PNGs
- [x] Isolated native drive temperature provider; bounded workers, timeouts, slow
  retry, honest cached data, hotplug duplicate prevention and stable zebra rows
- [x] Native read-only runtime probe limited to TEAM SSD: 45/45/43 C in 7.2355 ms;
  separate optimized alpha.7 EXE passes Windows-only dependency inspection
- [x] Alpha.7 Windows CI 33953608787 passed formatting/tests/Clippy/release/artifact
- [x] Native GPU refresh retained 690 handles with 690/690 valid rates afterward;
  2.938 ms inventory in one read-only run, not a whole-app performance benchmark
- [x] Separate optimized alpha.8 EXE and Windows-only import inspection; not launched
- [x] Alpha.8 Windows CI 33955277229 passed formatting/tests/Clippy/release/artifact
- [x] Real embedded executable artwork in process/Details rows and inspector, with
  bounded background cache, negative retry, fixed fallback geometry and safe drop
- [x] Native icon probe: 40 extracts, GDI/USER counts (4, 2) to (4, 2); source path
  gate and queued-upload limits tested; `docs/PROCESS_ICONS.md`
- [x] Separate optimized alpha.9 EXE, Windows-only import scan and light/dark visual
  review; not launched and no deployed-copy replacement
- [x] Alpha.9 Windows CI 33956631086 passed formatting/tests/Clippy/release/artifact
- [x] Separate optimized alpha.10 EXE, Windows-only import scan and selected
  startup/service state PNG review; independent preview opened on explicit request
- [x] Alpha.10 Windows CI 33958007255 passed formatting/tests/Clippy/release/artifact
- [x] Confirmed single-flight service Start/Stop/Restart worker, same-handle native
  preflight, uncertainty/access errors, no blocking render-thread commands
- [x] Fake-backend command/close tests and native read-only status probe; aligned
  Services controls, expiring confirmation and five new inspected review PNGs
- [x] Separate optimized alpha.11 EXE, Windows-only import scan; hash-verified preview
  opened on explicit request as PID 274860 (10:02:27 UTC), older previews untouched
- [ ] Verify alpha.11 in remote Windows CI; no alpha release is published
- [ ] Validate real service commands in an explicitly authorized isolated Windows
  fixture, including denied/dependent/pending/failure cases: `docs/SERVICE_CONTROLS.md`
- [ ] Retain per-service command observations/uncertainty across subsequent commands
  to other services; current override covers only the latest command
- [ ] Isolate potentially slow startup/service inventory calls from the sampler
- [ ] Integrate CPU/motherboard providers and broader storage-controller coverage.
  Existing GPU/SSD readings are real; CPU fields remain unconnected, not simulated.
- [ ] Measure real close latency only with a freshly authorized isolated app instance
- [ ] Discuss/prototype opt-in Ctrl+Shift+Esc interception while preserving Windows
  Task Manager through Ctrl+Alt+Del: `docs/HOTKEY_PLAN.md`. Research only; no hook
  installed or keyboard setting changed. Native validation needs fresh permission.

## Shipped in 0.2

- [x] Frameless Trontop chrome and native window controls
- [x] High-contrast TrontStack purple/teal visual system
- [x] Persistent gradient Theme Studio and branded presets
- [x] Theme-derived zebra tables with dual-signal hover and selection states
- [x] Non-selectable application labels with deliberate copy surfaces
- [x] Exact Windows GPU Engine PDH totals and per-process GPU values
- [x] CPU, memory, disk, network, and GPU performance drill-downs
- [x] Processes and dense Details tables
- [x] Lifetime resource History page
- [x] User resource aggregation
- [x] Startup Run key and Startup folder inventory
- [x] Windows service inventory
- [x] Run task and confirmed End task flows
- [x] Live CPU tray meter with resource tooltip, Show, and Quit

## Next high-value slice

- [x] True process tree ordering, expand/collapse, search context, and child resource aggregation
- [x] Priority class editor with explicit confirmation and current-priority display
- [x] CPU affinity editor with processor-group-aware logical processor labels
- [x] Exact native creation-time validation on the action handle for End Task/priority/affinity; critical-process protection and stale confirmations: `docs/PROCESS_ACTION_SAFETY.md`
- [ ] Suspend and resume controls with unmistakable state feedback
- [x] Start, stop, and restart service actions with confirmations and access errors;
  native end-to-end validation remains open above
- [ ] Startup enable/disable support with a reversible disabled-entry store
- [ ] Disk active-time and latency counters through Windows performance counters
- [ ] GPU adapter identity, dedicated/shared memory, temperature where a stable provider exists
- [x] Optional NVML GPU temperature/power/clocks/fan/VRAM provider with background sampling, partial-support states, and a read-only 5070 Ti probe
- [x] GPU Sensors page with rounded hover cards and UUID-keyed temperature/power histories and peaks; compact/light/dark/missing-data headless coverage
- [ ] Storage health/temperature, CPU package/core sensor investigation, and AMD/Intel/legacy NVIDIA coverage: `docs/SENSORS_PLAN.md`
- [ ] Network link speed, adapter type, address, and per-process ETW traffic

## Visual polish queue

- [ ] Apply the full app-wide zebra, rounded surfaces, badges/buttons, and spacing brief in `docs/UI_POLISH_BRIEF.md`, including Performance and every dialog/list
- [x] Dedicated native tray thread, broad CPU level fill, and advancing history trace
- [x] Alternating column bands, consistent table cell insets, reserved table-footer space
- [x] Replace broken character-wrapping Users cards with an aligned resource table
- [x] Scrollable inspector and visible non-floating scrollbars
- [x] Fix centered table labels caused by add_sized; headless tests check left/right alignment, vertical centers, single-line layout, and full-cell clicks
- [x] Verify table alignment across seven offscreen-rendered pages; alpha.2 release build contains the correction
- [x] Shared hover backgrounds for navigation, device tiles, metrics, cards, labels, badges, charts, table cells and custom action buttons; Theme Studio gets aligned rounded zebra control rows
- [x] Correct branded action-button baseline offset beside plain buttons; enabled/disabled geometry tests and confirmation PNG review
- [ ] Finish code-drawn vector navigation/window controls and stronger Tront branding (no emoji icons)
- [x] Independently scroll Performance rail/content and pin the sidebar footer; headless input tests verify compact scrolling and no GPU-label/footer overlap
- [ ] Continue remaining app-wide spacing, zebra details and visual-state audit; new QA is not a universal proof
- [ ] Diagnose sustained window-drag lag; findings in `docs/DRAG_INVESTIGATION.md`
- [ ] Port a VERIFIED drag fix to other egui apps only after isolated before/after measurement
- [ ] Replace the cloned/sorted process view with stable snapshot indices and cached sort keys
- [ ] Responsive compact navigation below 1150 logical pixels
- [ ] Configurable table columns and saved widths
- [ ] Per-device chart color controls in Theme Studio
- [ ] Multi-chart GPU layout for 3D, Copy, Compute, Encode, and Decode
- [x] Process icons with a bounded background cache; unsupported paths keep vector
  fallbacks, no shell association/UWP-specific icon provider yet
- [ ] Better empty states for machines with no active disk/network/GPU counters
- [ ] Optional 0.5, 1, 2, and 5 second sample intervals
- [ ] Keyboard navigation, command palette, and shortcut reference
- [ ] Accessibility pass for focus outlines, keyboard-only flows, and color-blind presets

## Release work

- [x] Build a separate optimized alpha.4 review EXE without touching the running release copy
- [ ] Trent manually reviews alpha.4 sensors, confirmation and hover/scroll behavior; no automated desktop interaction

- [x] Define private-alpha, release-candidate, public-preview, and rollback gates in `RELEASE_PLAN.md`
- [x] Create and push the private `TrentSterling/trontop` GitHub repository; first Windows CI passed
- [x] Push UI/tray/sensors/action-safety checkpoint `f2f725c`; Windows CI `33946298909` passed and its private artifact was inspected: `docs/CI_ALPHA4.md`
- [ ] Complete the remaining alpha gates in `RELEASE_PLAN.md`, then publish the private alpha
- [x] Embed version metadata and a generated multi-resolution Trontop application icon in the PE
- [x] Add a Windows GitHub Actions gate that uploads the portable review executable
- [ ] Update deprecated workflow action runtimes and add a correctly keyed Rust dependency cache; preserve cold-build and test gates
- [x] Add an About panel with build hash, provider health/freshness and privacy-safe support report: `docs/DIAGNOSTICS_PLAN.md`
- [ ] Add snapshot export to JSON/CSV
- [x] Add a HEADLESS fixture harness for all nine pages, compact layouts, themes, search, selection and text bounds, plus offscreen PNG output: `docs/HEADLESS_QA.md`
- [ ] Sign the portable executable when the Tront signing pipeline is available

## Non-negotiables

- Never invent telemetry. Show unavailable or warming states.
- Keep OS collection off the render thread.
- Confirm destructive process and service operations.
- Preserve the single portable executable build.
- Keep ordinary display text non-selectable and maintain high text contrast.
