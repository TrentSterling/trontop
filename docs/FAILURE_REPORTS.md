# Local failure reports (alpha.17)

## Alpha.25: non-blocking recoverable graphics diagnostics

Recoverable renderer callbacks no longer perform filesystem work. They first
update the application's recovery flag, capture the same allowlisted event/time/
caller-role metadata, and try one bounded enqueue. One background writer owns the
record buffers and the existing recorder. Capacity is 16 pending records plus one
in flight, each at most 2,048 bytes. A full/disconnected queue drops optional
diagnostics; no retry, worker respawn, growing queue or shutdown join is added.
The worker sleeps on its receiver between events. A healthy run creates no log.

Queued does not mean durably saved. Process exit can discard pending diagnostics,
and the shared writer's non-waiting guard can skip concurrent reports. Rust panic
hooks and terminal native-runner failures retain synchronous best-effort logging;
their disk I/O still has no latency guarantee. Schema, file limits, privacy and
the original stderr panic hook are unchanged.

Three new ordinary tests cover saturation while a writer is blocked, the actual
main renderer callback's recovery-state transitions despite a full queue, caller
role/capture time, fixture-file round trip, disabled/disconnected/healthy paths,
and owned worker completion. One optimized run measured 1,000 rejected enqueue
attempts plus two callback transitions in **781.2 us**, and controller drop in
**7.2 us**; all 17 accepted records drained after releasing the fixture worker.
This is injected-worker evidence, not a whole-app or physical-disk latency claim.
No real failure log or running preview was touched.

```powershell
cargo test --offline --release background_records_bound_queue_and_never_wait_for_a_blocked_writer -- --nocapture --test-threads=1
```

The historical alpha.17/20 implementation and evidence below remain applicable
except for recoverable renderer events' former synchronous dispatch.

Alpha.20 also records closed-category GPU device loss, failed uploads, recovery
attempt/success/failure and the `gpu_recovery` worker role. Schema/retention/privacy
limits are unchanged. Healthy execution still creates no log. See
`RENDERER_RECOVERY.md` for the renderer patch and real alpha.19 incident.

Trontop records Rust panics and errors returned by its native application runner
at `%LOCALAPPDATA%/Trontop/failures-v1.jsonl`. No upload, account, service, driver,
dump collector or new runtime dependency is involved. The portable app remains
one EXE; the optional log lives beside user settings, not beside the executable.
Healthy execution does not create the log. About shows the location, an explicit
copy-location action, and the limits. The normal support report does not read or
include the failure log.

## Data and retention

The first JSON line identifies `trontop-failure-log-v1`. Each subsequent JSON line
is one `trontop-failure-v1` record with only these fields:

- Build hash/modification marker, package version, target and build profile.
- Closed failure category: Rust panic, app creation, window system, event loop,
  graphics initialization/runtime error returned by eframe, or one of the five
  typed GPU recovery events listed above.
- UTC Unix milliseconds (null if unavailable), allowlisted application thread role,
  compiled source basename and line/column when a panic provides them.

No panic payload, raw native error, backtrace, memory dump, process list, command,
document path, source-directory path, account/host name, GPU ID or network address
is accepted by the record encoder. Unknown thread names become `other`; source
basenames are length-limited and sanitized. The existing panic hook still runs
after recording, so developer stderr behavior, including its original payload,
is unchanged. That output is not copied into the file or support report.

Retention is at most **32 records**, **2,048 bytes per record**, and **65,572 bytes
per file** including the format line. An OS file lock uses a single non-waiting
attempt across cooperating instances. Contention skips a report rather than
waiting; a process-local guard prevents recursive/concurrent entry into its writer.
Locks release with the file handle, including after process termination.

The writer refuses relative/UNC destinations, final-component Windows reparse
points, non-file targets, unknown headers, corrupt complete records, oversized
logs, read-only files and ordinary access errors. It does not change permissions
or try elevation. An incomplete trailing line in an otherwise recognized log is
discarded on the next write, retaining its complete predecessors. At capacity the
oldest records are replaced. Only the reserved failure-log file is written; there
is no wildcard cleanup, directory deletion or growing collection of crash files.
Path checks are not a security boundary against changes to ancestor directories.

## Limits

This is best-effort incident metadata, not a native exception handler or a
transactional/power-loss-safe journal. Failed or interrupted writes may leave a
partial log. No record is guaranteed if storage stalls or is unavailable; ordinary
file I/O has no hard latency deadline. From alpha.25 recoverable renderer events
dispatch it to a background worker; panic/terminal-error recording stays synchronous.
A malformed/oversized log is preserved unchanged; rename it manually if
you want a fresh one. There is no automatic read, upload or crash-recovery dialog.

Direct forced termination, hangs, OS/driver/native access violations, allocation
failure and some stack failures may not invoke the Rust panic hook. Unwinding
builds can also record a panic subsequently caught by a caller, so a panic record
alone does not prove the process terminated. No global exception, keyboard or
desktop hook is installed. Shutdown/drag/soak performance remains unmeasured.

Rust documents that a panic hook runs before both unwinding and aborting runtimes:
[panic hook documentation](https://doc.rust-lang.org/std/panic/fn.set_hook.html).
Direct `process::abort` does not invoke it:
[abort documentation](https://doc.rust-lang.org/std/process/fn.abort.html).
The lock is the stable, non-waiting standard-library API, available before this
project's Rust 1.93 minimum:
[File::try_lock](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock).

## Verification

```powershell
cargo test --offline failure -- --nocapture
cargo test --offline
cargo clippy --offline --all-targets -- -D warnings
cargo build --offline --release --target-dir target/review-build
cargo test --offline render_offscreen_visual_pass -- --ignored --nocapture
```

Nine new focused checks plus their child-test entry point cover fixed allowlisted
fields, complete JSONL parsing, latest-32 retention, unrelated-file preservation,
unknown/oversized/corrupt/read-only logs, invalid destinations, partial-tail recovery,
lock contention/recovery, concurrent recorders, disabled/busy/healthy paths,
unchanged native-runner error results, and local-only About copy/layout behavior.

The real hook test launches only its own hidden test EXE with an exact test filter
and an exclusively created fixture directory under `target/failure-tests`. It
checks a real panic with the ordinary unwinding test runtime and another real
panic whose previous hook calls `process::abort`. Both produce parseable metadata
without the synthetic private panic payload. This tests hook-before-abort behavior;
it is not an induced crash of the running GUI or a complete release-crash matrix.
The child has no native app, tray, sampler or global input; its Windows crash UI
is disabled only within that disposable child. Ten-second cleanup targets its
owned child handle only. No real working-machine failure log is created by tests.

Final local gate: **153 passed, 0 failed, 9 opt-in ignored** (25.43 seconds), strict
Clippy and formatting PASS, optimized review build PASS (32.47 seconds). The
offscreen UI pass produced **57 PNGs** (34.70 seconds); About dark, light and compact
were visually inspected. The existing-app redesign skill's scoped layout audit
caught the new controls below the compact opening viewport; a screen-relative
default height and geometry/copy tests corrected it while retaining the existing
Tront surfaces, hover behavior and palette.

Final review EXE: `target/review-build/release/trontop.exe`, 13,270,016 bytes,
version 0.3.0-alpha.17, built from modified df7e3fb source.
SHA-256 `0E9F89C8B983A0CDD2C24C8D4197AB2A0FDD85B21EE5F92A2A87BE0AB46CD9A6`.
Windows-only import inspection passed for this exact EXE; clean-machine portability
remains unverified. No alpha.17 app was launched; the explicitly opened alpha.16 is
untouched.
