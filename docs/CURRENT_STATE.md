# Trontop current state

Last updated: 2026-09-23 (alpha.37 polish gauntlet; the checklist below is
the saved 2026-09-06 handoff)

## Thread shutdown: saved resume checklist

Trent explicitly requested notes saved and this thread closed. **No further
implementation, testing, preview replacement or release action in this thread.**
Alpha.35 remains the review candidate below; this shutdown pass changes only
documentation. Its previous launch is recorded evidence, not a fresh runtime check.
No app instance, personal settings, other projects or desktop windows were touched.

Use this as the execution order for the existing ledger, not additional scope:

1. [ ] On an explicit new resume, read `CODEX.md`, this file and `ASK_LEDGER.md`;
   acquire the repository lease and check the worktree before editing. Confirm
   Trent's feedback on alpha.35 and choose one bounded acceptance gate.
2. [ ] Resolve D01-D05: supported CPU sensor access, isolated drag-test permission,
   private/public release boundary and signing/settings expectations, performance
   budgets, and whether Ctrl+Shift+Esc belongs in this release. Record approved
   deferrals explicitly; do not silently check off missing features.
3. [ ] A19-A22/A25: with fresh permission and an isolated desktop/test account,
   measure real titlebar movement versus Terminal/Explorer, close latency, tray
   Show/Quit, visible/minimized/**tray-hidden** CPU and resource usage. Exercise
   real tray updates/hover and repeated hide/restore beyond repaint deadlines;
   a minimized-only test cannot clear hidden-window repaint-loop regressions.
   Do not inject input or move/minimize/focus day-job windows. Do not propagate a
   drag patch to Boxel or another project until a real fix is verified separately.
4. [ ] A13/A21/A25: native restart persistence, dirty/failed/slow settings saves,
   close with slow providers, and renderer recovery. Record timestamps, exact
   executable hash, actual exit/window/tray state and relevant failure logs.
   Earlier freeze/close reports are unresolved, not attributed to Unity load or
   one provider without evidence. Read `docs/FAILURE_REPORTS.md`,
   `docs/SETTINGS_PERSISTENCE.md`, `docs/RENDERER_RECOVERY.md` and
   `docs/DRAG_INVESTIGATION.md` before the corresponding test.
5. [ ] A04/A25: use owned disposable child processes and an explicitly approved
   test service for process/service controls; cover denied/protected/stale targets,
   cancellation and failure recovery. Validate export contents and copy paths.
   Never target unrelated processes or services. See
   `docs/PROCESS_ACTION_SAFETY.md` and `docs/SERVICE_CONTROLS.md`.
6. [ ] A22/A25: after budgets and isolation are agreed, run the 60-minute mixed-load
   soak, recording CPU, memory, handles, frame times, growth and provider freshness
   through visible/hidden states. Retain missing/cached fields and chart gaps.
   Investigate failed budgets or reproducible failures only, not an endless gauntlet.
7. [ ] A04/A06/A10: finish or obtain explicit deferral for missing controls, CPU
   temperatures and remaining field-by-field Task Manager coverage. Use the source
   notes in `docs/HARDWARE_RESEARCH.md`, `docs/CPU_CLOCK.md`,
   `docs/GPU_ADAPTERS.md` and `docs/SENSORS_PLAN.md`. No silent driver installation.
8. [ ] A11/A13-A16/A31/A32: one finite visual acceptance pass at compact/normal
   sizes and multiple DPI scales, light/dark/extreme themes, empty/cached/error
   states and deep scrolling. Check alignment, readable contrast, zebra bands,
   hover/focus, icons and persistence; obtain Trent's approval of the exact build.
9. [ ] A26/A29: clean standard-user/no-Rust machine, relocated EXE and read-only
   app folder; verify Windows-only dependencies and settings behavior. Review and
   merge the private branch, then tag/package/publish only the agreed release scope.
   Verify the downloaded artifact's own hash/version and record known limitations.

Ordinary validation to rerun **after future code changes**, not during shutdown:
`cargo fmt --all --check`, `cargo test --offline`,
`cargo clippy --offline --all-targets -- -D warnings`, and
`cargo build --release --offline`. See `docs/HEADLESS_QA.md` for opt-in cases.
Never run every ignored test indiscriminately. Tests using fixtures or offscreen
rendering are not native drag, close, recovery or soak evidence. Freeze and record
the final candidate hash before native acceptance; a later rebuild needs its own
applicable checks. The CI artifact and local EXE are separate build outputs; do
not assume their hashes match without comparing them.

## Latest built candidate: alpha.37 polish gauntlet (A34)

Trent, 2026-09-22 (voice note over screenshots): "can you fix that actually and
also ... there are some other things ... look at every page headless ... use
your screenshot reading ability ... see how it still kinda sucks and many pages
layouts are ... this app is a fucking mess ... I just want you to do a polish
gauntlet, without my constant attention okay? Can you use your best judgement
please? Run a gauntlet on it." No new features: this is a finishing pass over
`render_gauntlet_all_pages` (`docs/HEADLESS_QA.md`), the offscreen harness that
renders every page, Graphs tab, Performance device, System section and dialog at
1000x580/1280x800/1600x1000 from the REAL sampler and specs workers.

- Rounds shipped this cycle (`git log --oneline` on `feat/system-specs`,
  commits `f0805a6`..`9f14ee8`): Performance page rail/axis/card-anatomy honesty;
  GPU adapter picker moved to hover; Hardware sensors compacted with drives above
  the fold; Startup/Services/System/dialog padding and truncation fixes; process
  pages standardized to 1-decimal percents and consistent account naming;
  Graphs wall calmed (grouped disks, idle iGPU folded, shared number formats);
  Overview reworked for roomy KPI rows with no bar collisions; units/axes/legends
  unified across Graphs and Performance; table right columns flexed and noise
  rows removed; Performance views got a single-adapter picker, engine grid and
  rail-follow behavior; a final pass fixed the remaining System/dialog/sidebar
  noise and one egui id clash.
- Final critic pass scored all 16 surfaces (Overview, Graphs, Processes,
  Performance, History, Startup, Users, Details, Services, Hardware sensors,
  System, Theme Studio, About, Export, Sidebar, title/command bar) from the
  rendered PNGs; average 7.2/10 (range 6-8), Overview up from an early 3/10.
  Honest remaining weaknesses (not fixed this pass, tracked under A34): Details
  still scrolls horizontally at 1000x580 (READ/CPU TIME clipped); Services stacks
  four pieces of disabled-toolbar noise above the table header; the CPU
  temperature gap is worded three different ways across Overview footer, Graphs
  footer and Hardware sensors; network vocabulary still splits across
  in/out, Receive/Send and Rx/Tx depending on the page; a few tab rows (Graphs,
  Theme Studio) shift horizontally when the selected pill changes. See the A34
  ledger entry for the full list.
- One `em dash` in a `src/widgets.rs` doc comment (`row_columns` flex
  documentation, introduced in an earlier round) was found by the finalize scan
  and replaced with a semicolon.
- Gate on this tree (`feat/system-specs`, pre-bump at alpha.36 identity):
  `cargo fmt --check` clean, `cargo clippy --all-targets -- -D warnings` clean,
  `cargo test` **405 passed, 0 failed, 42 ignored** (33.65 s), `cargo build
  --release` OK (1m 25s, no locked-binary fallback needed; no Trontop instance
  was running). `target/release/trontop.exe` **0.3.0-alpha.36** (pre-bump),
  14,639,616 bytes, SHA-256
  `64F0356F001560DA55F17D9A7939861F7BBE1EF7FDAF147FA65FB38726F17715` (unsigned,
  not launched). All 13 `native_specs_*_read_only_probe` tests rerun by exact
  name: 13 passed, 0 failed (0.67 s).
- Final render: `render_gauntlet_all_pages` wrote **157 PNGs** (39.04 s) to a
  scratch directory and every 1000x580 page was reviewed. The harness itself
  reported slow polling this run (worst accepted-sample gap 5.06 s against its
  own 3 s graph-gap threshold); that is a harness-timing artifact from the
  finalize session's own concurrent tool calls, not a product regression, and is
  visible only as a few extra graph breaks in that one render's screenshots.
  Version bumped to **0.3.0-alpha.37** after this gate; a rebuild under the new
  version number was not re-run (identical source, cosmetic version string
  only).
- Remaining: everything the final critic pass listed above, plus the still-open
  asks below (A04/A06/A10/A11 etc.) and D01-D05. No native launch, no signing,
  no publish.

## Previous candidate: alpha.36 System specs page (A33; A10/D01 bridge)

Trent, 2026-09-22: "clone speccy, IN trontop, make trontOP OP". The nine
parallel provider lanes returned nothing, so the integrator implemented every
provider on `feat/system-specs` (scaffold aed2cee) and finished the page. Not
launched: no window, input, focus change, driver, elevation, registry write or
network request was used. Details: `docs/SYSTEM_SPECS.md`.

- Providers: OS, CPU, RAM, Motherboard, Graphics, Storage, Optical Drives,
  Audio, Peripherals, Network and Sensor Sources, all read-only, each with an
  exact-name probe. Speccy's bugs on this PC are beaten (64-bit VRAM 15.9 GB, no
  shader clock, per-DIMM SMBIOS, "NVMe (PCIe 3.0 x4)" with live temperature and
  the NVMe health log, VT-x capability vs firmware vs Hyper-V, per-core-type
  caches, full CPUID brand, Arrow Lake-S, LGA1851).
- Page: section icons and status dots, Speccy Summary with colored live values
  (missing ones are a muted `--` with the reason), Expand/Collapse all, Copy all,
  section, group and row, wrapping long values, Save text/JSON through the export
  worker with private values off by default and reset after each save.
- Sensor bridge: LibreHardwareMonitor/OpenHardwareMonitor WMI and HWiNFO shared
  memory, read-only, feeding CPU/board keys with source labels on the System and
  Hardware sensors pages. None runs on the reference PC, so CPU and board
  temperatures are explicitly Unavailable with that reason.
- Probe timings (debug): OS 58 ms, CPU 2 ms, RAM 1 ms, board 121 ms, graphics
  30 ms, storage 323 ms, devices 704 ms, network 12 ms, bridge 10 ms. Private
  values masked in probe output; drive interface paths never shown or exported.
- Gate on this tree: `cargo fmt --check` clean, `cargo test` 331 passed / 40
  ignored, `cargo clippy --all-targets -- -D warnings` clean, `cargo build
  --release` OK. `target/release/trontop.exe` **0.3.0-alpha.36**, 14,344,704
  bytes, SHA-256 `9CEE0761C4F43FCE3942AC6305F5991949CD1A3A4DCE15C65CED89C60867EABF`
  (unsigned; built before the commit, not preserved under `target/review`).
- Visual QA: `render_system_specs_visual_pass` rendered ten PNGs from REAL data
  (Summary, OS, CPU, RAM, Graphics, Storage, light Motherboard, Sensor Sources,
  1040x640 Summary and Storage); reviewed, then fixed live-value truncation,
  "Unavailable" noise beside titles, cut instruction-set rows and provider rows
  that hid their reasons.
- Remaining: Trent's review of the page (A33), the real Save As picker for specs
  files (same gate as `docs/EXPORTS.md`), and D01.

### Verification fixes (same alpha.36 line, "System specs: verification fixes")

Four review lenses (truth, perf, safety, parity) found 18 issues; all fixed:

- Truth: the Intel iGPU video BIOS (REG_MULTI_SZ) now reads "Intel Video BIOS"
  instead of a false "not reported"; the WD60EZAX temperature reason is "Not
  reported by the drive's storage driver (code 1); SMART temperature requires
  administrator" instead of a bare code; Wi-Fi link speed 866.7 Mbps (was
  truncated to 866); the Bluetooth PAN adapter is "Bluetooth (PAN)" from its
  physical medium (GetIfEntry2), not "Ethernet"; TRIM reads "Supported" / "Not
  supported by the drive" (a capability, not a setting); the LGA socket and
  "-S" codename are given only when the brand names a desktop part, since HX
  laptops share the same CPUID models.
- Perf: every SetupDi enumeration takes the section Context and stops at the
  read budget (checked before the set is opened and before each device);
  callers in board, devices, graphics, network and storage pass it through.
- Safety: non-ASCII SUBSYS IDs no longer panic; SMBIOS type 16 slot and capacity
  totals cannot overflow (an overflowing capacity is Unavailable);
  GetAdaptersAddresses and QueryDisplayConfig never walk unfilled buffers; the
  64/128 display path caps that could undersize the buffer are gone; EDID IDs
  are used only when edidIdsValid is set; HWiNFO shared memory is copied raw
  without forming a Rust reference, and a non-"not found" open error reports its
  real reason; one unreadable disk interface is skipped instead of failing the
  Storage section.
- Parity: the left navigation scrolls the active page into view once per page
  or window height change (no animation, no per-frame scroll), so "System" is
  no longer half hidden under the stats footer at 900x600 and 1040x640; the
  small-window test now asserts that nav entry specifically. Network adds
  Speccy's WinInet proxy group (HKCU, values private) and a Connections group
  (GetExtendedTcpTable/UdpTable counts plus up to 40 established connections by
  PID, endpoints private).
- Evidence: all 13 `native_specs_*_read_only_probe` tests rerun by exact name
  (network: Wi-Fi 866.7 Mbps, Bluetooth (PAN), 240 TCP / 61 UDP; graphics: both
  BIOS strings; storage: TRIM wording; CPU: Arrow Lake-S, LGA1851);
  `render_system_specs_visual_pass` re-rendered and the PNGs reviewed (System nav
  entry fully visible at 1040x640). Gate: `cargo fmt --check` clean, `cargo
  test` 336 passed / 40 ignored, `cargo clippy --all-targets -- -D warnings`
  clean, `cargo build --release` OK: `target/release/trontop.exe`
  **0.3.0-alpha.36**, 14,370,304 bytes, SHA-256
  `2B3338BE402E45692598BFEDD9E18734CD6851C4152DEDE50298D83331E9B9DD` (unsigned,
  not launched).

## Previous running candidate: alpha.35 GPU adapters and release handoff

**Requested stopping point: build, launch, private GitHub check, then hand off.**
Do not start another feature, polish or speculative bug-hunting cycle without
Trent's direction. `ASK_LEDGER.md` remains the completion contract.

Alpha.35 adds identity-keyed GPU adapters, real DXGI names/capacities, individual
engine histories, and whole-adapter dedicated/shared/committed memory histories.
Performance / GPU adapters has aligned metric cards and compact engine charts;
Graphs and Overview share these histories. Missing and cached readings remain
explicit. The redesign skill guided that layout within the existing Tront theme.
All-core graphs, dynamic CPU clocks and ColorMagic from alpha.33/34 are included.

Candidate: `target/review/alpha35-gpu-adapters/trontop.exe`, **0.3.0-alpha.35**,
13,794,304 bytes, built **2026-09-06 20:34:43 UTC**. Source commit:
**bab22c8c9405813edb4bc506d64012faec95a3fc**. Later documentation commits do not
change this executable.
SHA-256: `21A8C13B71580777B2EBA484499513D2E6B76D48F0BC884F7A0BAC6DCEF3A6E0`.
The preserved copy matches the optimized release EXE; PE product version matches.
Windows-only imports, including DXGI, were inspected; no app-sidecar or VC runtime
DLL is required. The executable is **unsigned**. A clean-machine run is still due.

**Launched on Trent's normal desktop on explicit request**, PID 131980, HWND
23006288. Read-only inspection confirmed title Trontop, visible, not minimized,
responding, 1280x760 at (640,340). No global input, dragging, focus manipulation or
other app/window changes. The older alpha.32 preview was left untouched. This is
launch evidence, not a native interaction or sustained responsiveness test.

Local gate: **279 passed, 0 failed, 25 ignored** (55.95 s), strict Clippy (5.26 s),
formatting/diff checks and optimized release build (1m 06s) passed. The production
read-only GPU adapter probe passed (3.43 s). Initial inventory took 427.191 ms;
later memory queries took 0.195 / 0.122 ms. These are provider measurements, not
UI/drag/close latency claims. `docs/GPU_ADAPTERS.md` records identity semantics,
capacity scope, source links, limits and reproduction commands.

Four offscreen fixtures generated (4.17 s) and visually inspected under
`target/ui-smoke/`: `alpha35-gpu-wide.png`, `alpha35-gpu-compact.png`,
`alpha35-gpu-missing-light.png`, `alpha35-gpu-cached.png`. These are synthetic
fixtures, not screenshots proving native desktop behavior.

### Private GitHub readiness

- Repository remains private: https://github.com/TrentSterling/trontop.
- Source commit `bab22c8` was pushed successfully to `feat/provider-diagnostics`.
- Exact-source Windows verification:
  https://github.com/TrentSterling/trontop/actions/runs/34058415830.
  **SUCCESS**, completed **2026-09-06 21:00:38 UTC**, rechecked during shutdown.
  Formatting, tests, strict Clippy, the local renderer patch check, release build
  and portable artifact upload all passed. This clears the exact-source CI gate,
  not native acceptance or a release-download verification. The ordinary local
  suite was not rerun for these documentation-only changes.
- Default `main` is still the older `bfe8689` tip, not this feature branch. It is
  not protected, and no open PR or published GitHub release was present at check.
  No merge, tag, release upload, visibility change or public publication was made.

### What currently blocks release

1. **Native reliability acceptance (A19-A22/A25):** the exact EXE still needs safe,
   isolated close/drag/tray/restart/action/export checks and a mixed-load soak.
   Earlier freeze/close reports are not cleared by unit tests or offscreen renders.
2. **Agreed feature coverage (A04/A06/A10/D01):** CPU/core/board temperature provider
   is not connected; suspend/resume and Startup toggles remain missing; service
   command validation and the field-by-field Task Manager comparison remain open.
   Networking details/process rates, remaining CPU fields and NPU status still need
   completion or explicit approved deferral. No driver installation is implied.
3. **Final acceptance (A11/A13-A16/A31/A32):** Trent's cohesive branding/theme/layout
   review, native persistence and DPI checks remain open. These are finite checks,
   not permission for endless redesign.
4. **Distribution gate (A26/A29/D03):** run the single EXE on a clean standard-user
   Windows setup, confirm relocation/read-only-folder behavior, review/merge the
   branch, choose private alpha versus public
   distribution, then create the agreed versioned artifact/checksum/known-limits
   release and verify the downloaded file. Unsigned-binary handling needs agreement.

Performance budgets (D04) and optional Ctrl+Shift+Esc interception (D05) remain
explicit ledger decisions. A usable private preview is available now; the full
requested release is not declared complete. New brainstorm ideas are not blockers.

## Previous built candidate: alpha.34 dynamic CPU clocks (A06/A07/A22/A32)

CPU Performance now uses a Windows performance-state interval average rather than
the static-looking CurrentMhz field. It also shows fastest/slowest reporting
processors and a virtualized group-local clock list. Average/fastest histories are
shared by Graphs and Overview; wide Overview layouts now fit five chart columns.
Missing/cached data remain explicit and JSON records the source and interval.

Candidate: `target/review/alpha34-cpu-clocks/trontop.exe`, **0.3.0-alpha.34**,
13,714,432 bytes, built **2026-09-06 11:18:02.048 UTC** from clean source
**98695cc683e5afe97c069fc72f1e2db88ebfe508**. The later handoff-doc commit is not
the embedded source identity.
SHA-256: `ED9237BC721CF93F8286F1F076DAF99861FAC8832869D5F7AACBD3E5714B3B84`.
The preserved copy matches the optimized release EXE and both PE version strings.
**Built, not launched.** Existing previews and personal settings were untouched.
No native window/input, driver/service installation, source upload, tag or release.

Final gate: **273 passed, 0 failed, 23 ignored** (56.45 s), strict Clippy (2.32 s),
formatting/diff checks and optimized release build (1m 02s) passed. Four final
offscreen PNGs generated in 4.56 s and inspected: `alpha34-clock-cpu.png`,
`alpha34-clock-cached.png`, `alpha34-clock-missing.png`,
`alpha34-clock-overview.png` under `target/ui-smoke/`.
These are synthetic fixtures, not screenshots of a newly launched preview.

The production collector's separate read-only native probe passed (4.00 s).
After its baseline, three interval averages were 5165.58, 5147.64 and 5123.62 MHz,
with 24/24 contributing processors. Query timings: 0.6100 ms initial and
0.1890 / 0.1421 / 0.1427 ms subsequent. This is a short debug-provider measurement,
not native UI/drag/close/soak timing. A separate PDH reading supports the
distinction between performance percent and nominal frequency but is not a
synchronized cross-monitor parity test.

`docs/CPU_CLOCK.md` documents the pinned System Informer reference, deliberately
per-processor nominal weighting, private-API bounds, retry/reset behavior and tests.
The redesign skill guided retained-field alignment, readable status and the wide
Overview adjustment within the existing egui design. No theme/palette rewrite.
CPU temperatures, wider Task Manager field parity and the native acceptance gates
remain open. No brainstorm feature was silently added to the release checklist.

## Previous built candidate: alpha.33 cores, Overview and ColorMagic (A06/A13/A32)

Performance / CPU now defaults to distinct logical-processor graphs with a Total
CPU switch. Overview uses the same timestamped histories for a dense machine-wide
graph wall. Theme Studio adds coordinated four-peg Randomize, six ColorMagic
families plus Surprise me, and 12-roll undo without changing mode/layout settings.
The old Speed field is honestly labelled Power clock; live boost is not done.

Candidate: `target/review/alpha33-cores-colormagic/trontop.exe`,
**0.3.0-alpha.33**, 13,680,128 bytes, built at **2026-09-06 10:31:53.649 UTC**
from clean source **d00dc395a0e51c8cdb31f82b221963d484783109**. The later
handoff-doc commit is not the embedded source identity.
SHA-256: `6145E483E77E76CF1CC4F3E6139D367D6E47F6F7A2C4DE858FC1FD0204A0F9ED`.
The preserved copy matches the optimized release EXE; PE version strings match.
**Built, not launched.** Existing previews and personal preferences were untouched.
No source upload, tag, release, driver installation or native desktop input.

Final gate: **263 passed, 0 failed, 21 ignored** (55.66 s), strict Clippy (4.04 s),
formatting/diff checks and optimized release build (1m 02s) passed. Six final
offscreen views were generated and inspected. A production read-only CPU probe
confirmed 24 distinct logical readings; the optimized synthetic UI timing is
explicitly CPU layout/tessellation only, not a native drag/close/soak result.
See `docs/ALPHA33_REVIEW.md` and `docs/HARDWARE_RESEARCH.md` for exact scope,
reproduction commands, sources, findings and unfinished measurement/sensor work.

This completes the requested implementation slice, not the full product. The ask
ledger retains CPU temps, live frequency, native interaction gates, parity and
Trent's acceptance. Shared history cursor and developer-project grouping are
brainstorm proposals only. Review this candidate before further speculative polish.

## Previous built candidate: alpha.32 table keyboard focus (A16)

Clickable table labels, headers and heat cells now show a rounded contrast-safe
focus outline without moving text or changing row/column geometry. The outline
uses existing cell padding. Process row hit areas and icons retain mouse clicks
without adding invisible duplicate Tab stops; labeled targets and expansion
buttons retain keyboard activation. No timer, animation loop or dependency added.

Candidate: `target/review/alpha32-table-focus/trontop.exe`, **0.3.0-alpha.32**,
13,643,776 bytes, built at **2026-09-06 02:25:21.750 UTC** from clean
**3aab2cd93a233d92700c51c45abb8660856a6510**. The later handoff-doc commit is not
the embedded source identity.
SHA-256: `B028348498E60B60646E84EEB7A107E041001F647F0E2DF4342C1714404DC382`.
The preserved copy matches the optimized release EXE. **Built, not launched.**
Current previews and personal preferences were not touched; source remains local.

Final verification: **255 passed, 0 failed, 18 ignored** (55.65 s); strict Clippy
(5.06 s), formatting, diff check and optimized release build (1m 03s) passed.
Four new regressions cover Tab visibility/contrast/geometry, disabled and
Enter/Space behavior, real process-table sorting/selection at compact/normal sizes
and four egui scale factors, and preserved full-row mouse gaps. The original
missing-outline test failed before the implementation; the production-UI test
then found the duplicate row stop. All four final offscreen focus PNGs were
generated and inspected (3.66 s). See `docs/THEME_CONTRAST.md`.

This bounded redesign-skill pass preserves the existing theme, density, zebra
bands and hover behavior. Earlier graph/settings/tray changes are included.
Native close/drag timing, tray validation, soak, CPU temperatures and final visual
acceptance remain open. No native window/input, preview replacement or upload.
Stop at this tested review handoff, not another speculative polishing cycle.

## Previous built candidate: alpha.31 graph layout performance (A22/A32)

The graph wall now skips expensive card content layout outside the viewport while
keeping measured row heights and stable interaction IDs. A common row width fixes
fractional-scale column drift. Changing categories returns to the first graph;
switching Lines/Bars preserves the scroll position. Existing themes, rounded zebra
surfaces, hover behavior and provider histories are retained.

Candidate: `target/review/alpha31-graph-layout/trontop.exe`, **0.3.0-alpha.31**,
13,643,776 bytes, built at **2026-09-06 02:00:51.405 UTC** from clean
**d87354e579f0467287b736b073ac5ed92459ef37**. The later handoff-doc commit is not
the embedded source identity.
SHA-256: `E83D3676C5A4B79F0D7798AD520AD843D1210377CCF19C46F7F6C2E337887E83`.
The preserved copy matches the optimized release EXE. **Built, not launched.**
Current previews and personal preferences were not touched; source remains local.

Final verification: **251 passed, 0 failed, 17 ignored** (55.74 s); strict Clippy
(1.98 s), formatting, diff check and optimized release build (49.49 s) passed.
New regressions compare visible text and geometry against full layout through
scrolling and fractional scales, verify category reset after deep scrolling, and
reconstruct the complete font atlas across synthetic filter clicks. The last test
fixes missing glyphs in the offscreen screenshot helper, not the app renderer.
Six graph and six memory PNGs were generated; eight views were inspected across
the pass, including the corrected thermal labels.

One same-binary release CPU benchmark at 512 charts measured layout plus
tessellation p95 of **3.12 to 0.88 ms** at 1920x1080 and **2.48 to 0.28 ms** at
1040x640. This is a synthetic CPU comparison, not native FPS or a guaranteed
frame budget. Small workloads show smaller gains and occasional outliers.
Details and all six workload comparisons: `docs/GRAPH_WALL.md`.

Includes alpha.30 memory counters and earlier settings/tray changes. Native close
and drag timing, tray validation, soak testing, CPU temperature coverage and user
acceptance remain open. No global input, native window tests, preview replacement,
driver installation or upload occurred. Stop at this bounded build/review handoff.

## Previous built candidate: alpha.30 memory counters and graphs (A06/A07/A32)

Corrects COMMITTED and removes the misleading page-file graph: the old Windows
sysinfo swap estimate was not page-file occupancy. The existing background sampler
now reads actual commit/limit/peak, system cache and kernel pool counters. Memory
has eight aligned zebra fields with cached/unavailable states and responsive
wrapping. Graphs adds a Memory filter with five history series; missing samples
leave gaps. Overview and JSON use the same counters and explicit provenance.

Candidate: `target/review/alpha30-memory/trontop.exe`, **0.3.0-alpha.30**,
13,640,704 bytes, built at **2026-09-06 01:23:34.326 UTC** from clean
**8e337b61a2d8cb30916ce2299a02bef5c2ea516f**. The later handoff-doc commit is not
the embedded source identity.
SHA-256: `0330BE3E86C2187D313C3E4629C56B68DDDEFBD67898DB1072BDE2CF2E1C1C43`.
The preserved copy matches the optimized release EXE. **Built, not launched.**
Current previews and personal preferences were not touched; source remains local.

Final verification: **248 passed, 0 failed, 16 ignored** (31.10 s); strict Clippy
(2.21 s), formatting, diff check and optimized release build (1m 12s) passed.
Five new ordinary regressions cover counter conversion, retained failures,
timestamped graph gaps, JSON provenance and stable compact/light/dark geometry.
The opt-in read-only native probe passed 20 queries (median 18.9 microseconds,
max 176.6 microseconds in one debug run). This is not whole-app performance.
Six memory and six graph-wall PNGs were generated offscreen with synthetic data;
all six memory views plus the dark and compact graph walls were inspected.

See `docs/MEMORY_COUNTERS.md`. This does not complete Task Manager parity or CPU
temperature coverage. A21 native close timing, A19 native tray validation,
drag/soak measurements and user acceptance remain open. No global input, real
tray/window tests, preview replacement, driver installation or upload occurred.
Stop at this bounded build/review handoff, not another speculative audit.

## Previous built candidate: alpha.29 asynchronous tray startup (A22)

Removes the recorded app-construction wait for Shell tray creation and the
failed-constructor join. One owner worker, one latest-sample slot and one private
auto-reset event handle startup, updates, Windows messages and cancellation.
About reports tray lifecycle state. Late creation after close cleans up on its
owner thread; ordinary tray samples do not request extra UI repaints.

Candidate: `target/review/alpha29-tray-startup/trontop.exe`, **0.3.0-alpha.29**,
13,630,464 bytes, built at **2026-09-06 00:48:55.501 UTC** from modified a9df6e0.
SHA-256: `501D6B9E3392596F65D9BE4FCE89267941243BCE9E56E79B9398EBE2ED4F90C9`.
The preserved review copy matches the optimized release EXE. **Not launched**;
the current preview and personal settings were untouched. Source remains local.

Final verification: **243 passed, 0 failed, 14 ignored** (29.13 s); strict Clippy
(2.23 s), formatting, diff check and release build (53.05 s) passed. Seven new
worker tests cover blocked/failed startup, bounded samples and drop, owner-thread
late cleanup, update recovery/actions, repaint restraint and already-observed
messages in the worker's own disposable queue. Focused tray suite: 9 passed,
1 intentionally ignored native-icon test (0.22 s). One fixture startup returned
in 78.4 microseconds; this is not measured native app startup or close latency.
About's existing compact dark/light test now checks the tray row too. No new
visual redesign or screenshot evidence was claimed.

See `docs/TRAY_LIFECYCLE.md`. A19 is REVIEW again because the changed native pump
requires isolated icon/menu/hidden-window validation; historical captures do not
prove this version. A21/A22/A25 and the full objective remain incomplete. No
global input, real tray/window tests, preview replacement or upload were performed.

## Previous built candidate: alpha.28 settings close path (A21/A22)

Focused continuation of the responsiveness objective after the slow-close report.
Two regressions failed on alpha.27: dispatch/Retry deep-cloned captured egui memory,
and an unchanged snapshot failed on another instance's file lock. Alpha.28 shares
the immutable capture and skips filesystem operations for byte-identical local
state. Actual changes retain the conflict guard and safe-save close gate.

Candidate: `target/review/alpha28-close/trontop.exe`, **0.3.0-alpha.28**,
13,638,656 bytes, built at **2026-09-06 00:30:31.254 UTC** from modified 4419f7e.
SHA-256: `219CE12FC71F18A94031FC8E1DFD9C66AB4632711B583CDA4072C24E905B738D`.
**Built, not launched.** The review copy matches the optimized release build.
Existing windows and user settings were untouched; no upload.

Verification: **236 passed, 0 failed, 14 ignored** (29.22 s); strict Clippy
(5.03 s), formatting and release build (49.05 s) passed. Three new regressions
cover zero extra dispatch clones, unchanged snapshots under file contention with
preserved external edits, and the app-to-worker-to-file headless close gate.
The latter took 1.3972 ms in one debug fixture run, not native teardown timing.
No UI design changed; no new visual screenshots were needed or claimed.
See `docs/SETTINGS_PERSISTENCE.md`. A21 remains open. This does not establish that
Unity caused the observed delay or guarantee instant close when settings are dirty.
Stop this bounded follow-up at the build/review handoff.

## Previous built candidate: alpha.27 graph wall (A32)

Trent explicitly requested one mostly-graphs page after reviewing alpha.26.
Added Graphs (Ctrl+9): continuous 1-4 column grid, Lines/Bars, category filters,
CPU/memory/GPU usage, temperature/power, clocks/fan/VRAM, individual drive
temperature channels, physical-disk metrics and per-interface receive/send.
Overview is preserved. Histories use provider timestamps and leave cache/partial
gaps; current values remain labelled. No new provider or CPU temperature claim.

Candidate: `target/review/alpha27-graphs/trontop.exe`, **0.3.0-alpha.27**,
13,639,168 bytes, built at **2026-09-06 00:11:45.802 UTC** from modified afc6818.
SHA-256: `B6ACDA8C1F9B4D3B9B4AFA6C6D165E25C884CDE6F4C0F565930B1C243D16DA10`.
**Built, not launched.** No current user windows or desktop input were touched.

Verification: **233 passed, 0 failed, 14 ignored**, 480 page-matrix cases;
strict Clippy, formatting and release build passed. Six new offscreen graph PNGs
generated and reviewed (dark/light, Bars, thermal filter, compact and empty).
Details and limitations: `docs/GRAPH_WALL.md`. Source remains local only.

His additional slow-close question received a bounded source diagnosis: viewport
close waits for the asynchronous settings save; sampler has zero shutdown wait,
tray has a 100 ms budget. Build pressure may exacerbate this, but no native timing
was collected and no close fix is claimed. A21 and CPU-temperature A10/D01 remain
open. Stop at this focused review candidate instead of resuming the general
optimization/bug hunt.

## Last launched preview: clean alpha.26 (historical build)

Trent called out diminishing returns, authorized stopping old Trontop windows,
and explicitly requested a fresh build and launch. Rebuilt clean **b79708f** in
**1 min 14 s**, then copied the exact EXE without overwriting previous artifacts:
**`target/review/alpha26-b79708f-clean/trontop.exe`**, version **0.3.0-alpha.26**,
**13,598,208 bytes**, built **2026-09-05 23:37:40.445 UTC**.
SHA-256 **`5CC9CF248335353C821237229EFAED2E97BC9A082D8C11B8A2DA7CED9A82DF49`**.
This changes the embedded source identity to clean b79708f; application source
is unchanged from the previously verified alpha.26 below.

Opened at **23:38:40.145 UTC**, PID **242180**, HWND **8192130**. Initial
InputIdle/Responding checks passed, not a native drag/close/soak claim. Stopped
only old preview PIDs 259420, 262932, 263640, 273992 and 274860 after verifying
each exact executable path and start time. Their EXEs were retained. No unrelated
window, global input, tray test or upload was involved. This normal user-requested
app launch may load/save user settings, unlike the earlier isolated fixtures.

Read `../REVIEW.md`. Next step is hands-on review of this exact build. No alpha.27
tray rewrite was made; the wait remains documented. Do not continue a cosmetic
or speculative optimization loop while awaiting review. The goal/ledger remain
incomplete; isolated native-test and upload permissions remain unresolved.

A subsequent single passive check at 23:43 UTC observed 31/31 responding samples
over 31.39 s, 0.326% whole-machine CPU, 59 threads and 1173-1178 handles. Working
set rose from 250.4 to 306.4 MiB; this short unclassified workload is not sufficient
to attribute that growth or verify a memory budget. See `../REVIEW.md` for exact
scope/counters. The actual main HWND was 8653022, after initial startup HWND 8192130.
No window/input manipulation or further rebuild occurred during this check.

## Latest candidate: alpha.26 non-blocking settings

Settings no longer use eframe's synchronous startup read or unbounded save-thread
joins. One app-owned worker handles bounded reads, parsing, serialization and
staged replacement. The old `state-v2.ron` is migrated read-only into the new
`settings-v3.json` path on a later save; named palettes, theme and egui memory
remain supported. Cooperative instance conflicts, invalid files and write errors
preserve the existing data. See `SETTINGS_PERSISTENCE.md` for limits and policy.

The app stays navigable while settings load, with editing disabled until the
saved library is known. Close dispatches the final snapshot and lets the UI keep
processing events. Slow/failed saving exposes Keep open, Retry save and explicit
Close anyway. A fixed-height status footer reports load/save/error state. The
existing-app redesign audit caught a translucent disabled editor and cramped
warning dialog; the final frame/explanation remain readable with inset actions.

Final ordinary gate: **225 passed, 0 failed, 13 ignored** (26.16 s), strict Clippy
PASS (1.97 s), format/diff checks PASS, optimized build PASS (1 min 00 s). Twelve
new regressions cover migration, real app/worker/file/fresh-app persistence,
coalescing, invalid/read-only/conflicting saves, blocked reads/writes, non-waiting
drop and production-UI close choices. Eframe's feature tree confirms its file
persistence feature is disabled; egui memory serialization remains enabled.

Final offscreen pass: **101 PNGs** in **60.26 s** on RTX 5070 Ti/Vulkan, with all
eight new compact loading/saved/pending/error dark/light views plus normal
dark/light Theme Studio inspected after the final fixes. Default historical
fixtures keep persistence disabled; dedicated new fixtures cover the runtime
status footer and synthetic close events. No native window or OS input is used.
This is not measured native startup/drag/close/soak performance. No actual user
preferences were read, migrated or modified by these fixture tests.

Original verification artifact (now retained at
**`target/review/alpha26-b79708f/trontop.exe`**): **0.3.0-alpha.26**,
**13,598,208 bytes**, built **2026-09-05 23:20:30.442 UTC** from modified 3b80efb source.
SHA-256 **`C88474F57882E767E0C70A83C56F55E12BD87ACA162800268395930E08016450`**.
PE import inspection shows only Windows libraries, with no dynamic MSVC CRT.
Clean-machine portability remains unverified; no additional runtime assets.

Recorded wait: `TrayController::new` still blocks on `ready_rx.recv()` and
can join a failed worker during startup. Address under A22 with bounded worker
lifecycle and honest pending/failure states; no native tray test is authorized.
Removing settings joins does not prove GPU/window/tray teardown is fast or that
storage caused Trent's reported lag/crash. A13/A20/A21/A22/A25 remain unchecked.

Local only. No preview was touched during implementation; the later explicitly
requested launch is recorded above. No upload attempt. Isolated-desktop and
explicit source-upload approval remain unanswered. Remote last verified at
alpha.21; no alpha.26 CI/tag/release.
The persistent goal stays active and this is a checkpoint, not a finished app.

## Previous candidate: alpha.25 non-blocking recovery diagnostics

Recoverable GPU events now update recovery state before trying a bounded
background-log enqueue; slow disk I/O cannot directly block that callback.
Service-result polling now skips a busy publication mutex and retains its result
for a later frame. No native action guards, theme/layout behavior or renderer
recreation algorithms changed. See `FAILURE_REPORTS.md` and `SERVICE_CONTROLS.md`.

Final ordinary gate: **213 passed, 0 failed, 13 ignored** (25.63 s), strict Clippy
PASS (5.46 s), format/diff checks PASS, optimized build PASS (42.02 s). Four new
regressions cover the actual callback with a blocked/full writer queue, caller
metadata/file schema, disabled/healthy/disconnected paths and held service-result
publication. One optimized blocked-writer run measured **781.2 us** for 1,000
saturated enqueue attempts plus two real callback state transitions, **7.2 us**
for drop, and verified all 17 accepted records drained after fixture release.

Fresh optimized offscreen recovery on the RTX 5070 Ti/Vulkan: **3/3 pixel-identical
recoveries**, **112.99 / 975.97 / 1025.37 ms** (repeated losses rate-limited), longest
UI-side poll **0.066 ms**, replay storage **262,144 bytes**. Injected recovery
stall: 1,000 polls **14.7 us**, drop **5.9 us**, one attempt while blocked. These
tests destroy only their own offscreen device, not an adapter/driver or app window.
The offscreen harness uses its own event observer; the actual main callback is
covered separately by the saturated-log-queue regression. No native swapchain,
drag/close/soak claim. No visual code changed; alpha.24's 93 PNGs were not rerun.

Candidate: **`target/release/trontop.exe`**, **0.3.0-alpha.25**,
**13,551,616 bytes**, built **2026-09-05 22:41:17.966 UTC** from modified 735fa33 source.
SHA-256 **`737223AF89168D93A6057697D3973086A89D99EE64E20909DD2DA7ECAE019120`**.
PE import inspection shows only Windows libraries; no clean-machine claim.

### Next scoped wait: settings persistence (A13/A21/A22)

The exact cached `eframe-0.35.0/src/native/file_storage.rs` was inspected:
`flush` (line 167) joins a previous save thread without deadline; `Drop` (108)
joins the final save; `save_to_disk` (197) directly creates/truncates the target;
`read_ron` (230) loads synchronously without a size limit. Native integration calls
storage flush after autosave/app-save. Trontop currently uses that storage for
`state-v2.ron`, including the theme/library, and egui memory persistence is enabled.
Main also creates the state directory synchronously. These are code-level waits,
not a measured attribution of Trent's reported close lag or crash.

Next implementation must avoid UI-thread waits while preserving old themes and
named saves, with staged writes and explicit pending/error/close behavior. Do not
silently detach an unsaved theme write to claim fast close. Test blocked writer,
save coalescing, failure/retry, migration and shutdown policy using owned fixtures.
No persistence code or user settings changed in alpha.25. A separate startup
wait remains in `TrayController::new`: `ready_rx.recv()` and the failed worker's
join can wait on slow shell initialization. No tray code changed this slice.

Local only. No preview launched/replaced/closed, no global input and no new upload
attempt. The older previews were untouched. Relaunch, isolated-desktop and explicit
source-upload approval remain unanswered. Remote is still last verified alpha.21;
there is no alpha.25 CI, tag or published release. The ledger and goal remain open.

## Previous candidate: alpha.24 compact controls and History

Long dialog names no longer move action buttons. Fixed-height identity cards
retain full names on hover; PID/service identities have their own lines. History
reserves numeric tracks and uses compact alternating rows. Affinity has responsive
CPU tiles with a capped scroll area and a persistent review/cancel footer; final
confirmation shows the requested CPU ranges and count. Native action guards and
the alpha.20 recovery path are unchanged. See `COMPACT_CONTROLS.md`.

Final ordinary gate: **209 passed, 0 failed, 13 ignored** (30.06 s), strict Clippy
PASS (2.07 s), format PASS, optimized build PASS (47.59 s). Seven new regressions;
**93 offscreen PNGs** generated (62.99 s), all 14 new compact cases reviewed across
the pass. The two final affinity confirmations were reviewed after adding the
CPU-set summary. This is not native dragging, DPI-transition or close evidence.
The final optimized CPU-only UI/tessellation probe reports p95 **0.23-0.98 ms**
across nine pages with 500/5,000 fixture processes. Not an A/B speedup or native
frame/present/soak measurement; see `COMPACT_CONTROLS.md` for scope.

Candidate: **`target/release/trontop.exe`**, **0.3.0-alpha.24**,
**13,537,792 bytes**, built **2026-09-05 22:23:54 UTC** from modified e86c06e source.
SHA-256 **`0591BBD15332B9D56CF194CEC5FECDD076C4F977FBDCA4773891AD3EB50319C2`**.
Windows-only import inspection passes; clean-machine testing remains open.

Local only. No preview was launched/replaced/closed and no desktop input was sent.
The prior five older previews were left alone. The explicit relaunch question and
isolated-desktop permission are unanswered; automatic continuation is not consent.
The source-upload rejection also remains in effect; no push/tag/release/CI request
was attempted. A14/A15/A16 and the broader goal remain incomplete.

## Previous candidate: alpha.23 custom-theme readability

The A16 audit separated raw theme accents from text ink, bounded text-bearing
hover/selection/banded surfaces, corrected action-button foregrounds per state,
and kept badges, hints, warnings, numbered pegs, chart lines and meters readable
under extreme colors. Saved palettes are unchanged. Button border widths are
stable across states. See `THEME_CONTRAST.md` for implementation and limits.

Final gate: **202 passed, 0 failed, 13 ignored** (26.68 s), strict Clippy PASS
(4.35 s), formatting PASS and optimized build PASS (48.77 s). Six new contrast
tests include real egui widget shapes/states and unchanged text bounds. The final
offscreen pass generated **79 PNGs** (50.46 s); selected regular and extreme
Processes, History and Theme Studio cases were inspected. The rendered review
caught an additional nearly invisible yellow sidebar meter, now corrected.
Mid-pass optimized UI/tessellation p95 was **0.18-1.05 ms** across nine pages at
500/5,000 fixture processes, before the final meter correction. Not native FPS.

Candidate: **`target/release/trontop.exe`**, version **0.3.0-alpha.23**,
**13,532,672 bytes**, built **2026-09-05 21:50:47 UTC** from modified f321e07 source.
SHA-256 **`EE3B3B3086B42F63022D291243B893FEED0361BCB4476FC68BC0CC16C01D4996`**.
PE imports still list Windows DLLs only, without a dynamic MSVC CRT. Clean-machine
portability and native drag/close/soak remain unverified. No preview was opened,
replaced or closed, and no OS input was injected. The exact-build review request
and isolated-desktop permission remain unanswered.

This checkpoint is local only. The previous source-upload rejection remains in
effect; no push, CI job, release or tag was attempted. No automatic continuation
counts as approval to bypass it. A16 remains PARTIAL / REVIEW, not checked off on
the strength of the selected-widget tests alone.

## Previous candidate: alpha.22 non-blocking process and shell actions

The UI-thread audit found five native action calls still inside rendering callbacks:
End task, priority, affinity, Run task and Reveal in Explorer. All now use a
dedicated single-flight worker. Confirmed process identity and display target are
frozen; existing same-handle native guards remain. No duplicate submission or
automatic retry, and no join on a stalled call during shutdown. See
`PROCESS_ACTION_SAFETY.md` for exact behavior and limitations.

The UI requests an immediate pending-state repaint, stays navigable during a slow
call, and shows an honest five-second waiting message. A late outcome identifies
the original target even after selection changes. Message text has stable height,
horizontal insets, hover detail and a neutral pending tint; success is only
reported after the native worker returns. The redesign skill's targeted state and
spacing audit informed that small status-bar pass, not a new branding redesign.

Final local gate: **196 passed, 0 failed, 13 ignored** (28.76 s), formatting and
strict Clippy PASS (2.67 s), optimized EXE build PASS (46.16 s). Ten new tests cover
worker dispatch/stalls/validation plus actual UI confirmation/navigation/late-result
behavior. Native mutation checks use only an owned hidden disposable test child;
shell/Explorer UI paths use injected backends, not external programs. A stalled
fake call measured **25.3 us for 1,000 polls**, **4.9 us worker drop**, with duplicate
rejection. These are scoped worker timings, not end-to-end native close latency.

Final offscreen pass: **73 PNGs** (49.35 s), pending compact, slow light and error
compact reviewed after final tint/padding changes. At minimum size a visible status
bar leaves the navigation rail scrollable; it is not a claim that every navigation
item is simultaneously visible. Mid-pass optimized UI/tessellation probe: p95
**0.11-1.12 ms** across nine pages with 500/5,000 fixture processes, before the final
pending-status inset/repaint tweak. No native presentation/FPS conclusion follows.

Candidate: **`target/release/trontop.exe`**, version **0.3.0-alpha.22**,
**13,494,784 bytes**, built **2026-09-05 21:21:39 UTC** from modified a6dfbe1 source.
SHA-256 **`6563FDABFFC66A2A12ED7FD3FD79A7637D0A895F6AB54E9986E7D18682658C1E`**.
PE import inspection lists Windows DLLs only, not dynamic MSVC CRT or application
assets. Clean-machine portability is still a separate gate. This replaces the
alpha.21 target/release artifact; old identities below are historical.

Crash follow-up recheck (September 5, 21:32 UTC): the local failure log and a
12-hour Application-event query still identify the alpha.19 renderer crash at
18:29 UTC; no newer Trontop incident was returned. Five older previews (alpha.10,
alpha.11, two alpha.12 copies and alpha.14) remain running and were left untouched.
The alpha.22 release EXE still matches the SHA-256 above. Fresh optimized tests
passed: three offscreen device-loss recoveries restored pixel-identical UI in
132.71 / 959.70 / 1006.52 ms, with a longest UI-side poll of 0.087 ms. The separate
injected setup-failure/stall check passed with one retry attempt, 1,000 polls in
15.3 us and worker drop in 10.2 us. These test-owned offscreen devices do not
exercise native window dragging/presentation or establish why the original GPU
upload failed. Competing compilation is not a verified cause of the crash.

No app preview was opened, replaced or closed; no global input/window manipulation,
driver installation, service command, shortcut hook or public release. Native
surface recovery, real drag/close and mixed-load soak remain unverified. Permission
for a separate non-visible desktop remains unanswered. Suspend/resume, CPU sensor
access and remaining ask-ledger gates are not quietly treated as completed.

Previous exact alpha.21 Windows CI
[33991747758](https://github.com/TrentSterling/trontop/actions/runs/33991747758)
for a6dfbe1 completed successfully (checked September 5). Alpha.22 remote gate has
not run yet. Source checkpoint f321e07 is local only: auto-review rejected its push,
and explicit approval to upload it to the private repository was requested but
has not been received. Do not retry or bypass that block without approval. There
is no alpha.22 CI job to monitor yet. No new release/tag is published.

## Previous candidate: alpha.21 compact Processes and inspector

Focused A15/A16 polish on top of alpha.20 recovery. An empty inspector no longer
reserves table width. The toolbar Inspector toggle reclaims that width without
clearing selection or search. At 1040x640, all seven default process columns fit
when the inspector is hidden. When shown at that size, the telemetry cards switch
to two rows; full metric values remain readable. The table deliberately retains
horizontal scrolling when both the inspector and columns cannot fit.

Inspector names now have a single-line title with full-name hover text. PID and
account use separate aligned detail rows, so long names/accounts cannot push the
remaining fields around. Truncated metric descriptions also have full hover text.
This is a targeted existing-layout audit, not another visual identity redesign.

Final local gate: **186 passed, 0 failed, 13 ignored** (29.22 s), strict Clippy,
formatting and optimized build PASS (release EXE build 41.88 s). Four new compact
tests cover dark/light, logical 1040x640/1280x760, selected/hidden inspectors,
long identity text, toolbar/column clipping and local toggle state retention.
The header test also uses egui scale factors 1/1.25/1.5/2; this does not test native
mixed-monitor DPI transitions. Offscreen pass: **70 PNGs** (48.95 s); Processes,
normal/compact inspectors, hidden compact inspector and wide light Processes
inspected. The final two inspector images were rechecked after the title fix.

Selected optimized recovery retest: **3/3 pixel-identical recoveries**,
**127.45 / 1003.43 / 998.75 ms** (repeat-loss retry spacing is intentional),
longest polling call **0.088 ms**, replay buffer **262,144 bytes** for this fixture.
Nine-page UI CPU/tessellation p95 at 500/5,000 fixture processes: **0.10-0.95 ms**.
These short offscreen probes exclude native presentation, dragging and whole-app
sampling overhead; differing fixture counts are not a scaling benchmark.

Candidate: **`target/release/trontop.exe`**, version **0.3.0-alpha.21**,
**13,462,528 bytes**, built **2026-09-05 20:55:02 UTC** from modified fb4c88f source.
SHA-256 **`DA27B0AD97FB7338E839131FB0EF6B9370076170E45A525753EFBEE95E648BF6`**.
PE imports list only Windows DLLs, no dynamic MSVC runtime. This is not a
clean-machine portability result. This build has not been launched or substituted
for an existing preview. The previous alpha.20 target/release artifact was replaced
by this build; its identity below is historical.

Native surface recovery, drag/close/soak, mixed-monitor DPI and final app-wide
layout/contrast acceptance remain open. Wide-table spare-width allocation also
needs a final pass without breaking manual column resizing. No desktop input,
window manipulation, cross-project changes or release publication. Permission for
a separate non-visible native test desktop remains unanswered. Alpha.20 Windows CI
[33990642654](https://github.com/TrentSterling/trontop/actions/runs/33990642654)
for fb4c88f completed successfully at **20:58:28 UTC**, including tests, both strict
Clippy checks, optimized build and artifact upload. Alpha.21 remote CI has not run
yet; record its exact run in the coordination journal after the private push.

## Previous candidate: alpha.20 renderer recovery / UI handoff

Trent reported that alpha.19 froze and closed. Both local failure metadata and
Windows events confirm its renderer panic at **18:29:11.956 UTC**. The resumed
objective is to fix responsiveness/stability and polish the app, still scoped by
`../ASK_LEDGER.md`, not to add unrelated features. `RENDERER_RECOVERY.md` documents
the exact failure branch, patch, evidence and limitations.

Alpha.20 replaces the UI's blocking snapshot read/deep clone with non-blocking
ownership transfer. Its repository-local egui-wgpu patch rebuilds a lost device
on one background worker, replays managed textures and recreates surfaces on
existing windows. It suspends stale process actions during recovery. No app,
window, global input, driver or unrelated project was manipulated for testing.

Final local gate: **182 passed, 0 failed, 13 ignored** (28.83 s), root/vendor strict
Clippy and formatting PASS; optimized build PASS (1m 17s). Selected optimized GPU
fault tests: three pixel-identical recoveries, first **210.72 ms**, repeated immediate
losses **888.40 / 1018.02 ms** (intentional retry spacing), longest polling call
**0.088 ms**. Injected stalled setup: 1,000 polls **17.6 us**, drop **4.6 us**, no
extra worker. UI-only p95 including CPU tessellation: **0.13-1.01 ms** over nine
pages at 500/5,000 fixture processes. This is not native drag/present FPS.

Offscreen pass generated **67 PNGs** (49.44 s); compact About and Processes inspected.
Existing compact horizontal table scrolling is unchanged, not a completed A15 audit.

Candidate EXE: **`target/release/trontop.exe`**, version **0.3.0-alpha.20**,
**13,458,944 bytes**, built **2026-09-05 20:35:16 UTC**, build ID
`5e845dffcb9804fc52dab1e7dfa16322490f0033+modified`.
SHA-256 **`16F2655AA3AFE23996A2AFAF51DC15D95082AD4189B6E4A3CDEC2F8671E1518F`**.
PE import inspection lists Windows DLLs only, no dynamic MSVC runtime or app asset
dependency. This is not clean-machine portability proof. The old
`target/review-build/release` still contains alpha.19; do not open that as latest.

No alpha.20 preview was launched/replaced. Native surface recovery, real dragging,
close timing and mixed-load soak remain unverified. A separate non-visible desktop
test was requested but not authorized yet. Do not interfere with existing previews.
No release/tag/publication or cross-project patch. Alpha.20 private CI subsequently
passed; see the newer checkpoint above for its exact run and completion time.
Previous alpha.19 Windows CI 33983943962 passed for 5e845df (checked September 5).
The repository is still private. Source checkpoints are not released artifacts.

## Latest explicitly requested preview: alpha.19

On Trent's explicit "open the new build" request, the hash-verified optimized
alpha.19 EXE was copied to `target/preview/alpha19-20260905-182659/trontop.exe`
and opened at **18:27:00 UTC**, PID **280516**. Initial read-only observation:
Responding=true and a native window handle present. SHA-256 matches the alpha.19
review identity below. Older instances and all unrelated windows were untouched.
No automated UI input or feature testing was performed. This launch does not close
the native persistence/visual acceptance gates in `../ASK_LEDGER.md`.

## Previous explicitly requested preview: alpha.17

On Trent's explicit request, the hash-verified optimized alpha.17 EXE was copied
to `target/preview/alpha17-20260905-134108/trontop.exe` and opened at
**13:41:08 UTC** on 2026-09-05 as **PID 275020**. A subsequent read-only desktop
check confirmed Responding=true, HWND 9577990 and title Trontop. Version, size
and SHA-256 match the alpha.17 review build below. Older instances and other
windows were untouched. This is a launch observation, not a native performance
test. Do not restart or replace this preview without fresh permission.

## Previous explicitly requested preview: alpha.16

On Trent's explicit request, the optimized alpha.16 review EXE was hash-verified
and copied to `target/preview/alpha16-20260905-1251/trontop.exe`. It launched at
**12:51:50 UTC** on 2026-09-05 as **PID 272352**. A read-only check from the desktop
session confirmed Responding=true, HWND 6370918 and title Trontop. Sandboxed window
enumeration could not see it; that was not evidence of an app startup stall.
Version **0.3.0-alpha.16**, **13,224,448 bytes**, SHA-256
`54B051804B52548B4386DB089601655723FFECA8AED54DEDC6A86DA2E2406918`.
Older instances and other windows were untouched. Do not automatically replace,
restart or manipulate this preview. This observation is not an ongoing liveness
guarantee or a native performance test.

## Previous source: alpha.19 four-peg themes and finite ask ledger

Trent called out diminishing returns and requested one checklist of every ask with
a clear stopping condition. **`../ASK_LEDGER.md` is now authoritative for product
completion.** The older task board is historical engineering detail. No new feature
work until the next bounded unchecked ask is chosen with Trent. Do not convert an
unverified or difficult ask into a checked item to declare the app done.

Four independently positioned/colorable gradient pegs now render as exact linear
bands at any angle. Theme Studio adds Palette/Appearance/Presets/My themes tabs,
hex edits, native egui local drag/keyboard controls, named saves and import/export,
revert/reset, eight presets and legacy v2 migration. Its footer is pinned outside
the scrolling editor body. Background brightness is constrained for ordinary text;
the whole-app arbitrary-accent audit is still open. `THEME_STUDIO.md` has limits.

Final local gate: **175 passed, 0 failed, 10 ignored** (22.57 s), formatting/strict
Clippy PASS, optimized release PASS (31.31 s). Offscreen pass: **67 PNGs**, with
Palette dark/light/compact, Appearance, Presets and My themes reviewed. No native
window, global input, focus changes or tray tests. Real eframe restart persistence
has not been tested by closing any existing preview; A13 remains PARTIAL/REVIEW.

Review EXE: `target/review-build/release/trontop.exe`, **13,407,232 bytes**, version
**0.3.0-alpha.19**, built **2026-09-05 18:20:28 UTC** from modified 0a176d3.
SHA-256 `18FE0B97338C3DCEE74F83568FCFC9C2D1D0B0A3C5AEE4F00888895E79D62F13`.
Dependency scan lists only Windows DLLs, no dynamic MSVC runtime or application
asset dependency. Clean-machine portability remains unverified. No alpha.19 preview
was launched, no old instance closed, no driver/hotkey/release installed/published.

Alpha.18 local checkpoint: 0a176d3. The last verified remote checkpoint is alpha.17
run 33969106100 (02ea7f8). Alpha.19 remote CI is pending; record the exact new run
in the coordination journal after push. Do not launch duplicate verification runs.

## Previous source: alpha.18 physical disks

Independent bounded Windows PDH collection now provides physical-disk active time,
response time, queue depth and read/write rates. Volume entries remain separate.
Missing/cached readings retain their fields and chart gaps. JSON export includes
metric freshness; raw instance names require private-details opt-in. See
`PHYSICAL_DISKS.md` for semantics, limitations and the native read-only probe.

Final local gate: **166 passed, 0 failed, 10 ignored** (23.05 s), strict Clippy and
formatting PASS, optimized release PASS (38.58 s). The offscreen pass generated
**60 PNGs** (34.28 s); physical disk dark/light/compact were inspected. No native
window or desktop input was used. Probe: five valid fields on three disks; cold
query 136.823 ms, subsequent queries 0.210/0.168/0.188 ms. Not whole-app timing.

Review EXE: `target/review-build/release/trontop.exe`, **13,337,600 bytes**, version
**0.3.0-alpha.18**, SHA-256
`FAB727152D5C54DCFF27567837B5B288DF716BA1FAAE7D8F9DC06CF737FA47DC`.
No alpha.18 launch; the earlier requested previews are unchanged. Alpha.17 CI
[33969106100](https://github.com/TrentSterling/trontop/actions/runs/33969106100)
passed for 02ea7f857245d49df18681eee602a9d883a54b58. Alpha.18 remote gate pending.
No release published. CPU temperatures and full Task Manager parity remain open.

Next explicit request: four-peg color gradients and a more complete, polished
theme system. Preserve old saved themes, avoid global desktop testing.

## Previous source: alpha.17 bounded local failure reporting

Rust panics and errors returned by the native runner now leave best-effort local
metadata beside settings. The JSONL log keeps the latest 32 records and excludes
raw messages, process details, absolute source paths and dumps. Non-waiting OS
locking coordinates instances; malformed/oversized/readonly logs are preserved.
About exposes the location and limits, with explicit copy only. Its compact
opening height now keeps the added controls visible. `FAILURE_REPORTS.md` documents
data, retention, failure coverage and the safe child-process verification.

Local gate: **153 passed, 0 failed, 9 opt-in ignored** (25.43 s), formatting/strict
Clippy PASS, optimized review build PASS (32.47 s). Ten new tests include the hidden
panic child entry point; real hook-before-unwind/abort tests never touch the running
GUI. Offscreen pass produced **57 PNGs**; About dark/light/compact were reviewed.
No live user failure log was generated. No native drag/close/soak claim is made.

Review EXE: `target/review-build/release/trontop.exe`, **13,270,016 bytes**, version
**0.3.0-alpha.17**, built at **13:25:26 UTC** from modified df7e3fb source.
SHA-256 `0E9F89C8B983A0CDD2C24C8D4197AB2A0FDD85B21EE5F92A2A87BE0AB46CD9A6`.
The subsequent explicitly requested alpha.17 preview launch is recorded above;
no existing instance was replaced.

Alpha.16 Windows CI [33967714299](https://github.com/TrentSterling/trontop/actions/runs/33967714299)
passed for df7e3fbc4aea6f2a1ae49b65ac83156dd4fc8c4e. Alpha.17 remote verification is
pending. There is still no published release or full Task Manager parity.

## Previous: alpha.16 displayed sorting and retained tree state

Tree CPU/GPU/memory/read/write ordering now follows the full subtree totals shown
in each row, including hidden descendants. Partial GPU values sort numerically;
missing values remain last in either direction. Flat sorting is unchanged.
Expansion choices survive refresh and search without transferring across observed
PID reuse. Missing identity access retains the last known identity; continuous
unknown-only identity is explicitly best effort for presentation, not action authority.
Processes/Details switches reset unsupported hidden sort keys to CPU descending.
See `PROCESS_SORTING.md` for the contract and regression evidence.

Local gate: **143 passed, 0 failed, 9 opt-in ignored** (27.32 s), formatting and
strict Clippy PASS, optimized release PASS. Offscreen pass produced **57 PNGs**;
grouped sorting dark/light, deep tree and Details were actually visually inspected.
Review build at `target/review-build/release/trontop.exe` was produced at 12:42:26 UTC
from modified 7df48c9 source, with the same identity as the preview above. Windows-only
imports were verified; clean-machine portability remains open. No new FPS claim.

Alpha.15 Windows CI [33965959216](https://github.com/TrentSterling/trontop/actions/runs/33965959216)
passed for 7df48c99c8e6bcff5d653ff8f9eab3460139ec17. Alpha.16 remote verification is
pending. No release is published and full Task Manager parity remains incomplete.

## Previous explicitly requested preview: alpha.14

On Trent's request to open the latest build, the alpha.14 review EXE below was
hash-verified and copied to `target/preview/alpha14-20260905-1200/trontop.exe`.
It launched at **12:00:07 UTC** on 2026-09-05 as **PID 273992**; a subsequent
read-only check observed Responding=true, native HWND 1181862 and title Trontop.
Version **0.3.0-alpha.14**, **13,236,224 bytes**, SHA-256
`8E847CEA142ABC05B88F8F5E8AEE16EEEAF16DFA987B39B5BD737AEA617CA63C`.
Older instances and all other windows were left untouched. No global input or
window manipulation was used. This is a launch observation, not a native smoke
test or ongoing liveness guarantee. Earlier launch notes below are historical.

## Previous: alpha.15 iterative process hierarchy

Branch `feat/provider-diagnostics`. Hierarchy building now uses iterative indexed
walks instead of recursion and repeated ancestor scans. Known newer-parent IDs are
detached; cyclic members become separate roots without losing valid children or
double-counting their resources. Full subtree totals and search ancestor context
are preserved. Deep indentation is capped for readability, with actual depth and
reported parent PID on name hover. See `PROCESS_TREE.md` for semantics and evidence.

Nine new ordinary regressions pass, including 50,000-level chains/cycles on a
256 KiB thread stack, 128 arbitrary graphs, 2,880 valid reference comparisons, and
compact light/dark scroll/tooltip/name-selection checks. Final gate: **134 passed,
0 failed, 9 opt-in ignored** (25.85 s), formatting/strict Clippy/release PASS.
Offscreen pass produced **55 PNGs** (34.14 s); the two deep-tree variants, normal
Processes and partial-GPU light tree were visually inspected, not all 55 images.

Three paired optimized headless timing runs measured 1,000-node collapsed-chain
building at **16,771.9 us before, 132.8 us after**; wide expanded **364.9 to 141.2 us**.
These are tree-construction medians, not native drag/FPS/whole-app measurements.
The alpha.15 review EXE is **13,212,160 bytes**, built from modified 5b000f4 source
at 12:17:49 UTC. SHA-256
`8087381610511F7A219182CC5E1308A182CF2EC36B621F29989FC605827CE7C5`.
Path: `target/review-build/release/trontop.exe`. Windows-only import scan passed;
clean-machine testing remains open. This EXE was not launched. The explicit
alpha.14 preview above was left untouched.

Alpha.14 Windows CI
[33964418983](https://github.com/TrentSterling/trontop/actions/runs/33964418983)
completed successfully for 5b000f4328701f3c051740da6464d4fbe6e3e83a. Alpha.15 has
not yet passed its remote gate. No release is published. Full Task Manager parity
remains incomplete. Next process-view issue: sort resource columns by the displayed
subtree values; current ordering still uses individual process counters.

## Previous: alpha.14 snapshot-indexed process views

Branch `feat/provider-diagnostics`. Processes/Details no longer clone every displayed
process, command line and path on each repaint. Flat views hold indices into the
current snapshot; Copy tree metadata references the same source. History caches its
top twelve indices on view rebuild rather than cloning/sorting the whole filtered
set every frame. ASCII-folded name/account ordering uses allocation-free comparisons.
Indices are rebuilt at snapshot replacement and are not cross-sample identities.
Selection and native action identity checks remain PID/creation-time based.

Measured before/after with an opt-in, optimized, headless production-UI probe:
three baseline/indexed pairs, 500 and 5,000 synthetic processes, five warmup frames
then 60 measured frames per page. At 5,000 processes the median-of-medians tree
frame fell **3,888.1 us to 206.9 us**; flat **4,106.8 to 205.7 us**, Details
**4,389.5 to 299.0 us**, History **4,850.4 to 79.8 us**. At 500 processes the tree
frame fell **307.6 to 211.2 us**. View rebuild mean also improved. Full methodology,
binary identities and limitations: `PROCESS_VIEW_PERFORMANCE.md`. These are
CPU-side headless timings, not a native FPS, drag or whole-app overhead claim.

Final local gate: formatting PASS; **125 passed, 0 failed, 8 opt-in tests ignored**
(26.30 s); strict Clippy PASS (3.13 s); optimized release PASS (32.49 s). Four new
ordinary tests cover index/sort equivalence, snapshot replacement/empty/filter,
History invalidation, reused-PID selection safety and repaint storage reuse. The
eighth opt-in test is the new headless timing probe, not a native desktop test.
Offscreen QA produced **53 PNGs** in 34.20 s; Processes, Details, History and the
partial-GPU light tree were visually reviewed, not all 53 images.

Review EXE: `target/review-build/release/trontop.exe`, **13,236,224 bytes**, PE version
**0.3.0-alpha.14**, built from modified 3142016 source before checkpoint.
SHA-256 `8E847CEA142ABC05B88F8F5E8AEE16EEEAF16DFA987B39B5BD737AEA617CA63C`.
Final dependency inspection shows only Windows imports, no dynamic MSVC runtime;
clean-machine portability remains unverified. No preview was launched during the
performance implementation; the subsequent explicit alpha.14 launch is recorded above.

Alpha.13 Windows CI
[33963422802](https://github.com/TrentSterling/trontop/actions/runs/33963422802)
passed formatting, tests, strict Clippy, release and artifact upload for
3142016d43a7b9937ff46cb04a2ea64e10f9cc1b at 11:49:30 UTC. Alpha.14 has not passed
its remote gate. No release is published.

Full Task Manager parity remains incomplete: sensors/vendor coverage, suspend/resume,
startup controls, richer device counters, native export/service validation, crash
logging, actual performance/accessibility/soak and release checks remain open.

## Previous: alpha.13 explicit JSON/CSV export

Branch `feat/provider-diagnostics`. Export now offers JSON for the published sampler
snapshot and CSV for every process row. The user explicitly chooses a destination
through Windows Save As. Capture happens on Save, not when the panel opens or the
picker completes. Encoding/file I/O use one export worker; live sampling continues.
Private details default off and reset on idle reopening together with the previous
result, so an old private save cannot label new limited options Saved. Names still
identify software; this is not an anonymous report. See `EXPORTS.md` for schema 1,
scope, CSV formula-text handling, privacy, size limits and cancellation caveats.

The writer exclusively stages a sibling temporary file, then flushes/syncs/closes
before replacement. Cancellation, encoding limits and locked-target tests preserve
the original fixture. Worker drop signals cancellation without joining a blocked
picker/filesystem call. App exit can leave partial temporary files; it does not
promise forced cancellation or secure deletion. A native Save As end-to-end test
remains an explicit isolated-validation release gate. No picker was opened on the
working desktop, and no real process snapshot was exported in this slice.

The redesign skill's state/layout audit guided a wrapped privacy warning and pinned
Save/Close/status area. New tests require text inside both screen and clip bounds at
1040x640 in light/dark, stable outcome geometry, disabled missing-sample dispatch,
explicit-only capture, and private-option reset. The three final layout PNGs reviewed
were `export`, `export-private-light` and `export-failed-compact`; all use fixtures.

Final local gate on 2026-09-05: formatting PASS; **121 passed, 0 failed, 7 opt-in
tests ignored** (27.64 s); strict Clippy PASS (1.58 s); optimized release PASS
(31.77 s). The selected offscreen pass generated **53 PNGs** in 34.30 s with no
native window or OS input. Three export PNGs were inspected, not all 53. The final
reopen-result reset followed that render pass; its headless input test passed in
the final suite and it does not change panel geometry.

Optimized review EXE: `target/review-build/release/trontop.exe`, **13,239,296 bytes**,
PE version **0.3.0-alpha.13**, built from modified cc2b793 source before checkpoint.
SHA-256: `BBA6F177B6B8A97EFFA3F4FF34600F3406660B2E35AE45FB65095608C6F75C16`.
Final dependency inspection shows only Windows imports, no dynamic MSVC runtime;
this is not clean-machine validation. This alpha.13 EXE was not launched.

Alpha.12 Windows CI
[33961633447](https://github.com/TrentSterling/trontop/actions/runs/33961633447)
passed for cc2b793184431abd02116f9eb54f19fceb2b124a at 11:09:25 UTC.
Alpha.13 has not yet passed its remote gate. No release or tag is published.

### Latest explicitly requested preview launch

On Trent's latest launch request, the final alpha.12 review build was hash-verified
and copied to `target/preview/alpha12-final-20260905-111553/trontop.exe`.
It launched at **11:15:53 UTC** on 2026-09-05 as **PID 262932**, subsequently observed
Responding=true with native HWND 3553034. Version 0.3.0-alpha.12, 13,109,248 bytes,
SHA-256 `A11F950774733466C09D08DEC7594E418E71BE91164C43A282F9CE2ADE2D6678`.
The three older preview instances were left alone. This is historical launch
evidence, not permission to restart it, close old instances or manipulate windows.

Next: native export/service isolated validation, CPU/motherboard/vendor sensors,
suspend/resume, reversible startup controls, richer disk/network/GPU data, crash
logging, performance/accessibility review and release gates. Full Task Manager
parity, native close/drag timing and clean-machine/soak coverage remain unproven.

## Previous: alpha.12 independent inventories and retained service state

Branch `feat/provider-diagnostics`. Startup and Services each have one independent
read-only inventory worker. They no longer enumerate on the live system sampler.
Refresh requests coalesce; a five-second reporting timeout keeps cached fields and
does not spawn another worker. Immutable snapshots use nonblocking publication reads.
Workers have bounded shutdown waiting, not forced native-call cancellation. Other
sysinfo/PDH/NVML/process-control paths can still delay sampling. Full architecture,
cadence, source cache behavior and limitations: `INVENTORY_WORKERS.md`.

Service state observations and unresolved outcomes now survive subsequent commands
to other services. The bounded 256-service cache never silently evicts an unknown
outcome; new targets require a successful refresh when capacity is exhausted.
Only newer complete inventory resolves those uncertainty records. Detailed progress
and error text remains latest-command-only, not a persistent activity journal.
See `SERVICE_CONTROLS.md` for age rules and native-command validation limits.

The redesign skill's state/alignment audit guided timeout and retained-result
surfaces while preserving Trent's gradients, rounded controls and zebra bands.
Regression tests require retained rows and unchanged table-header geometry across
timeout and worker-failure states in light/dark modes. The three final reviewed
PNGs are `startup-timeout`, `services-timeout-light`, and `service-outcomes`.
The last of these distinguishes Command read from Pre-command rows after three
different service commands. These are synthetic fixtures, not live service actions.

Final local rerun on 2026-09-05: formatting PASS; **111 passed, 0 failed, 7 opt-in
tests ignored** (26.03 s); strict Clippy PASS (1.27 s); optimized release PASS
(30.51 s). The specifically selected offscreen pass produced **50 PNGs** in 31.44 s
without native windows or OS input. Three new final images were inspected, not all
50. The read-only inventory-worker probe returned 13 Startup entries and 303 services:
0.7659 ms Startup collection, 1.6804 ms Services collection, 211.1 microseconds for
1,000 snapshot-pair reads. This is a short provider check, not a whole-app benchmark.

Final rebuilt review EXE: `target/review-build/release/trontop.exe`, **13,109,248
bytes**, PE version **0.3.0-alpha.12**, built from modified 8e343e2 source before
checkpoint. SHA-256:
`A11F950774733466C09D08DEC7594E418E71BE91164C43A282F9CE2ADE2D6678`.
Dependency inspection shows Windows-only imports, no dynamic MSVC runtime or
required vendor DLL/assets directory. The clean-machine release gate remains open.

### Alpha.12 preview launch (explicitly requested)

Trent requested the latest available build without touching old instances. A
hash-verified independent copy was opened at **10:42:33 UTC** on 2026-09-05:
`target/preview/alpha12/trontop.exe`, **PID 263640**. A subsequent read-only check
found Responding=true and native window handle 5976312. Its size is 13,109,248 bytes;
its SHA-256 is `47A7D048003B2CB133651864433F5D156CD020D2D1D60A1244C96F021E8BE3F3`.
This preview predates the final rebuild above and has a distinct binary hash.
It was not replaced or restarted after that rebuild. No old instance was closed,
and no focus, global input, move, minimize or restore commands were executed.
These are timestamped observations, not current liveness guarantees. Do not
automatically replace/restart any preview. The legacy `target/release` is alpha.5.

Alpha.11 private Windows CI
[33960026614](https://github.com/TrentSterling/trontop/actions/runs/33960026614)
passed formatting/tests/Clippy/release/artifact for
8e343e2d98f0e192f2b31da1153f9a736752aa9b at 10:32:13 UTC on 2026-09-05.
Alpha.12's remote gate is pending; that older success does not verify alpha.12.
No tag or alpha release is published.

Still open: CPU/motherboard and broader vendor sensors, suspend/resume, startup
enable/disable, export, native service-command validation, real close/drag timing,
soak/clean-machine and other release gates. Ctrl+Shift+Esc is proposal-only. No
driver/hook installation or desktop-test authority is implied. Not Task Manager parity.

## Previous: alpha.11 confirmed service controls

Branch `feat/provider-diagnostics`. Services now has selectable zebra rows,
Start/Stop/Restart actions, a named expiring confirmation, stable command-status
surfaces and an independent single-flight command worker. Native SCM state/PID
and Stop capability are rechecked on the same handle used for the operation.
Restart waits for Stopped before Start; errors warn that it can remain stopped.
No elevation, recursive dependent-service stop, host kill or configuration edit.
See `SERVICE_CONTROLS.md` for the full behavior, race limits and verification gap.

The redesign skill's state/alignment audit guided fixed-height controls and status
surfaces without replacing Trent's gradient/zebra styling. An uncertain command's
old row is labeled Pre-command rather than Live. A failed inventory refresh cannot
overwrite a newer command observation with cached state. The latest-command record
is not yet a per-service outcome journal. Slow inventory calls still share the sampler.

Final local rerun: formatting PASS; **101 passed, 0 failed, 6 opt-in tests ignored**
(26.14 s); strict Clippy PASS. Optimized release PASS (41.03 s). The specifically
selected offscreen pass produced **47 PNGs** in 29.50 s on RTX 5070 Ti/Vulkan, with
no native window or OS input. Five final images were inspected: normal, light and
compact Services controls, Restart confirmation, and uncertain/error state. Not all
47 were inspected. New service actions were tested only through an injected backend;
the native SCM probe queries status with read-only permissions. Real native command
execution requires a separately authorized isolated fixture before release.

Review EXE: `target/review-build/release/trontop.exe`, **13,090,304 bytes**, PE version
**0.3.0-alpha.11**, built from modified 22825c5 source before checkpoint. SHA-256:
`7AFD2B045B6369FEFDE0FD9BAB03E6C79A758C557231D3C8AFE1E037037AE7C7`.
Dependency inspection shows Windows-only imports and no dynamic MSVC runtime or
required vendor DLL/assets directory. This does not replace the clean-machine gate.

### Explicitly requested preview launches

On 2026-09-05 Trent requested opening the newest build without disturbing the old
one. Hash-verified independent copies were launched, leaving other windows alone:

| Preview | Path | Launch UTC | PID | Read-only result after launch |
| --- | --- | --- | --- | --- |
| alpha.10 | `target/preview/alpha10/trontop.exe` | 09:48:31 | 259420 | Responding; native window handle 3869444 |
| alpha.11 | `target/preview/alpha11/trontop.exe` | 10:02:27 | 274860 | Responding; native window handle 5453272 |

Alpha.10's hash is in the previous entry; alpha.11 matches the review hash above.
These are timestamped observations, not a promise that either PID remains alive.
The legacy `target/release/trontop.exe` remains alpha.5. No instances were terminated,
and no global input, focus, move, minimize or restore commands were used. Do not
automatically restart/replace these previews during subsequent work.

Alpha.10 private Windows CI
[33958007255](https://github.com/TrentSterling/trontop/actions/runs/33958007255)
passed for 22825c594bbb35dc4f2546f876de5d98902e7bd9 at 09:46:46 UTC on 2026-09-05.
Alpha.11's remote gate is pending. No tag or alpha release is published.

Still open: CPU/motherboard and broader vendor sensors, suspend/resume, startup
enable/disable, export, native service-command validation, real close/drag timing,
soak/clean-machine and other release gates. Ctrl+Shift+Esc is proposal-only. No
driver/hook installation or desktop-test authority is implied. Not Task Manager parity.

## Previous: alpha.10 stable startup and service inventories

Branch `feat/provider-diagnostics`. Startup now retains results independently for
three Run keys and two Startup folders. Failed enumeration preserves the last known
entries for that source; a complete read can authoritatively remove entries. New
rows from an incomplete read are marked Observed, older rows Cached. Source cards
distinguish starting, live, empty, absent, partial, cached and unavailable results.
Services keeps its last complete list and explicitly labels stale reported states.
No startup enable/disable or service action support is implied.

The redesign skill's state/alignment audit guided fixed-height status surfaces and
stable source fields/table headers, retaining Tront zebra, hover and rounded styling.
Startup retention is bounded per source. Alphabetical indices rebuild on inventory
refresh; both tables format only visible rows. Full behavior and limitations:
`STARTUP_INVENTORY.md`. Slow native inventory calls still share the sampler; this is
not a claim that every provider stall or refresh issue is solved.

Final local gate: formatting PASS; **84 passed, 0 failed, 6 opt-in tests ignored**
(27.99 s); strict Clippy PASS; optimized release PASS (53.98 s). The explicit
offscreen test produced **42 PNGs** in 28.13 s on RTX 5070 Ti/Vulkan. Selected visual
reviews covered partial Startup, compact starting/unavailable Startup, light cached
Startup, cached Services and light unavailable Services. Not all 42 were inspected.
The new geometry test covers 32 inventory page/theme/state combinations, and a
20,000-entry test verifies that one viewport formats fewer than 100 rows.
No native GUI, desktop input, focus or window manipulation was used.

Separate review EXE: `target/review-build/release/trontop.exe`, **13,041,664 bytes**,
PE version **0.3.0-alpha.10**, SHA-256:
`F2148ADF82876419CBDD8D9DAFF8FE05729562DDDB0748816954678F00ECDF1A`.
Built from modified 018a17a source before checkpoint. Dependency inspection shows
Windows-only imports, no dynamic MSVC runtime or required vendor DLL/assets folder.
It has not been launched or copied over the deployed alpha.5 EXE. A read-only process
check at 09:22 UTC on 2026-09-05 found no running Trontop process; prior PID 255824
is historical, not current runtime verification.

Alpha.9 private Windows CI
[33956631086](https://github.com/TrentSterling/trontop/actions/runs/33956631086)
passed formatting/tests/strict Clippy/release/artifact for
018a17aa7646def8b2601510f049dd2ce67b243d at 09:14:03 UTC on 2026-09-05.
Alpha.10's remote gate is pending. No tag or alpha release is published.

Still open: CPU/motherboard sensors, broader vendor coverage, suspend/resume,
service actions, startup enable/disable, export, real close/drag measurements,
soak/clean-machine/release gates. Ctrl+Shift+Esc remains a proposal only. Do not
install drivers/hooks, replace the deployed EXE or manipulate desktop windows
without the appropriate fresh permission. This is not full Task Manager parity.

## Previous: alpha.9 background executable icons

Branch `feat/provider-diagnostics`. Processes/Details rows and the inspector now
request real embedded executable artwork through one bounded background worker.
No native file queries happen on the render thread. The in-memory cache has at most
256 entries, 32 outstanding requests and eight texture uploads per frame. Failed
lookups back off; a failed refresh keeps prior artwork. Fixed-size vector fallbacks
keep names aligned before/after loading. The inspector icon is hover-only, not a
dead button. The redesign skill's state/alignment audit guided this integration;
Trent's gradient/zebra theme and existing vector vocabulary are preserved.

Only local fixed-drive executable paths pass the native provider's best-effort
ancestor checks. Remote/removable/reparse/cloud-placeholder paths fall back; no
shell extension handlers, target execution, downloads or new runtime assets.
Artwork is never used as process identity or publisher trust. Full architecture,
limits and primary API references: `PROCESS_ICONS.md`.

Final local gate: formatting PASS; **74 passed, 0 failed, 6 opt-in tests ignored**
(23.16 s); strict Clippy PASS; optimized release build PASS (43.91 s). Explicit
offscreen pass PASS, **36 PNGs** in 22.52 s on RTX 5070 Ti/Vulkan. Reviewed mixed
loaded/fallback light/dark rows, selected inspector and compact copper Processes.
The compact table remains horizontally scrollable; this is not universal layout or
native DPI verification. No native window/input/clipboard commands were executed.

The separately selected read-only icon probe extracted this test EXE's own embedded
icon in 2.4387 ms, then ran 40 repeats: GDI/USER resources (4, 2) to (4, 2). An earlier
run was 2.9378 ms with identical counts. This is a bounded resource-lifetime check,
not total app overhead or end-to-end closing/dragging performance.

Separate optimized review EXE: `target/review-build/release/trontop.exe`,
12,960,256 bytes, PE version 0.3.0-alpha.9, SHA-256:
`D17075C24E012E7465355BCDD8D494AE9548FDCA4DF34AFC3541683AC9C804B1`.
Built from modified 2282469 source before checkpoint. `dumpbin /dependents` shows
Windows-only imports, no dynamic MSVC runtime or required vendor DLL. It was not
launched. The separately deployed alpha.5 EXE was not replaced/restarted.

Alpha.8 private Windows CI
[33955277229](https://github.com/TrentSterling/trontop/actions/runs/33955277229)
passed formatting/tests/strict Clippy/release/artifact for
2282469d6df0c7da2f16b1a4f6044e1574d5a947 at 08:47:16 UTC on 2026-09-05.
That run is not verification of alpha.9, whose remote gate is pending. No tag or
alpha release has been published.

Next: startup still replaces its whole row cache after partial source failures.
Preserve rows and freshness independently per source and show their state on the
Startup page; service failures also need a visible cached-state explanation there.
CPU/motherboard sensors, native performance measurements and all remaining release
gates stay open. Ctrl+Shift+Esc remains a proposal only. No desktop automation.

## Previous: alpha.8 GPU lifecycle and truthful activity states

Branch `feat/provider-diagnostics`. GPU inventory reconciliation now preserves
existing PDH handles and their rate samples. Only newly added counters warm up.
Failed enumeration leaves existing handles owned and explicitly marks incomplete
coverage. A failed collection re-primes rates instead of bridging the failed interval.

- Engine identity includes adapter LUID, physical adapter and engine index. Per-PID
  utilization uses the busiest engine. Physical-engine load sums its PID readings;
  the machine summary is the maximum engine load. It no longer sums parallel engines
  or unrelated adapters into a misleading engine-type total.
- Missing GPU data is distinct from zero through process/Details lists, process-tree
  and account totals, inspector, summary cards, meters and sorting. Partial totals
  use `>=` with explanations. Unknown readings sort last in both directions.
- Group GPU totals remain sums of process peaks, explicitly described on hover,
  not whole-GPU utilization. Partial/unavailable machine samples make history gaps;
  compact meters and the tray GPU tooltip conservatively omit partial values.
- The redesign skill's state/alignment audit led to fixed-height short inspector
  status text with the full reason on hover, main-token PID/account contrast, and
  five new GPU-state visual fixtures. Existing rounded zebra/hover surfaces remain.

Verification: formatting PASS; 64 tests passed, 0 failed, 5 opt-in tests ignored
(22.48 s); strict Clippy PASS. Explicit offscreen pass PASS, 33 fixture PNGs in
20.05 s on RTX 5070 Ti/Vulkan. Reviewed compact Processes, light tree/inspector,
Users, partial GPU Performance and Overview. The extra inspector regression requires
identical neighboring-field geometry through five live/missing states in both modes.
Headless UI checks do not execute viewport or clipboard commands.

The separately selected native PDH probe retained 690 handles, refreshed inventory
in 2.938 ms, and produced 690/690 valid rates and 39 PID readings after refresh.
Earlier probe: 690 retained, 5.351 ms. These are short read-only lifecycle checks,
not benchmarks of total app overhead, dragging or exact Task Manager parity.

Separate optimized review EXE: `target/review-build/release/trontop.exe`,
12,910,080 bytes, PE version 0.3.0-alpha.8, SHA-256:
`E25B63533890B395EC3AFE9E7E6FABAB9FF8BA65A3B5E8F9119334133584B53D`.
Build PASS (39.62 s), from modified e6db3e7 source before checkpoint. `dumpbin`
shows Windows-only imports, no bundled vendor library/dynamic MSVC runtime or runtime
asset folder. The EXE was not launched. No deployed alpha.5 replacement/restart,
desktop input, focus change, native window manipulation or shortcut installation.

Alpha.7's private Windows CI run
[33953608787](https://github.com/TrentSterling/trontop/actions/runs/33953608787)
passed all steps, including artifact upload, for e6db3e734bcb5a8eed967f85b62b66330fbb9719
at 08:10:14 UTC on 2026-09-05. That run does not verify alpha.8; its remote gate is
still pending. No release/tag is published.

Next: finish per-source startup status, CPU/motherboard coverage, executable icons
and release gates. GPU process creation may await the 30-tick inventory cycle plus
rate priming. Per-adapter GPU pages/VRAM mapping remain open. The Ctrl+Shift+Esc
proposal remains research only, with no hook or system-setting changes.

## Previous: alpha.7 isolated drive temperatures

Branch `feat/provider-diagnostics`. Added a native read-only storage provider and
slim zebra/hover sensor rows with fixed numeric alignment, guided by the supplied
component studies and the redesign skill. GPU, drive and CPU provider labels are
explicitly scoped; a live GPU does not imply complete sensor coverage.

- SetupAPI enumerates opaque disk interfaces. Access-0 handles query only
  `StorageDeviceTemperatureProperty`; no sectors, SMART pass-through, writes,
  threshold changes, additional drivers, services or elevation.
- One storage coordinator and at most 32 drive workers, one request per worker.
  Successful requests repeat after 5 s; failed/slow completed requests back off
  60 s. At 1 s, cancellation is requested on that owned worker only. A stuck call
  retains its slot/resources until completion, including across unplug/replug;
  it cannot create replacement-thread growth or block the regular sampler/UI.
- Failures keep the last usable readings and original timestamp marked Cached.
  Unsupported data remains unavailable. Per-drive query costs and provider-health
  coverage are visible. Overview reports each drive's hottest available sensor.
- Descriptor bounds/indices are validated. The SSD's unset -274 C threshold and
  SDK missing sentinel are unavailable, never presented as valid limits.

Verification: 55 tests passed, 0 failed, 4 opt-in tests ignored (29.45 s); strict
Clippy PASS; optimized build PASS (48.18 s). Explicit offscreen pass PASS with
28 PNGs (20.77 s); reviewed compact/light/cached/unavailable Sensors and Overview.
New tests cover parser bounds, stale cache, worker limits, failed-query backoff,
lost worker state, blocked-query isolation, hotplug duplicate prevention, bounded
drop and stable sensor text geometry. UI checks use fixture data and no OS input.

The explicitly selected native runtime probe queried ONLY the previously identified
TEAM TM8FP6002T SSD: 45/45/43 C, query 7.2355 ms, warning 90 C, critical 95 C.
The earlier run was 44/44/41 C at 7.2946 ms. No HDD query was repeated this slice.
These are short read-only measurements, not a sustained runtime/close benchmark.

Separate review EXE: `target/review-build/release/trontop.exe`, 12,900,864 bytes,
PE version 0.3.0-alpha.7, SHA-256:
`94B0396710AA005A3F5C169FCAAF72A6282479F65EA46CE2D6D5C4883C39537D`.
Built from modified b157f7e source before checkpoint. Windows-only imports including
SetupAPI; no bundled vendor DLL, dynamic MSVC runtime or runtime asset directory.
Not launched. The separately deployed alpha.5 copy was not replaced/restarted.
No native window manipulation, global input or unrelated process changes.

CPU/motherboard, unsupported storage-controller coverage, wear/error counters and
the alpha release gates remain open. Cancellation is a request, not a guaranteed
driver completion; coordinator inventory itself can stall only the storage provider.
No new end-to-end closing/dragging measurement. Remote CI for alpha.7 is pending;
no release/tag has been published. See `SENSORS_PLAN.md` for sources and exact limits.

## Previous: alpha.6 diagnostics, overview and vector controls

Branch `feat/provider-diagnostics`. The existing running alpha.5 preview was NOT
closed, replaced, moved or relaunched for this work. The separate optimized alpha.6
review EXE is `target/review-build/release/trontop.exe`, 12,839,936 bytes, SHA-256:
`91BDB02B4A0AB2744F4E8B3093F27FC1A8693E94FF16BBBC155E8B2DEB3E19E6`.
PE version 0.3.0-alpha.6. Built from the modified bfe8689 worktree before checkpoint;
About honestly records that modified build identity. It has NOT been launched.
`dumpbin /dependents` shows only Windows imports, no bundled NVML or dynamic MSVC CRT.
No installer or runtime image directory was introduced. No alpha.6 remote CI/release.

Implemented and locally verified:

- Overview dashboard, Hardware sensors navigation and About/provider-health dialog.
  Reports contain only build/provider status, timing, coverage and static reason
  strings. Copy is explicit, never uploaded; headless tests do not access clipboard.
- Stable sensor fields during startup/failure; the last complete NVIDIA snapshot is
  labeled Cached with original freshness. Cached values do not extend live graphs.
  GPU Engine fields remain present with dashes, and absent history samples form gaps.
- Native read-only startup queries, service cache retention/failure metadata, periodic
  CPU frequency refresh, unknown-account labeling and validated PDH counter status.
- Worker waiting is bounded at shutdown: sampler detaches after signaling stop, tray
  posts quit with at most 100 ms join waiting. Threads retain their own query/resource
  ownership. A blocked-worker regression test passes; whole-app close latency has NOT
  been measured, and no claim covers renderer/settings-save delays.
- Original 18-symbol vector vocabulary for navigation, window controls, action buttons
  and tree chevrons. No emoji or icon-font dependency. Accessible names/tooltips remain;
  disabled actions cannot click or shift. Selected navigation has a thin accent marker.
- Four supplied inspiration boards and a generated Signal icon candidate saved under
  `docs/inspiration`. PNG inspection confirmed ARGB with transparent corner pixels.
  Candidate is not embedded/shipped; existing theme-aware Tront mark remains in use.

Verification: formatting and diff whitespace checks PASS; 48 tests passed, 0 failed,
3 opt-in tests ignored (26.50 s); strict Clippy PASS; optimized build PASS (54.44 s).
Explicit offscreen rendering PASS, 25 PNGs using RTX 5070 Ti/Vulkan. Inspected compact
Processes, Overview, Hardware sensors and About, plus light/cached variants. The
headless page matrix covers 432 page/size/theme/data combinations. No native window,
tray test, global input, focus changes or user process mutations were used for UI QA.

Still open: individual startup-source status retention/display, per-process GPU
missing-value semantics, full Task Manager parity and release gates. CPU/motherboard
and drive temperature fields are explicit placeholders, NOT connected sensors. The
SSD probe proved one supported native path but the HDD query stalled for 4.25 seconds;
integrate storage on its own bounded slow worker. GitHub sensor research is saved in
`SENSORS_PLAN.md`. No approval exists to install drivers/services or elevate Trontop.

## Previous: alpha.5 preview refreshed at Trent's request

On 2026-09-05 at approximately 01:24 CDT, replaced the old alpha.1 EXE with a new
optimized build from the uncommitted `feat/provider-diagnostics` worktree. Trent
explicitly requested killing/replacing his running instance. The old PID 62220
exited before replacement; the guarded stop command refused its changed process
set and terminated nothing. Confirmed no Trontop remained before deployment.

The previous EXE is recoverable at
`target/replaced-builds/alpha1-20260905-0124/trontop.exe`. Deployment retained the
existing `target/release/trontop.exe` path and did not edit settings. New PID 255824
was observed with a responsive native window titled Trontop and PE version
0.3.0-alpha.5. No desktop input, focus manipulation or other app changes were used.
This one refresh does not authorize automatic restarts during further iteration.

New EXE: 12,785,152 bytes; SHA-256:
`A43DCD9DE277B4717EA4376B1D5EB06ECCC7D2597454A7CFD948A81B9BD6E778`.
At this historical checkpoint the separate review EXE matched this hash. Imports
remain Windows-only, with no NVML or dynamic MSVC runtime dependency.

Local source checks: formatting PASS; 40 tests passed, 0 failed, 3 ignored;
optimized build PASS (48.95 seconds). Eight unused diagnostics warnings remain
because the About/provider-health UI has not yet been connected. Strict Clippy,
new diagnostics offscreen fixtures, and remote CI are NOT verified for alpha.5.
The launched build includes the preceding hover/sensors/action-safety features and
the in-progress sampler corrections, but is not a completed release milestone.

At the alpha.5 checkpoint the next work was to connect the About dialog and report
action and expose independent
provider freshness and stale inventories; preserve startup-source status detail;
finish GPU unavailable-versus-zero presentation; test failure/recovery and report
copy without accessing the real clipboard; then run the full gate and visual pass.
See `DIAGNOSTICS_PLAN.md` and the newer alpha.6 entry above for the completed portion.

## Previous: alpha.4 process-action safety

Version 0.3.0-alpha.4 binds End Task, priority and affinity actions to the exact native
process creation FILETIME. Each operation verifies identity and Windows-critical
status on the same handle used for the action. Missing/changed identity refuses the
action. Pending End Task names/targets do not follow reused PIDs; stale selections
clear. New process rows receive identity queries on their first sample.
See `PROCESS_ACTION_SAFETY.md` for behavior, native/headless tests and limits.

The offscreen pass exposed an action-button baseline offset next to Cancel. The
shared helper now scopes visual colors without nesting layout allocations. A new
headless test checks enabled/disabled geometry and visual-state restoration; both
confirmation PNGs were inspected again after the fix.

Local gate: formatting PASS; 35 tests passed, 0 failed, 3 opt-in tests ignored;
strict Clippy PASS; optimized release build PASS. The explicitly selected offscreen
visual pass passed with 17 PNGs, including valid/stale confirmations. Native mutations
were restricted to harness-owned hidden child processes. PID 62220 was left untouched.

Review EXE remains `target/review-build/release/trontop.exe`; 12,784,128 bytes,
PE version 0.3.0-alpha.4; SHA-256:
`D4FCD918DBC411A2967A3088F53023F3418F8FA808CFFEC205D1FAB91A34CCC3`.
`dumpbin /dependents` shows only Windows imports, no NVML or dynamic MSVC runtime.
It has not been launched. Code checkpoint `f2f725c` is pushed to the private repo and
passed Windows CI run `33946298909`: formatting, 35 tests, strict Clippy, optimized
build and artifact upload. See `CI_ALPHA4.md` for the separate CI EXE, checksum and
scope. Neither review EXE was launched. No release/tag is published.

The native NVML probe passed again after this checkpoint: 0.327-0.354 ms warm query
times in a short five-sample run. A separate read-only storage capability probe
returned three temperatures from the TEAM SSD, but the HDD attempt took 4.25 seconds.
Storage sensors are NOT integrated; a dedicated slow worker is needed. Exact evidence,
permission context and remaining work are in `SENSORS_PLAN.md`.

The release audit found substantive alpha gates still open: suspend/resume, service
actions, About/provider diagnostics, JSON/CSV export, crash logs, administrator paths,
and the mixed-load soak. Do not weaken those gates or call the product complete.
The next bounded About/provider-health slice has code-audit findings and a privacy-safe
support-report plan in `DIAGNOSTICS_PLAN.md`.

## Earlier: alpha.3 GPU sensors

Version 0.3.0-alpha.3 adds optional read-only NVIDIA sensors on the background
sampler. Performance > GPU Sensors shows per-adapter temperature, board power,
graphics/memory clocks, fan target and VRAM. Rounded theme-aware cards include
two-minute temperature/power histories with rolling peaks and explicit missing data.
UUID-keyed histories survive enumeration reorder and expire on disconnect.
Existing PDH GPU Engine/per-process usage is unchanged. Read `SENSORS_PLAN.md` for
provider scope, safe loading, tests, measured query timings and future CPU/storage work.

Final verification: formatting PASS; 30 unit/headless tests passed, 0 failed,
3 opt-in tests ignored; strict Clippy PASS; optimized release build PASS. Explicit
native read-only NVML probe PASS; explicit offscreen visual pass PASS (15 PNGs).
No interactive tray test or desktop input automation was used. New dark/light,
compact and unavailable sensor images were inspected. These images use fixture data.

Review EXE: `target/review-build/release/trontop.exe`, 12,783,104 bytes,
SHA-256 `C957FA769F89CE73D7A2B6967B8433D355B47951E23AA4E16F5EFCF7A4387A2A`.
PE version is 0.3.0-alpha.3. `dumpbin /dependents` shows only Windows imports, no
NVML or dynamic MSVC runtime dependency. Sensor support dynamically uses the installed
driver, but no vendor DLL is required to launch the app. Clean-machine QA remains open.

This build has NOT been launched. PID 62220 still runs the older alpha.1 release-path
EXE and was left untouched. No new drag measurement, cross-project patch, push, or
GitHub release was performed in this slice. The broad product goal remains unfinished.

## Earlier: alpha.2 hover and safe UI verification

Work resumed on the UI after the parked checkpoint below. Version 0.3.0-alpha.2
adds shared hover backgrounds for navigation, device tiles, metrics, detail rows,
stat cards, badges, graphs, table cells, passive labels and custom action buttons.
Selected items retain a distinct state. Theme Studio uses aligned, rounded zebra
control rows and scrolls on short windows; slider tracks now contrast with cards.
Performance rail/content scroll independently. The sidebar footer has reserved
space and cannot overlap its GPU meter at the tested minimum size.

`docs/HEADLESS_QA.md` documents the new harness, local-pointer navigation/selection
and scrolling tests, 336 page/size/preset/mode/data cases, and eleven actual egui-WGPU
offscreen PNGs. No native app window or desktop input is used. This supersedes the
older statement below that only two cell tests exist. The wider branding/icon and
app-wide zebra/detail polish are still incomplete; do not call the whole UI done.

Review EXE: `target/review-build/release/trontop.exe`, 12,756,992 bytes,
SHA-256 `42B4AB4D8D16DAF19D8CC71FA47DC39674961031B4842992FFD60CA5A2DD1FF8`.
The optimized build succeeds and PE metadata reports Tront / Trontop /
0.3.0-alpha.2. It has NOT been launched on Trent's desktop. The running PID 62220
was confirmed to be the older release-path EXE, not debug; it was left untouched.
No new native dragging measurement was performed. Trent's later feedback is that
dragging now feels noticeably smoother; preserve that observation without claiming
a verified 60 FPS fix. No cross-project patch or GitHub release is published.

Final alpha.2 verification: formatting PASS, 23 unit/headless tests passed with
2 opt-in tests ignored by default, strict Clippy PASS, optimized release build PASS.
The offscreen GPU test was explicitly selected and passed separately (11 PNGs).
The native tray test was not run in this continuation. The UI images use fixtures,
not live system samples; native drag performance and real sensor integration are
outside this gate.

Sensor expansion is planned in `SENSORS_PLAN.md`. A read-only 5070 Ti query confirmed
temperature/power/clocks/fan/VRAM availability on this machine. Those fields are not
yet integrated into Trontop; CPU temperature needs separate provider research.

## Earlier parked checkpoint (historical)

Read `DRAG_INVESTIGATION.md` for unresolved window-movement lag and the explicit
ban on global desktop input automation. No dragging fix was validated or ported.

The newest local checkpoint adds a dedicated native tray thread fed directly by the
sampler, a full-area CPU level with scrolling history, alternating gradient column
bands, padded/vertically aligned table cells, corrected footer space, a rebuilt Users
resource table, wider inventory tables, and scrollable inspector content. Ordinary
selected labels now use the high-contrast text token. A final source follow-up fixes
centered table labels caused by add_sized forcing a centered widget layout. Two
headless tests check actual text geometry and full-cell input. This follow-up is NOT
release-built or visually checked in the running app; the visual pass is not finished.

Final source verification: formatting PASS; unit tests 18 passed, 0 failed,
1 interactive tray test ignored; strict Clippy PASS. No desktop interaction was used
for the alignment tests, and the running executable was not restarted.

Pre-alignment checkpoint verification: formatting PASS; unit tests 16 passed, 0 failed,
1 interactive tray test ignored; strict Clippy PASS; release build PASS. Earlier in
this session the ignored native tray test was explicitly run and passed without any
application UI frames. Four real tray captures also differed while the window was
hidden. A repeated hidden/hover sample measured about 0.228% whole-machine CPU,
213.1 MiB working set, 1,163 handles and 46 threads on this machine. These are limited
smoke observations, not a sustained soak or evidence that dragging is fixed.

The private GitHub baseline CI succeeded at run 33940457527. The newer local checkpoint
still needs its own CI run. No alpha tag or release is published. No full headless UI
harness or new branding/icon system is implemented yet. See `TASK_BOARD.md`.

## Product identity

Trontop is Trent Sterling's native Windows system control deck. It exists because
Windows 11 Task Manager traded too much density and immediacy for generic shell UI.
The target is a portable tool that feels at home beside TrontSnap, SpaceView,
TrontEQ, Photochop, and the rest of the TrontStack native apps.

The visual direction is dark, technical, and readable. Purple and teal are the
default TrontStack signal colors. Demigod orange, Monke Portal cyan/violet, and the
original copper theme are included as presets. The UI should carry Trent's actual
identity: high-performance Unity and VR work, multiplayer systems, procedural tools,
and practical utilities built for demanding machines.

## Version 0.3 alpha checkpoint

Implemented:

- custom frameless window with branded drag region and window controls
- Processes, Performance, History, Startup, Users, Details, and Services pages
- process search by name, user, PID, path, and command line
- real process-tree ordering with expand/collapse, automatic ancestor context during
  search, descendant counts, and clearly labeled subtree CPU/GPU/memory/I/O totals
- live process CPU, GPU, memory, disk rates, cumulative I/O, state, user, parent,
  executable, command line, working directory, start time, and accumulated CPU time
- exact Windows GPU Engine PDH enumeration with machine, engine, and PID aggregation
- CPU, memory, disk, network, and GPU performance device views
- Windows Service Control Manager inventory
- HKCU/HKLM Run key and Startup folder inventory
- guarded End task flow and Run task launcher
- current priority class and CPU affinity sampled outside the UI thread every five
  seconds; guarded priority and affinity editors validate PID start identity and require
  explicit confirmation before applying a change
- live Theme Studio with dark/light modes, primary and secondary colors, gradients,
  frost, corner control, presets, and persistence
- theme-derived zebra rows across dense grids, stronger two-signal hover/selection
  states, and a branded process-inspector empty state
- globally non-selectable display labels; explicit paths and command lines remain
  selectable for copying
- live tray icon whose meter and color track CPU load, plus a CPU/memory/GPU/process
  tooltip and Show/Quit actions
- generated multi-resolution Trontop PE icon and embedded Windows company, product,
  description, filename, and prerelease version metadata
- one-second background snapshots; no OS query runs on the egui render thread
- checked-in Windows GitHub Actions verification and portable-executable artifact upload

## Verification baseline

The expected gate is:

```powershell
cargo fmt --all --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```

The v0.3 alpha visual validation used a real Windows 11 machine with an RTX 5070 Ti. PDH
reported real machine and per-process GPU utilization. Visual captures live under
`C:/trontstack/tmp/` during the active session and are not repository assets.

The process-control integration test launches a hidden disposable child, reads its
native priority and affinity, changes both, verifies the new values, restores them, and
terminates the child. No production workload is modified by that test.

The final release runtime sample was responsive at 0.3125% whole-machine CPU over
10 seconds and 210.6 MiB working set while sampling roughly 450 processes. The main
window opened centered at 1280 by 760 and the native tray host was present. Treat
these numbers as a comparison baseline, not a machine-independent budget.

The earlier 0.3.0-alpha.1 process-tree baseline was responsive at 0.1947% whole-machine
CPU over 10 seconds, 211.7 MiB working set, 1,181 handles, and 47 threads while
sampling roughly 445 processes. Its portable executable is 12,623,872 bytes with
SHA-256 `04A2C7C478AD506F33E7A5ACA524D12E2DF5DCFF32B63EB1652818CA053402CD`.

## Architecture map

- `src/sampler.rs`: one long-lived background telemetry worker and immutable snapshots
- `src/windows_metrics.rs`: GPU PDH, Service Control Manager, and startup inventory
- `src/model.rs`: snapshot and table models
- `src/app.rs`: navigation, pages, actions, persistence, and window shell
- `src/widgets.rs`: Tront visual primitives, charts, meters, tables, and marks
- `src/theme.rs`: derived tokens, gradient painter, presets, and serialization
- `src/tray.rs`: native live tray icon and tray actions
- `src/platform.rs`: guarded process and shell actions

Read `docs/TELEMETRY.md` before changing providers. Read `TASK_BOARD.md` and
`RELEASE_PLAN.md` before choosing the next slice.
