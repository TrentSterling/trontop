# Confirmed service controls (alpha.11)

Services now offers Start, Stop and Restart on a selected Windows application
service. These are real SCM operations, not process termination or startup-setting
edits. The UI uses Tront's existing rounded hover surfaces, row/column bands and
original vector actions. Controls, status surfaces and table headers retain their
positions through selection, progress, failure and missing inventory.

## Command path

1. Select a row and action. The confirmation retains the service name, displayed
   name, reported state and host PID; changing the selection cannot retarget it.
2. Confirm within 30 seconds. Expired, unavailable or changed observations disable
   confirmation. Cancel and closing the dialog send nothing.
3. One background worker opens the local SCM with CONNECT, then one service handle
   with QUERY_STATUS and only the START/STOP rights needed for this action.
4. Requery on that handle, compare state/PID and recheck accepted controls before
   mutation. Drivers, system-process-hosted services, invalid names and unsupported
   current states are refused. This is not a universal critical-service detector.
5. Send the command and poll for the target state. Restart observes Stopped before
   sending Start, using the same handle throughout. A competing Start is not repeated.
6. Report completion only after observing the target state, then request inventory
   refresh. Windows errors retain their numeric codes and useful explanations.

Start requires Stopped. Stop/Restart require Running or Paused, a valid host PID,
and the reported Stop capability. A fresh native query always precedes mutation;
the inventory's freshness alone is not authority. Service name/state/host PID are
not a process-creation identity or protection against every possible external race.

`StartServiceW` returning successfully does not mean initialization has completed,
so Trontop separately observes status. Windows may start required dependencies;
Trontop supplies no extra service arguments and never changes disabled startup
configuration. [Microsoft StartServiceW](https://learn.microsoft.com/en-us/windows/win32/api/winsvc/nf-winsvc-startservicew)

Stop sends only the selected service's Stop control. Running dependents can cause
Windows to refuse it; Trontop never recursively stops dependents or kills the host
process. [Microsoft ControlService](https://learn.microsoft.com/en-us/windows/win32/api/winsvc/nf-winsvc-controlservice)

Status queries use QUERY_STATUS only. PID validity depends on the reported state;
Stopped and start/stop-pending rows do not expose their native PID as an action
identity. [Microsoft QueryServiceStatusEx](https://learn.microsoft.com/en-us/windows/win32/api/winsvc/nf-winsvc-queryservicestatusex),
[Microsoft SERVICE_STATUS_PROCESS](https://learn.microsoft.com/en-us/windows/win32/api/winsvc/ns-winsvc-service_status_process)

## Responsiveness, uncertainty and permissions

- One command can be active, with one coalesced progress slot. Commands never run
  on the render thread or live telemetry sampler, and repeated clicks cannot queue
  a backlog. Worker failure leaves inventory readable and controls disabled.
- Observation polls every 250 ms with a 30-second deadline per target state.
  This is not an upper bound on a blocking native call. The worker can wait on
  Windows while the UI continues; actual UI responsiveness under a stuck native
  command has not been measured on the working desktop.
- Closing signals cancellation and does not wait for this worker. Checks before
  the remaining stages prevent a later restart stage where possible. An already
  accepted or in-flight command can still complete; closing cannot undo it.
- Restart is not atomic. If Start fails, times out or is cancelled after Stop,
  the service can remain stopped. Both confirmation and failure text say so.
- No automatic elevation, service installation/deletion, driver installation,
  permission changes, force-kill fallback or configuration writes are performed.

The latest command's observed state can override older inventory for up to 75
seconds. A failed inventory attempt cannot replace this with an older cached row;
only a newer complete read can. If a command may have run after the last observation,
the prior row is retained as Pre-command and retry is disabled until a newer read.
The overall inventory banner describes the list's last complete read, not proof of
every row's current state. The override/uncertainty record currently covers only
the latest command, not a persistent per-service command journal; submitting a
different command replaces it. Native state/PID preflight still applies to every
subsequent request. Per-service retained outcomes are follow-up reliability work.

Refresh list schedules collection on the next sampler cycle. Existing startup and
service inventory calls still share that sampler, so isolating slow inventory is
separate work. The new command worker does not solve all provider stalls.

## Verification and remaining release gate

Local alpha.11 gate: 101 tests passed, six opt-in tests ignored, formatting and
strict Clippy passed, optimized EXE built. The offscreen pass generated 47 PNGs;
five service-control variants were visually inspected. Exact artifact and runtime
observations are in `CURRENT_STATE.md`.

Ten injected-backend tests cover command order, one-handle restart, changed
state/PID/capabilities, invalid/expired requests, failures, competing controllers,
single-flight behavior, cancellation boundaries, stopped-during-start exit codes
and bounded drop with a blocked backend. Four headless UI tests cover actual local
egui selection/confirmation/cancel, expired and changed requests, stable layouts,
uncertain outcomes and command/inventory timestamp ordering. Fixtures never send
commands to actual services or use global input.

Two native mapping/error tests and one native read-only status test passed. The
read-only test opens QUERY_STATUS handles for bounded inventory candidates; it
does not call the native command opener, StartServiceW or ControlService.

**Real native Start/Stop/Restart end-to-end behavior is not yet verified.** Before
release, use an explicitly authorized disposable test service in an isolated Windows
VM. Verify start/stop/restart, pending/failure/dependent-service behavior, normal-user
access denial, explicit administrator operation, handle cleanup and close during a
pending command. No existing service on Trent's working machine may be used for
that test. Installing the fixture and changing privileges require fresh authority.
