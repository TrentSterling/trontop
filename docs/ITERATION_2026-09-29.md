# Alpha.42: responsiveness, Performance detail and compact polish

Trent requested another Trontop iteration and selected all three proposed areas:
responsiveness/stale readings, UI polish, and hardware/graphs. This pass advances
A06/A07/A15/A22/A25; it does not close the broader completion ledger.

## Changes

- Reproduced a frozen Live badge: with no new sample and no input, the real UI
  returned `Duration::MAX` for its next repaint. Provider and graph freshness now
  schedule only their remaining state transitions. Expired readings stop scheduling;
  a new sample restores the deadline. Minimized/occluded views skip these timers.
  The scheduler compensates for egui's frame-time anticipation to avoid immediate
  repaints just before expiry. Retained values and history are preserved.
- CPU Performance exposes the measured average clock beside the fastest/slowest
  readings, plus physical cores/logical processors and uptime. Compact app views
  shorten the extrema labels; very narrow detail panes stack the average above
  them. The plot reserves space for the full summary, including at 1000x580.
  Missing clock/topology/uptime data stays missing rather than using defaults.
- Network Performance shows the actual adapter description, type, link speed,
  connection state and MTU beside its existing traffic graph. The background System
  inventory supplies these fields; it is polled on the device-selection frame.
  Exact interface matching prevents metadata from another adapter being substituted.
  Slow/failed inventory retains its fields with a Cached label and original read time
  on hover. Private addresses are omitted here; System details opens the full Network
  section with the existing privacy controls. Network-only views leave the live
  sensor bridge at its idle cadence.
- Selected interfaces and volumes follow their name/mount identity through sampler
  reordering. Removal shows No longer present until another device is selected;
  subsequent refreshes cannot silently select the device occupying the old index.

## Evidence

Before changing the sampler UI, the ordinary baseline passed: 429 passed,
0 failed, 48 ignored. The separately selected read-only production sampler probe
returned 60 samples in 60 seconds with 490 processes:

```text
SAMPLER_PROBE after worker interval between samples: n=59 p50=1.000s p95=1.001s max=1.006s
SAMPLER_PROBE after observed gap between samples: n=59 p50=0.997s p95=1.094s max=1.433s
SAMPLER_PROBE after system+process refresh: n=60 p50=0.064s p95=0.130s max=0.477s
```

This is a short read-only sampler observation, not native interaction or a soak.
The sampler implementation is unchanged in this pass.

The initial real-data gauntlet wrote 157 PNGs in 36.6 seconds, accepted 21 samples
in its 20-second warmup, had all 11 specs sections ready, and reported a 1.06-second
worst accepted-sample gap. Overview, compact Network Performance, Appearance,
Services and normal CPU Performance were inspected before the detail changes.

Eight additional ordinary regressions cover input-free expiry/recovery/no history
fabrication, finite independent provider deadlines, minimized/occluded versus
unfocused scheduling, interface-matched metadata and compact light/dark cached
geometry, private-value omission, local System navigation, missing inventory, and
interface/volume selection reordering/removal, and the complete CPU summary staying
visible at 1000x580/1040x640. The existing CPU clock regression now checks all three
clock fields across live/cached/missing states and three detail-pane widths.

Final checks after the compact CPU correction:

```text
cargo fmt --all --check: PASS
cargo test --offline --quiet: 437 passed; 0 failed; 49 ignored
cargo clippy --offline --all-targets -- -D warnings: PASS
git diff --check: PASS
```

The final real-data gauntlet wrote 157 PNGs in 37.5 seconds, accepted 21 real
samples during the 20-second warmup, had all 11 specs sections ready and reported
a 1.12-second worst accepted-sample gap. Captures live under
`target/ui-smoke/iteration-20260929/final`.
The compact and normal CPU/Network Performance pages, compact All cores and light
Overview were inspected; the compact CPU correction keeps its average/extrema,
topology, uptime and source expander above the fold.

The separately selected network visual test wrote eight synthetic fixture PNGs
under `target/ui-smoke/iteration-20260929/network-fixtures`, all inspected:
complete/cached/loading/unmatched in light and dark modes. The selected CPU-clock
visual test wrote four synthetic fixture PNGs under `target/ui-smoke`, all
inspected: normal, compact cached, compact light missing clocks, and wide Overview.
Fixture images are QA material, not public marketing telemetry.

`cargo build --release --offline` PASS after the final source changes. Review copy:
`C:/trontstack/trontop/target/review/alpha42-iteration/trontop.exe`.
The copy matches `target/release/trontop.exe` by SHA-256. PE ProductVersion and
FileVersion both report `0.3.0-alpha.42`; size is 17,635,840 bytes. SHA-256:

```text
12580232E9109B400C764349B3F9696EB452CB0E6F6709B68AD33208EAFEF7CC
```

Source is local and uncommitted on `feat/system-specs`, based on
`c8ed88f5ec57f79be717151ab44c43826b4a4e5a`. This is a local review executable,
not a published release or a Windows CI artifact.

## Native boundary

All verification in this pass is headless, offscreen or read-only. No app window
is launched, replaced, focused, moved or manipulated. No user preference file,
process/service control, native picker, tray interaction, driver installation,
commit, upload or public release is part of this pass. Native drag/close/tray/soak,
clean-machine acceptance and remaining sensors/data parity stay open in the ledger.
