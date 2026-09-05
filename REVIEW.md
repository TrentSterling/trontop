# Alpha.26 review

Review the current app before starting another implementation cycle. Trent called
out the diminishing returns of continued bug hunting on 2026-09-05. No alpha.27
tray changes were made; that investigation is recorded, not implemented.

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

Native drag/close timing and crash-free mixed-load use are not yet verified. CPU
and motherboard sensors, suspend/resume, Startup enable/disable and several Task
Manager data fields remain incomplete. This is a review candidate, not a parity
or finished-release claim. `ASK_LEDGER.md` remains the full completion checklist.

Next action is Trent's feedback on this exact build. Keep the confirmed tray
startup wait and other outstanding asks recorded; do not launch a new cosmetic
or speculative optimization cycle while awaiting that review. The broader goal
is not complete. Source-upload and isolated native-test approval remain separate.
