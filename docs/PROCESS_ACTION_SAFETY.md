# Process-action identity checks

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
