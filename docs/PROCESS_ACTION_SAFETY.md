# Process-action identity checks

## Alpha.22: responsive action dispatch

End task, priority, affinity, Run task and Reveal in Explorer now use one dedicated
action worker. These native calls are no longer made by `TrontopApp::ui`. The
existing platform identity/critical-process checks are unchanged and still span
the same native handle used for each process mutation. No privileges are enabled.

- Requests freeze the confirmed action, exact identity and display target. A
  subsequent selection change cannot redirect the command or its result message.
- There is at most one in-flight process/shell action, no queued duplicate and no
  automatic retry. The UI uses non-waiting channel operations. A slow native call
  blocks further process/shell commands, not navigation, sampling or window chrome.
- The fixed-height status bar changes from Working to Still waiting after five
  seconds, without clearing the request or pretending it failed. Long text is
  truncated with full hover detail; Dismiss applies only to completed messages.
  Submission requests an immediate repaint. Pending uses a neutral theme tint,
  not success green; horizontal insets keep text off the window edges.
- Requests not begun within 30 seconds are refused before entering the native
  backend. Once a native call is in flight, that deadline does not cancel it.
- Shutdown signals the worker and detaches without a join. A native call already
  entered can still complete before application-process exit. No force-killed
  thread, freed in-flight resources or rollback is implied.
- A disconnected worker reports an unknown outcome once and is not restarted.
  A normal error permits a later explicit request. Release panic=abort still
  applies; the injected unwinding-worker test is not release panic recovery.
- Default/headless app construction cannot execute native actions. Test backends
  are explicitly injected; only the real app constructor enables native dispatch.
  Graphics-recovery gating also rejects direct submission while reconnecting.

Success now says **Request accepted**, not that the target has exited or the
launched program initialized. Windows documents external-process termination as
asynchronous: [TerminateProcess](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-terminateprocess).
Run task retains its existing command-shell behavior; dispatcher success only
acknowledges shell creation, not the command's eventual result. This slice does
not install hooks, elevate, change shell quoting semantics, or add suspend/resume.

Verification: six worker tests cover exact off-thread dispatch for all five action
kinds, invalid/expired requests, stalls, duplicate suppression, errors, disconnected
workers and one owned hidden native child. The native child rejects all three
process mutations with a creation time off by 100 ns, retains priority/affinity,
then exits only after the exact-identity End request. Four production-UI tests
exercise confirmation, all-page navigation while stalled, selection changes,
stale confirmations, busy/Enter/recovery guards and stable message geometry.
No test runs a user command, opens Explorer or manipulates desktop windows.
The latest build-wide gate and visual evidence are in `CURRENT_STATE.md`.

## Original identity fix (alpha.4)

Alpha.4 fixes an action-targeting gap found during the private-alpha release audit.
Earlier End Task stored only a PID. Priority/affinity compared a rounded start time
against the cached snapshot, then reopened the PID without native verification.
Neither check proved that the native action still targeted the confirmed lifetime.

## Current behavior

- The background sampler records `GetProcessTimes` creation FILETIME at its original
  100-nanosecond precision. New rows get identity queries immediately; scheduler
  information continues its five-sample refresh. A native creation time contradicting
  the enumerated display timestamp is discarded instead of attached to that row.
- A request captures PID plus exact creation time. End Task also freezes the displayed
  name. Missing identity refuses the request instead of falling back to PID-only action.
- On confirmation, the platform layer opens the target with action rights plus limited
  query rights. It checks creation time, then invokes the requested operation on that
  SAME handle. It never closes/reopens by PID between validation and the operation.
- Kernel/self PID exclusions remain. `IsProcessCritical` blocks actions on processes
  Windows marks critical. Failure to query protection status also refuses the action.
  No elevation, privilege enabling, or fallback to an unchecked operation occurs.
- RAII closes process handles on success and on every validation/error return.
- Selection clears when the sampled lifetime changes or disappears. A pending End
  Task retains its original target/name and disables confirmation when that identity
  no longer appears. Scheduling confirmations also reject a changed snapshot identity.

Snapshots are not atomic with the operating system. A process can still exit after
validation, causing a normal native action failure. The held handle refers to that
same process object, never a replacement process with a reused PID.

## Verification

Default tests create their own hidden disposable `ping.exe` children, never use a
desktop application as a mutation target, and kill/wait their children through RAII.

- Exact creation time is cross-checked against the real sysinfo display timestamp.
- A deliberately incorrect creation time differing by only 100 ns must fail End Task,
  priority, and affinity. The child must remain alive with unchanged priority/mask.
- Missing identity must also refuse termination. The exact valid identity may then
  terminate that disposable child, proving the positive path still works.
- Existing valid priority/affinity tests still apply and restore their child's settings.
- Critical-process policy tests use supplied booleans. They do NOT mark a child critical
  or test destructive operations against any real Windows-critical process.
- A headless confirmation test simulates PID reuse in fixture data, checks the original
  target/name and stale warning, then clicks only Cancel through local egui input.
- Offscreen images include valid and stale End Task confirmation dialogs.

This is not proof of administrator action paths, service control safety, a workday
soak, or every process lifecycle race. Those broader release gates remain open.

## Primary references

- [Process handles and identifiers](https://learn.microsoft.com/en-us/windows/win32/procthread/process-handles-and-identifiers)
- [GetProcessTimes and FILETIME precision](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getprocesstimes)
- [IsProcessCritical access requirements](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-isprocesscritical)
