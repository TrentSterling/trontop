# Trontop renderer patch

Based on the local crates.io `egui-wgpu 0.35.0` sources used by alpha.19.
Upstream: https://github.com/emilk/egui (tag 0.35.0). The upstream dual-license
offer permits the MIT option, selected here. `LICENSE-MIT` is retained and embedded
in Trontop's About window. No global Cargo cache or other application is modified.
The root `[patch.crates-io]` selects this directory.

Local modifications (2026-09-05):

- `renderer.rs`: stop failed staging-buffer uploads without a panic/stale geometry.
- `recovery.rs`: live CPU texture mirror, one non-blocking device-creation worker,
  retry backoff and typed callbacks.
- `lib.rs`: opt-in recovery configuration/events. Off by default.
- `winit.rs`: detect loss, retain deltas, rebuild GPU resources and surfaces on
  existing windows, report recovery after submitting/presenting a frame. Preserve
  dimensions/retry state if surface recreation fails. Keep full texture replay
  pending through surface retries; upload only under the queue-submit guard.
- `Cargo.toml`: license paths and native `pollster` dependency (already in the
  application's dependency graph).

Recovery is for Trontop's managed-texture-only UI. Native texture IDs, GPU callbacks,
custom `WgpuSetup::Existing` and objects retaining a CreationContext render-state
clone are not supported recovery contracts. Trontop does not use them. eframe's
Frame retains its initial render-state clone; this is not an API-wide solution for
arbitrary GPU applications or a verified cross-project patch.

The texture mirror is bounded by live app allocations plus the current frame's
retirements. It is not an upload history. Trontop also bounds its executable-icon
cache. Long-run production memory still needs measurement.

Tests/limitations: `../../docs/RENDERER_RECOVERY.md`.
Related upstream report: https://github.com/emilk/egui/issues/8450 . This patch is
not upstream-endorsed and has not been ported to other projects.
