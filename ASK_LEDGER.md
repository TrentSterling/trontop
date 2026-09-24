# Trontop ask ledger

Owner: Trent Sterling. Consolidated from this conversation, 2026-09-05.
This is the **single product-completion checklist**. `TASK_BOARD.md` is historical
engineering detail; `docs/CURRENT_STATE.md` records builds and test evidence.

## What counts as done

- An unchecked item is OPEN, PARTIAL, or REVIEW, as stated. Code existing is not
  enough when the acceptance check still needs a real run or Trent's visual review.
- A checked item has the evidence linked beside it. The release-wide checks still
  have to pass against the final exact executable, not an old alpha.
- Close the decision items explicitly with Trent. Do not silently drop CPU
  temperatures, smooth dragging, branding, or another difficult ask to get green.
- **The release is done when every Required item is checked, every Decision is
  resolved, and Trent accepts that exact build. Stop feature work at that point.**
  An approved deferral records who approved it and why, not a pretend completion.
- New ideas go to Future ideas. They do not expand this checklist automatically.
  No new feature, optimization investigation, or visual redesign without an open
  ask ID. Bug fixes stay attached to the acceptance check they repair.
- This ledger covers Trontop. The earlier Photochop/Boxel/project-naming conversation
  is context, not additional Trontop release work.

## Public preview decision, 2026-09-24

Trent explicitly requested a public source repository, Windows download, product
page, blog post, themed screenshot automation and OG image. Licensing discussion
settled on personal/workplace use, attribution retained and restrictions on
selling Trontop or a lightly rebranded copy: Apache 2.0 + Commons Clause 1.0.
Alpha.41 is an unsigned development preview. Native/clean-machine/extended-soak
checks below remain open and are disclosed in `docs/RELEASE_ALPHA41.md`; this
launch does not assert that the broader completion ledger is closed.

- [ ] **A36: Public preview and launch materials. IN PROGRESS.** Build/test/package
  locally, verify exact CI artifact, publish repository/release/site/blog, and
  check anonymous URLs. Media: 16 real UI renders with demo data and 12 themes.

## Required: useful task manager

- [x] **A35: Daily-use polish follow-up, alpha.40. ACCEPTED.** Trent, 2026-09-24:
  brighter dialog X outlines, movable dialogs, easier Lines / Bars access,
  End task higher in the inspector, and logo contrast under random themes.
  Implemented and checked with 429 passing tests and 16 inspected offscreen
  views; `docs/POLISH_2026-09-24.md`. Trent reviewed the running build and said it looks great.

- [x] **A01: Trontop, a native Rust Windows app.** Short working name, standalone
  system monitor, no webview/installer required. Evidence: `Cargo.toml`, `src/main.rs`.
- [x] **A02: Process list, tree, search, sorting and inspector.** Expand/collapse,
  parent totals, PID/name/user/path/command search; selection stays attached to the
  process. Evidence: `docs/PROCESS_TREE.md`, `docs/PROCESS_SORTING.md`, headless checks.
- [x] **A03: Basic process control.** Run task, confirmed End task, priority and CPU
  affinity with stale-PID/critical-process guards. Evidence: `docs/PROCESS_ACTION_SAFETY.md`.
- [ ] **A04: Finish daily-use controls. PARTIAL.** Suspend/resume and reversible
  Startup enable/disable are still missing. Service Start/Stop/Restart is coded but
  requires the isolated native command gate in `docs/SERVICE_CONTROLS.md`. Done when
  each advertised action succeeds or explains failure, and recovery is verified.
- [x] **A05: Dashboard plus all requested page families.** Overview, Processes,
  Performance, History, Startup, Users, Details, Services and Hardware sensors
  exist with real backing data. History is explicitly lifetime CPU/I/O, not claimed
  to reproduce Windows App history. Evidence: `README.md`, `docs/HEADLESS_QA.md`.
- [ ] **A06: Finish the Task Manager data comparison. PARTIAL.** Existing CPU/memory,
  disk rates/activity/latency/queue and network rates are real. Remaining comparison:
  network adapter details and process network rates; CPU/memory detail fields; History semantics;
  NPU exposure or an explicit unsupported status. Done with a field-by-field parity
  sheet against Trent's supplied Task Manager screens, not a blanket parity claim.
  Evidence so far: `docs/TELEMETRY.md`, `docs/PHYSICAL_DISKS.md`.
  Alpha.30 corrects the COMMITTED calculation and mislabelled page-file graph,
  connecting Windows commit/limit/peak, cache and kernel pools with provider health.
  Five regressions and a real read-only probe pass; `docs/MEMORY_COUNTERS.md`.
  Remaining CPU/memory details and the wider comparison are still unchecked.
  Alpha.33 adds per-logical-processor histories and the requested All cores grid.
  The old Speed field is corrected to Windows Power clock, not claimed boost
  frequency. Native counter evidence and GitHub source research are recorded in
  `docs/HARDWARE_RESEARCH.md`; live frequency and the parity sheet remain open.
  Alpha.34 connects dynamic Windows performance-state clocks, per-processor nominal
  references, average/fastest histories and source-labelled exports. The native
  production probe returns changing 5.12-5.17 GHz averages on this box; tests cover
  groups, reset/recovery, missing intervals and presentation. `docs/CPU_CLOCK.md`.
  Exact Task Manager comparison and broader field parity remain unchecked.
  Alpha.35 adds identity-keyed Windows adapters, per-engine histories and whole-
  adapter dedicated/shared/committed memory with DXGI capacities. Read-only native
  probe and isolated tests: `docs/GPU_ADAPTERS.md`. Networking, remaining CPU
  details, NPU identity/support and field-by-field comparison remain unchecked.
- [x] **A07: Never flash empty fields or invent data.** Keep last usable readings
  with cached/partial/unavailable labels and chart gaps; preserve stable geometry.
  Evidence: GPU, inventory, sensor and disk tests in `docs/HEADLESS_QA.md`. Final
  integrated slow/failing-provider behavior is rechecked under A25.

## Required: every sensor the supported box can expose

- [x] **A08: GPU watts, temperature, clocks, fan and VRAM.** NVIDIA NVML fields and
  histories are connected; missing fan data is not fabricated zero/RPM. Read-only
  RTX 5070 Ti probe recorded in `docs/SENSORS_PLAN.md`.
- [x] **A09: All-sensors page and accessible drive temperatures.** One Hardware
  sensors page, independent slow storage queries, retained fields and freshness;
  TEAM SSD returned three real readings. Evidence: `docs/SENSORS_PLAN.md`.
- [ ] **A10: CPU/core/motherboard temperatures and remaining box sensors. OPEN.**
  CPU is still not connected. Inventory what Trent's CPU, board and drives expose,
  identify the source/unit of every reading, and connect the agreed safe providers.
  Unsupported must be explicit, not guessed. Depends on D01; no driver installation
  is authorized merely by this item. AMD/Intel/other-controller limits must be listed.
  Alpha.36 bridge outcome: a read-only sensor bridge now maps CPU package/core,
  motherboard, package power and core voltage (with source labels) from an
  already-running LibreHardwareMonitor or OpenHardwareMonitor (WMI) or HWiNFO
  (shared memory) onto the System and Hardware sensors pages. None runs on the
  reference PC, so CPU and board temperatures stay explicitly Unavailable there;
  no driver is installed. The NVMe health log (wear, data written, hours, errors)
  is now read without admin; ATA SMART still requires administrator. Still OPEN
  until D01 is decided. Evidence: `docs/SYSTEM_SPECS.md` (Sensor Sources).

## Required: Speccy-class system specifications

- [ ] **A33: Speccy-class System specs page. REVIEW.** Trent, 2026-09-22: "clone
  speccy, IN trontop, make trontOP OP". Alpha.36 fills every section from
  read-only sources (OS, CPU, RAM, Motherboard, Graphics, Storage, Optical,
  Audio, Peripherals, Network, Sensor Sources) with a Speccy-style Summary,
  section icons, collapsible groups, live colored temperatures and clocks, copy
  (all, section, group, row), TXT/JSON save with private values off by default,
  and explicit Waiting/Slow/Partial/Unavailable states. It beats Speccy's bugs on
  this PC (64-bit VRAM, no invented shader clock, per-DIMM SMBIOS, NVMe interface
  and temperature, VT-x capability vs firmware vs hypervisor, per-core-type
  caches, full CPUID brand). Evidence: `docs/SYSTEM_SPECS.md`, nine
  `native_specs_*_read_only_probe` runs, `app::ui_smoke::system` tests, the
  real-data `render_system_specs_visual_pass` PNGs and `docs/CURRENT_STATE.md`
  (alpha.36). A four-lens verification pass (truth, perf, safety, parity) fixed
  18 findings, including the Intel video BIOS, the HDD temperature reason, Wi-Fi
  866.7 Mbps, Bluetooth PAN type, TRIM wording, HX-safe socket, budget-bounded
  SetupDi, overflow and panic guards, the half-hidden System nav entry, and
  Speccy's WinInet and Connections groups; evidence in `docs/CURRENT_STATE.md`.
  Remaining: Trent's visual review of the page, and the real Windows
  Save As picker for specs files is compiled but not validated end to end (same
  gate as `docs/EXPORTS.md`). CPU/board temperatures depend on A10/D01.

## Required: Tront look, readable everywhere

- [ ] **A11: Stronger Tront branding. PARTIAL / REVIEW.** Research and reflect Trent's
  work and `tront.xyz/games`; show a recognizable Trontop identity in chrome, About,
  app icon and tray. Current T mark exists, but Trent explicitly said the branding
  still needed work. Done after one cohesive final identity pass and Trent approval,
  not endless new concepts. References: `docs/inspiration/README.md`.
  Alpha.39 integrates the September 24 generated mark using theme-colored alpha
  masks in chrome/About/inspector/preview and the native window/taskbar icon.
  The executable embeds the original-color mark; the live tray graph stays.
  See `docs/THEME_CONTROLS_2026-09-24.md` for scope and verification.
- [x] **A12: Save the supplied inspiration and use real icons, not emoji glyphs.**
  Four supplied boards plus a transparent generated icon candidate are saved.
  Navigation/action/window icons are code-drawn; process icons use a bounded native
  cache with fallbacks. The generated candidate is not the shipped app icon.
  Evidence: `docs/inspiration/`, `docs/PROCESS_ICONS.md`, `src/icons.rs`.
- [ ] **A13: Four-peg gradients and a polished full Theme Studio. PARTIAL / REVIEW.**
  Acceptance: four editable colors/positions; ramp dragging and numeric/hex edits;
  direction/intensity, reverse/even spacing; independent accents; dark/light;
  rounded surfaces, row/column bands, opacity and hover controls; presets, named
  saves, import/export, reset/revert; old themes migrate; changes survive restart.
  No forced random theme on the real app. Implemented in alpha.19; 175 ordinary
  tests pass, including new theme checks. Six theme views inspected among 67 PNGs.
  Native restart persistence and Trent's visual review remain. See `docs/THEME_STUDIO.md`.
  Alpha.26 replaces eframe's blocking file store with bounded app-owned background
  persistence, staged replacement, read-only legacy migration and explicit unsaved
  close choices. Twelve new tests include a real app/worker/file/fresh-app theme,
  named-palette and zoom round trip. Eight new settings views reviewed. See
  `docs/SETTINGS_PERSISTENCE.md`. Native restart and Trent's review remain open.
  Alpha.33 adds the requested ColorMagic Randomize button, coordinated four-peg
  palettes, six families plus Surprise me and 12-roll undo. Mode/layout stay
  intact; contrast and actual UI interaction checks pass. `docs/ALPHA33_REVIEW.md`
  records dark/light renders; native persistence and final review remain open.
  Alpha.39 adds full-range intensity/frost, separate dark/light frost, composed
  previews, surface tint, secondary-text contrast and bundled font selection.
  Export v4 migrates older settings. Cross-app engine parity is explicitly deferred
  by Trent. See `docs/THEME_CONTROLS_2026-09-24.md`.
- [ ] **A14: Zebra rows AND columns throughout. PARTIAL / REVIEW.** Check every
  table, device list, inspector/detail list, History/Startup/Users/Services list,
  sensor group and dialog. Alternation must remain distinct beneath selection and
  hover in light/dark themes. Many shared surfaces are done, final coverage is not.
- [ ] **A15: Rounded layout boxes, padding and text alignment everywhere. PARTIAL / REVIEW.**
  One final page/dialog inventory at 1040x640 and 1280x760, then DPI checks: no
  character-stacked values, clipped primary controls, misaligned numeric columns,
  jammed edges or footer overlap. Check empty/loading/error/scrolled states too.
  Fixed Users wrapping and table baselines already have tests. Alpha.21 reclaims
  empty/hidden inspector width, reflows compact metric cards and stabilizes long
  inspector identities; four new headless regressions pass. Native DPI/full-page
  coverage and final review remain. See `docs/UI_POLISH_BRIEF.md`.
  Alpha.24 fixes displaced dialog actions and hidden History values from long
  names, bounds the affinity grid, and shows the requested CPU set on review.
  Seven new tests pass; 14 new compact dark/light views reviewed. See
  `docs/COMPACT_CONTROLS.md`. Full-page/native/final acceptance remains open.
- [ ] **A16: Readable text and complete hover/focus states. PARTIAL / REVIEW.**
  Shared hover backgrounds exist for controls, rows, cards, badges, charts and
  labels. Final audit must include selected/disabled/focused states and extreme
  custom gradients, not just default colors. Hover must not move layout or steal
  child input. Primary text and muted labels must remain readable.
  Alpha.23 adds bounded interaction/band surfaces, independent foreground ink,
  adaptive action text and six contrast/state regressions (202 ordinary tests
  pass). Extreme palette renders were reviewed; full inventory/native/final
  acceptance remain. Evidence: `docs/THEME_CONTRAST.md`.
  Alpha.32 fixes missing table-cell focus outlines and duplicate invisible
  row/icon Tab stops. Four regressions cover keyboard sorting/selection, disabled
  cells, preserved mouse gaps and geometry/contrast. Four focus views inspected;
  this is a bounded interaction fix, not final whole-app/native acceptance.
- [x] **A17: Display labels must not be highlightable.** Global non-selectable
  labels; only deliberate editable/copy fields select text. Evidence: `src/theme.rs`
  and headless text/control checks. Recheck as part of the final UI audit.
- [x] **A18: Remove native chrome; retain working window controls.** Frameless custom
  titlebar and code-drawn minimize/maximize/close buttons are implemented. Smooth
  native movement and DPI behavior are separate unchecked gates below.
- [ ] **A34: Polish gauntlet over every page. REVIEW.** Trent, 2026-09-22 (voice note
  over screenshots): "can you fix that actually and also ... look at every page
  headless ... use your screenshot reading ability ... see how it still kinda
  sucks and many pages layouts are ... this app is a fucking mess ... I just want
  you to do a polish gauntlet, without my constant attention okay? Can you use
  your best judgement please? Run a gauntlet on it." A finishing pass over
  `render_gauntlet_all_pages` (`docs/HEADLESS_QA.md`): Overview KPI rows/thermals
  band/top lists grouped by app, unit/axis/legend vocabulary unified across
  Graphs and Performance, table right columns flexed and noise rows dropped,
  Performance rail/card anatomy and GPU adapter picker moved to hover, Hardware
  sensors compacted with drives above the fold, Startup/Services/System/dialog
  padding and truncation fixed, one egui id clash and remaining sidebar noise
  removed. A 16-surface final critic pass over the rendered PNGs scored an
  average 7.2/10 (range 6-8; Overview up from an early 3/10). Evidence:
  `docs/CURRENT_STATE.md` (alpha.37), commits `f0805a6`..`9f14ee8` on
  `feat/system-specs`. Honestly still open, not fixed this pass: Details still
  scrolls horizontally at 1000x580 (READ/CPU TIME clipped); Services stacks four
  pieces of disabled-toolbar noise above the table header; the CPU-temperature
  gap is worded three different ways (Overview footer, Graphs footer, Hardware
  sensors); network units still split across in/out, Receive/Send and Rx/Tx by
  page; the Graphs and Theme Studio tab rows shift horizontally when the
  selected pill changes. Remaining: Trent's own visual pass on this build, then
  fold the open items above into a follow-up ask or close them explicitly.

## Required: feels fast and leaves the desktop alone

- [ ] **A19: A genuinely live tray icon, not just a changing tooltip. REVIEW.** CPU fill and
  scrolling history update on an independent native worker; tooltip includes other
  resources, Show/Quit available. Earlier native captures differed with the main
  window hidden. Evidence: `docs/TELEMETRY.md`, historical `docs/CURRENT_STATE.md`.
  Alpha.29 makes tray construction asynchronous and replaces thread-ID messages
  with a private event/message wait. Seven isolated lifecycle tests pass, but the
  changed native pump needs the A25 isolated tray/menu/hidden-window gate again.
  Historical captures are not verification of this new worker. See `docs/TRAY_LIFECYCLE.md`.
- [ ] **A20: Smooth real titlebar dragging. PARTIAL, NOT VERIFIED FIXED.** Compare an
  optimized Trontop with Terminal/Explorer using real input on an isolated desktop.
  Record move/present timing, not just a synthetic drag video. Trent noticed some
  improvement but still reported lag. Investigation remains parked pending D02.
  Evidence and failed approaches: `docs/DRAG_INVESTIGATION.md`.
- [ ] **A21: Fast close. PARTIAL.** Worker waits are bounded and regression-tested;
  actual native close latency still needs an authorized isolated measurement,
  including a slow provider. Window/tray should disappear promptly with no orphaned
  app instance. Do not call the thread unit test an end-to-end close test.
  Alpha.26 disables eframe's file store and removes its unbounded settings join.
  Close queues the final save without waiting on the UI thread; failures/slow
  storage offer Keep open, Retry and explicitly lossy Close anyway. Blocked-worker
  and synthetic OS-close tests pass. This is not native teardown timing or proof
  that settings caused Trent's reported slow close. See `docs/SETTINGS_PERSISTENCE.md`.
  Alpha.28 removes duplicate UI-memory clones during save dispatch/Retry and
  skips filesystem work for byte-identical local state. Three new regressions
  cover clone count, busy files/external conflicts and the headless close gate.
  Dirty saves still gate close; native close timing remains unmeasured.
- [ ] **A22: Low overhead and stable responsiveness. PARTIAL.** Indexed process views,
  iterative trees and isolated workers have real improvements and synthetic timing
  evidence. Final release needs visible/hidden/tray/mixed-load CPU, memory, handles,
  frame-time and growth measurements over a 60-minute soak. Investigate only failed
  budgets/reproducible stalls, not open-ended optimization. See A25 and D04.
  Alpha.19 suffered a verified renderer crash. Alpha.20 adds non-blocking snapshot
  transfer and device recovery, with 3/3 offscreen pixel-identical fault recoveries.
  Native surface recovery/drag/soak are still open: `docs/RENDERER_RECOVERY.md`.
  Alpha.22 removes native process/shell action calls from the UI thread, with
  single-flight dispatch, honest pending/results, and stalled-navigation tests.
  See `docs/PROCESS_ACTION_SAFETY.md`; this does not prove native drag/close timing.
  Alpha.25 moves recoverable GPU diagnostics off renderer callbacks and makes
  service-result mailbox polling non-waiting. Four new tests pass, including
  callback state changes with a saturated queue. See `docs/FAILURE_REPORTS.md`.
  Alpha.26 moves settings read/parse/serialization/write off the UI thread, with
  bounded coalescing, preserved failures and no worker join. Migration/blocked
  storage tests pass. Alpha.29 removes the recorded synchronous tray-construction
  ready wait and failed-constructor join. Seven new worker/event tests cover blocked
  startup/update, late cleanup, bounded samples, failure recovery and UI repaint
  restraint. About reports real lifecycle status. Native gates remain open;
  this is not an assumed cause of the crash. See `docs/TRAY_LIFECYCLE.md`.
  Alpha.31 bounds graph-card layout to the visible rows plus one measuring row;
  a 512-chart reference comparison verifies scroll/scale geometry and capped work.
  CPU-only A/B timing is documented in `docs/GRAPH_WALL.md`, not a native FPS claim.
  Alpha.33 shares histories with the dense Overview, clips its rows and core grid,
  and moves physical-core inventory out of the per-second refresh. Native CPU
  micro-probe and optimized synthetic UI timing: `docs/ALPHA33_REVIEW.md`.
  Neither clears the unmeasured native gates.
- [x] **A23: Safe automation that does not mess with other work.** Headless fixtures
  do not open native windows, inject global input, change focus or execute viewport
  commands. `AGENTS.md` forbids the previous unsafe desktop behavior. Any future
  native interaction requires fresh permission and isolation, not blanket consent.
- [x] **A24: More smoke-test coverage and visual passes.** Production UI headless
  matrix, geometry/state/input tests, opt-in offscreen PNG renderer and read-only
  provider probes exist. Baseline alpha.18: 166 tests pass, 60 PNGs generated.
  Scope and limitations: `docs/HEADLESS_QA.md`. A25 covers the final build.
  Alpha.31 also fixes discarded click-frame font updates in graph/memory visual
  captures; an exact atlas regression reproduces the old missing-glyph failure.
- [ ] **A25: Final integrated verification. OPEN.** Exact candidate passes fmt,
  all ordinary tests, strict Clippy, release build, Windows CI, relevant reviewed
  PNGs, isolated process/service/export/tray/close tests and the mixed-load soak.
  Missing/protected/failed providers retain honest UI. No blanket ignored-test run.

## Required: a usable, portable release and a clear handoff

- [ ] **A26: One portable trontop.exe. PARTIAL.** Static CRT, embedded icons and no
  application asset directory already build and pass dependency inspection. Finish
  clean-machine/no-Rust launch, portable relocation, standard-user and read-only
  application-folder tests. Settings currently live per-user, not inside the EXE;
  decide whether travelling settings are wanted under D03.
- [x] **A27: Embedded Windows icon and version identity.** Multi-resolution icon
  and PE metadata exist. Visual identity approval is A11; exact final hash is A29.
- [x] **A28: Private GitHub and a release plan.** Private `TrentSterling/trontop`,
  versioned source and Windows verification workflow exist. Last verified remote
  checkpoint at ledger creation: alpha.17, run 33969106100. No release published.
- [ ] **A29: Ship the agreed build. OPEN.** Exact tagged artifact, EXE, SHA-256,
  version, concise changelog, provider limitations, signature status and rollback
  path. Verify private download; make nothing public without the release decision
  in D03. No surprise uploads to scanning services or website changes.
- [x] **A30: Save summary, task/ask board and resume instructions.** This ledger,
  `CODEX.md`, `docs/CURRENT_STATE.md` and build/test docs are present. Keep these
  current at each meaningful handoff instead of accumulating contradictory boards.
- [ ] **A31: Let Trent review the latest build and call it done. REVIEW.** Older
  explicit launch requests were fulfilled and logged. Deliver this final candidate
  with its exact path/version and short visible-change list, then obtain Trent's
  acceptance of A11/A14/A15/A16. Launch/replace only when explicitly requested;
  never accumulate more previews automatically. No new polishing cycle after signoff.

- [ ] **A32: One graph-first page for the whole machine. REVIEW.** Explicitly
  requested after alpha.26 review on September 5: mostly graphs in one scrollable
  dashboard, including load, temperatures, watts, storage and network activity.
  Alpha.27 adds Graphs with Lines/Bars, category filters, a continuous responsive
  grid, hover readings and timestamped two-minute history. Overview remains.
  Missing/partial data make gaps; retained values are labelled. CPU temperature
  remains A10/D01, not a fabricated chart. Acceptance: `docs/GRAPH_WALL.md` and
  Trent's review of this specific layout.
  Alpha.30 adds real commit/pressure/cache/kernel-pool histories and a Memory
  filter, with missing intervals and retained labels. Six memory views and the
  overall dark/compact graph layouts reviewed offscreen; Trent's review remains.
  Alpha.31 removes offscreen card layout, fixes fractional column-width drift,
  and returns category selections to their first graph. Histories/metrics are not
  reduced. Two regressions pass; final layout acceptance still belongs to Trent.
  Alpha.33 supplies the requested dense Overview and All cores grid, including
  process-count and available-RAM histories without extra polling. Verification:
  `docs/ALPHA33_REVIEW.md`. Trent's layout review remains open.

## Decisions to resolve, not silently implement or discard

- [ ] **D01: CPU sensor access.** Trent wants every temperature and doubts a new
  driver is needed. Research notes compare other open-source monitors, but no
  verified driver-free CPU provider exists here. Agree on a supported existing
  provider/optional integration, or explicitly accept and document the limit.
  Read `docs/SENSORS_PLAN.md`. Do not substitute ACPI zones for CPU package sensors.
  September 6 source audit and read-only probes: `docs/HARDWARE_RESEARCH.md`.
  Alpha.36 implements the "supported existing provider" option read-only: the
  sensor bridge reads LibreHardwareMonitor/OpenHardwareMonitor WMI or HWiNFO
  shared memory when Trent already runs one, and otherwise shows the reason CPU
  and board temperatures are unavailable. Whether that is the accepted answer (or
  the limit is accepted as documented) remains Trent's decision.
- [ ] **D02: Drag investigation and cross-project rollout.** Trent requested the
  fix across egui apps, then questioned it and asked to park disruptive testing.
  Findings are saved. Resume only after approval for isolated measurement; port
  only a verified fix. Boxel/other repositories are a separate follow-up, not
  permission to mutate them now.
- [ ] **D03: Release boundary.** Confirm private alpha versus public preview, signing
  status/unsigned warning, supported Windows/hardware matrix and portable-settings
  expectations. Original instruction was private GitHub, later "ship Trontop";
  public visibility is not inferred. Resolve any approved release deferrals here.
- [ ] **D04: Freeze acceptance budgets.** Agree native interaction/close and resource
  budgets before the final soak; 60 FPS is the desired feel, not a current measured
  claim. Accept/reject against those budgets; stop adding optimization chores when
  they pass. No need for every PC's inaccessible sensor to pass on the reference PC.
- [ ] **D05: Optional Ctrl+Shift+Esc takeover.** User asked whether it is feasible;
  discussion/proposal saved in `docs/HOTKEY_PLAN.md`. Decide whether it belongs in
  this release. If yes: explicit opt-in/off switch, precise chord interception,
  isolated validation, Windows Task Manager still reachable through Ctrl+Alt+Del.
  No hook or registry redirection has been installed. A portable stopped app cannot
  own a shortcut. Deferral requires Trent's decision, not a checked feature box.

## Future ideas, not additional completion gates

Agent-proposed extras not specifically agreed as release requirements: command
palette, arbitrary configurable columns, selectable polling intervals, automatic
theme randomization, alerts, storage wear/error dashboards, deeper ETW tooling,
crash-dump collection, shader/noise effects, perpetual animation, website marketing
expansion. Keep these here unless Trent explicitly promotes one to an ask ID.

September 6 brainstorming: a shared history cursor for cross-metric correlation
and developer-project grouping for Unity/compiler families are proposals, not new
release gates. Scope and rationale: `docs/ALPHA33_REVIEW.md`.

## Current handoff: alpha.35 review checkpoint, thread closed

The requested adapter slice, local gate, optimized build and native launch are
complete. Source and handoff notes are on the private feature branch. Exact-source
Windows CI run 34058415830 succeeded, including artifact upload. Native acceptance,
remaining features and release decisions above are still open.

Trent then explicitly requested testing/resolution notes saved and this thread
closed. Stop here; do not automatically launch, test, implement or publish more.
`docs/CURRENT_STATE.md` contains the ordered resume checklist, desktop-safety
constraints, exact executable/hash, prior launch evidence and final remote status.
`docs/GPU_ADAPTERS.md` records the implementation scope. A new explicit resume is
required; brainstorm features and another speculative polish cycle are not implied.

### Previous alpha.34 handoff

A06 now has a measured dynamic clock path, not just the relabelled power clock.
See `docs/CPU_CLOCK.md` and `docs/CURRENT_STATE.md` for source semantics, limits,
native evidence and build verification. No new brainstorm requirement was added.
CPU temperatures, native gates, the broader parity sheet and final review remain.

### Previous alpha.33 handoff

September 6 resumes A06/A13/A32: All cores, graph-heavy Overview and coherent theme
randomization. Implementation and isolated checks are complete; evidence is in
`docs/ALPHA33_REVIEW.md`, candidate identity in `docs/CURRENT_STATE.md`. CPU temps,
live boost speed, native responsiveness and final aesthetics remain unchecked.
Review this build before another speculative visual cycle. The brainstorm does
not expand release requirements by itself.

## Historical handoff notes

1. Current resumed slice is the alpha.19 crash and responsiveness regression
   (A20/A22/A25), plus focused A15/A16 alignment polish, not a feature expansion.
   Alpha.22 additionally removes synchronous UI process/shell calls (A22).
   Deliver the current tested candidate with its exact identity and native limits.
   **Review checkpoint:** clean alpha.26 was rebuilt and opened on Trent's explicit
   request at 23:38 UTC, replacing five old previews with permission. `REVIEW.md`
   has the exact identity and remaining limits. Trent again flagged diminishing
   returns; get feedback on this build before another implementation cycle.
2. Alpha.26/28 address settings waits and duplicate work with preservation and
   unsaved/error tests. Alpha.29 removes the recorded synchronous tray startup
   wait with bounded samples and truthful state. This completes that code slice,
   not the native tray/close/soak gates. Keep A13/A19/A21 unchecked pending those gates.
3. Resolve permission for isolated native measurements before touching any windows.
   Work the remaining bounded asks and release decisions, not new alpha features.

Historical progress is not a percentage-complete estimate. Remaining items differ
greatly in effort, and CPU sensors/native safety tests have unresolved dependencies.
