# Coverage toward the paused completion goal

Trent's goal, paused at his request on 2026-09-30, is to make Trontop amazing with 100% coverage. Passing the
ordinary suite or rendering every page does not establish that goal. Product
acceptance remains in `ASK_LEDGER.md`; measured source execution is tracked here.

Current checkpoint: alpha.51, **93.34%**, 1,739 missed.
Read [RESUME_ANCHOR.md](RESUME_ANCHOR.md) before resuming and
[the complete remaining-line inventory](COVERAGE_REMAINING_2026-09-30.md).
Historical ACTIVE records below describe earlier checkpoints.

## Reproducible baseline

```powershell
./scripts/measure-coverage.ps1
# On the reference hardware, add only the 24 reviewed read-only native probes:
./scripts/measure-coverage.ps1 -ReadOnlyProbes
```

The script uses the active Rust toolchain's matching LLVM tools, compiles with
`-C instrument-coverage` in a separate target directory, and runs only ordinary
tests. It never executes a blanket ignored-test run. Reports are isolated under
`target/coverage/runs/<UTC timestamp>`; `target/coverage/latest-run.txt` names the
latest passing run. Summary, complete LLVM JSON/LCOV, HTML, raw/indexed profiles,
the exact copied instrumented binary and a toolchain/source receipt remain
available. The receipt hashes every application/build input, checks that source
stays unchanged throughout the measurement, and identifies the test object and
scope analyzer. Release/review artifacts are preserved.

The initial LLVM denominator includes every compiled application `src/*.rs` file,
including inline tests and test-only modules. It has no exclusions for untested
production code. This mixed-source baseline is explicitly not a production-only
percentage, branch coverage or proof of feature acceptance. Test-only execution
is distinguished in `production-lines.json` and `production-summary.txt` by the
standalone `scripts/coverage-scope` analyzer. It parses Rust syntax with Syn;
only test-required cfg scopes, test functions and their transitive modules are
test-only. Production functions called from tests remain production. A nested
test statement or test-only struct field cannot exclude its surrounding production
code. Shared lines are reported separately; unknown module paths or conditional
scope attributes fail measurement instead of guessing. Ten fixture checks cover
these boundaries, missing production coverage and report validation.

The production metric uses unique file/line entries from LLVM LCOV `DA`, counted
once across overlapping functions and instantiations. LLVM's summary/JSON/LCOV
`LF` and `LH` totals aggregate function-level source lines and can differ. The
analyzer verifies those summary receipts agree and preserves both metrics. The
raw function/region summary remains available; production line coverage is not
branch coverage, every generic instantiation, or feature acceptance.

Vendor renderer patch
coverage, opt-in read-only/offscreen probes, and authorized isolated native gates
also need their own evidence; none is silently considered covered by unit tests.

`-ReadOnlyProbes` resolves a fixed allowlist to exact ignored-test names and
records each successful probe. It reads hardware/inventory/sampler data and the
test EXE's own icon; it sends no service commands, runs no native tray or desktop
automation and creates no application window. Native stdout/stderr are captured
separately with no console window; a probe's real exit code determines success.
No blanket ignored-test run is used. The NVIDIA and reference-machine checks
make this option hardware-specific, so generic CI uses the ordinary suite only.

The manual `.github/workflows/coverage.yml` prepares locked dependencies, selects
Rust 1.93.0 with matching LLVM tools, measures ordinary tests and retains receipts
as artifacts. It has been authored and parsed locally; no remote run or upload has
been performed by this iteration.

The Windows/static-CRT instrumentation smoke and first mixed-source baseline
succeeded with installed Rust 1.93.0 / LLVM 21.1.8 tools. Baseline ordinary suite:
437 passed, 0 failed, 49 ignored; raw LLVM summary line coverage 80.92%, region
coverage 81.67%, function coverage 80.63%. The first extended attempt stopped at
PowerShell's native stderr handling, not a failing probe assertion; it has no
passing final receipt.

## Verified alpha.42 measurement, 2026-09-29 local / 2026-09-30 UTC

This historical report captures alpha.42 source. Its exact instrumented object
and verified review executable in `CURRENT_STATE.md` are preserved. Alpha.43's
process controls change the application source and require a fresh measurement.

| Scope | Ordinary suite | Ordinary suite + 24 read-only probes |
| --- | ---: | ---: |
| Production unique file lines covered | 18,955 / 24,014 (78.93%) | 21,661 / 24,014 (90.20%) |
| Raw LLVM line summary (includes tests) | 80.92% | 88.32% |
| Raw LLVM region summary (includes tests) | 81.67% | 88.72% |
| Raw LLVM function summary (includes tests) | 80.63% | 88.91% |

Both production breakdowns use the final corrected scope analyzer: 19,261
test-only executable lines, zero mixed lines, 92 production files and 44 test-only
files. The first provisional struct-field classification has been corrected in
the baseline report as well. No production exclusion is used to increase coverage.

Passing extended run:
`target/coverage/runs/20260930-012709-677/`. The application suite passed 437 tests
(0 failed, 49 ignored); all 24 exact read-only probes passed individually, and the
analyzer's 10 tests and strict Clippy passed. Format, PowerShell AST, workflow YAML
and diff checks passed. The CI workflow has not been run remotely.

The receipt was independently checked against the copied test binary, measurement
script, all 163 hashed source/build inputs, every probe log and the ordinary-test
log. Windows PowerShell writes its Tee-Object log as UTF-16; the verifier detects
that BOM. Probe logs, JSON and text reports use UTF-8 without BOM.

- Instrumented test EXE SHA-256:
  `62C85DA2005BDFE55946F95F0645C9E0E8A781B9ACB529F2908DFF6166A48622`.
- Application/build source fingerprint:
  `7F7A1B50DA9CCCCD02FB050BE52467E246B2C0CC119991F98EE3162E32FB6772`.
- Complete per-file gaps: `production-lines.json`; ranked gaps:
  `production-summary.txt`; browsable raw evidence: `html/index.html`.

There are **2,353 unexecuted production source lines**. Large remaining groups
include UI actions, native failure/optional-provider paths, export dialogs, native
tray/application initialization and service commands. The local renderer patch
has its own tests but is outside this application-source percentage. Uncompiled
non-Windows paths are not covered by the Windows measurement. Line coverage cannot
close field parity, sensor decisions, isolated native interaction, exact-candidate
review, clean-machine or soak requirements. The 100% completion goal remains active.

## Verified alpha.43 measurement, 2026-09-29 local / 2026-09-30 UTC

Final process-control source is measured at
`target/coverage/runs/20260930-022413-980/`: **455 ordinary tests, 0 failures,
52 ignored**, all 24 exact read-only probes and all 10 analyzer checks pass.
Production unique source lines: **22,165/24,569 (90.22%)**, with **2,404 uncovered**
and zero mixed lines. Test-only lines: 20,051. Raw LLVM summaries include tests:
line 88.34%, region 88.78%, function 88.92%. These are measured separately from
alpha.42 because the denominator now includes the new production controls.

The exact instrumented object, all 171 source/build inputs, script/analyzer hashes,
ordinary-suite log and each exact probe log were independently verified against
the receipt. The optimized review copy is separately hashed in `CURRENT_STATE.md`.
Native action tests own their hidden disposable children; the extra 24 probes are
read-only. No native window, global input, real-session process termination,
service command or remote workflow was exercised by this measurement.

- Instrumented test EXE SHA-256:
  `AB5C2F233C9A9568DD02B65A525024A23891E473F9C22EB238B829E7974FEF87`.
- Application/build source fingerprint:
  `87BC0BDFC29B6D25B4A5359294AFE478E3EB40B7CC8CB91FCA109FB0D23408CA`.
- The earlier alpha.43 pre-layout-adjustment report is preserved at
  `target/coverage/runs/20260930-021823-956/`; use the final run above for current
  source. The alpha.42 report also remains historical evidence, not current coverage.

Source execution is still below 100%. Startup controls, optional/native failures,
export dialogs, application/tray initialization, sensor decisions and authorized
native/release acceptance remain distinct work. The persistent goal stays active.

## Verified alpha.44 measurement, 2026-09-29 local / 2026-09-30 UTC

Final Startup source and corrected offscreen capture are measured at
`target/coverage/runs/20260930-043043-870/`: **476 ordinary tests, 0 failures,
53 ignored**, all 24 exact read-only probes and all 10 analyzer checks pass.
Production unique source lines: **23,223/25,522 (90.99%)**, with **2,299 uncovered**,
zero mixed lines and 20,958 test-only lines. Raw LLVM summaries include tests:
line 88.99%, region 89.47%, function 89.77%. There are 40 raw profiles.

All 181 application/build inputs, the exact copied instrumented object, script
and analyzer hashes, ordinary-suite log and each exact probe log were independently
verified. The new native Startup tests create only exclusively owned private
HKCU registry keys and owned fixture files; sandbox registry access required a
scoped exception. Real Startup records, desktop, tray and service commands stay
outside this measurement. The optimized review EXE and verified receipt are in
`target/review/alpha44-startup-controls/`.

- Instrumented test EXE SHA-256:
  `23B6AAFF77B2CDAF091C4E4DE8919998B6E53C4910A89B504C98376CA5154AF0`.
- Application/build source fingerprint:
  `8A00638E4FD37677D9A1000CDAAC41648CFD24C9863F8BF4917210FFE146F2F7`.
- Earlier alpha.44 run `20260930-042346-713/` is historical pre-capture-fix evidence:
  23,228/25,522 (91.01%). Final coverage has six fewer Windows inventory helper
  lines and one extra tray-worker fixture publication line, a net difference of
  five. The per-file delta was independently checked.
  The final source receipt above is authoritative; reports are not merged to
  inflate coverage or retried to choose the higher percentage.

The goal remains ACTIVE. Unexecuted UI/native failure paths, export picker,
application/tray initialization and service commands remain source gaps. Real
Startup/sign-in acceptance, field parity, sensor decisions and native/release
acceptance remain separate gates. The local renderer patch and uncompiled
non-Windows code are outside this Windows application-source percentage.

## Verified alpha.45 measurement, 2026-09-30

Twenty new ordinary regressions exercise daily actions, Theme Studio and System
interactions. They exposed and repaired verified suspension state/STATE sorting,
compact status clipping and gradient arrow-key focus. See
`DAILY_CONTROLS_2026-09-30.md` for tested behavior and commands.

Final exact-source run: `target/coverage/runs/20260930-051951-393/`.
**496 ordinary tests, 0 failures, 54 ignored**, all 24 exact read-only probes and
10 analyzer checks pass. Production unique source lines:
**23,434/25,560 (91.68%)**, **2,126 uncovered**, zero mixed and 21,812 test-only
lines. Raw LLVM lines/regions/functions include tests: 89.48% / 89.94% / 90.05%.
There are 40 raw profiles. The previous alpha.44 report remains historical;
these reports are not combined. No production exclusions are used.

All 183 application/build inputs, copied instrumented object, measurement tools,
ordinary-suite log and each exact probe log independently checked by
`target/review/alpha45-daily-controls/verify-receipt.py`. Format, strict all-target
Clippy and release pass. The review artifact's receipt includes its release hash
and the three inspected compact PNG hashes. Post-run audit finds no test
executable running and zero owned Startup registry leaves.

- Instrumented test EXE SHA-256:
  `763131951DD406488F5096EEAD3C8527A9E075EE653B3FBB2AD8BED8F2CC9243`.
- Application/build source fingerprint:
  `DDA72E3153562656452F4F5148ECF6B6296F3721E234329D0FF1114BFF3CB436`.

The new interaction checks reduce System gaps from 97 to 29 lines. App gaps are
158 lines; remaining leading groups include Overview (78), CPU provider (74),
board provider (69), tray (61), native sensor bridge (60), export picker (57)
and WMI (57). The complete remaining list is preserved in `production-lines.json`
and `production-summary.txt`, alongside HTML/LCOV.

The goal remains ACTIVE. This Windows application-source line metric does not
include uncompiled non-Windows paths or the vendor renderer. Branch coverage,
optional-provider/native error paths and the broader product/native/release
acceptance gates remain open. New source work requires a new measurement.

## Verified alpha.46 measurement, 2026-09-30

Twelve new ordinary native/UI regressions exercise owned shared-memory producers,
malformed/reserved/partially committed pages, WMI row conversion, stopped-provider
expiry/recovery, invalid temperatures and cached multi-adapter GPU data/history.
They repaired stale core chips, f64-to-f32 overflow and the uncommitted-page read
assumption. Behavior and commands: `SENSOR_GUARDS_2026-09-30.md`.

Final exact-source run: `target/coverage/runs/20260930-060006-932/`.
**508 ordinary tests, 0 failures, 55 ignored**, all 24 exact read-only probes and
10 analyzer checks pass. Production unique source lines:
**23,567/25,596 (92.07%)**, **2,029 uncovered**, zero mixed and 22,242 test-only
lines. Raw LLVM lines/regions/functions include tests: 89.72% / 90.18% / 90.31%.
There are 40 raw profiles. Earlier reports remain historical and are not combined.
No production exclusions or selection of a higher repeated run are used.

All 185 application/build inputs, copied instrumented object, measurement tools,
ordinary-suite log and each exact probe log independently checked by
`target/review/alpha46-sensor-guards/verify-receipt.py`. Format, strict all-target
Clippy and release pass. The review receipt includes the release hash and three
inspected sensor PNG hashes; alpha.42/43/44/45 review hashes remain verified.
Post-run audit finds zero test processes and zero owned Startup registry leaves.

- Instrumented test EXE SHA-256:
  `6C17EB239EEAA2AA20FEB013BE2E863C5E02EA4146D0F3CE546756F2C33DB9E9`.
- Application/build source fingerprint:
  `AE86010845497FE54F7BCD38D75F10BFF3476AFB656BEBB9BAE5A327FA0E0F17`.

Native bridge gaps fall from 60 to 13 lines; Sensors from 49 to 20. The production
denominator adds 36 lines and covered lines increase by 133 against alpha.45.
Of these additional covered lines, 119 are in the edited files and 14 in unchanged
WMI/worker/Windows-metrics paths that vary with native fixture/probe execution.
The per-file delta was independently checked; the final receipt is authoritative.
Largest remaining groups include App (158), Overview (78), CPU provider (74),
board provider (69), tray (61) and the native export picker (57). Full gaps remain
in `production-lines.json`, `production-summary.txt` and HTML/LCOV.

The goal remains ACTIVE. This is compiled Windows application-source line
coverage. Branch coverage, the vendor renderer, uncompiled non-Windows paths,
real optional providers and product/native/release acceptance need distinct
evidence. D01 remains open; no provider was installed or started by this pass.

## Verified alpha.47 measurement, 2026-09-30

Eleven new ordinary regressions cover Overview and production plots. Six display
failures reproduced before fixes: unavailable-bridge retention, temperature
overflow, suppressed CPU gaps, unmarked retained VRAM, remote tooltip samples
and missing lower-bound markers. Other checks exercise navigation, grouped
selection, core hover, partial totals, multi-GPU states and line/fill gaps.
Behavior and commands: `OVERVIEW_GRAPHS_2026-09-30.md`.

Final exact-source run: `target/coverage/runs/20260930-064051-633/`.
**519 ordinary tests, 0 failures, 56 ignored**, all 24 exact read-only probes and
10 analyzer checks pass. Production unique source lines:
**23,650/25,621 (92.31%)**, **1,971 uncovered**, zero mixed and 22,831 test-only
lines. Raw LLVM lines/regions/functions include tests: 89.84% / 90.29% / 90.51%.
There are 40 raw profiles. Earlier reports remain historical and are not combined.
No production exclusions or selection of a higher repeated run are used.

All 186 application/build inputs, copied instrumented object, measurement tools,
ordinary-suite log and each exact probe log independently checked by
`target/review/alpha47-overview-graphs/verify-receipt.py`. Format, strict all-target
Clippy and release pass. The review receipt includes its release hash and three
inspected final PNG hashes; alpha.42/43/44/45/46 review hashes remain verified.
Post-run audit finds zero test processes and zero private Startup fixture leaves.

- Instrumented test EXE SHA-256:
  `FF74AF476CE67A0BA7A649DDD2E5DC50D51E4376A9A22BCFA3B7C0D806DE4307`.
- Application/build source fingerprint:
  `424935B642E9C5ABB84A30D85CECAA0A2352A177C9788370BDF9EC790C985E42`.

Overview gaps fall from 78 to 32 lines; Graphs from 44 to 34. Against alpha.46,
the production denominator adds 25 lines and covered lines increase by 83: 61
in Overview, 19 in Graphs, one in Sensors and two in the Diagnostics page opened
by the navigation check. Other per-file covered totals are unchanged. The delta
was independently checked; the final receipt is authoritative.
Largest remaining groups include App (158), CPU provider (74), board provider
(69), tray (61) and the native export picker (57). Complete gaps remain in
`production-lines.json`, `production-summary.txt` and HTML/LCOV.

The first offscreen review exposed a suppressed CPU gap after the initial
519-test pass. A new assertion reproduced it; final source, three images,
ordinary suite, strict Clippy and release were verified again after its fix.
The coverage run above measures only that final source.

The goal remains ACTIVE. This compiled Windows application-source line metric
does not establish branch coverage, vendor/non-Windows execution, real provider
acceptance, field parity or the broader product/native/release gates.

## Verified alpha.48 measurement, 2026-09-30

Nine new ordinary regressions cover System live resolver/report/UI consistency:
GPU age/cache/recovery across all six metrics, CPU clock/commit provider health,
processor-group identity, bridge units/stable IDs, exact network aliases, drive
composite/fallback/error/recovery, and masked/revealed private summaries and CPU
headline overrides. Five targeted tests failed before fixes. Scope and commands:
`SYSTEM_LIVE_2026-09-30.md`.

Final exact-source run: `target/coverage/runs/20260930-071924-165/`.
**528 ordinary tests, 0 failures, 57 ignored**, all 24 exact read-only probes and
10 analyzer checks pass. Production unique source lines:
**23,743/25,663 (92.52%)**, **1,920 uncovered**, zero mixed and 23,536 test-only
lines. Raw LLVM lines/regions/functions include tests: 89.96% / 90.42% / 90.65%.
There are 40 raw profiles. No production exclusions or merged reports.

All 187 application/build inputs, copied instrumented object, measurement tools,
ordinary-suite log and each exact probe log independently verified by
`target/review/alpha48-system-live/verify-receipt.py`. Format, strict all-target
Clippy and release pass. Its receipt includes the release and three inspected
PNG hashes; alpha.42/43/44/45/46/47 review hashes remain verified and preserved.
Post-run audit: zero test processes and zero owned Startup fixture leaves.

- Instrumented test EXE SHA-256:
  `0AC9DB311F8CD4E12C75E4446D5940822910745BF8E3AA1A4C5AC95090BA3F78`.
- Application/build source fingerprint:
  `3F882D83D5687CCCAC2D7575468FBD6C901C05C50251A3C5733D3E85E6410B0D`.

Resolver gaps fall from 42 to 7 lines; report gaps from 6 to 3; System page gaps
from 25 to 21. Against alpha.47, the production denominator adds 42 lines and
covered lines increase by 93. Edited production paths add 84 covered lines
(resolver 68, reports 9, page 7); unchanged native sampler/worker paths contribute
net 9 (sampler +9, specs worker -1, native tray worker +1). These native outcomes
vary between runs; the final receipt is authoritative and no higher repeated run
was selected. Largest remaining groups include App (158), CPU provider (74),
board provider (69), tray (61) and the native export picker (57). Full gaps remain
in `production-lines.json`, `production-summary.txt` and HTML/LCOV.

The initial run `20260930-071412-562` failed an overbroad new assertion that also
matched the public CPU meter outside its hidden summary row. It has no passing
coverage receipt. The assertion was scoped to the target row; full tests,
Clippy/release/offscreen and the final isolated measurement above passed after
correction. Only final source/profiles contribute to the reported percentage.

The goal remains ACTIVE. This compiled Windows application-source line metric
does not establish branch coverage, vendor/non-Windows execution, optional-provider
acceptance, field parity or the broader product/native/release gates.

## Verified alpha.49 measurement, 2026-09-30

Final CPU/motherboard fallback source is measured at
`target/coverage/runs/20260930-081614-796/`. Sixteen new ordinary checks cover
missing/unknown CPU facts, group-aware nominal references, partial/budget failures,
bus fallback, independent PCI retention and BIOS date/unit/revision markers,
plus chassis/slot encodings, cache notes and private paths. Nine before-fix failure
logs are preserved beside the review candidate. Scope and sources:
`HARDWARE_FALLBACKS_2026-09-30.md`.

**544 ordinary tests, 0 failures, 57 ignored**, all 24 exact read-only probes and
10 analyzer checks pass. Final production unique file lines:
**23,947/25,782 (92.88%)**, 1,835 uncovered, zero mixed, 24,062 test-only lines.
Raw LLVM line/region/function summary (includes tests): 90.25% / 90.68% / 90.84%.
There are 40 raw profiles. No production exclusions or merged/repeated reports.

All 189 application/build inputs, copied instrumented object, measurement tools,
ordinary-suite log and each exact probe log independently verified by
`target/review/alpha49-hardware-fallbacks/verify-receipt.py`. Formatting, strict
all-target Clippy and release pass. The verifier also checks the release/review
hash/size/version receipt, nine failure logs and all alpha.42 through alpha.48
review hashes. Post-run audit: zero test processes and owned Startup fixture leaves.
UI layout is unchanged; ordinary UI checks pass. No alpha.49 offscreen image
inspection is claimed; earlier visual evidence remains historical.

- Instrumented test EXE SHA-256:
  `283B7E55D218E2830BA191B88E44F7F38EF10275B3CC602B687BCA7BABDF281C`.
- Application/build source fingerprint:
  `C64311A4774320404CE3B1B3BD72E7BE1693219679C655C7ADA06489C33D3E8A`.
- Release/review EXE SHA-256:
  `AE57293D520090869FE27DDF9B432B8726F9AB03B071B727658240F7A1D8F67A`.

Against alpha.48, the production denominator adds 119 lines and covered lines
increase by 204. Edited provider paths add 217 covered lines: CPU facts +120,
board +80, shared CPU query +9 and CPU native inventory +8. CPU fact gaps fall
74 to 30; board gaps 69 to 15; shared-query gaps 15 to 13. The native CPU file
adds ten production lines and its gaps rise 17 to 19. Unchanged native/history
paths contribute net -13 covered lines: sampler -9, Windows metrics -6, tray
worker -1, specs native +2, history +1. Native outcomes vary; this one final
receipt is authoritative and no higher repeat was selected. Remaining large
groups include App 158, tray 61, native export picker 57 and storage/Startup 52
each. Complete gaps remain in `production-lines.json`, ranked summary and HTML.

The final-source CPU probe reports 24 cores (8P + 16E), 24 threads, P 3700 MHz /
E 3200 MHz in 2.162 ms. That is one instrumented read-only provider observation,
not release UI performance, cross-hardware accuracy or field parity. The full
goal remains ACTIVE; source lines cannot close provider decisions, native UI,
soak, clean-machine or exact-build release acceptance.

## Alpha.50 smart tray and RAM/storage report checks, 2026-09-30

Final-source run: `target/coverage/runs/20260930-095122-382/`. The ordinary
instrumented suite passes **577 tests, 0 failures, 58 ignored**, followed by
the exact 24 reviewed read-only probes and ten analyzer checks. Production
unique file lines: **24,267/26,000 (93.33%)**, 1,733 missed, zero mixed;
25,020 test-only lines and 40 raw profiles. Raw LLVM lines/regions/functions
(includes tests): 90.62% / 91.01% / 91.10%. No production exclusions or merging.
All 193 application/build inputs and copied executable hashes independently
verified against current source by `target/review/alpha50-smart-tray/verify-receipt.py`.

Thirty-three new ordinary checks: twelve RAM, ten storage, ten app/tray and
one hidden mailbox. Seventeen hardware failures reproduced before fixes. New
tray checks exercise X/OS close, Show, explicit Quit, failures/recovery and
settings waits, inspecting commands against a fake backend on the production
worker. Ten thousand hidden publications retain one latest snapshot without
sample UI repaint callbacks. They do not execute native window commands.
Two fixture-only About views were inspected. A compact About regression was
caught during development, corrected, and its original assertion preserved.
The first alpha.50 instrumented attempt failed that assertion and has no
passing report; the final run follows the source correction.

Against alpha.49, production adds 218 lines and covered lines increase by
320. RAM gaps fall 40 to 3 (400/403 covered); storage builder/decoder gaps
fall 52 to 0 (359/359 covered). Native storage stays at 38 missed lines.
New window logic executes 51/53 lines; the sampler visibility hook is used
by the native app and remains outside the headless app fixture. Native tray
creation/menu/tooltip stays at 61 missed lines. Largest remaining groups:
App 150, tray 61, native export picker 57, Startup native controls 52, WMI 49
and native OS inventory 47. Full rankings and HTML remain in the run folder.

- Instrumented EXE SHA-256:
  `53FCAB607B0D22D13A44ABECC74709953A5E4912C50BA9A8BCB837804F4AE767`.
- Application/build source fingerprint:
  `FE18AB7F532AC021EC775AAFA598D37C1928424EF4AC6F92ECF8B47FA9ED4BD4`.
- Release/review EXE SHA-256:
  `1179784C3B92DFD822BF1F9DB26BC8BB9829DA9613843285F88330329F3CAF1F`.

Final read-only probes preserve the real NVMe health log (45 C, 4% used)
and RAM headline (64 GiB/6400 MT/s/2 of 4 slots). These are local provider
observations, not cross-hardware parity. Goal remains ACTIVE. Native tray,
hidden CPU, teardown, provider decisions and exact-build acceptance are open.

## Alpha.51 final paused measurement, 2026-09-30

Final run `target/coverage/runs/20260930-103155-387/`: **589 ordinary tests pass,
0 fail, 58 ignored**; 24 exact read-only probes and ten analyzer checks pass.
Production unique lines: **24,354/26,093 (93.34%)**, 1,739 missed across 92 files,
zero mixed, 25,318 test-only lines, 40 raw profiles. Raw LLVM line/region/function
summary (includes tests): **90.68% / 91.07% / 91.25%**. All 195 source/build
inputs, copied test/release objects and prior alpha.42-50 hashes independently
verified. No exclusions, historical report merging or repeat selection.

Twelve new storage management/report fixtures pass; ten regressions reproduced
before fixes. Native WMI now invokes the same pure production join as fixtures.
Management executes 145/161 lines; 16 remain uncovered. Alpha.51 adds 93
production lines and 87 covered lines versus alpha.50; missed count rises six.
The complete [remaining inventory](COVERAGE_REMAINING_2026-09-30.md) preserves
every gap instead of hiding difficult native paths. First measurement attempt
`20260930-102850-340` was cancelled for an obsolete-helper Clippy cleanup and has
no passing receipt; final source was frozen for this report.

- Source fingerprint: `6919A9395DB334DB3C597584F80A6829F303167E6AA340FA3AC029841B84487F`.
- Instrumented EXE: `753AC3387736400CBDB6076FE04B3D57B52605076F4541F21AED6F3638A78F8D`.
- Release/review EXE: `59930255969997BD93B34E36DD34808D46A5358B4396689B3BF29B9CC91FEEC0`.

Trent requested a stopping point. Goal is **PAUSED**, not complete. Native tray,
export/service/process acceptance, performance budgets/soak, clean-machine,
provider decisions and exact-build release acceptance remain separate gates.
No mouse testing was used; A39 is done for its expressly requested scope.

## Primary references

- [Rust source-based instrumentation](https://doc.rust-lang.org/rustc/instrument-coverage.html)
- [LLVM coverage report/export/show](https://llvm.org/docs/CommandGuide/llvm-cov.html)
- [LLVM LCOV exporter: DA file lines and summary totals](https://github.com/llvm/llvm-project/blob/release/21.x/llvm/tools/llvm-cov/CoverageExporterLcov.cpp)
- [LLVM summary aggregation](https://github.com/llvm/llvm-project/blob/release/21.x/llvm/tools/llvm-cov/CoverageSummaryInfo.cpp)
