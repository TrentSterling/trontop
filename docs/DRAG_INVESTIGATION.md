# Window drag investigation: parked, not fixed

Checkpoint: September 4, 2026. Trent chose to save findings and switch projects.

September 5 alpha.20 stability update: the later alpha.19 disappearance was a
confirmed renderer panic, not a verified drag-loop failure. See
`RENDERER_RECOVERY.md`. The current local renderer configuration inherits
`SurfaceConfig::HIGH_THROUGHPUT` (AutoVsync, desired frame latency 2). The
`LOW_LATENCY` note below describes the earlier investigation, not alpha.20's
current default. No new native drag measurement or cross-project fix is claimed.

## Report and current conclusion

Later update (September 4, about 23:06): Trent is now confident dragging feels
noticeably smoother, while still asking whether it could improve further. Record
that positive subjective feedback without turning it into a measured 60 FPS claim.
A read-only process check confirmed PID 62220 was already running the older
`target/release/trontop.exe`, not a debug build. The alpha.2 UI review build uses a
separate target directory so that instance stays untouched. There is still no
validated cross-project drag patch or controlled native before/after measurement.

Trontop lags while the whole window is moving, not merely at the beginning of a drag.
Terminal and Explorer look smoothly 60 FPS to Trent on the same desktop. Synthetic
dragging sometimes looked smoother than his real mouse. Trent briefly thought it was
fixed, then confirmed the problem persists. There is NO validated fix, and no drag
change was applied to Boxel or any other project.

Do not attribute this generally to Rust, egui, a debug build, or the machine's power.
The test executable was a release build. The CPU tray fix is separate from dragging.

## Verified code path

- Trontop uses eframe/egui 0.35.0, winit 0.30.13, and the WGPU renderer.
- `src/app.rs::custom_chrome` uses `Sense::click_and_drag`, then sends
  `ViewportCommand::StartDrag` when `drag_started()` becomes true.
- egui-winit checks window focus, then calls `winit::Window::drag_window()`.
- The Windows winit implementation hands off to native caption dragging via
  `WM_NCLBUTTONDOWN` and `HTCAPTION`. Trontop does not manually reposition the
  window every frame with `OuterPosition`.
- egui-winit 0.35.0 `on_window_event` includes `WindowEvent::Moved` among events
  that unconditionally return `repaint: true` (local source around line 498).
- egui-wgpu's default `SurfaceConfig::LOW_LATENCY` uses `AutoVsync` and a desired
  maximum frame latency of one on Windows.

Working hypothesis, NOT a proven cause: move-triggered app redraws and GPU present
waits may interfere with the native move loop. Investigate this before replacing the
chrome or introducing manual cursor-following logic. A click/drag threshold can affect
drag start but does not explain sustained movement lag by itself.

Official references:

- https://docs.rs/egui/0.35.0/egui/viewport/enum.ViewportCommand.html
- https://docs.rs/winit/0.30.13/winit/window/struct.Window.html#method.drag_window
- https://github.com/emilk/egui/issues/5037 (historical WGPU input latency report;
  not proof this is the same bug or that its old remedies apply)

## Measurements and their limitations

A synthetic 125 Hz-ish cursor trajectory on the default backend produced 220 sampled
intervals, 110 observed window-position changes, mean cursor/window anchor error
14.85 pixels, and maximum 24 pixels. This is NOT a presented-frame FPS measurement.
It excluded the first 20 samples, but did not compensate for the initial drag anchor.
Other automated attempts did not actually move the window during the measured interval.
Do not use them as before/after evidence.

A separately launched process requested `WGPU_BACKEND=dx12` only through its child
environment. No machine-wide environment or driver settings were changed. The renderer
identity was not logged, and its automated trajectory was invalid, so there is no valid
backend A/B result. The last such app instance was PID 62220 during this session; never
reuse that PID later without verifying process identity.

`scripts/measure-window-drag.ps1` is a PASSIVE recorder: it observes foreground PID,
left-button state, cursor coordinates, and the target window rectangle for up to
60 seconds. It injects no input and changes no window state. Its first 20-second run
captured no sustained drag and exited with an explicit failure. No conclusion follows.
Its gap statistics include pauses; anchor deviations can be skewed by snapping or
changing direction. It does not measure DWM presentation or actual mouse polling rate.

## Safety rule from Trent

STOP desktop-wide mouse/keyboard injection. An automated test almost closed another
Claude session doing day-job work. Do not minimize, restore, move, click, activate, or
steal focus from windows for testing on the working desktop. The temporary
`tmp/trontop-desktop-qa.ps1` now rejects its mutating actions. Old `click-trontop.ps1`
and `keys-trontop.ps1` helpers must not be used either.

Future automated GUI tests must be headless or run in an explicitly approved isolated
desktop/VM. Do not silently introduce another visible self-driving test window. Passive
measurement can be scheduled only when Trent is ready to perform the drag himself.

## Resume sequence

1. Keep normal desktop input untouched. Finish a headless egui harness with fixed
   TEST-ONLY snapshots, real layout execution, and assertions for text wrapping and
   bounds. eframe 0.35 exposes `CreationContext::_new_kittest` and
   `Frame::_new_kittest`; inspect availability before using. Avoid constructing native
   tray/sampler workers in headless fixtures.
2. Reproduce dragging in an isolated desktop or collect a user-driven passive trace.
   Record backend, monitor refresh, real mouse polling, native move-message cadence,
   UI CPU time, and present/acquire waits. Compare native frame versus custom chrome
   and move-only versus resize. Do not change NVIDIA/global settings.
3. Test one narrowly scoped candidate, such as avoiding redundant repaint on a pure
   position change or changing presentation behavior. Preserve telemetry refresh,
   input, DPI changes, snapping, resize, and hidden-window behavior.
4. Only after a measured improvement, inventory each app's actual backend and framework
   version, acquire its repo lease, read its instructions, and port the compatible fix.
   Read-only searches found shared chrome helpers in Boxel and TrontEQ, plus a separate
   focus/drag retry helper in TrontSnap. Photochop needs deeper inspection. Nothing
   outside Trontop was edited.

## Other work to resume

The Users wrapping defect and broader tray meter have been implemented and release-built.
Column/row bands and improved spacing are in progress. A final source-only follow-up
fixes centered name/header labels, with two headless geometry/input tests; it is not
release-built or visually checked yet. Compact Performance details still need an
overflow pass. Stronger Tront
branding and a unified vector icon set are requested, not implemented in this checkpoint.
Do not use emoji as icons. The later alpha.2 follow-up adds a safe headless UI harness
and Performance scrolling; see `HEADLESS_QA.md` for actual coverage and limitations.

The private repository exists and baseline CI run 33940457527 passed. No alpha tag or
release was published. Re-run CI on the newer local checkpoint before publishing its
single portable EXE with checksum and explicit known limitations. Do not make the repo
public or claim the broader alpha-exit gates have passed.
