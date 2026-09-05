# Trontop release plan

Trontop is moving from a private native prototype to a public Tront desktop product.
The first public build should be useful, safe, portable, and honest about every
telemetry provider. It does not need to reproduce every Task Manager page before it
ships.

## Product promise

- A fast Windows 11 process and performance manager that respects expert users.
- One portable `trontop.exe`; no installer or runtime asset directory required.
- Local-only telemetry and settings. No account, analytics, advertising, or phone-home.
- Exact counters where Windows exposes them. Clear unavailable and warming states
  everywhere else.
- Destructive and privilege-sensitive actions are guarded, named, and recoverable
  where the operating system permits.

## Release train

### 0.2 foundation (complete)

- Seven functional pages, exact GPU Engine PDH telemetry, portable release build,
  frameless Tront chrome, live theme editor, and dynamic resource tray icon.
- Process termination requires confirmation. Sampling remains off the render thread.

### 0.3 private alpha

Goal: Trent can use Trontop as a daily secondary task manager without guessing what
an action will do.

- Process hierarchy with expand/collapse, descendant counts, and clearly labeled
  aggregate resource totals.
- Current priority and affinity telemetry collected off the UI thread.
- Guarded priority, affinity, suspend, and resume actions. Realtime priority is not
  offered in the normal control surface.
- Guarded service start, stop, and restart actions with useful access-denied states.
- About and diagnostics surface with version, build identity, provider health, and a
  copyable support report.
- JSON and CSV snapshot export.
- Stable icon and Windows PE version metadata.
- Panic/crash logging beside user settings, with no telemetry upload. Alpha.17
  records bounded Rust-panic/native-runner metadata; direct native crashes/hangs
  and forced termination are not covered: `docs/FAILURE_REPORTS.md`.
- Private GitHub repository at `TrentSterling/trontop`; protected `main`, short-lived
  feature branches, tagged alpha builds, and GitHub Actions verification.

Alpha exit gate:

- Formatting, unit tests, strict Clippy, and release build all pass.
- Standard-user and administrator action paths produce deliberate results.
- Service Start/Stop/Restart passes an explicitly authorized disposable-service
  fixture in an isolated Windows VM, including pending/failure/dependent-service
  and denied-access paths. Alpha.11's fake-backend/native-query tests do not satisfy
  this gate; do not test commands against existing working-machine services.
- A 60-minute mixed-load soak shows no UI stalls, runaway handles, or sampler growth.
- Every missing provider renders an unavailable state instead of fabricated data.
- Native export Save As passes isolated cancellation, overwrite, Unicode filename,
  rejected/locked destination and app-close-during-export checks. Alpha.13's injected
  picker and owned-file tests do not validate the native dialog: `docs/EXPORTS.md`.

### 0.4 release candidate

Goal: produce the exact artifact and presentation that can be published publicly.

- Test fresh launch, maximize/restore, tray restore, multi-monitor placement, and
  100%, 125%, 150%, and 200% DPI behavior on Windows 11.
- Test the portable executable on a clean Windows user profile with no Rust toolchain.
- Verify graceful behavior for protected processes, missing PDH categories, disabled
  WMI/SCM access, no discrete GPU, and standard-user permissions.
- Run Microsoft Defender and VirusTotal false-positive checks on the final artifact.
- Sign the executable when the Tront signing certificate and pipeline are available.
- Finish README, screenshots, keyboard map, privacy statement, proprietary license,
  changelog, and known limitations.
- Freeze noncritical feature work during the candidate soak. Fix release blockers only.

Candidate exit gate:

- No known crash, destructive-action ambiguity, unreadable primary text, or fake data.
- No runtime files beyond the documented portable executable and optional exported data.
- Final SHA-256, file size, version resources, and signature status are recorded.
- At least one clean-machine launch and one normal workday soak pass.

### 0.5 public preview

Goal: ship early, learn quickly, and keep the support promise small enough to honor.

- Publish a tagged GitHub release with `trontop.exe`, SHA-256 checksum, release notes,
  limitations, and rollback link to the prior known-good build.
- Publish a concise Trontop page on tront.xyz with screenshots, download link, privacy
  statement, and GitHub issue link.
- Keep networking ETW, advanced GPU device sensors, startup disable/restore, configurable
  columns, and process icons eligible for post-launch updates unless they become release
  blockers during daily use.

### 1.0 stable

Promote after the public preview has survived real usage and the core control paths no
longer need compatibility changes. A 1.0 tag promises stable settings, documented export
formats, predictable updates, and a maintained signed Windows artifact.

## Repository and build policy

- The checked-in Windows GitHub Actions workflow enforces the same gate and uploads the
  resulting portable executable for private review.
- `main` must always pass `cargo fmt --all --check`, `cargo test`,
  `cargo clippy --all-targets -- -D warnings`, and `cargo build --release`.
- Versions use `major.minor.patch`. Every distributed artifact is built from a signed or
  annotated tag named `vX.Y.Z`.
- Release notes list telemetry providers, control changes, compatibility risks, and exact
  verification evidence.
- GitHub release artifacts come from a reproducible release command or CI job, never an
  unidentified local binary.
- Do not publish secrets, machine-specific paths, diagnostic captures, or exported process
  command lines.

## Hotfix and rollback

- Keep the previous public binary and checksum attached to its tag.
- A crash, unsafe action, corrupt settings migration, or materially incorrect counter is a
  hotfix blocker. Disable the affected control/provider if a correct repair is not immediate.
- Patch releases contain the smallest safe fix, rerun the entire release gate, and document
  both the affected versions and the rollback target.

## Immediate path

1. Land the process-tree and guarded process-control slice as 0.3 development work.
2. Connect this local repository to the private GitHub repository and push the clean history.
3. Add About/provider health, export, and PE metadata.
4. Run the alpha verification matrix and use Trontop during normal high-load work.
5. Cut a private `v0.3.0-alpha.1`, address the observed blockers, then prepare 0.4 RC.
