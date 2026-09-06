# Alpha.28 close-path follow-up

Optimized release is ready at `target/review/alpha28-close/trontop.exe`.
Built, not launched; the existing alpha.26 preview is unchanged.

- Saves share the already-captured UI memory instead of deep-cloning it again.
- Byte-identical settings skip disk access, including lock contention. Changed
  settings still save safely and refuse to overwrite another instance's changes.
- **236 tests passed**, 14 ignored; strict Clippy, formatting and release passed.
- Three new regressions include a headless close gate with busy fixture files.
  Native window/tray teardown and the user's reported delay remain unmeasured.

The alpha.27 Graphs page is included. Exact identity and limitations are recorded
in `docs/CURRENT_STATE.md` and `docs/SETTINGS_PERSISTENCE.md`. No automatic preview
replacement, desktop test or upload. A21 remains open; this is a focused handoff.

## Previous alpha.27 graph-wall review

Trent's alpha.26 feedback explicitly requested this focused addition (A32).
The new optimized build is ready, **not launched**; current windows are untouched.

- EXE: `target/review/alpha27-graphs/trontop.exe`, **0.3.0-alpha.27**.
- New Graphs sidebar entry / Ctrl+9, Lines/Bars and category filters.
- One continuous grid of usage, temperatures, watts, clocks, fan/VRAM, physical
  disk activity and network traffic. Overview stays intact; personal themes apply.
- **233 tests passed**, strict Clippy/formatting/release build passed, six new
  offscreen graph images generated and reviewed. These are synthetic fixtures,
  not a native responsiveness or closure measurement.
- CPU temperature is still missing its provider. Slow close is still unmeasured;
  source review confirms visible close can wait for the background settings save.

Exact hash, sources, behavior and limits: `docs/GRAPH_WALL.md`.
Review this graph layout next. No automatic launch, push, tray rewrite or unrelated
optimization investigation. A32, A21 and A10/D01 remain review/open as documented.

## Previous alpha.26 review (historical)

Trent called
out the diminishing returns of continued bug hunting on 2026-09-05. No alpha.27
tray changes were made; only the subsequently requested graph wall was implemented.

## Build opened for Trent

- Version: **0.3.0-alpha.26**, optimized release, rebuilt from clean **b79708f**.
- EXE: `target/review/alpha26-b79708f-clean/trontop.exe`.
- Built: **2026-09-05 23:37:40.445 UTC**, **13,598,208 bytes**.
- SHA-256: `5CC9CF248335353C821237229EFAED2E97BC9A082D8C11B8A2DA7CED9A82DF49`.
- Opened **23:38:40.145 UTC**, PID **242180**, HWND **8192130**. Initial input-idle
  and responding checks passed; those do not prove smooth dragging or stability.
- Five older Trontop previews were stopped on explicit permission. Their files
  were retained. Other applications were untouched. No global input was injected.

## Changes since the crashed alpha.19 preview

- Graphics-device recovery candidate with retained application state and replayed
  drawing resources, plus non-blocking sampler publication.
- Native process/shell actions and recoverable-event logging moved off UI callbacks.
- More readable custom palettes, compact dialogs, aligned History values and
  usable affinity controls with an explicit selected-CPU summary.
- Background settings load/save, legacy theme migration, staged writes and clear
  unsaved-close choices. Four-color themes and named palettes remain supported.

The underlying source passed **225 ordinary tests**, strict Clippy/formatting and
the release build. **101 offscreen PNGs** were generated with the changed states
reviewed. Today's final rebuild changes build identity from modified 3b80efb to
clean b79708f, not application source. Exact evidence: `docs/CURRENT_STATE.md`.

## Still open

### One passive runtime check

At **23:43:02.624 to 23:43:34.021 UTC**, 31 read-only samples of that exact
PID/path/start-time identity found **31/31 responding**, HWND **8653022** throughout,
and **0.326% whole-machine CPU** on 24 logical processors. Process CPU time advanced
from 26.984375 to 29.437500 s over 31.394595 s. No test input or build ran during
this check; current user interactions/workload were not classified.

Working set increased from **250.4 to 306.4 MiB** (peak **307.4 MiB**); private bytes
from **516.4 to 572.5 MiB** (peak **573.4 MiB**). Threads stayed at **59**; handles
ranged **1173-1178**, ending at the initial **1175**. The memory increase is recorded,
not explained away as caching and not diagnosed as a leak from a 31-second sample.
This is neither a 60-minute soak nor a drag/frame-time/close measurement. No extra
implementation or repeated monitoring cycle was started from these observations.

Native drag/close timing and crash-free mixed-load use are not yet verified. CPU
and motherboard sensors, suspend/resume, Startup enable/disable and several Task
Manager data fields remain incomplete. This is a review candidate, not a parity
or finished-release claim. `ASK_LEDGER.md` remains the full completion checklist.

Next action is Trent's feedback on this exact build. Keep the confirmed tray
startup wait and other outstanding asks recorded; do not launch a new cosmetic
or speculative optimization cycle while awaiting that review. The broader goal
is not complete. Source-upload and isolated native-test approval remain separate.
