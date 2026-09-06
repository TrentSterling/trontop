# Trontop task board

**Product completion now lives in [ASK_LEDGER.md](ASK_LEDGER.md).** This file is the
historical engineering log, not an expanding release checklist. Its old unchecked
extras do not become release requirements unless linked to an agreed ask ID.
Keep the current source/build evidence in `docs/CURRENT_STATE.md`.

## Current: alpha.33 cores, Overview and ColorMagic

- Completed the September 6 A06/A13/A32 implementation slice: logical-processor
  histories/grid, dense shared-history Overview, coordinated randomizer and undo.
- 263 ordinary tests pass; strict Clippy passes. Six offscreen views reviewed;
  read-only CPU counter and optimized CPU-only UI probes recorded. No native
  dragging/close/soak claim. See `docs/ALPHA33_REVIEW.md`.
- `docs/HARDWARE_RESEARCH.md` explains Power clock versus live frequency and
  reference-monitor sensor backends. CPU temperatures/live boost remain open.
- Build identity: `docs/CURRENT_STATE.md`. Review before more speculative polish.
  New shared-cursor/project-grouping brainstorm ideas are not release gates.

## Previous: alpha.19 theme checkpoint and ask ledger

- [x] Four-peg gradient renderer and tabbed Studio, strict v2 migration/v3 import,
  named presets, surface controls, local drag/keyboard and fixed footer tests
- [x] Local gate: 175 passed, 0 failed, 10 ignored; strict Clippy/release PASS;
  67 offscreen PNGs with six theme layouts reviewed: `docs/THEME_STUDIO.md`
- [x] Consolidated `ASK_LEDGER.md` with evidence, outstanding reviews/decisions and
  a stopping condition; it replaces this file as the product completion checklist
- [ ] A13 native restart persistence and Trent's final theme review

## Previous: alpha.18 physical-disk activity

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
  geometry coverage: 153 tests pass, strict Clippy clean, 57 offscreen review PNGs
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
- [x] Alpha.11 Windows CI 33960026614 passed formatting/tests/Clippy/release/artifact
- [ ] Validate real service commands in an explicitly authorized isolated Windows
  fixture, including denied/dependent/pending/failure cases: `docs/SERVICE_CONTROLS.md`
- [x] Retain per-service command observations/uncertainty across commands to other
  services; bounded cache, no unknown-outcome eviction, refresh-before-new-target guard
- [x] Isolate Startup and Services reads on two fixed workers, coalesce refreshes,
  retain cached fields on timeout and avoid blocking joins or duplicate workers
- [x] Read-only native worker probe: 13 Startup entries, 303 services; 1,000 paired
  snapshot reads in 211.1 microseconds. This is not a whole-app benchmark
- [x] Timeout/worker-failure cached-row geometry and cross-service retention tests;
  three new final state screenshots inspected in the 50-image offscreen pass
- [x] Optimized alpha.12 preview opened on explicit request as PID 263640 at
  10:42:33 UTC; responding with a native HWND, older previews untouched
- [x] Alpha.12 Windows CI 33961633447 passed for cc2b793 at 11:09:25 UTC
- [x] Explicit JSON snapshot/CSV process export, private details off by default,
  freshness/missing-state preservation, one worker and staged file replacement
- [x] Ten new export data/worker/owned-file/headless UI tests and three reviewed
  export layouts; final optimized alpha.13 EXE dependency-inspected, not launched
- [x] Final alpha.12 preview opened on fresh explicit request as PID 262932 at
  11:15:53 UTC; `docs/CURRENT_STATE.md` records path/hash, older previews untouched
- [ ] Validate the native export Save As flow in an authorized isolated instance;
  injected picker/owned-file tests do not satisfy this gate: `docs/EXPORTS.md`
- [x] Alpha.13 Windows CI 33963422802 passed for 3142016 at 11:49:30 UTC;
  no alpha release is published
- [x] Snapshot-indexed flat/tree rows and cached History ordering; no whole-process
  record copies on table repaint, allocation-free name/account comparisons
- [x] Four index/sort/invalidation/identity/repaint tests and paired headless timings;
  5,000-process tree 3,888.1 to 206.9 us, not native drag/whole-app performance proof
- [x] Optimized alpha.14 EXE with Windows-only import scan and four process-page
  visual reviews; no new preview launch or existing-window manipulation
- [x] Alpha.14 Windows CI 33964418983 passed for 5b000f4
- [x] Alpha.14 preview opened on explicit request at 12:00:07 UTC as PID 273992;
  confirmed responding, older previews and other windows untouched
- [x] Iterative hierarchy/totals/search, known newer-parent rejection and cycle
  normalization; 50,000-level chain/cycle and independent reference stress coverage
- [x] Readable deep names with full depth on hover; light/dark local scrolling and
  selection checks, two new rendered variants inspected, alpha.15 EXE not launched
- [x] Three paired headless hierarchy probes: collapsed 1,000-node chain rebuild
  16,771.9 to 132.8 us; exact evidence/limits in `docs/PROCESS_TREE.md`
- [x] Alpha.15 Windows CI 33965959216 passed for 7df48c9
- [x] Sort displayed tree totals; keep missing GPU last and preserve flat ordering
- [x] Retain expansion through refresh/search, expire disappearance/observed PID reuse,
  and reset invisible sort keys after Processes/Details switches
- [x] Nine new model/state/headless UI tests and two grouped-sort visual fixtures;
  optimized alpha.16 preview explicitly opened as PID 272352 at 12:51:50 UTC
- [x] Alpha.16 Windows CI 33967714299 passed for df7e3fb
- [x] Bounded local panic/native-runner failure records, with private payloads excluded,
  non-waiting OS lock, retention/error tests and safe hidden child panic probes
- [x] About failure-log location/limits and explicit copy, compact light/dark geometry
  checks and three inspected offscreen layouts: `docs/FAILURE_REPORTS.md`
- [x] Alpha.17 Windows CI 33969106100 passed for 02ea7f857245d49df18681eee602a9d883a54b58
- [x] Independent physical-disk PDH worker: active time, latency, queue depth and
  physical read/write rates, bounded latest-snapshot mailbox and non-waiting drop
- [x] Stable cached/missing fields, instance-based selection, gap-aware charts,
  separate Volume labels, provider health and privacy-gated JSON instance names
- [x] Read-only probe returned all five metrics on three physical disks after warmup;
  warm queries 0.210/0.168/0.188 ms in one short debug run: `docs/PHYSICAL_DISKS.md`
- [ ] Verify alpha.18 in remote Windows CI
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
- [x] Disk active-time, response latency and current queue counters through Windows PDH
- [ ] Broaden physical-disk native verification to localized Windows, unplug/replug,
  disabled categories and storage-controller/volume-extent identity mappings
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
- [x] Replace cloned/sorted process records with snapshot indices and allocation-free
  name/account comparisons; History sorting happens only on view rebuild
- [x] Stress and repair deep/cyclic process hierarchies: `docs/PROCESS_TREE.md`
- [x] Sort tree resource columns by displayed subtree values, not individual counters
- [x] Expire expanded-PID view state on observed PID reuse; unknown identity remains
  best-effort presentation only, never native action authority
- [ ] Cache expensive search metadata; `docs/PROCESS_VIEW_PERFORMANCE.md`
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
- [x] Add snapshot export to JSON/CSV; native picker validation remains open above
- [x] Add bounded local Rust-panic/native-runner logs beside settings; direct native
  crashes, hangs, forced termination and power-loss durability are not covered
- [x] Add a HEADLESS fixture harness for all nine pages, compact layouts, themes, search, selection and text bounds, plus offscreen PNG output: `docs/HEADLESS_QA.md`
- [ ] Sign the portable executable when the Tront signing pipeline is available

## Non-negotiables

- Never invent telemetry. Show unavailable or warming states.
- Keep OS collection off the render thread.
- Confirm destructive process and service operations.
- Preserve the single portable executable build.
- Keep ordinary display text non-selectable and maintain high text contrast.
