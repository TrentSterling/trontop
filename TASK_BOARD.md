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
- [ ] Suspend and resume controls with unmistakable state feedback
- [ ] Start, stop, and restart service actions with confirmations and access errors
- [ ] Startup enable/disable support with a reversible disabled-entry store
- [ ] Disk active-time and latency counters through Windows performance counters
- [ ] GPU adapter identity, dedicated/shared memory, temperature where a stable provider exists
- [ ] Network link speed, adapter type, address, and per-process ETW traffic

## Visual polish queue

- [ ] Apply the full app-wide zebra, rounded surfaces, badges/buttons, and spacing brief in `docs/UI_POLISH_BRIEF.md`, including Performance and every dialog/list
- [x] Dedicated native tray thread, broad CPU level fill, and advancing history trace
- [x] Alternating column bands, consistent table cell insets, reserved table-footer space
- [x] Replace broken character-wrapping Users cards with an aligned resource table
- [x] Scrollable inspector and visible non-floating scrollbars
- [ ] Recheck left alignment: some labels still appear centered despite Label::halign
- [ ] Finish code-drawn vector navigation/window controls and stronger Tront branding (no emoji icons)
- [ ] Finish small-window Performance content scrolling and remaining spacing audit
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

- [x] Define private-alpha, release-candidate, public-preview, and rollback gates in `RELEASE_PLAN.md`
- [x] Create and push the private `TrentSterling/trontop` GitHub repository; first Windows CI passed
- [ ] Push the parked local UI/tray checkpoint, run CI for that exact commit, and publish the private alpha
- [x] Embed version metadata and a generated multi-resolution Trontop application icon in the PE
- [x] Add a Windows GitHub Actions gate that uploads the portable review executable
- [ ] Add an About panel with build hash and provider health
- [ ] Add snapshot export to JSON/CSV
- [ ] Add a deterministic HEADLESS telemetry replay harness for all seven pages, compact layouts, themes, search, selection, and text bounds. No desktop input injection.
- [ ] Sign the portable executable when the Tront signing pipeline is available

## Non-negotiables

- Never invent telemetry. Show unavailable or warming states.
- Keep OS collection off the render thread.
- Confirm destructive process and service operations.
- Preserve the single portable executable build.
- Keep ordinary display text non-selectable and maintain high text contrast.
