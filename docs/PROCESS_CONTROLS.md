# Process tree and suspension controls, alpha.43

Trent requested End process tree because ending a selected Chrome or Firefox child
can leave the rest of the browser running. The inspector now pins End process tree
below End task. Both require confirmation; End task still targets one process.

The tree dialog starts with the selected process and its descendants. Check
**End all instances of this executable** to include independent processes using
the same executable path and their descendants, including other browser windows
and profiles. Processes with the same filename in a different directory are
excluded. Missing paths or creation times explain why a complete list cannot be
confirmed. The list shows every captured target with its PID and name, with a
scrollbar for large lists. Cancel, Refresh targets and confirmation remain visible
at 1000x580 in both themes and scopes.

The dialog freezes the original process identity and the reviewed list. Changing
the table selection cannot redirect it. Changed membership, parent relationships,
names or identities disable confirmation until Refresh targets rebuilds a list
for that original process. A reused root PID cannot become a new target. CPU and
memory counter changes do not invalidate review. Cancel and the dialog close
button never dispatch an action.

The worker opens and verifies all surviving targets before sending the first
termination request. Each retained handle must match the exact nonzero Windows
creation FILETIME and pass the critical-process check. Child parent PIDs are
queried on those handles and compared with the captured older parent identity.
All-instances roots also recheck the executable path on the action handle.
Kernel PIDs, Trontop itself, duplicate PIDs, unknown identities and unsafe lists
are refused. No elevation or privilege adjustment occurs. Missing/exited targets
are skipped; a reused identity or inaccessible live target aborts before mutation.

Parents precede children in the reviewed traversal. The action never searches for
new processes after confirmation. Processes created after review, a browser
relaunched by another application and unreviewed independent executables need a
new action. Windows termination is asynchronous: a success message acknowledges
accepted requests, rather than promising all processes have already exited.
Failures after preflight report a partial result with accepted and failed counts.

The inspector also offers confirmed Suspend and Resume for A04. Trontop owns a
Windows 11 process-state handle per suspended process. Duplicate suspensions are
refused, a failed Resume retains its handle for another explicit attempt, and
closing Trontop or losing its worker releases its suspension references. These
are owned suspension states, rather than labels inferred from idle CPU readings.
Independent suspension references held by other tools can remain. When Trontop
holds no reference, confirmed Resume sends the classic Windows resume request.
The native exports are resolved dynamically, so a missing provider gives an
explicit failure instead of preventing application startup.

All OS work runs in the existing bounded action worker. The render thread builds
plans only from already sampled data. One in-flight action stays pending until a
terminal result; a slow call is never retried automatically. Graphics recovery,
stale confirmations and unavailable workers retain the existing action guards.

Verification uses headless local egui input, offscreen renders and owned hidden
test children. No native window, global input, tray operation or mutation of
Trent's Chrome/Firefox/session processes is part of these checks. Native tests
verify real process exits and stopped/resumed child heartbeats. Broader release
and native desktop acceptance remain separate, as does A04 Startup enable/disable.

Native interface references: Microsoft's
[NtQueryInformationProcess](https://learn.microsoft.com/en-us/windows/win32/api/winternl/nf-winternl-ntqueryinformationprocess)
documents the parent-information layout and dynamic-linking requirement;
[QueryFullProcessImageNameW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-queryfullprocessimagenamew)
documents handle-based executable lookup. The primary
[NtCreateProcessStateChange description](https://github.com/m417z/ntdoc/blob/main/descriptions/ntcreateprocessstatechange.md)
records the required process access and automatic undo on final state-handle
release. The [phnt declarations](https://github.com/winsiderss/phnt/blob/master/ntpsapi.h)
provide the native ABI reference (MIT); no third-party control implementation was
ported.

Verification: the ordinary suite passes **455 tests, 0 failures, 52 ignored**.
Strict all-target Clippy and format checks pass. The five process-control UI
regressions pass after the final geometry change. Three final offscreen PNGs
(compact dark tree, compact light all-instances, compact dark inspector) were
inspected with full button rows visible. The full suite is also rerun by the fresh
instrumented coverage measurement; its exact source/object receipt is recorded
in `COVERAGE.md` and `CURRENT_STATE.md`.
