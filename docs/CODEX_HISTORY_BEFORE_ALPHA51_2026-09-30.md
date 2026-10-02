# Historical CODEX notes before the alpha.51 pause

Archived verbatim for provenance. Current CODEX.md and RESUME_ANCHOR.md
override all active/launch/next-work instructions below. Do not execute
historical instructions as current authorization.

# CODEX.md: Trontop handoff

Trontop is an active hand-driven Codex project. The overnight coordination loop
must not modify this repository.

## Active completion goal: make Trontop amazing, 100% coverage

Goal remains ACTIVE. Continue the complete `ASK_LEDGER.md` scope; do not treat
alpha.50 or a coverage percentage as completion. Reproducible Windows
instrumentation measures production separately from tests without production
exclusions. The final alpha.50 run passes 577 ordinary tests, 24 exact read-only
probes and 10 analyzer checks; production unique source-line coverage is
**93.33% (24,267/26,000)**; 1,733 production lines remain uncovered.
Scope, source/binary hashes, known gaps and commands are in
`docs/COVERAGE.md`; passing reports are at
`target/coverage/runs/20260930-095122-382/`. The measurement tools and manual CI
workflow are local additions; no remote run or upload occurred. The alpha.42
review copy is preserved and its hash remains verified.

Next work: meaningful checks for uncovered behavior and A04's remaining isolated
native acceptance. Startup controls are implemented and verified using owned
fixtures; real Task Manager/sign-in behavior remains open. Keep identity guards,
safe owned fixtures and the desktop-isolation rule. Sensor decisions and broader
native/release acceptance remain open. Do not restart completed passing probes
without a new change or unresolved concern.

## Verified alpha.50: smart tray and RAM/storage fixtures, 2026-09-30

Alpha.49 is now open on Trent's desktop (PID 164012, window title `Trontop`). The
first sandboxed launch, PID 165360, was not visible to him; do not equate a blank
window handle with desktop visibility. He explicitly repeated the open request;
the verified alpha.49 EXE was started outside the sandbox with WindowStyle Normal.
No global input/focus/window automation is authorized. Keep this running candidate
untouched while continuing coverage.

Source is the final verified alpha.50 candidate. Twenty-two RAM/storage
fixtures and fixes pass the targeted specs suite (103 passed, 13 ignored).
Seventeen valid before-fix logs are in `target/review/alpha50-memory-storage/`.
The initial two failing 32 GiB fixtures encoded zero KiB and were corrected;
they are not counted as implementation bugs. The source backup in that folder
predates the final slot/speed fixes; do not restore it.

Trent added A39: smart tray and close to tray. X/OS close now hide only with a
Ready tray; failed/starting/missing tray stays reachable. Tray Show restores;
Quit uses the durable preference-save gate, exposing blocked/error controls.
About and the titlebar X context menu have explicit Quit. Hidden sampling
updates the tray/latest mailbox without sample UI repaint wakes. Test backend
and Context commands are headless; native acceptance is not authorized.

Final: 577 ordinary tests pass, 0 fail, 58 ignored; fmt/strict Clippy/release
PASS. Instrumented ordinary tests, 24 exact read-only probes and ten analyzer
checks pass. Coverage is 93.33%, zero mixed, 193 verified source/build inputs.
Two offscreen About views inspected. No command/build remains running.
Review EXE: `target/review/alpha50-smart-tray/trontop.exe`, 17,838,592 bytes,
SHA-256 `1179784C3B92DFD822BF1F9DB26BC8BB9829DA9613843285F88330329F3CAF1F`.
All alpha.42 through alpha.49 hashes preserved/verified. Zero test processes
and private Startup leaves. Native NVMe health still present after decoder
checks (45 C, 4% used); RAM 64 GiB/6400 MT/s/2 of 4 slots. Local observations,
not field parity. Read SMART_TRAY_2026-09-30, CURRENT_STATE and final receipts.

Next: continue meaningful remaining coverage/acceptance work. Preserve all
prior candidates and the alpha.49 desktop instance. Do not launch/replace
automatically; only on explicit request. Native tray/menu/hide CPU/teardown
needs fresh isolated-desktop permission. The goal remains ACTIVE; broader
acceptance and provider decisions stay open. Do not rerun passed probes without
a new source change or unresolved concern.

Trent explicitly directed A39 done using implementation/headless checks,
without mouse testing. A39 is marked done for that scope. Do not automate
systray interaction, ask for mouse testing, or keep this requested tray change
open solely for native testing. Other release gates are separate; no native
tray interaction was claimed verified.

## Verified candidate: alpha.49 CPU and motherboard fallbacks, 2026-09-30

Sixteen new ordinary checks cover CPU/provider and firmware fallback contracts.
Nominal clocks share the sampler's per-processor group/number query, preserving
partial readings and marking missing data. Missing CPUID/packages, unknown
vendor/suffixes, bus fallback, PCI retention and BIOS date/ROM/revision markers
are verified. Nine regressions failed before fixes. Full suite: 544 pass, 0 fail,
57 ignored; fmt/strict Clippy/release PASS. All 24 exact read-only probes and
10 analyzer checks pass. Production coverage: 92.88%, 1,835 missed, zero mixed;
all 189 source/build inputs independently verified. CPU probe reports 24 cores
(8P + 16E), nominal P 3700 MHz / E 3200 MHz in 2.162 ms, one local observation.
UI layout unchanged; ordinary UI checks pass, no new offscreen image review.
Read `docs/HARDWARE_FALLBACKS_2026-09-30.md` and CURRENT_STATE for final receipts.
Review EXE: `target/review/alpha49-hardware-fallbacks/trontop.exe`, SHA-256
`AE57293D520090869FE27DDF9B432B8726F9AB03B071B727658240F7A1D8F67A`.
Prior candidates preserved; zero test processes and private Startup leaves.
Trent requested opening it; the exact alpha.49 review copy is running visibly
outside the sandbox (PID 164012, window title `Trontop` verified). No desktop
automation, installation or publication.
Keep native-interaction isolation rules. Continue the active full goal; broader
native/provider/release gates stay open.

## Previous iteration: alpha.48 System live values and privacy, 2026-09-30

Nine new ordinary regressions cover resolver/report/UI consistency for native
GPU age/cache/recovery, CPU clock/commit failure and expiry, per-processor group
identity, missing counters, exact network aliases, bridge units and drive
composite/fallback/error/recovery. Private summaries suppress all live metadata
and CPU overrides until explicitly revealed. Cached readings carry explicit
metadata and no fresh temperature coloring. Five regressions failed before fixes.
Full suite: 528 pass, 0 fail, 57 ignored; fmt/strict Clippy/release PASS.
All 24 exact read-only probes and 10 analyzer checks pass. Final production line
coverage: 92.52%, 1,920 missed, zero mixed. All 187 source/build inputs verified.
Three offscreen views inspected. An initial assertion matched the public CPU
meter outside the private row; it was scoped correctly and all checks repeated.
Read `docs/SYSTEM_LIVE_2026-09-30.md` and CURRENT_STATE for final-source receipts.
Review EXE: `target/review/alpha48-system-live/trontop.exe`, SHA-256
`F95288E2FFE46A2C3B8A8AD6A9A19303750F8B907F9B9A9A4E99D75F1D01D83F`.
Prior candidates preserved; zero test processes and private Startup leaves.
Trent wants to play with Trontop soon and has this verified candidate link.
No launch/install/publication or desktop input; continue the active full goal.

## Previous iteration: alpha.47 Overview and graph truthfulness, 2026-09-30

Eleven new ordinary regressions cover Overview and production plots. Fixed
unavailable-bridge retention, Overview temperature overflow, suppressed CPU gaps,
unmarked cached VRAM, remote tooltip samples and missing lower-bound markers.
Also verified native/NVML VRAM freshness, partial totals, multi-GPU states,
navigation, heaviest-instance selection, core hover and line/fill gaps.
Full suite: 519 pass, 0 fail, 56 ignored; fmt/strict Clippy/release PASS.
All 24 exact read-only probes and 10 analyzer checks pass. Final coverage is
92.31%, not 100%; all 186 source/build inputs and receipts independently checked.
Three final offscreen views inspected after extending the CPU gap assertion and
repeating validation. Read `docs/OVERVIEW_GRAPHS_2026-09-30.md` and CURRENT_STATE.
Review EXE: `target/review/alpha47-overview-graphs/trontop.exe`, SHA-256
`F954B803347829E860520C76F05FB40BBCE7D1F314286C206EBEA9D89EC33C89`.
Prior review copies preserved. Zero test processes and private Startup leaves
after the run. No launch/install/publication or desktop input.
Continue the active goal; D01 and native/field-parity/release gates stay open.

## Previous iteration: alpha.46 sensor guards, 2026-09-30

Twelve new ordinary regressions cover the native sensor bridge and production
sensor pages. Fixed stale CPU core chips, nonfinite/overflowing temperature
publication and counts, and uncommitted shared-memory reads. Owned Local mapping
fixtures verify publisher updates, malformed/capped tables, first-readable-region
bounds, missing/colliding names and handle/view release. WMI row conversion is
tested without claiming an installed provider. Full suite: 508 pass, 0 fail,
55 ignored; fmt/strict Clippy/release PASS. All 24 exact read-only probes and 10
analyzer checks pass. Final coverage is 92.07%, not 100%; all 185 source/build
inputs and receipts independently checked. Three final offscreen views inspected.
Read `docs/SENSOR_GUARDS_2026-09-30.md` and CURRENT_STATE.
Review EXE: `target/review/alpha46-sensor-guards/trontop.exe`, SHA-256
`F17472411104195BC53F56716005AA9D685BAFD073E1DB7969105E18A17FD832`.
Prior review copies preserved. Zero test processes and private Startup fixture
leaves after the run. No launch/install/publication or desktop input.
Continue the active goal; D01 and native/field-parity/release gates stay open.

## Previous iteration: alpha.45 daily-use regressions, 2026-09-30

Trent explicitly requested continued complete coverage. Twenty new ordinary
regressions exercise daily actions, Theme Studio and System. Fixed verified
suspension labels and immediate STATE sorting, compact Suspended text, and
gradient arrow-key focus (Tab/Escape still leave). Full suite: 496 pass, 0 fail,
54 ignored; fmt/strict Clippy/release PASS. All 24 exact read-only probes and 10
analyzer checks pass; final coverage is 91.68%, not 100%. All 183 source/build
inputs and test/review receipts independently checked. Three final offscreen
views inspected. Read `docs/DAILY_CONTROLS_2026-09-30.md` and CURRENT_STATE.
Review EXE: `target/review/alpha45-daily-controls/trontop.exe`, SHA-256
`7557959B73D2E65E8331F7DD8399B2F0018C787EB5774E580E9E61D9F898401F`.
Prior review copies preserved. No launch/install/publication or desktop input.
Continue the active goal; native/sensor/field-parity/release gates stay open.

## Previous iteration: alpha.44 Startup controls, 2026-09-29

Approval states, confirmed Enable/Disable, exact Undo and background Refresh are
implemented. Run and file guards prevent stale/concurrent changes through commit;
unknown outcomes require refresh, and original commands/shortcuts are preserved.
Eight native owned-fixture tests and four UI tests pass, three corrected offscreen
views inspected. Full suite: 476 pass, 53 ignored; fmt/strict Clippy/release PASS.
Final coverage: 90.99%, not 100%. Read `docs/STARTUP_CONTROLS.md` and
`docs/CURRENT_STATE.md`. Review EXE is
`target/review/alpha44-startup-controls/trontop.exe`, with an independently checked
receipt. Prior review copies are preserved. No launch, install, upload or desktop
interaction; public preview and running copy are unchanged.

## Previous iteration: alpha.43 process controls, 2026-09-29

Trent explicitly added A38: End process tree and all Chrome/Firefox instances.
The pinned inspector action opens a frozen target-list dialog with an explicit
all-instances scope using the executable path. Changed membership requires new
review; native batch preflight checks every surviving handle before mutation.
A04 also gains confirmed Suspend/Resume with worker-owned Windows 11 state
handles, duplicate protection and automatic release when the worker closes.
Owned hidden children prove real tree exit, identity/path/parent refusal and
stopped/resumed heartbeats. Five UI checks and three inspected offscreen views
cover compact geometry, frozen selection, refresh, cancel and busy/recovery.
Full suite: 455 pass, 52 ignored; fmt/strict Clippy/release PASS. Final coverage:
90.22%, not 100%. Read `docs/PROCESS_CONTROLS.md` and `docs/CURRENT_STATE.md`.
Review EXE: `target/review/alpha43-process-controls/trontop.exe`, SHA-256
`56503FF0D04DCD12F8AB360431DA8E3B93E38D35980AD713230E25BB1A18949F`.
Not launched/installed/committed/uploaded. Public alpha.41 and the running preview
stay unchanged. Keep the goal active and desktop-isolation rules in force.

## Previous iteration: alpha.42, 2026-09-29

Trent resumed iteration and requested responsiveness, UI polish and hardware/graphs
together. This bounded pass advances A06/A07/A15/A22/A25: finite freshness repaints,
CPU average clock/topology/uptime, matched network metadata in Performance, and
identity-preserving interface/volume selection. The local review build is ready:
437 tests pass, 49 ignored; fmt/strict Clippy/release pass. Final live gauntlet:
157 PNGs, 1.12 s worst accepted-sample gap. Review EXE:
`target/review/alpha42-iteration/trontop.exe`; PE versions and copy SHA-256 verified.
Read `docs/ITERATION_2026-09-29.md`
and the newest `docs/CURRENT_STATE.md` entry for gates and exact candidate identity.
Changes are local on `feat/system-specs`. No preview replacement, desktop input,
personal settings or upload is authorized by this iteration. Broader native and
release acceptance remains open.

## Previous checkpoint: public preview published

Follow-up, 2026-09-24: A37 is published. Trent rejected the synthetic screenshot
graphs; all 16 public images and the OG card now use real sampled telemetry from
his PC. Marketing capture is explicitly opt-in, read-only and offscreen. The
article now leads with his Task Manager stale-reading/lag experience, credits
Discord's theme system and describes Speccy-level detail as a goal. See
`docs/LIVE_MEDIA_2026-09-24.md`. The alpha.41 release tag/download is unchanged.

2026-09-24: Trent approved source publication and distribution, including the
product page/blog/gallery/OG image. License: Apache 2.0 + Commons Clause 1.0,
retained attribution, free personal/workplace use. Alpha.41 adds packaging and
222 dependency notices; alpha.39/40 feature changes are included. Current
verification: 429 ordinary tests pass, 48 ignored; fmt/strict Clippy/release pass;
16 synthetic demo-data UI renders and three-width dark/light browser checks pass.
Source and alpha.41 release are now public. The release is the passing Windows CI
artifact from commit `92a6d4cc5d136592699904482b11808bd1189ca2`, with embedded identity
and ZIP checksums verified. Product page, blog, 35 assets and public browser
interactions pass anonymous checks. See A36, `docs/PUBLIC_LAUNCH_2026-09-24.md`
and `docs/MARKETING.md`. Native/clean-machine/soak acceptance remains open.

## Previous checkpoint

**2026-09-24: alpha.40 daily-use polish, built and running.** Trent requested
movable dialogs, bright close-button outlines, easy Lines / Bars access, pinned
inspector End task, and random-palette logo contrast repair. Read
`docs/POLISH_2026-09-24.md`, the newest `docs/CURRENT_STATE.md` and A35.
All 429 ordinary tests pass, strict Clippy/format/release pass, all 16 selected
offscreen PNGs inspected. The running alpha.39 closed normally; alpha.40 opened
at `target-latest/release/trontop.exe`, PID 74912, visible HWND 29361722,
responding at 21:22:19 UTC. Exact hash and backup are in CURRENT_STATE.
Local source remains uncommitted, including the previous alpha.39 changes.
Await Trent's review; broader native acceptance and cross-app parity stay open.

## Previous checkpoint: alpha.39

**2026-09-24: explicitly requested theme/logo pass, alpha.39.** Trent accepted the
generated transparent T mark and asked for theme-aware branding, stronger frost/
intensity control like TrontSnap, and Boxel-inspired color/contrast/font controls.
He explicitly deferred cross-app engine parity. Read
`docs/THEME_CONTROLS_2026-09-24.md` and the newest `docs/CURRENT_STATE.md` entry.
Code is scoped to Trontop; no other Rust app was changed. Local source is
uncommitted on `feat/system-specs`. The animated tray graph stays. After the
headless checks, Trent explicitly requested replacement of his running copy.
Alpha.38 closed normally and the verified alpha.39 opened from
`target-latest/release/trontop.exe` at 2026-09-24 19:53:41 UTC: PID 278768,
visible HWND 106565772, responding. Backup and hash are in `docs/CURRENT_STATE.md`.
Broader native theme/window/tray/drag/close/soak acceptance remains separate.

## Earlier checkpoint (2026-09-06)

**Thread closed at Trent's request on 2026-09-06. Documentation-only shutdown.**
Do not automatically resume implementation, launch/replace previews, run a soak,
or manipulate the desktop. Read `docs/CURRENT_STATE.md` (shutdown checklist and
exact candidate), then `ASK_LEDGER.md` (remaining asks/decisions). On a new explicit
resume, agree the bounded next gate before starting work. This is a saved review
checkpoint, not a declaration that the product is complete.

**Alpha.35 adds per-adapter GPU history and memory (A06/A07/A15/A32).** Read
`docs/GPU_ADAPTERS.md` and `docs/CURRENT_STATE.md`. Trent explicitly requested
the newest build launched, private GitHub readiness checked, and a wrap-up spot.
Stop at that handoff; no more feature/polish iterations until further direction.
Release blockers remain in `ASK_LEDGER.md`, not a new expanding task list.
Exact-source Windows CI run 34058415830 succeeded, including artifact upload.
Local candidate and native launch evidence remain unchanged by these notes.

### Previous alpha.34 checkpoint

**Alpha.34 connects dynamic CPU clocks (A06/A07/A22/A32).** Native performance-state
deltas are weighted by each processor's nominal MHz, with explicit source and
interval semantics. Performance shows average/fastest/slowest readings and a
virtualized per-processor list; Graphs and Overview share average/fastest histories.
Read `docs/CPU_CLOCK.md` for the pinned reference, native probe and private-API
limits. CPU temperatures and exact Task Manager frequency aggregation are not
claimed. Build/test evidence belongs in `docs/CURRENT_STATE.md`. Nothing uploaded
or launched, and no personal settings/desktop input changed. The brainstorm remains
proposals, not additional release requirements. Continue existing unchecked asks,
not an unrequested recorder or another speculative styling audit.

### Previous alpha.33 checkpoint

**Alpha.33 adds the requested all-core grid, dense Overview and ColorMagic
randomization (A06/A13/A32).** Distinct logical-processor histories share Graphs'
bounded store; visible-row layout is tested through 256 cores. Six palette families,
Surprise me and 12-roll undo preserve mode/layout and pass seeded contrast tests.
The old Speed label is corrected to Power clock; measured boost speed is not done.
Read `docs/ALPHA33_REVIEW.md` for the 263-pass final suite, six inspected offscreen
views and CPU-only timing. `docs/HARDWARE_RESEARCH.md` records the GitHub backend
audit and read-only Windows counter evidence. Exact built candidate identity is in
`docs/CURRENT_STATE.md`. No current preview/settings, native desktop input, drivers
or remote uploads were touched. CPU temps and native drag/close/tray/soak remain
open. Shared history cursor and developer-project grouping are brainstorm proposals,
not additional required asks. Stop this slice at build/review handoff.

### Previous alpha.32 focus checkpoint

**Alpha.32 fixes keyboard-focus visibility in custom tables (A16).** The local
Tab test failed before the outline existed; the real process-table test also found
an invisible duplicate row target. Labels/heat cells now paint a contrast-safe
outline using existing padding. Rows/icons still accept mouse clicks but no longer
duplicate labeled keyboard targets. Four new regressions cover traversal, keyboard
sorting/selection, disabled states, contrast/geometry and outer-row mouse clicks.
Four offscreen focus PNGs were generated and inspected. Read `docs/THEME_CONTRAST.md`
and `docs/CURRENT_STATE.md` for the final gate/build. No native window/input,
preview replacement, personal preferences or upload. Stop this bounded pass at
handoff; remaining native/acceptance gates and decisions are still open.

### Previous alpha.31 graph-layout checkpoint

**Alpha.31 bounds graph-wall layout work (A22/A32).** The supported 512-chart wall
was still laying out all card contents. It now measures one real row and reserves
offscreen space, with stable IDs, shared row widths and category scroll reset.
Two regressions compare visible text/geometry through deep scrolling and fractional
scales. A third regression fixes dropped font-texture updates in visual click
fixtures; 251 ordinary tests pass, strict Clippy passes. The same-binary CPU probe
and current build are recorded in `docs/GRAPH_WALL.md` / `docs/CURRENT_STATE.md`.
No native input/window/tray interaction, preview replacement, user settings or
upload. Native drag/close/soak and the remaining decisions are still open. This is
a measured graph-page improvement, not a claimed explanation of native drag lag.

### Previous alpha.30 memory checkpoint

**Alpha.30 fixes memory data and expands the requested graphs (A06/A07/A32).**
The old COMMITTED formula and page-file graph used sysinfo's commit-minus-physical
estimate incorrectly. A background K32GetPerformanceInfo query now supplies actual
commit/limit/peak, system cache and kernel pools. Layout reflows and keeps labelled
cached/missing fields; JSON retains explicit legacy-estimate semantics. Five new
regressions and a read-only native probe pass. `docs/MEMORY_COUNTERS.md` has scope
and evidence. Read `docs/CURRENT_STATE.md` for the latest build, not earlier paths.
No preview launch/replacement, native interaction or upload. CPU temperature,
native close/drag/tray/soak, remaining data parity and acceptance are still open.
Stop at this tested memory-data/build handoff; do not start speculative audits.

### Previous alpha.29 tray-startup checkpoint

**Alpha.29 removes the recorded tray-construction wait (A22).** The resumed full
responsiveness objective authorizes this bounded next item from the ledger.
Controller startup no longer awaits Shell tray creation or joins its failed
worker. One private event and latest-sample slot coalesce startup/update backlogs;
the owner thread cleans late construction after cancellation. About reports
Starting/Ready/Update failed/Unavailable/Stopped. Seven new isolated worker tests
pass. See `docs/TRAY_LIFECYCLE.md` and exact build in `docs/CURRENT_STATE.md`.
No native tray/window created in tests, no current preview touched, no upload.
A19 is now REVIEW because the changed native pump needs its isolated gate again;
historical icon captures are not enough. A21/A22/A25 remain incomplete. Stop this
slice at verification/build handoff; do not invent another speculative audit.

### Previous alpha.28 close-path handoff

**Alpha.28 is a bounded A21/A22 settings-path follow-up under the continuing
responsiveness objective.** Two regressions reproduced redundant egui-memory
clones and unchanged saves failing on file contention. Share the captured snapshot
instead of deep-cloning it at dispatch/Retry; skip filesystem work for identical
local bytes while retaining conflict checks for actual changes. Three new tests
include the app-owned headless close gate with busy files. See
`docs/SETTINGS_PERSISTENCE.md`. This is not measured native teardown or proof that
Unity caused the reported delay. No desktop input, preview changes or user-settings
access. A21 stays open. Stop this slice after its verification/build handoff;
do not turn it into a new general audit.

### Previous alpha.27 graph-wall handoff

**Alpha.27 is the explicitly requested Graphs-page slice (A32).** Trent reviewed
alpha.26 at 18:43 local and asked for one mostly-graphs page with all available
temperatures, usage and activity. This authorizes this focused implementation,
not another open-ended bug hunt. Read `docs/GRAPH_WALL.md` for implementation and
verification. CPU sensor integration is still A10/D01. His new question about
slow closing is a diagnosis request, not permission for desktop drag/close tests.
No desktop window was opened or closed during this slice. Source upload remains
unapproved. The graph-wall candidate needs Trent's review before more expansion.

### Previous alpha.26 review handoff (historical)

Trent called out the open-ended bug hunt,
then explicitly authorized replacing old Trontop windows and requested a fresh
build. Clean b79708f was rebuilt and opened from
`target/review/alpha26-b79708f-clean/trontop.exe` at 23:38:40.145 UTC, PID 242180,
HWND 8192130; initial responding/input-idle checks passed. Five verified older
Trontop previews were stopped; other apps were untouched. Read `REVIEW.md` for
exact identity and short changes. This handoff was followed by the explicit A32
request above; alpha.27 now exists locally.
Permission to replace these previews was received and fulfilled. Isolated native
testing and source-upload approval are still separate and unanswered.

Alpha.26 is the local settings-responsiveness candidate. Eframe file persistence
is disabled; one bounded app-owned worker loads/migrates/saves settings. Staged
replacement preserves the existing file, conflicting instances are refused, and
pending close offers Keep open/Retry/Close anyway rather than joining storage.
Theme Studio editing waits for loaded preferences; its frame stays readable and
Revert uses the loaded baseline. Twelve new tests, including a real fixture-file
app-to-worker-to-fresh-app round trip. Final gate: 225 passed, 13 ignored,
strict Clippy/format/release PASS. Final offscreen pass: 101 PNGs, all eight new
settings states plus normal dark/light Theme Studio reviewed. Read
`docs/SETTINGS_PERSISTENCE.md` and latest `docs/CURRENT_STATE.md` for exact identity.
The implementation tests did not touch user settings, previews, desktop input or
remote uploads; the subsequently requested launch is recorded above. The
headless synthetic-close tests are not native drag/close/soak evidence. Tray
startup still has an unbounded ready/failed-worker wait; this recorded A22 finding
is not proven as the reported crash trigger. A13/A20/A21/A22/A25 remain open.
Review comes next. Isolated-desktop and upload approval remain unanswered.
Do not retry the rejected upload by any route. Keep the persistent goal active.

The alpha.25 and older checkpoints below are historical.

Alpha.25 is the local callback-responsiveness candidate: recoverable GPU diagnostics
use one bounded background writer, and service-result polling uses `try_lock`.
Four new regressions; final gate 213 passed, 13 ignored, strict Clippy/format/release
PASS. Fresh optimized offscreen recovery: 3/3 pixel-identical cycles. The actual
main callback also passes a separate full-queue/blocked-writer regression. See
`docs/FAILURE_REPORTS.md` and latest `docs/CURRENT_STATE.md` for exact identity.
No layout changes or new PNG claims; no preview opened/closed/replaced or upload.
The next verified code-level wait is eframe settings persistence: autosave and
drop join a writer indefinitely, startup reads synchronously and writes truncate
directly. Fix with preservation/migration/unsaved/error tests, not silent detached
theme saves. Tray startup also has an unbounded ready wait. These findings do not
establish the original crash or slow-close trigger. A13/A20/A21/A22/A25 remain open.
Relaunch, isolated-desktop and upload approval remain unanswered. Do not retry the
rejected source upload by any route. Keep the persistent goal active.

The alpha.24 and older checkpoints below are historical.

Alpha.24 is the local compact-control candidate: fixed-height dialog identity
cards, aligned History numeric tracks, bounded responsive affinity tiles and an
explicit requested-CPU summary on confirmation. Seven new regressions; final gate
209 passed, 13 ignored, strict Clippy/format/release PASS. All 14 new compact
dark/light cases reviewed among 93 generated PNGs. Read `docs/COMPACT_CONTROLS.md`
and the latest `docs/CURRENT_STATE.md` for exact identity and limitations.
No preview opened/closed/replaced; no upload. Relaunch, isolated-desktop and upload
approval are still unanswered. The persistent fix/fast/snappy/polish objective
remains active; do not mark A14/A15/A16 or native drag/close/soak complete.

The alpha.23 and older checkpoints below are historical.

Alpha.23 is the local custom-theme contrast candidate. It preserves saved RGB
while separating foreground ink and bounding shared text-bearing surfaces. Real
egui state tests cover action text, badges, selected-device labels, heat tiles and
geometry. Final gate: 202 pass, 13 ignored, strict Clippy/format/release PASS;
79 offscreen PNGs with regular/extreme cases inspected. See
`docs/THEME_CONTRAST.md` and the newest `docs/CURRENT_STATE.md` for identity.
No alpha.23 preview was opened. A16 and native drag/close/soak remain unchecked.
The prior push was rejected by auto-review; explicit upload approval remains
unanswered. Do not retry via another route. Work below is historical.

Alpha.22 addresses another A22 responsiveness gap: End task, priority, affinity,
Run task and Reveal in Explorer now dispatch through one non-blocking action
worker instead of invoking Windows on the UI thread. It retains exact confirmed
identity and label, prevents duplicates, reports pending/unknown states honestly,
and never joins a stalled call on close. `docs/PROCESS_ACTION_SAFETY.md` describes
the boundary. Default headless apps cannot execute native actions. UI tests use
injected backends; a native safety test owns its hidden disposable child.
Latest ordinary gate: 196 pass, 13 ignored; strict Clippy/formatting PASS, 73 PNGs
generated with pending/error variants reviewed. See `docs/CURRENT_STATE.md` for
final build identity. No alpha.22 preview has been opened. The active objective
and unanswered isolated-desktop permission below are unchanged.

The alpha.21 and older entries below are historical checkpoints.

New active request after alpha.19 crashed: fix the app, make it fast/snappy and
polish it. Alpha.21 adds a focused A15/A16 compact layout pass to the alpha.20
A20/A22/A25 stability candidate: non-blocking snapshot transfer and local egui-wgpu
device recovery. Empty/hidden inspectors release table width, compact metrics use
two rows, and long inspector names/accounts keep stable aligned fields. Read
`docs/RENDERER_RECOVERY.md` and the newest `docs/CURRENT_STATE.md` for exact EXE
identity. Local gate: 186 ordinary tests, strict Clippy/format/release PASS;
70 offscreen PNGs; three pixel-identical device-loss recoveries. Native
surface/drag/close/soak remain unproven. No alpha.20/21 preview has been opened.
Permission for a separate non-visible test desktop was asked but not received.
No cross-project rollout or new visible test windows.

The alpha.19 preview below crashed at 18:29:11.956 UTC, confirmed by local logging
and Windows events. Initial Responding was not a stability test. The remaining
alpha.19 and older notes are historical checkpoints.

Latest explicitly requested preview: alpha.19 opened at 18:27:00 UTC on 2026-09-05
from `target/preview/alpha19-20260905-182659/trontop.exe`, PID 280516. Initial
read-only check: Responding=true/native HWND present; hash matches the review build.
Older previews and unrelated windows were untouched. No further restart/replace
or desktop interaction is authorized by that completed launch request.

Read `ASK_LEDGER.md` first for the finite completion checklist. Trent called out
diminishing returns and requested one consolidated ask ledger. The resumed stability
objective takes priority. Keep work tied to its asks; do not add unrelated features
or restart an open-ended visual concept loop.

Working branch: `feat/provider-diagnostics`. Read the newest
`docs/CURRENT_STATE.md` entry for verification and review-EXE identity.

Alpha.19 implements four-peg gradients and a tabbed Theme Studio: editable positions,
hex colors, independent accents, eight presets, surface controls, named saves,
import/export, reset/revert and v2 migration. Final local gate: 175 passed, 0 failed,
10 ignored; strict Clippy/formatting/release PASS. Sixty-seven offscreen PNGs; six
theme layouts inspected. See `docs/THEME_STUDIO.md`. No preview was opened/replaced.
`ASK_LEDGER.md` now owns the finite definition of done; A13 remains unchecked for
native restart persistence and Trent's review. No complete-app/parity/drag claim.
Alpha.18 was checkpointed locally as 0a176d3; remote alpha.19 gate pending.

The following alpha.18 and older implementation notes are historical.

Alpha.18 adds independent physical-disk PDH activity, latency, queue and read/write
rates, retained missing states, gap-aware charts and JSON diagnostics. Local gate:
166 passed, 0 failed, 10 ignored; strict Clippy/formatting/release PASS. Sixty
offscreen PNGs produced, physical disk dark/light/compact inspected. Native read-only
probe returned all five fields on three disks. See `docs/PHYSICAL_DISKS.md`.
Alpha.17 CI 33969106100 passed for 02ea7f8. Alpha.18 remote gate not yet run.
No alpha.18 preview launched. Next requested work: four-peg gradients and polished
Theme Studio, with saved-theme migration and offscreen tests. Current open previews
must remain untouched unless Trent explicitly requests a new launch/replacement.

The following alpha.17 implementation and preview details are historical.

Alpha.17 adds best-effort bounded local Rust-panic/native-runner failure metadata,
an allowlisted JSONL format, non-waiting file locking and About discoverability.
See `docs/FAILURE_REPORTS.md`: this is not a native crash dump/hang handler.
Local gate: 153 passed, 0 failed, 9 ignored; formatting/strict Clippy/release PASS.
57 offscreen PNGs produced; About dark/light/compact inspected. Actual panic probes
used only owned hidden test children and fixture files, not the running GUI.
Alpha.16 CI 33967714299 passed for df7e3fb; alpha.17 remote gate pending.
On explicit request, alpha.17 was opened at 13:41:08 UTC on 2026-09-05 from
`target/preview/alpha17-20260905-134108/trontop.exe`, PID 275020. A read-only desktop
check confirmed Responding=true, HWND 9577990 and title Trontop. Its hash matches
the verified review build. Older instances and other windows were untouched.
Do not restart or replace this preview without fresh permission.
Next major gaps: suspend/resume, CPU/motherboard sensors, startup controls, native
isolated service/export and close/drag/soak verification. Do not restart the demo.

The alpha.16 implementation details below are historical.

Alpha.16 sorts tree resources by displayed subtree totals, retains expansion using
observed process identity, and prevents invisible sort keys after page switches.
Local gate: 143 passed, 0 failed, 9 ignored; formatting/strict Clippy/release PASS.
57 offscreen PNGs produced; four relevant layouts inspected. See `docs/PROCESS_SORTING.md`.
Alpha.15 CI 33965959216 passed for 7df48c9. Alpha.16 remote verification is pending.
On explicit request, alpha.16 was opened at 12:51:50 UTC on 2026-09-05 from
`target/preview/alpha16-20260905-1251/trontop.exe`, PID 272352, HWND 6370918,
Responding=true. Sandbox window enumeration cannot verify desktop liveness; a
read-only desktop-session check confirmed this launch. Older instances untouched.
Do not restart or replace this preview without fresh permission.

The alpha.15 implementation details below are historical.

Alpha.15 replaces recursive hierarchy building with indexed iterative traversal,
rejects known newer-parent links, makes cycle members honest separate roots, and
keeps full totals/search context. Indentation stays readable with actual depth on
hover. Nine new tests cover 50,000-level chains/cycles on a 256 KiB stack, arbitrary
graphs, reference equivalence and compact light/dark scroll/tooltip/selection.
Gate: 134 passed, 0 failed, 9 opt-in ignored; formatting/strict Clippy/release PASS.
55 offscreen PNGs produced, four process-tree variants actually inspected.
Three paired headless probes: 1,000-node collapsed-chain rebuild 16,771.9 to 132.8 us;
not native drag/FPS/whole-app proof. See `docs/PROCESS_TREE.md` for measurements,
binary identities and remaining visible-sort/expansion-state issues.
Alpha.14 CI 33964418983 passed for 5b000f4. Alpha.15 remote gate not yet passed.
No alpha.15 preview was launched; the explicitly opened alpha.14 below is untouched.

The alpha.14 implementation details below are historical.

Alpha.14 replaces per-frame full-process copies with snapshot indices in Processes/
Details and caches the History top twelve on view rebuild. Name/account comparisons
no longer allocate folded strings. Four new ordinary regression tests pass. Local
gate: 125 passed, 0 failed, 8 ignored; strict Clippy/release PASS; 53 offscreen PNGs,
four process-related variants inspected. Three paired optimized headless runs measured
5,000-process tree frame medians of 3,888.1 us before and 206.9 us after; 500-process
tree 307.6 to 211.2 us. This is not GPU/native drag/FPS validation. Read
`docs/PROCESS_VIEW_PERFORMANCE.md` for the exact fixture and remaining costs.
On Trent's subsequent explicit request, alpha.14 was opened from
`target/preview/alpha14-20260905-1200/trontop.exe` at 12:00:07 UTC on 2026-09-05,
PID 273992. Responding=true/HWND 1181862 observed afterward. Older instances and
other windows were untouched. See `docs/CURRENT_STATE.md` for the verified hash.
Alpha.13 CI run 33963422802 for 3142016 passed
formatting, tests, strict Clippy, release and artifact upload at 11:49:30 UTC.
Alpha.14 has not yet passed its remote gate.

The alpha.13 section below is historical; use the newer gate and build above.

Alpha.13 adds explicit JSON/CSV snapshot export with private fields off by default,
one background picker/encoding/file worker, staged replacement, and stable compact
UI. Read `docs/EXPORTS.md`: names are not anonymous; service-command annotations and
chart history are excluded. Native Save As is compiled, not interactively verified.
No live snapshots were exported, and no picker or new preview was opened during this
slice. Final gate: 121 passed, 0 failed, 7 ignored; strict Clippy and release PASS.
The offscreen pass produced 53 PNGs; three export layouts were visually inspected.

Previous explicit preview launch: final alpha.12 at
`target/preview/alpha12-final-20260905-111553/trontop.exe`, PID 262932, 11:15:53 UTC
on 2026-09-05. Responding=true/HWND 3553034 observed after launch. Older previews
were untouched. Alpha.13 review EXE exists separately and was not launched.
Alpha.12 Windows CI 33961633447 passed for cc2b793 at 11:09:25 UTC. The new alpha.13
remote gate has not passed yet; no alpha release is published.

The following alpha.12 details are historical; the latest state above takes precedence.

Implemented: Overview and Hardware sensors navigation, About/provider diagnostics
and explicit privacy-safe report copy, stable missing/cached GPU sensor fields,
gap-aware GPU activity charts, bounded worker-shutdown waiting, and original vector
navigation/window/action/tree icons. Four user mockup boards and a generated
transparent Signal icon candidate are saved in `docs/inspiration`; the candidate
is not installed as the application icon. Alpha.7 integrates read-only Windows drive
temperatures using isolated, bounded workers with timeout/backoff/cache states.
The TEAM SSD returned three real readings in the opt-in native test. CPU/motherboard
providers remain unconnected. No full Task Manager parity.

Alpha.8 preserves warmed PDH handles across inventory refreshes, aggregates GPU
activity by physical engine identity, and distinguishes measured/partial/warming/
unreported/unavailable process values. Missing data no longer becomes fake zero in
process, tree or user cells. Partial totals use `>=`; compact meters/tray omit partial
values, and history records gaps. Inspector status stays one line with hover detail.

Alpha.9 adds real process executable icons with one bounded background worker/cache,
fixed-size vector fallbacks and a hover-only inspector icon. See `docs/PROCESS_ICONS.md`
for local-path restrictions, request/upload limits, stale artwork and native resource
ownership. Test fixtures never start this worker or query their executable paths.

Alpha.10 retains Startup entries independently across five native sources. Failed
reads cannot silently delete cached entries; source and row freshness remain visible.
Services labels its retained list as cached after failure. Both inventory tables keep
their fields in place and format only visible rows. See `docs/STARTUP_INVENTORY.md`.

Alpha.11 adds confirmed Start/Stop/Restart through one independent service-command
worker, with same-handle state/PID checks, access errors and bounded worker drop.
Stable status surfaces and Pre-command rows expose uncertain outcomes without
invented state. Read `docs/SERVICE_CONTROLS.md`: injected command tests and native
read-only queries passed, but actual native service commands need isolated validation.
Alpha.12 retains state observations and uncertain outcomes per service until newer
complete inventory resolves them. Its bounded cache never evicts unknown outcomes;
at capacity, new service targets require a successful refresh. Detailed command
progress/error text still shows the latest command, not a persistent activity log.
Startup and Services now each have an independent read-only inventory worker.
Blocked reads cannot directly hold up the sampler; five-second reporting deadlines
preserve cached fields without spawning duplicate workers. See `docs/INVENTORY_WORKERS.md`.

Local tests: 111 passed, 0 failed, 7 opt-in tests ignored. Strict Clippy passes.
Offscreen QA produced 50 PNGs without native windows/input; the two new timeout
variants and retained-service-outcomes image were inspected, not all 50 images.
The read-only inventory-worker probe returned 13 Startup entries and 303 services;
1,000 snapshot-pair reads took 211.1 microseconds in one short test, not a whole-app
performance measurement. The earlier read-only icon probe
extracted this test EXE in 2.4387 ms; 40 repeats left GDI/USER counts at (4, 2).
The earlier PDH refresh test retained 690 handles with 690/690 valid rates afterward.
The optimized alpha.12 review EXE is built and dependency-inspected. Native end-to-end close
latency and dragging performance are NOT measured. CPU provider research and the
slow SSD/HDD probe findings are in `docs/SENSORS_PLAN.md`; no driver install authority.

On explicit launch requests, hash-verified preview copies were opened on 2026-09-05:
alpha.10 at `target/preview/alpha10/trontop.exe`, PID 259420 at 09:48:31 UTC;
alpha.11 at `target/preview/alpha11/trontop.exe`, PID 274860 at 10:02:27 UTC;
alpha.12 at `target/preview/alpha12/trontop.exe`, PID 263640 at 10:42:33 UTC.
All were observed responding with native window handles immediately afterward.
No old instances or other windows were touched. These observations are historical;
recheck runtime state if needed. The legacy `target/release` copy remains alpha.5.
Do not automatically restart or replace any preview. See `docs/CURRENT_STATE.md`
for exact hashes and the earlier alpha.1 backup.

Next: CPU/motherboard coverage, suspend/resume, native service/export validation and the
release gates in `RELEASE_PLAN.md`. Other sysinfo/PDH/NVML/process-control paths still
need broader stall/overhead measurements; inventory isolation does not prove these.
Alpha.11 Windows CI run 33960026614 passed for checkpoint 8e343e2; that run does not
verify the new alpha.12 source, whose remote gate is pending. No alpha release is
published. No new permission to restart
the deployed copy, manipulate windows or install a shortcut hook.

## Previous verified checkpoint

Latest local version: 0.3.0-alpha.4. Read the latest section of `docs/CURRENT_STATE.md`
and `docs/HEADLESS_QA.md`. The hover/rounded-controls slice and safe headless harness
are now implemented. A separate optimized review EXE exists under
`target/review-build/release/trontop.exe`; do not confuse it with the still-running
older `target/release` copy. Seventeen test-fixture PNGs were generated without opening
any app window. No native drag fix or published release is verified.

Trent reports a clear subjective drag improvement; there is no measured 60 FPS claim.
Alpha.3 integrates real NVIDIA NVML temperature/power/clocks/fan/VRAM on the sampler,
with a dedicated GPU Sensors page and bounded, gap-aware histories. Native read-only
probe and headless checks passed. Details and remaining CPU/storage/vendor work are
in `docs/SENSORS_PLAN.md`. No app windows were launched or manipulated for this slice.

Alpha.4 adds exact native process-creation identity checks on the same handle used
for End Task/priority/affinity, Windows-critical-process refusals, and stale-confirmation
protection. Read `docs/PROCESS_ACTION_SAFETY.md`. Local gate: 35 tests passed, strict
Clippy and optimized build pass. The shared action-button helper also fixes the
confirmation baseline offset, verified by geometry tests and offscreen images.
The larger alpha release gates are still incomplete. Code checkpoint `f2f725c` is
pushed privately and Windows CI run `33946298909` passed all its gates. Read
`docs/CI_ALPHA4.md` for the CI artifact/hash and `docs/DIAGNOSTICS_PLAN.md` for the
next bounded slice. Storage temperature probing succeeded on the TEAM SSD but is
not integrated; HDD queries were slow, so do not put them on the system sampler.

## Earlier checkpoint (historical)

Version 0.3.0-alpha.1 builds on the seven-page native system control deck with a real
process hierarchy, guarded scheduler controls, theme-derived zebra tables, and an
embedded multi-resolution Windows icon and version resource.
Read `docs/CURRENT_STATE.md` for the full handoff and `TASK_BOARD.md` before choosing
new work. The private repository is connected at `TrentSterling/trontop`; authentication
and the first Windows CI run succeeded. The private alpha release has NOT been published.

Trent parked this session to work on another project. Read `docs/DRAG_INVESTIGATION.md`
first when resuming. Window dragging is still visibly laggy compared with Terminal and
Explorer. There is no validated drag fix to port to Boxel or other egui projects.
Trent's latest global zebra/rounded-controls/branding request is saved in
`docs/UI_POLISH_BRIEF.md`; it is not limited to the process table.

The latest local slice adds a dedicated native tray worker with a full-width CPU meter
and scrolling history, column bands, consistent table cells, a rebuilt Users resource
table, and scrollable inspector content. Branding/icon polish and a proper headless UI
smoke harness remain unfinished. Do not mistake the passive drag recorder for that harness.

Two small headless widget tests now check left-aligned labels, right-aligned numbers,
vertical centers, and full-cell clicks. The final table-label alignment fix is tested
in source but not release-built or visually checked; the running EXE is older.

No global mouse/keyboard injection, focus stealing, or automatic minimize/restore/move
tests on Trent's working desktop. Earlier automation almost closed a day-job Claude
session. Use headless tests. The old temporary desktop-control helper is now guarded
against those actions. Other day-job windows are off-limits.

GPU Engine instances are enumerated through PDH and aggregated by PID. Do not
synthesize counter paths or display estimated GPU values. All slow inventory and
live telemetry remain outside the render thread.

## Verification

```powershell
cargo fmt --all --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```
