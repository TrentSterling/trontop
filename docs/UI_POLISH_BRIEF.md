# App-wide UI polish brief

Saved September 4, 2026, as Trent parked the session.

Trent wants the surface treatment applied throughout the app, not only to Processes.
The latest screenshot shows Performance: a bare device rail, flat metric labels and
details beneath the graph, and many controls without a cohesive rounded container.
These still feel unfinished to him.

## Requested scope

- Alternating row AND subtle column treatment across all lists, grids, and tables.
  Audit Processes, Details, Users, History, Startup, Services, performance device
  lists, inspector detail rows, and dialog lists. Do not stop after one table.
- Rounded layout boxes for meaningful rows/items and consistent rounded treatment
  for buttons, badges, grouped metrics, and controls. Carry this through each window
  and Theme Studio. Empty states and small status elements are part of the pass.
- Consistent cell padding, panel margins, header baselines, and spacing between groups.
  Do not leave text pressed against borders, footers clipped, or huge unstructured gaps.
- Keep the Tront gradient/theme system and readable foreground contrast. Decorative
  tint must not obscure text or flatten hover, selected, focused, and disabled states.
- Stronger Tront identity. The current generic T badge and text chrome are not enough.
  Use a cohesive code-drawn vector or embedded SVG icon family for navigation and
  window controls. No emoji glyphs; embedded bitmaps are acceptable where necessary.
- Preserve density and the single portable EXE. No runtime asset directory or new
  mandatory font install. Do not add artificial continuous animation to a task manager.

## Verification and current gaps

Run headless layout smoke tests at 1040x640 and 1280x760 logical pixels, plus larger
sizes, with each preset and light/dark modes. Include long names, narrow columns,
empty/unavailable data, selections, expanded trees, scrolled inspectors, and dialogs.
Tests may vary their own themes; do not randomly replace Trent's saved theme.

The comprehensive headless harness does not exist yet. Build it without native
sampler/tray initialization or global mouse/keyboard input. Confirm all primary text
stays legible and values do not wrap into single characters, especially on Users.
Review rendered output, not just successful compilation. Working-desktop automation
is prohibited by the explicit safety rule in `AGENTS.md`.

Already implemented in the checkpoint: row/column bands in process and inventory
tables, an aligned Users table, normalized cell insets, reserved footer space,
scrollable inspector, and higher-contrast selected labels. Remaining issues include
apparently centered table labels, incomplete Performance overflow handling, incomplete
global rounded surfaces, and unfinished branding. Do not report the visual pass done.
