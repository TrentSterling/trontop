# Daily-use action and UI regressions, alpha.45

This pass advances A04/A07/A16/A25 and the active coverage goal. The requested
End process tree and all-instances controls remain included.

The inspector and Details now display `Suspended` after Trontop's worker reports
a successful owned suspension. Matching requires the exact process creation
identity. A reused PID cannot inherit the label; failed Suspend leaves the
provider state, and failed Resume retains the verified hold. Successful Resume
returns to the sampled provider state. This label describes Trontop's own hold;
it does not infer external suspension from idle CPU readings. Details STATE
sorting uses the same displayed state and updates when an action completes,
without waiting for the next sampler publication.

The compact Details column widths reserve enough space for `Suspended` without
clipping the numeric counters or introducing a horizontal scrollbar at 1000x580.
The first offscreen capture exposed clipping that the original text-presence
check did not catch. The final regression also checks that the galley is not
elided and its bounds fit the cell.

Theme Studio's gradient keyboard editing now retains focus for horizontal and
vertical arrows. Repeated up/down selection previously moved focus to nearby
controls after the first change. Tab and Escape still leave the gradient. The
regression first reproduced this failure, then passed with the focus filter.

Twenty new ordinary regressions exercise these production behaviors:

- Ten daily-action UI checks: owned suspension state, PID reuse, failure state,
  immediate STATE sorting, compact text, original-target priority and affinity
  confirmation, invalid affinity masks, lost identity, typed Run task through
  button and Enter, and Reveal worker failure. Workers are fixture-only.
- Six Theme Studio interaction checks: named Save/Replace/Load/Delete, library
  limits, captured Copy/Import/Revert, disabled editing during preference load,
  gradient keyboard movement/selection/limits/focus exit, and incomplete or
  invalid hex input retaining the last valid color.
- Four System interaction checks: recursive Expand/Collapse all through
  navigation, hidden-private row refusal and unavailable-reason copy, section
  and group copy following explicit reveal, and slow/stopped/missing/aged reads.

The local egui harness rejects native viewport commands. Copy commands are
inspected, never forwarded to the OS clipboard. These checks run without windows,
desktop input, tray operations or real process/service/Startup actions.

```powershell
cargo test --offline app::ui_smoke::daily_actions -- --nocapture
cargo test --offline theme_studio::interaction_tests -- --nocapture
cargo test --offline app::ui_smoke::system -- --nocapture
cargo test --offline app::ui_smoke::daily_actions::render_verified_suspension_status -- --ignored --exact --nocapture
```

The selected offscreen renderer writes three synthetic fixture views under
`target/ui-smoke/process-state-alpha45/`: dark/light inspector and dark Details
at 1000x580. Exact final build, full-suite and measured coverage evidence belong
in `CURRENT_STATE.md` and `COVERAGE.md`. Native desktop, service/Startup acceptance,
sensor decisions, field parity and final release acceptance remain in the ledger.
