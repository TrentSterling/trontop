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

## Required: useful task manager

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
  per-adapter GPU names, dedicated/shared memory and engine charts; network adapter
  details and process network rates; CPU/memory detail fields; History semantics;
  NPU exposure or an explicit unsupported status. Done with a field-by-field parity
  sheet against Trent's supplied Task Manager screens, not a blanket parity claim.
  Evidence so far: `docs/TELEMETRY.md`, `docs/PHYSICAL_DISKS.md`.
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

## Required: Tront look, readable everywhere

- [ ] **A11: Stronger Tront branding. PARTIAL / REVIEW.** Research and reflect Trent's
  work and `tront.xyz/games`; show a recognizable Trontop identity in chrome, About,
  app icon and tray. Current T mark exists, but Trent explicitly said the branding
  still needed work. Done after one cohesive final identity pass and Trent approval,
  not endless new concepts. References: `docs/inspiration/README.md`.
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
- [x] **A17: Display labels must not be highlightable.** Global non-selectable
  labels; only deliberate editable/copy fields select text. Evidence: `src/theme.rs`
  and headless text/control checks. Recheck as part of the final UI audit.
- [x] **A18: Remove native chrome; retain working window controls.** Frameless custom
  titlebar and code-drawn minimize/maximize/close buttons are implemented. Smooth
  native movement and DPI behavior are separate unchecked gates below.

## Required: feels fast and leaves the desktop alone

- [x] **A19: A genuinely live tray icon, not just a changing tooltip.** CPU fill and
  scrolling history update on an independent native worker; tooltip includes other
  resources, Show/Quit available. Earlier native captures differed with the main
  window hidden. Evidence: `docs/TELEMETRY.md`, historical `docs/CURRENT_STATE.md`.
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
  storage tests pass. Tray startup still waits for its worker; this audited wait
  remains open, not an assumed cause of the crash. See `docs/SETTINGS_PERSISTENCE.md`.
- [x] **A23: Safe automation that does not mess with other work.** Headless fixtures
  do not open native windows, inject global input, change focus or execute viewport
  commands. `AGENTS.md` forbids the previous unsafe desktop behavior. Any future
  native interaction requires fresh permission and isolation, not blanket consent.
- [x] **A24: More smoke-test coverage and visual passes.** Production UI headless
  matrix, geometry/state/input tests, opt-in offscreen PNG renderer and read-only
  provider probes exist. Baseline alpha.18: 166 tests pass, 60 PNGs generated.
  Scope and limitations: `docs/HEADLESS_QA.md`. A25 covers the final build.
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

## Decisions to resolve, not silently implement or discard

- [ ] **D01: CPU sensor access.** Trent wants every temperature and doubts a new
  driver is needed. Research notes compare other open-source monitors, but no
  verified driver-free CPU provider exists here. Agree on a supported existing
  provider/optional integration, or explicitly accept and document the limit.
  Read `docs/SENSORS_PLAN.md`. Do not substitute ACPI zones for CPU package sensors.
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

## Next handoff, then stop expanding

1. Current resumed slice is the alpha.19 crash and responsiveness regression
   (A20/A22/A25), plus focused A15/A16 alignment polish, not a feature expansion.
   Alpha.22 additionally removes synchronous UI process/shell calls (A22).
   Deliver the current tested candidate with its exact identity and native limits.
2. Alpha.26 addresses the settings-store wait under A13/A21/A22 with preservation
   and unsaved/error tests. Next bounded code-level wait is tray initialization:
   remove the synchronous ready/failed-worker join without duplicating workers or
   making tray availability dishonest. Keep A13/A21 unchecked until native gates.
3. Resolve permission for isolated native measurements before touching any windows.
   Work the remaining bounded asks and release decisions, not new alpha features.

Historical progress is not a percentage-complete estimate. Remaining items differ
greatly in effort, and CPU sensors/native safety tests have unresolved dependencies.
