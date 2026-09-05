# Alpha.20: renderer recovery and non-blocking UI snapshots

Scope: A20/A22/A25 in `../ASK_LEDGER.md`. Trent resumed with the objective to fix
the app, make it fast/snappy and polish it. This is a stability candidate, not a
claim that the whole ledger is complete.

## Confirmed incident

Alpha.19 PID 280516, launched at 18:27 UTC on 2026-09-05, panicked at
18:29:11.956 UTC. Its failure record identifies `renderer.rs:981:17`, main thread,
optimized release, version 0.3.0-alpha.19. Windows Application Error 1000 at
13:29:12 CDT identifies the same PID/path/version, exception 0xc0000409; WER 1001
followed at 13:29:17. This was not voluntary close or a debug-only issue.

The exact branch is the egui-wgpu index upload: `Queue::write_buffer_with` returns
None, then egui panics. In wgpu 29.0.4 a lost device returns None here; other error
classes normally reach the uncaptured-error handler. Device loss is a supported
hypothesis, not proof of the original driver/memory trigger. The old log lacks
that detail. The scoped System-log query returned no display-driver/resource-
exhaustion events; that does not exclude device-specific faults or memory pressure.

Upstream has an [open matching report](https://github.com/emilk/egui/issues/8450).
The cached 0.36.1 renderer has the same panic branch. No blind upgrade, global
driver change, adapter reset or cross-project patch was applied.

## Changes

- Single-slot sampler mailbox transfers existing allocations with `try_lock`,
  replacing a blocking read and deep process-metadata clone on the UI thread.
  Superseded snapshots are dropped by the worker after publication unlocks.
- Local MIT renderer patch skips failed uploads and starts one replacement-device
  worker. Polling/drop never joins a blocked driver call. Failed creation backs off
  2/4/8/16/32 seconds; repeated immediate device failures have a 1-second minimum
  between attempts, even when creation itself succeeds.
- Live managed textures have a CPU mirror. Partial atlas updates merge; retired
  textures disappear. Surfaces are recreated on existing windows, then fonts/icons
  replay into the new renderer under its queue-submit guard. A failed surface
  recreation keeps replay pending and merges subsequent deltas until it succeeds.
  No process/application/context restart.
- Potentially stale process/service/dialog interactions are suspended during
  recovery. Window controls remain available; selection, theme and telemetry remain
  owned by the same app. Normal provider updates do not enter this exceptional state.
- Typed GPU lost/upload/start/recovered/failed events use the existing bounded
  local log. No driver messages, private application data or dump collection.
- MIT notice embedded in About; no new runtime asset folder.

## Verification (2026-09-05)

- Ordinary suite: **182 passed, 0 failed, 13 ignored**, 28.83 s. Root and vendored
  renderer strict Clippy pass. Formatting passes. Optimized build passes;
  exact EXE identity is recorded in `CURRENT_STATE.md`.
- Offscreen visual matrix: **67 PNGs**, 49.44 s; compact About and Processes inspected.
  No native window/input/tray. This does not finish the page-by-page polish audit.
- Mailbox tests prove transferred allocation identities, coalescing, monotonic
  concurrent updates and immediate return while the publication lock is held.
- Texture tests check partial-update pixels/options/retirement and 1,000 updates
  without accumulation. UI checks retain view/theme and suspend stale actions.
- Optimized fault injection: **3/3 pixel-identical recoveries**, using production
  UI meshes and the production recovery helper. Destroys only the test's offscreen
  device, never an adapter/driver. The first rebuild/readback took **210.72 ms**;
  immediate repeated losses took **888.40 / 1018.02 ms**, intentionally rate-limited.
  Longest UI-side poll **0.088 ms**. Replay storage stayed **524,288 bytes**. Also
  simulates skipped frames before replay and a texture created/retired in that gap.
- Optimized injected setup failure/stall: one attempt despite 1,000 polls, no
  extra worker while blocked; 1,000 polls **17.6 us**, drop **4.6 us**. The test
  releases and waits for its owned worker before finishing.
- Full-UI CPU-only release probe: nine pages, 500/5,000-process fixtures, 16 warm-up
  plus 60 measured hover-changing frames per page. Includes layout/chrome/inspector/
  CPU tessellation. **p95 0.13-1.01 ms**. Not an A/B speedup claim; some larger
  fixtures ran faster in this one mixed-load run. Excludes GPU submit/present,
  native movement, OS scheduling and live-provider acceptance. NOT native 60 FPS.

Commands (never use a blanket ignored-test run):

```powershell
cargo test --offline --release offscreen_device_loss_recovers_production_ui_pixels -- --ignored --nocapture --test-threads=1
cargo test --offline --release offscreen_recovery_failure_and_stall_keep_ui_nonblocking -- --ignored --nocapture --test-threads=1
cargo test --offline --release full_ui_cpu_timing_probe -- --ignored --nocapture --test-threads=1
cargo test --offline render_offscreen_visual_pass -- --ignored --nocapture
cargo clippy --offline -p egui-wgpu --all-targets -- -D warnings
```

## Still unproven

Native swapchain/surface recovery after real device loss, multi-monitor/DPI moves,
tray transitions, actual close latency and mixed-load soak remain open. So does
the original driver/allocation trigger. Unrelated validation or unrecoverable
allocation errors can still abort; permanently unavailable hardware cannot be
guaranteed to recover. This is not arbitrary-egui-app GPU recovery; see the vendor
patch note for native textures/callbacks/external render-state limitations.

Full Task Manager parity, CPU sensors, final theme/visual acceptance and the other
ledger asks remain unchecked. No visible launch, desktop switch, global input,
focus change or process termination occurred. Permission for a separate non-visible
test desktop was requested but not received; the broad fix objective is not that
permission. No running preview should be replaced without a fresh request.
