# CODEX.md: Trontop

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
