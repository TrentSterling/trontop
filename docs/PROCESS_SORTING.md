# Displayed process sorting and tree state (alpha.16)

## Contract

- Tree CPU, GPU, memory, read and write ordering compares the full valid subtree
  totals displayed in each row. Root groups and siblings sort independently;
  children remain attached to their parent. Collapse and filtering do not change
  totals or group order. Flat view still compares individual process counters.
- Partial GPU totals compare by the displayed lower bound. Missing GPU readings
  remain last in either direction, not zero. Ties use the existing PID/direction
  policy. Other columns retain their previous comparison rules.
- Switching Processes/Details retains a sort only when its column is visible.
  Otherwise it returns to CPU descending with a marked header. There are not yet
  separate persisted sort preferences for each page.
- Expanded groups remember nonzero native creation time and Unix start seconds.
  A conflicting observed identity or disappearance clears expansion. Missing
  access does not erase a previously known identity. Continuous unknown-only
  identity is best-effort layout and cannot detect unobserved same-PID replacement.
  Search temporarily reveals ancestor context without changing saved expansion.
- Retention scans the new snapshot once and prunes dead entries. View state is not
  native-action authorization. Original process records and control guards are
  unchanged. No extra Windows queries run in the render thread.

## Verification

The new aggregate-sort regression failed against alpha.15 before implementation:
CPU ascending produced source order `[1,2,4,3,5,6]` where displayed totals required
`[5,6,1,3,2,4]`. It now passes for all five resource columns, both directions,
collapsed/expanded groups, siblings and filtered descendants. A separate test covers
partial/missing/zero GPU groups and PID ties. The existing 2,880-case independent
reference now compares disposable test records populated with recursively computed
totals; production sorting instead borrows indexed totals and clones no records.

Four expansion tests cover temporary access loss, enrichment, contradictory times,
unknown continuity, disappearance, toggles, duplicate-last semantics and churn.
Three production-UI tests click actual headless headers and Tree/List controls,
inspect painted row order in both themes, preserve search/collapse decisions across
refresh, expire reused identities, and check all twenty page/sort combinations.

Local gate: **143 passed, 0 failed, 9 opt-in ignored** (27.32 seconds). Formatting,
strict Clippy and optimized release passed. The selected offscreen pass produced
**57 PNGs** (34.32 seconds); `process-sort-totals.png`,
`process-sort-totals-light.png`, `process-tree-deep.png` and `details.png` were viewed.
These are synthetic fixtures, not live-telemetry accuracy or native drag/FPS tests.

Optimized review EXE: 13,224,448 bytes, version 0.3.0-alpha.16.
SHA-256 `54B051804B52548B4386DB089601655723FFECA8AED54DEDC6A86DA2E2406918`.
Windows-only dependency inspection passed. Clean-machine portability is not proved.
The separate preview was opened only after Trent explicitly requested it; its
path and observed runtime identity are in `CURRENT_STATE.md`.
