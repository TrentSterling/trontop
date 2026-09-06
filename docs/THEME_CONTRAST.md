# Theme contrast and keyboard focus

## Table interaction follow-up (alpha.32, A16)

A local Tab regression reproduced focused table labels/heat cells with no visible
indicator. The production process-table test then exposed an additional invisible
stop on the encompassing row hit area before the actual header control.

Custom table labels, sort headers and heat-value cells now paint a rounded
contrast-safe focus outline. It borrows the existing outer-cell padding and clips
to the column, without changing text, allocation, selection or hover geometry.
The process table's whole-row hit area and redundant process-icon target retain
mouse clicks but no longer add keyboard stops; labeled cells and expansion buttons
retain keyboard activation. No extra timer, worker, animation or dependency.

Four new ordinary regressions cover:

- Real Tab traversal through labels and zero/full heat cells, outline removal,
  disabled targets, unchanged text geometry and dark/light extreme-accent contrast.
- Enter/Space activation and disabled-cell gating.
- Production process-table keyboard sorting and PID selection in compact/normal
  sizes and four egui scale factors. No native/platform commands are executed.
- Mouse selection in the row's outer padding, preserving the full-row hit area.

The specifically selected `render_table_keyboard_focus_visual_pass` produces four
offscreen PNGs in `target/ui-smoke`: dark/light sort headers, a dark heat cell and
a light process name. All four were inspected after widening text clearance.
The final selected render took 3.66 seconds on RTX 5070 Ti / Vulkan 591.86.
Fixture data is synthetic; these are not real keyboard, native DPI, close or drag
measurements. The entire A16 page/state inventory and Trent's acceptance remain
open. Current ordinary gate and exact EXE identity are in `CURRENT_STATE.md`.

## Original alpha.23 contrast pass

Scope: A16, with the related stable-layout and zebra constraints in A14/A15.
This is an existing-interface audit, not another brand or layout redesign.

## Failures found

- End/confirmation actions forced white RichText over bright button fills. The
  existing dark-mode danger color fails a 4.5:1 white-text check.
- Raw accents were also foregrounds in History, status badges and warning text.
  Black-on-dark and yellow/teal-on-light themes could hide these labels.
- Selected/pressed/hover colors and stacked column/heat overlays could escape the
  background brightness protection already used by four-peg gradients.
- Small meter fills disappeared with yellow-on-light or black-on-dark accents.
- The new state test caught a half-pixel text shift when button border widths
  differed. Their state widths are now equal; geometry tests pass.

## Display behavior

Saved accents, four gradient pegs, swatches and exported themes keep their raw RGB.
Text-bearing shared surfaces reuse the backdrop brightness envelope: dark channels
at most 64, light channels at least 205. Hover, selection, open controls, row and
column bands stay inside it, including layered bands. Heat tiles use an opaque
bounded tint so their labels do not depend on unknown stacked row/column colors.
Striping, hover and selection remain; their extreme tints may be less intense.

Foreground ink is independent of decoration. A color that already passes remains
unchanged; otherwise it is lifted/darkened toward white/black until it passes.
Filled action buttons retain saturated fills and choose ink for each interaction
state. They resolve RichText's placeholder color through the actual egui button
painter, preserving font/size while preventing a caller's forced white override.
Current action callers use plain/RichText labels, not precolored LayoutJobs/Galleys.
Disabled actions retain egui's disabled appearance and input gating.

Badges use explicit bounded faces instead of unknown translucent backgrounds.
History colors, warnings, hyperlinks, placeholder text, numbered pegs, vector T
strokes, selected/focused outlines, chart lines and sidebar meters use readable
ink where appropriate. No native process-icon artwork is modified.

The 4.5:1 small-text target and opaque sRGB calculation follow the
[W3C contrast guidance](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html).
This is not a whole-app WCAG certification. Glyph anti-aliasing, animated window
opening, platform presentation and the final page/state inventory still need
their corresponding visual/native acceptance. Disabled controls and decoration
are not silently counted as passing text checks.

## Verification

- Six new ordinary tests: known bad/reference pairs; 4,096 RGB values per mode;
  production action/icon button idle/hover/press/focus and unchanged geometry;
  badge/selected-device/inventory labels; installed selection/hover/open/weak-text
  colors; and heat-value tiles. All input is local egui input, never Windows input.
- Final ordinary gate: **202 passed, 0 failed, 13 ignored**, 26.68 seconds.
  Strict Clippy and formatting pass. Optimized EXE build passes (48.77 seconds).
- Final offscreen pass: **79 PNGs**, 50.46 seconds. Six new extreme-color fixtures;
  dark white Processes, light yellow History, both extreme Theme Studio views,
  dark black History, regular Processes and wide light Processes were inspected.
  The first new Studio captures sampled its opening fade too early; the fixtures
  now use the same 20 settling frames as the other dialog cases. Both were reviewed
  again after settling. Production animation behavior was not changed.
- The visual review identified the sidebar yellow-meter issue and led to its final
  ink correction. The final suite/visual pass includes that correction.
- Optimized UI-only/tessellation probe before that final meter correction:
  p95 **0.18-1.05 ms** across nine pages at 500/5,000 fixture processes. Not a
  before/after speedup result, native presentation rate or dragging measurement.
  Luminance uses a single lazy 256-entry lookup table, not per-label powers.

Exact artifact identity and upload status: `CURRENT_STATE.md`. No new runtime
dependency, timer, app window, tray, driver, hook or desktop interaction was added.
A16 remains PARTIAL / REVIEW until the complete page/state inventory and Trent's
review are finished. A20/A21/A22 native drag/close/soak gates remain separate.
