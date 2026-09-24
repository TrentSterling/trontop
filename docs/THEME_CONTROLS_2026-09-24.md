# Theme controls and generated logo

Follow-up in alpha.40: `POLISH_2026-09-24.md` records movable studio/dialog chrome
and contrast-corrected logo rendering. The alpha.39 scope below is historical.

Trent requested theme-aware branding, stronger color like TrontSnap, and more
control over frost, contrast and fonts. Eventual parity across his Rust apps was
explicitly deferred. This change is confined to Trontop; TrontSnap and Boxel were
read as references, not modified.

## Controls

- Intensity and frost cover 0 through 100 percent. Frost means panel opacity over
  the gradient, not desktop blur. Dark and light remember independent values.
- Palette keeps four draggable pegs, hex/numeric edits, angle, reverse and even
  spacing. Intensity/frost sit below the ramp, with Background and Through frost
  strips using the same composition as production rendering.
- Appearance adds surface tint and secondary-text contrast strength. Existing
  roundness, row/column bands, hover tint and Extra text contrast remain.
- Type offers Sans, Rajdhani, Rajdhani SemiBold and Monospace. Numeric columns keep
  their monospace family. UI scale resizes text and layout together with a 100%
  reset; it uses app-local egui zoom persistence, separately from theme exports.
- ColorMagic and its 12-roll undo preserve frost, tint, typography, contrast and
  layout. Reset restores both frost values; Revert restores the session theme.
  UI scale remains a separate app preference.

## Color and readability

Previously intensity stopped at 75%, frost stopped at 45%, and the backdrop was
then limited by per-channel bounds. The new backdrop checks the composed panel
against the existing surface/ink luminance envelope. Only the required adjustment
toward black/white is applied; raw peg/accent RGB is never rewritten. Opaque
surfaces use this envelope too, retaining more saturation than the old ceiling.
Light mode uses a bound preserved through interpolation. Quantized panel alpha
is included in previews. Dialogs and menus stay opaque at zero frost.

The existing WCAG-ratio implementation is retained. This is not a port of Boxel's
APCA/OKLCH tonal ladder or a claim of complete cross-app parity.

## Logo and fonts

The generated `assets/branding/trontop-logo-v2.png` supplies the shape. At build
time it becomes 64px/128px alpha masks; runtime needs no PNG decoder or image file.
Theme pegs, positions and angle color the mask. Turning off gradients selects the
primary accent. Frost/intensity do not dim the logo. Chrome, About, empty inspector
and component preview share the mark. The native window/taskbar icon updates on
palette changes with an app-owned cache that survives egui-memory restoration.
Headless contexts emit no native icon commands. Explorer's static EXE resource
uses the original generated colors. The animated tray graph is retained.

Rajdhani Medium and SemiBold are the same files used by Boxel and TrontSnap, with
built-in glyph fallbacks. The [SIL OFL license](https://github.com/google/fonts/blob/main/ofl/rajdhani/OFL.txt)
is in `assets/fonts/OFL.txt`, embedded and visible under Type > Font license.

## Persistence

Theme JSON exports now use version 4. Version 3 and the legacy eight-field code
remain readable; their single opacity migrates to both modes. Version 4 requires
the new fields and rejects unknown fonts. Named libraries share the codec. The
existing settings envelope/key is retained for in-place background loading.
Older executables reject the newer theme schema instead of losing preferences.
Current user settings and the running preview were not replaced during this pass.

## Future shared contract, deferred

Reference sources: `trontsnap/src/theme.rs`, `trontsnap/src/app.rs`, and Boxel's
`crates/boxel/src/ui/theme.rs`, `theme_window.rs` and `color.rs`.

A shared implementation should standardize palette intent, harmonies, per-mode
frost, contrast targets and font roles before sharing code. It should preserve
controls through randomization, match composed previews, bundle licensed fonts
and migrate saved preferences. Trontop's independent accents/four positioned pegs
and the other apps' linked first peg are a product difference to resolve explicitly.

## Verification

The selected `render_theme_controls_visual_pass` uses production UI on an
offscreen GPU for eleven views: zero frost/full intensity, dark/light mode,
new fonts, compact Palette/Appearance/Type dialogs and process/Overview pages.
No native windows, tray icons or OS input are used. New regressions cover old-theme
migration, full-range color/contrast, independent frost, typography metrics,
font/zoom layout and cached logo recoloring. Final results are in CURRENT_STATE.
