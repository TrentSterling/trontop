# Four-peg Theme Studio (alpha.19)

Ask A13 in `../ASK_LEDGER.md`. This is a bounded theme-editor pass, not a claim
that every application page has completed its final accessibility/visual audit.

## Available controls

- Palette: four independent RGB colors and positions, draggable ramp pegs, hex
  input, numeric positions, reverse/even spacing, direction and intensity.
- Primary/secondary UI accents are independent of the background ramp.
- Appearance: panel opacity, corner roundness, row stripes, column tint and hover
  intensity. Extra text contrast raises ordinary/muted foreground contrast.
- Eight visual presets: TrontStack, Demigod, Monke Portal, Copper Legacy, Porcelain,
  Carbon, Phosphor, Vector. These are palette/surface variants, not exact replicas
  of every shape/font in the supplied generated mockups.
- My themes: up to twelve named local presets, explicit replacement, load/delete,
  explicit copy-current and paste/import. Names are bounded to 32 characters.
- Changes apply live. Revert session restores the appearance from when the editor
  opened; Done and the close button keep changes. Footer controls stay outside the
  scrolling editor content. No continuous animation or randomization is installed.

Pegs keep their order and at least 1% separation; first/last colors extend to the
edges when their positions move inward. Arrow keys move the focused ramp's selected
peg 1%, Shift 5%; up/down chooses a peg. Numeric/hex fields offer a non-drag path.
Invalid/incomplete hex retains the last valid color and shows an error outline.

The native background is a small band-clipped mesh with vertices at actual peg
planes. It does not approximate close pegs on a fixed grid. Background RGB bounds
protect ordinary/muted backdrop text at maximum intensity; the test samples 125
RGB combinations in each mode against a 4.5:1 threshold. This is not a certification
of every arbitrary accent/status/disabled/hover combination across the entire app;
that final audit remains A16. Panel opacity is tint compositing, not desktop blur.

## Persistence and portability

Current settings use `trontop.theme.v3` in eframe's existing per-user storage.
When no v3 key exists, the app reads `trontop.theme.v2`. The original two-color ramp
migrates into four collinear stops, within one RGB code value of the original ramp.
The legacy key is not deliberately removed. Preset library uses
`trontop.theme-library.v1` in the same storage. The application still ships as one
EXE; neither JSON themes nor inspiration images are runtime asset dependencies.

Current exports are versioned JSON with only appearance values, no telemetry,
paths or process data. Import is limited to 16 KiB, requires the complete v3 shape
or legacy eight-field code, rejects non-finite/wrong-type/out-of-range RGB data,
and normalizes bounded numeric settings. Unsupported versions do not partially
apply. Library loading is bounded/atomic; malformed or duplicate-name input does
not replace the current in-memory library. Files are not opened by the theme UI;
copy happens only after the user's explicit button click.

Restart serialization/migration has unit coverage; no current preview was closed
or reopened to test real eframe disk persistence. Concurrent older previews share
the per-user eframe storage file. Avoid concurrent settings edits while reviewing;
multi-instance last-save behavior is not a travelling-settings solution.

## Verification

Final ordinary suite: **175 passed, 0 failed, 10 ignored**, strict Clippy PASS.
New checks cover four-stop interpolation/reversal, malformed input, exact bounded
mesh area across angles, migration/round-trip, backdrop contrast, library bounds,
local peg dragging/keyboard, session lifetime, and every editor tab's visible
footer at 1040x640 and 1280x760 in light/dark. Headless input never reaches Windows.

Offscreen suite: **67 PNGs**. Palette dark/light/compact, Appearance, Presets and
My themes were actually inspected. The final build identity and any later remote
verification are recorded in `CURRENT_STATE.md`. Native persistence, full custom
color accessibility and Trent's final visual approval remain release checks.
