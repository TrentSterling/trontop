# CODEX.md: Trontop

## Local cleanup and minimized-app build, alpha.57, 2026-11-24

**Toolbox → Cleaner** now recognizes 15 desktop app profiles, including Steam,
plus dated npm and OBS logs, locally built pip wheels and cached Electron runtime
archives. Scheduled cleanup uses separate opt-in category bits for these additions.
The previous local executable is at `target/release/trontop-alpha56.exe`.

**Toolbox → Maintenance → Performance → Sleep when minimized** arms a rule for
the selected process lifetime. After 30 seconds with all visible windows
minimized, the worker acquires a Windows suspension reference. A visible or
foreground window, disarming or Trontop exit releases that reference. No rule
is enabled by default or saved across restarts.

The read-only category scan and targeted sleep tests are in
`target/maintenance-alpha57-catalog.log` and `target/maintenance-alpha57-sleep.log`.
The window observations for the pause and wake check were simulated. The
heartbeat process and its Windows suspension were real. The native worker
also rejected an actual windowless process and a mismatched creation time.
The complete regression run is in `target/maintenance-alpha57-verified-tests.log`.
`target/maintenance-alpha57-release.json` records the release hash and
read-only EXE previews for the original, added and combined categories.
The 18,567,168-byte local executable has SHA-256
`ACCA2AB5B54D867B5F141079CF21BFBBAEEFCEA193C88FC2B634F7A175DD8A2F`.

## Local maintenance build, alpha.56, 2026-11-24

Open **Toolbox → Installed Apps** for desktop installers and current-user
Windows packages. **Toolbox → Maintenance** contains health checks, WinGet,
driver and browser tools, reversible App Paths cleanup, System Restore,
drive operations, schedules, process ranking and cloud storage shortcuts.
SpaceView now supports reviewed deletion of a single verified duplicate.
The README describes which actions use Windows or the browser.

The portable build is 18,519,040 bytes. SHA-256 is
`E68CC353D93D245E7C0D5B6A6F0F6701943D446276FC488B2953692930F4A567`.
The previous local build is `target/release/trontop-alpha55.exe`.
Receipts live under `target/`:

- `maintenance-alpha56-tests.log` contains the regression suite. The
  disposable vendor fixture checks installer dispatch. The registry fixture
  checks archive and restore under a test-only key.
- `maintenance-alpha56-providers.log` records desktop and packaged app
  inventory, extension counts, health preview and the held-file test.
  The System Restore provider required administrator access on this machine.
- `maintenance-alpha56-scheduler.log` covers registration, XML inspection and
  removal of a uniquely named preview-only task. The normal schedule is opt-in.
- `maintenance-alpha56-release.json` identifies the EXE and records its
  read-only preview. The local WinGet source also listed 50 available upgrades.

Machine checks exercised discovery. Package upgrades, removal of installed
software, full drive erasure and live restore-point removal were excluded from
machine verification. The browser and Windows handoffs use the owning application's
UI. A broader third-party cleaning catalog and automatic background-app policies
remain outside this build.

## Local Cleaner build, alpha.55, 2026-11-24

Open **Toolbox → Cleaner → Preview cleanup**. Category totals cover temporary
files, crash dumps, supported browser caches and NuGet HTTP downloads. Selecting
a category narrows the file list. Review shows the complete selection across
categories before permanent deletion. Windows Disk Cleanup opens its own dialog.

The release at `target/release/trontop.exe` has SHA-256
`BEEE8F47358B2D2268ED474722D0CE11AD1618971DF5BB647E8118C02CBD7925`.
The previous executable is at `target/release/trontop-alpha54.exe`.
The read-only machine preview is in `target/cleaner-alpha55-preview.log` and
the regression run is in `target/cleaner-alpha55-tests.log`.

## Local Toolbox build, alpha.54, 2026-11-23

Open **Toolbox** in `target/release/trontop.exe`. The README lists the integrated
disk map, duplicate finder, project cache inventory, scan comparison, cleaner,
mount history and Windows file-user lookup. The cleaner asks for selection,
review and permanent-delete confirmation. Verification used disposable files.

The portable EXE is 18,138,624 bytes. SHA-256 is
`80744200EBE8C1280B382804948BD79ABB8507ABE4673BD29999E44B94E69D96`.
Build evidence is in `target/toolbox-full-test.log` and
`target/toolbox-native-test.log`. The populated disk map capture is
`target/ui-smoke/toolbox/spaceview.png`. The GPU engine grid width fixture now
uses a tall enough canvas to measure all six rows at its narrowest width.

## Latest focused update: alpha.53, 2026-10-12

Overview process selection now opens the inspector in place. End process, branch,
and all instances keep Overview selected through review, cancellation and submission.
Open in Details is an explicit link. Existing frozen identities and confirmation remain.

Verification: 624 passed, 60 optional ignored; fmt, strict Clippy and release passed.
Six offscreen reviews include Overview in both themes: `target/ui-smoke/process-controls-alpha53/`.
Receipts: `target/overview-test.log`, `target/overview-clippy.log`, `target/overview-build.log`,
`target/overview-visual.log`. One parallel test run crossed a fixture freshness deadline;
rerun with two test threads passed. No global input or real process termination used.

Release `target/release/trontop.exe` SHA256:
`AC53E2AD93762F2D257BD45654DB81BC250795C35B1F6A2484C0BE4A850FA60E`.
Installed and restarted after Trent explicitly approved on 2026-10-12. Installed
SHA256 AC53E2AD93762F2D257BD45654DB81BC250795C35B1F6A2484C0BE4A850FA60E;
new PID 115940 remained alive after launch. Previous binary is backed up under
target/update-alpha53. No real user process was terminated to test the controls.
Running observed PID 328184 is the installed LocalAppData copy, not this new build.
The broader paused coverage goal stays paused. Earlier alpha.52 notes follow.

## Latest focused update: alpha.52, 2026-10-04

Trent requested **End all instances** as the prominent, fast action. The command
bar and inspector now open that scope directly with one counted confirmation.
The scope checkbox is removed; the target list is expandable. **More process
actions** retains confirmed single-process and branch termination. Existing
executable-path matching, frozen identities and native preflight are preserved.

Ordinary checks: **622 passed, 0 failed, 60 ignored**. Format, strict all-target
Clippy and optimized release PASS. Eight focused UI checks and four inspected
offscreen renders cover the new flow and granular alternatives. Evidence lives
in `target/review/alpha52-end-all/` and `target/ui-smoke/process-controls-alpha52/`.
The required instrumented run also passed all 622 ordinary tests, 24 fixed
read-only probes and ten analyzer checks. Current production line coverage is
24,674/26,449 (93.29%), with zero mixed lines. Its receipt and source hashes are in
`target/coverage/runs/20261004-153510-785/`; this is measurement, not closure of
the broader goal. Release SHA-256:
`062B7954366CAD21FF548F0BAB96AE62ED9B2544EAB8657FC0805CF705878845`.

The updated release at `target/release/trontop.exe` is open: PID 194272, title
Trontop, visible and responding, verified 2026-10-04 15:38 UTC. The existing
Windows Startup shortcut points to this path. Old alpha.51 PID 138352 was
replaced after exact identity checks; its binary is preserved as
`target/release/trontop-alpha51-before-end-all.exe` and in the alpha51-hotkey
review folder. These process IDs are observations, not future action targets.

This user-requested update does not resume the broader paused coverage goal or
authorize desktop-input testing. See `docs/PROCESS_CONTROLS.md` for current UI
behavior and the historical resume anchor below for separate outstanding scope.

## Historical paused handoff: alpha.51

Trontop is a hand-driven Codex project. The overnight loop must not modify it.
Trent requested a good stopping point and future resume notes on 2026-09-30.
The goal "make trontop amazing 100% coverage" is **PAUSED at his request**.
Do not continue feature work automatically or mark the full goal complete.

**Read [docs/RESUME_ANCHOR.md](docs/RESUME_ANCHOR.md) first on resume.**
It records the exact candidate, hashes, all remaining source/native/decision
gates, safe commands, lease rules and suggested next scope.

Checkpoint alpha.51 is open: PID 126156, title Trontop, nonzero handle 75901536,
at `target/review/alpha51-provider-joins/trontop.exe`. Older Trontop instances
were stopped at Trent's explicit request. No global input or focus automation.
Leave it running. PIDs are historical observations and must be checked again.

Final checks: **589 passed, 0 failed, 58 ignored**, fmt/strict Clippy/release PASS;
24 exact read-only probes and ten analyzer checks PASS. Coverage is **93.34%**
(24,354/26,093), 1,739 missed, zero mixed. All 195 source/build inputs verified.
No compiler/check handle remains active; zero test children/Startup fixture leaves.
Receipt and verification scripts are beside the EXE.

All remaining lines are in [docs/COVERAGE_REMAINING_2026-09-30.md](docs/COVERAGE_REMAINING_2026-09-30.md).
`ASK_LEDGER.md` remains the product checklist. A39 is done for the requested
implementation/headless scope. Trent explicitly said not to hijack his mouse
for tray testing and to call it done. Do not reopen A39 or request mouse testing.
Other native gates remain separate; source coverage is not acceptance evidence.

Worktree is local and uncommitted on `feat/system-specs`, base
`c8ed88f5ec57f79be717151ab44c43826b4a4e5a`. Preserve tracked/untracked work and
all alpha.42-51 review artifacts. Source ZIP and tracked patch are beside alpha.51.
No commit, upload, install or publication at this checkpoint; public alpha.41
is unchanged. Reacquire the exclusive repository lease before any future write.

Root/project AGENTS.md and CLAUDE.md apply. Never inject global desktop input,
steal focus, run blanket ignored tests, touch real services/sign-in settings or
restore an old source backup. Use headless checks and the fixed read-only probes.
Historical notes: `docs/CODEX_HISTORY_BEFORE_ALPHA51_2026-09-30.md`.
