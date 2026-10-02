# Trontop resume anchor: paused at alpha.51, 2026-09-30

Trent asked for a good stopping point and a future resume anchor after saying
Trontop rocks and is basically fixed. The completion goal is **PAUSED at his
request**, with a verified, running candidate. This is a checkpoint, not a claim
of 100% coverage or closure of every item in `ASK_LEDGER.md`. Do not resume
feature work automatically. Read this first when Trent requests another iteration.

## Exact checkpoint

- Version: **0.3.0-alpha.51**.
- Running review EXE: `C:/trontstack/trontop/target/review/alpha51-provider-joins/trontop.exe`.
- Release/review SHA-256: `59930255969997BD93B34E36DD34808D46A5358B4396689B3BF29B9CC91FEEC0`.
- Size: **17,837,568 bytes**; embedded File/Product versions both alpha.51.
- **589 ordinary tests passed, 0 failed, 58 ignored**. Formatting, strict
  all-target Clippy and optimized release passed. The instrumented suite also
  passed 589 ordinary tests, 24 exact read-only probes and ten analyzer checks.
- Production unique line coverage: **24,354 / 26,093 = 93.34%**;
  **1,739 uncovered**, zero mixed lines, 25,318 test-only lines, 40 raw profiles.
- Coverage run: `target/coverage/runs/20260930-103155-387/`.
- Application/build source fingerprint:
  `6919A9395DB334DB3C597584F80A6829F303167E6AA340FA3AC029841B84487F`;
  all **195 inputs** independently verified against the measured source.
- Instrumented EXE SHA-256:
  `753AC3387736400CBDB6076FE04B3D57B52605076F4541F21AED6F3638A78F8D`.
- Review `candidate-gates.json`, `receipt.json`, `verification-output.json`,
  `verify-receipt.py` and ordinary/lint/build logs are beside the review EXE.
- Native launch at Trent's explicit request: PID **126156**, title **Trontop**,
  nonzero window handle **75901536**, outside sandbox with WindowStyle Normal.
  These are checkpoint observations; check process identity again on resume.
  Alpha.50 PID 209092 was stopped after an identity check. Earlier alpha.49
  PID 165360 was stopped when alpha.50 opened. No desktop input was injected.
- Post-check audit: zero test processes, zero private Startup fixture leaves.
  No build/check session remains active. Public alpha.41 was not changed.

## What is finished for this iteration

The candidate includes End process tree, confirmed End all instances by executable
path, guarded Suspend/Resume, reversible Startup controls, hardware/provider
fallback fixes, graph/freshness/privacy fixes, and close to the live tray.
Window X hides when the tray is Ready; Show restores; explicit Quit saves and
exits. Missing/failed tray and failed/slow settings saves remain reachable.

**A39 is DONE for Trent's requested implementation/headless scope.** His exact
instruction was: "dont hijack my mouse to test systray tho just fix it and call
it done". Do not reopen A39, request mouse testing, or run native tray automation.
Broader optional native release measurements below remain distinct.

Alpha.51 finishes the storage changes already in progress when Trent requested
the checkpoint. Physical media joins use unambiguous UniqueId plus format,
not PhysicalDisk DeviceId as an OS disk number. Overflowing identifiers are
rejected. OS capacity survives unavailable physical metadata; unknown partition
number/size stays unavailable. Volume errors and ambiguous joins are explicit;
impossible free space is rejected while known zero free space is retained.
Twelve new fixtures pass; ten regressions failed before fixes. Unused permissive
WMI array helpers were removed after strict Clippy caught them. No tests were
disabled to get a passing suite. Scope: [storage provider checks](STORAGE_PROVIDER_JOINS_2026-09-30.md).

The final read-only storage probe retains Healthy on both native NVMe and SATA
disks, GPT partitions and volume metadata, plus the NVMe health log (45 C,
4% used, no critical warning). The empty USB disk stays unavailable. This is
one local observation, not cross-hardware correctness or Task Manager parity.

## Source coverage still remaining

[Complete remaining-line inventory](COVERAGE_REMAINING_2026-09-30.md) lists all
92 production files with misses, with exact compressed line ranges. It sums
to all 1,739 missed lines. The run's `production-lines.json`,
`production-summary.txt` and `html/index.html` are the authoritative artifacts.
This is source line execution, not branch coverage or native product acceptance.
Do not exclude production files or merge historical runs to claim 100%.

Useful next work, when requested:

1. Pure Storage Management boundary cases: `src/specs/storage/management.rs`
   has 16 missed lines. Fixtures for duplicate OS disk identities, missing native
   disk numbers/OS rows, missing OS identifiers, physical capacity disagreement,
   text/malformed drive letters and malformed AccessPaths can exercise production
   guards without COM, disk handles or mouse input. Inspect the actual uncovered
   lines first; do not add tests that merely mirror implementation.
2. Headless app/worker failure, recovery, shutdown and persistence behavior.
   App has 150 missed lines; sampler 36 and preferences 35. Use injected providers
   and owned temporary files. Real startup/main/tray paths remain native gates.
3. Native failures that can safely be decoded or injected in pure helpers:
   WMI 49, OS inventory 47, Windows metrics 45 and native specs helpers 43.
   Keep production paths wired to the tested helpers; avoid a test-only clone.
4. Native export picker (57 misses), tray lifecycle (61), Startup controls (52)
   and service command paths require their separate isolation/authorization
   gates. They are not permission to control Trent's desktop or real services.

Stop after the agreed next scope. One final frozen-source measurement is enough
after meaningful fixes; do not repeatedly run passing probes for a higher number.

## Product and native acceptance still remaining

`ASK_LEDGER.md` remains the authoritative checklist. The following items were
not silently checked by this positive checkpoint feedback:

| Ask / decision | Remaining work |
| --- | --- |
| A38 | Review End tree / all instances on the exact candidate; identities and confirmation are already tested. |
| A04 | Real Startup next-sign-in/Task Manager behavior and isolated service command/recovery acceptance. |
| A06 | Field parity sheet; process network rates, remaining adapter/CPU details and NPU support or explicit status. |
| A10 / D01 | Agree the CPU/core/board sensor provider or documented limit. Existing read-only bridges work with an already running supported provider; none runs here. No driver install authorized. |
| A33 | System page review and native TXT/JSON Save As picker validation. |
| A11, A13-16, A34 | Final branding/page/dialog/graph/theme review; native restart persistence and DPI/focus checks. Historical polish findings must be rechecked before assuming they still exist. |
| A19, A21-22 / D04 | Separate native tray pump/hidden CPU/teardown, actual explicit-Quit latency, agreed performance budgets and 60-minute mixed-load soak. A39 stays done without mouse testing. |
| A20 / D02 | Real dragging investigation remains parked. Fresh isolated measurement permission required; no cross-project rollout without a verified fix. |
| A25 | Windows CI plus final integrated native process/service/export/close gates, slow/failing-provider behavior and soak on the exact EXE. |
| A26 | Clean-machine/no-Rust, relocation, standard-user and read-only application-folder portability checks. |
| A29, A31-32 / D03 | Exact-build acceptance, graph layout review, release boundary/deferrals, tag/hash/download and rollback identity. No publication is requested at this checkpoint. |
| D05 | Decide whether optional Ctrl+Shift+Esc takeover belongs in scope. No hook or registry redirection installed. |

The 58 ignored tests are intentional opt-in gates, not 58 failures. The coverage
run executed only its fixed 24 read-only names. Never run all ignored tests:
some create native windows or carry separate process/service/UI acceptance scope.

## Preserve and resume safely

Repository: `C:/trontstack/trontop`, branch `feat/system-specs`, base HEAD
`c8ed88f5ec57f79be717151ab44c43826b4a4e5a`. Alpha.42 through alpha.51 changes
are local and uncommitted; preserve tracked and untracked work. Do not reset,
clean, restore an old source backup, or overwrite another session's changes.
Alpha.42 through alpha.50 review binaries are preserved and independently hashed.
`checkpoint-source.zip` and `checkpoint-working-tree.patch` beside alpha.51
preserve its hashed application/build inputs and tracked diff for recovery;
they are not instructions to overwrite the workspace on resume.

Read root/project AGENTS.md and CLAUDE.md. Acquire the repo lease with exclusive
create before writing. Canonical path is `c:\trontstack\trontop`; lease key
`dbe1e0d7ef154a5db36b758fa5c61726f4b53acc5be534394367a3afc4752328`.
The current session releases its lease at the pause checkpoint. `CODEX.md`
keeps this repository reserved from the overnight loop. Journal is append-only.

No global mouse/keyboard injection, focus stealing, native drag/resize tests or
real service/sign-in mutations. Trent authorized stopping older Trontop versions
and opening the verified latest candidate; that does not authorize desktop tests
or touching other applications. Leave the checkpoint app running at handoff.

When another iteration changes source, bump the local alpha, finish edits, then
freeze inputs until all known compiler/check/coverage handles are terminal:

```powershell
cargo fmt --all --check
cargo clippy --offline --all-targets -- -D warnings
cargo test --offline -- --quiet --test-threads=4
cargo build --offline --release
./scripts/measure-coverage.ps1 -ReadOnlyProbes -TestThreads 4
```

Use `-ReadOnlyProbes` only on the supported reference hardware. Ordinary fixtures
use hidden owned children and private HKCU Startup leaves; no global input.
Audit cleanup, hash/copy/version the exact review EXE and verify every source
input/probe receipt. Preserve older review binaries. Documentation-only updates
do not require rerunning passing application checks.

Suggested next message: **"Resume Trontop from docs/RESUME_ANCHOR.md; start with
the remaining pure storage identity fixtures and headless coverage."**
