# Reversible Startup controls (local alpha.44)

Startup shows the captured Windows approval state and offers Enable, Disable,
Undo and background Refresh. Each mutation requires confirmation of the original
source, value/file name and command. It affects subsequent sign-in; an already
running program stays open. Commands and shortcut contents are preserved.

| Inventory source | Registration | Approval namespace |
| --- | --- | --- |
| Current user Run | HKCU Run | HKCU StartupApproved\Run |
| Machine Run | HKLM Run, 64-bit view | HKLM StartupApproved\Run |
| 32-bit machine Run | HKLM Run, 32-bit view | HKLM StartupApproved\Run32 |
| Current user Startup folder | Windows known Startup folder | HKCU StartupApproved\StartupFolder |
| Machine Startup folder | Windows known Common Startup folder | HKLM StartupApproved\StartupFolder |

Run paths are `Software\Microsoft\Windows\CurrentVersion\Run`. Approval paths
are below `Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved`.
Registry views are explicit; folder paths come from Windows known-folder APIs.
Machine confirmations disclose the possible effect on other users. The worker
uses existing permissions and reports Windows errors without automatic elevation.

StartupApproved is an undocumented compatibility format. Trontop recognizes
12-byte REG_BINARY records with DWORD 2/6 (enabled) or 3/7 (disabled), preserving
the record family when toggling. Missing overrides display Windows' enabled
default. Unknown types, sizes, states and unreadable records remain visible and
refuse changes. Other sign-in policies can affect actual launch behavior.

The worker validates the captured registration and exact approval bytes, then
uses one registry transaction for the update. Run values are staged byte for byte
unchanged to acquire a write conflict guard. A separate committed-view read
detects an edit that won before the guard. Approval has the same committed-view
check. Retained file handles verify volume/file identity, creation/modification
times and length, and block write/delete access through commit. Precommit failures
roll back; commit/readback uncertainty requires a new inventory read before retry.
There is no automatic retry.

Undo restores the exact prior approval bytes or removes the override if it was
originally absent. Its guard requires the same registration and last verified
approval. The session retains at most 64 records within a 2 MiB budget; eviction
retains a freshness fence so an older inventory cannot authorize another change.
It may leave an empty approval key after removing an originally absent value.

Command readback outranks inventory reads started before the command completed.
A newer matching inventory preserves Undo; a newer external change invalidates
it. Unknown outcomes display Unavailable / Refresh required. Command reads expire
after 75 seconds, with one scheduled freshness repaint. Registry/file calls and
refreshes stay on bounded workers; normal table painting borrows captured records.
JSON exports include approval state from the inventory, while raw registration
and approval metadata remain private implementation data.

## Verification scope

The native fixtures exclusively create
`HKCU\Software\Trontop\Tests\Startup\<PID>-<nonce>` and owned files below
`target/test-startup-fixtures`. They exercise real transactional enable/disable,
exact Undo, rollback, stale command/approval refusal, edits before/after guard
acquisition, file replacement/missing/path refusal, and retained file sharing.
Fixed owned children are cleaned up without recursive registry deletion. The
sandbox initially denied fixture creation; the isolated native runs required a
scoped sandbox exception, using the existing Windows account.

Pure and headless checks cover metadata validation, worker contention and shutdown,
unknown outcomes, expiry, retention limits, source-specific selection, frozen
confirmation identity, compact dark/light dialog geometry, cancellation and
graphics recovery. No real Startup registration is changed by these checks.
Actual Task Manager interpretation and next-sign-in launch behavior remain an
isolated native acceptance gate under A04; this compatibility format is not an
official Microsoft contract. Exact build/test/coverage receipts are recorded in
`CURRENT_STATE.md` and `COVERAGE.md`.

## References

- [Microsoft: alternate registry views](https://learn.microsoft.com/en-us/windows/win32/winprog64/accessing-an-alternate-registry-view)
- [Microsoft: registry transactions](https://learn.microsoft.com/en-us/windows/win32/api/winreg/nf-winreg-regcreatekeytransactedw)
- [Microsoft: file access and sharing](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew)
- [Author's StartupApproved observations](https://github.com/crystalidea/uninstall-tool/issues/180)
- [Author's approval namespace mapping](https://gist.github.com/mkyutani/8e93866b355473ded3dd9c885005dc7d)

The implementation is original. No external Startup implementation was copied.
