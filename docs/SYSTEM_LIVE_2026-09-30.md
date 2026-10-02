# System live values and private summaries (alpha.48)

This continues A07/A16/A18/A25/A33. It preserves the requested End process tree
and all-instances controls, available in this candidate and alpha.47.

## Behavior

System uses the GPU sensor timestamp as well as its cached flag. Native readings
are fresh through three seconds; older or undated retained values display
`(cached)`. CPU average/fastest/per-processor clocks and Windows commit charge
follow their provider health, including stopped-sampler expiry and failed queries.
Per-processor identity includes both processor group and number.

The resolver carries an explicit cached bit. Cached temperatures have no fresh
color band; hover text says `Cached:` and JSON live payloads add `cached: true`.
Fresh readings retain the existing payload. Drive cached readings use the same
metadata, keeping their existing composite-sensor preference and fallback.

A hidden private SummaryLine now suppresses its entire live payload, including
key, value, source and unavailable reason, in text/JSON and on the page. The CPU
headline override follows the same screen privacy gate. Revealing values or
opting into private file content restores them explicitly. Raw drive interface
paths remain absent from reports even when private values are included.

## Evidence and scope

Nine new ordinary regressions cover resolver-to-summary/row text and JSON
consistency, GPU age/cache/recovery across all six metrics, CPU group identity,
clock/commit failure and recovery, missing counters, exact network aliases,
bridge RPM/voltage/watts/MHz/percentage units with shared labels and stable IDs,
drive composite/fallback/error/disconnection/recovery, private text/JSON and
masked/revealed production UI rows, CPU overrides and hover privacy.

Five targeted tests failed against the previous implementation before fixes.
Their logs are preserved under `target/review/alpha48-system-live/before-*.log`.
They reproduced private text-key leakage, private UI-value leakage, GPU expiry,
CPU clock expiry and commit expiry. No real provider or personal record is
needed to reproduce these failures.

Final full suite: **528 passed, 0 failed, 57 ignored**. Formatting, strict
all-target Clippy and optimized release PASS. The final instrumented suite also
passes 528 tests, all 24 exact read-only probes and 10 analyzer checks.
Production unique file lines: **23,743/25,663 (92.52%)**, 1,920 uncovered,
zero mixed, 23,536 test-only lines and 40 raw profiles. Final run:
`target/coverage/runs/20260930-071924-165/`. All 187 source/build inputs and
exact test/probe/tool receipts independently verified by the review verifier.
The first full run found an assertion matching the public CPU meter outside
its private summary row. It was scoped to that row, a Clippy boolean simplified,
and all final validation repeated afterward. The failed measurement is not used.

Selected visual command:
`cargo test --offline app::ui_smoke::system::render_system_live_freshness_review -- --ignored --exact --nocapture`.
Three fixture-only 1280x900 views inspected in `target/ui-smoke/system-live-alpha48/`:
Summary cached dark, Graphics cached light and RAM cached dark. Cached labels fit
and retained temperatures have no fresh coloring. These are synthetic test data,
not public screenshots or proof of native provider behavior.

Review EXE: `target/review/alpha48-system-live/trontop.exe`, File/Product version
`0.3.0-alpha.48`, 17,792,512 bytes, SHA-256
`F95288E2FFE46A2C3B8A8AD6A9A19303750F8B907F9B9A9A4E99D75F1D01D83F`.
Release/review match; alpha.42 through alpha.47 review hashes remain preserved
and verified. `receipt.json` and `verify-receipt.py` sit beside the candidate.
Post-run audit finds zero test processes and zero owned Startup fixture leaves.
Trent received this candidate link after the full ordinary/Clippy/release pass.
Only owned test fixtures and the fixed read-only probe list are used. No native
window/input, service command, real Startup change, installation or publication.
CPU/motherboard provider decisions and the full ledger acceptance gates remain
open; production source-line coverage does not close those gates.
