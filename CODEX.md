# CODEX.md: Trontop handoff

Trontop is an active hand-driven Codex project. The overnight coordination loop
must not modify this repository.

## Current checkpoint

Working branch: `feat/provider-diagnostics`, alpha.13 source. Read the newest
`docs/CURRENT_STATE.md` entry for verification and review-EXE identity.

Alpha.13 adds explicit JSON/CSV snapshot export with private fields off by default,
one background picker/encoding/file worker, staged replacement, and stable compact
UI. Read `docs/EXPORTS.md`: names are not anonymous; service-command annotations and
chart history are excluded. Native Save As is compiled, not interactively verified.
No live snapshots were exported, and no picker or new preview was opened during this
slice. Final gate: 121 passed, 0 failed, 7 ignored; strict Clippy and release PASS.
The offscreen pass produced 53 PNGs; three export layouts were visually inspected.

Most recent explicit preview launch: final alpha.12 at
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
