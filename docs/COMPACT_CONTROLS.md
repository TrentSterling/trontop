# Alpha.24: compact identities, History and control review

Scope: A14/A15/A16, with no native process-control policy change. The existing-app
redesign skill's audit-first layout checks guided this pass; Tront colors, rounded
surfaces, gradients and density remain intact.

## Reproduced problems

On alpha.23, a 220-character process name displaced the End confirmation action
by 63 logical pixels compared with a short name. Long History names consumed the
space for PIDs and lifetime totals. Both new checks failed before the layout fix.
The short-name 64-processor affinity fixture already kept its footer visible;
this pass does not claim it reproduced a clipped footer there.

## Changes

- End, priority, affinity and service dialogs separate the action heading from a
  fixed-height identity card. Name and PID/service identity have separate lines;
  truncated names retain their full text on hover. Labels remain non-selectable.
- Affinity uses responsive, selected-state CPU tiles and a capped scrolling grid.
  Select all, Current mask, Cancel and Review change stay outside that grid.
  The final confirmation now displays the frozen requested processor ranges and
  count within the process's Windows processor group, not an implied whole-machine
  mask. Sparse/high-bit masks are preserved. Zero masks still cannot be reviewed.
- History uses compact alternating rows with a flexible name track and reserved,
  right-aligned PID, lifetime CPU and total I/O tracks. Numbers stay single-line;
  full row details are available on hover. Summing extreme I/O counters saturates
  instead of overflowing. No fabricated live values or new polling timers.
- Existing same-handle native identity checks, asynchronous dispatch, stale-target
  refusals and confirmation requirements are unchanged.

## Verification

- **209 passed, 0 failed, 13 ignored** (30.06 s); seven new ordinary regressions.
- Strict Clippy PASS (2.07 s), formatting PASS, release build PASS (47.59 s).
- **93 offscreen PNGs** generated (62.99 s). All 14 new compact views reviewed
  across the pass: six dialog types and History in dark/light. Both final affinity
  confirmations were inspected again after adding the requested-CPU summary.
- Dialog bounds cover 1040x640 and 1280x760 logical points, egui scale factors
  1/1.25/1.5/2, short/long/Unicode names, and unchanged action positions. These are
  not native DPI-transition or font-script-coverage tests.
- Local egui input verifies scrolling to the highest CPU bit, selecting/deselecting,
  staging the exact mask/identity, zero-mask refusal, restoring Current mask and
  Cancel. It also checks the final requested-CPU summary. No OS input or native
  action is executed by these new tests.
- Full-name hover checks cover all six dialogs. History tests cover alternating
  long/short names, aligned numeric tracks, and maximum u32/u64 PID/counter text.
- Final optimized UI/CPU-tessellation probe: p95 **0.23-0.98 ms** across all nine
  pages with 500/5,000 synthetic processes. History p95 was **0.395/0.242 ms**.
  This is one local sample, not an A/B speedup, native FPS, GPU-present, or live
  provider measurement. Different fixture sizes ran under different cache/load
  conditions. The probe does not establish the still-open acceptance budgets.

Commands:

```powershell
cargo test --offline --quiet
cargo clippy --offline --all-targets -- -D warnings
cargo fmt --all --check
cargo build --offline --release
cargo test --offline render_offscreen_visual_pass -- --ignored --nocapture --test-threads=1
cargo test --offline --release full_ui_cpu_timing_probe -- --ignored --nocapture --test-threads=1
```

The geometry matrix reuses an egui context within each mode/size/scale and changes
names in place. Rebuilding hundreds of font contexts was unnecessary fixture cost;
the final ordinary suite remains about 30 seconds. This is a harness improvement,
not a claimed native application speedup.

## Candidate and remaining gates

`target/release/trontop.exe`, **0.3.0-alpha.24**, **13,537,792 bytes**, built
**2026-09-05 22:23:54 UTC** from modified e86c06e source.
SHA-256: **`0591BBD15332B9D56CF194CEC5FECDD076C4F977FBDCA4773891AD3EB50319C2`**.
Import inspection lists Windows DLLs only, without a dynamic MSVC CRT. This is not
clean-machine portability proof.

No preview launch/replacement/close, desktop input, focus change, driver install,
native service command or source upload occurred. Native drag, close, surface
recovery, mixed-load soak, full UI audit and Trent's review remain open. This
targeted pass does not complete A14/A15/A16 or the ask ledger. The pending preview,
isolated-desktop and source-upload questions require an actual user response.
