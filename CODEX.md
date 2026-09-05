# CODEX.md: Trontop handoff

Trontop is an active hand-driven Codex project. The overnight coordination loop
must not modify this repository.

## Current checkpoint

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

Working branch: `feat/provider-diagnostics`, alpha.22 source. Read the newest
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
