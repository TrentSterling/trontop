# Trontop task board

This is the durable resume board. Keep it honest and update it when a slice lands.

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
- [ ] Start, stop, and restart service actions with confirmations and access errors
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
- [ ] Process icons with a bounded background cache
- [ ] Better empty states for machines with no active disk/network/GPU counters
- [ ] Optional 0.5, 1, 2, and 5 second sample intervals
- [ ] Keyboard navigation, command palette, and shortcut reference
- [ ] Accessibility pass for focus outlines, keyboard-only flows, and color-blind presets

## Release work

- [x] Build a separate optimized alpha.4 review EXE without touching the running release copy
- [ ] Trent manually reviews alpha.4 sensors, confirmation and hover/scroll behavior; no automated desktop interaction

- [x] Define private-alpha, release-candidate, public-preview, and rollback gates in `RELEASE_PLAN.md`
- [x] Create and push the private `TrentSterling/trontop` GitHub repository; first Windows CI passed
- [ ] Push the local UI/tray/sensors/action-safety checkpoint and run CI for that exact code commit
- [ ] Complete the remaining alpha gates in `RELEASE_PLAN.md`, then publish the private alpha
- [x] Embed version metadata and a generated multi-resolution Trontop application icon in the PE
- [x] Add a Windows GitHub Actions gate that uploads the portable review executable
- [ ] Add an About panel with build hash and provider health
- [ ] Add snapshot export to JSON/CSV
- [x] Add a HEADLESS fixture harness for all seven pages, compact layouts, themes, search, selection and text bounds, plus offscreen PNG output: `docs/HEADLESS_QA.md`
- [ ] Sign the portable executable when the Tront signing pipeline is available

## Non-negotiables

- Never invent telemetry. Show unavailable or warming states.
- Keep OS collection off the render thread.
- Confirm destructive process and service operations.
- Preserve the single portable executable build.
- Keep ordinary display text non-selectable and maintain high text contrast.
