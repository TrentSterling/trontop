# Snapshot export (alpha.13)

Use **Export** in the command bar, select JSON or CSV, then **Save as...**.
Opening the panel does not collect new telemetry, write a file or copy anything.
Save captures the current sampler snapshot before the Windows file picker opens.
Sampling continues, but that export does not change while you choose its destination.
Only one export can be in progress. No automatic upload or opening of saved files.

## Scope and privacy

JSON exports the system summary, seven provider-health records, processes, disks,
network interfaces, GPU engines, GPU and drive sensors, user totals, Startup sources
and entries, and Services inventory. CSV exports processes only, one row per PID.
Both export all sampler rows, not the search-filtered view. No subtree totals,
chart history, process artwork, or service-command annotations are included.
Services records explicitly identify themselves as the last complete inventory;
use provider freshness rather than treating them as current command results.

**Include private details** defaults off and resets when an idle export panel is
reopened. It is not saved in settings. With it off, accounts, host name, executable
and working paths, commands, mounts, adapter names, GPU UUIDs, drive interface IDs
and Startup keys are excluded. Raw native error messages are never exported.
Names of processes, services, devices and Startup entries remain: this is NOT an
anonymous support report. These names may themselves be user-defined. For a smaller
allowlisted support report, use the existing About panel instead.

Enabling private details can expose credentials in command lines and identifying
paths/accounts/hardware. Review the file before sharing. Files inherit destination
directory permissions; they are not encrypted. Windows may remember the chosen
folder. Choosing a network share or cloud-synced folder puts the file there; Trontop
does not add a separate network upload. No application export history is retained.

## Format contract: schema 1

JSON metadata records schema version, app version/build, Unix capture milliseconds,
snapshot sequence, whether a system sample exists, private-details mode and scope.
The system object is null before the first sample; the UI disables Save until then.
Provider state, attempt age, last-usable age, query duration and available coverage
travel with the data. Ages are seconds at capture time, not at save completion.
Inventories and sensor providers sample independently; a capture is a coherent copy
of the published model, not a simultaneous hardware measurement.

Field suffixes identify bytes, seconds, milliseconds, Celsius, watts, MHz and
percent. JSON integers preserve the stored u64 value, but consumers using floating
point for all JSON numbers may lose precision. Optional/excluded and nonfinite
numeric values become null. Windows paths are converted to Unicode text with
replacement characters if their UTF-16 cannot be represented. No sensor data is
invented. CPU temperature remains null with `temperature_provider: not_connected`.
Fan percentage is the driver-reported target, not measured RPM. Existing sampler
fields without per-field availability metadata are exported as stored; export does
not establish that every zero-valued system/process counter was independently read.

GPU usage carries `state` plus `percent` and `lower_bound_percent`: measured zero
stays zero; partial coverage has only a lower bound; warming, unreported and
unavailable retain their distinct states with null numbers. Nonfinite GPU values
are `invalid`. Cached sensor values keep their cache state and last-usable age.

CSV is UTF-8 with a BOM and CRLF record endings. Every field is quoted; embedded
quotes double and embedded line breaks remain inside the quoted field. Header order:

```text
schema_version,captured_unix_ms,snapshot_sequence,system_state,system_age_seconds,
gpu_provider_state,gpu_age_seconds,pid,parent_pid,name,status,cpu_percent,gpu_state,
gpu_percent,gpu_lower_bound_percent,memory_bytes,virtual_memory_bytes,
read_bytes_per_sec,write_bytes_per_sec,total_read_bytes,total_write_bytes,
accumulated_cpu_millis,started_at_unix,priority
```

Private CSV adds `user,executable,command,cwd`. Missing values are empty, not zero.
An empty process set produces headers only. Formula-like text is apostrophe-prefixed
after checking leading whitespace/control characters and common fullwidth operator
variants; leading tab/newline text is prefixed too. This intentionally changes those
CSV strings. Use JSON for exact source text. Spreadsheet applications have differing
formula interpretation rules; this is defense in depth, not a universal guarantee.
See [OWASP's CSV injection guidance](https://owasp.org/www-community/attacks/CSV_Injection).

## Worker and file ownership

The UI clones the model once on Save. Encoding, Save As and file I/O run on one
dedicated export thread, not on the sampler or render thread. This avoids a blocked
picker or disk write holding the UI, but the model clone has a size-dependent cost.
There is no unbounded queue or worker respawn while a job is blocked.

Windows Save As uses an STA-owned
[Common Item Dialog](https://learn.microsoft.com/en-us/windows/win32/shell/common-file-dialog).
It preserves default options, requests filesystem destinations, checks the parent
directory, prompts before overwrite, and does not add the file to Recent Documents.
The selected format must match the filename extension. The dialog is ownerless on
its own worker; placement, cancellation and close behavior still need native review.

The writer exclusively creates a sibling `.trontop-export-<pid>-<counter>.tmp`,
streams at most 64 MiB, flushes/syncs/closes it, checks cancellation, then uses
[Rust's filesystem rename](https://doc.rust-lang.org/std/fs/fn.rename.html) to replace
the chosen destination. It never truncates that destination before encoding. A
failed write/limit/rename returns Failed and attempts to remove only its own temporary
file. No automatic retry or directory creation occurs. This does not promise power-
loss durability, behavior on every remote filesystem, or protection against other
writers modifying the destination after the user's overwrite confirmation.

Closing the export panel does not cancel the worker. Cancel the Windows picker to
abandon it. App shutdown signals cancellation but does not join a blocked picker or
file operation. Cancellation checks cannot interrupt an OS rename already underway.
An abrupt exit or failed cleanup may leave a partial sibling temporary file, possibly
containing private data. No later launch silently deletes such files. They are not
securely erased. Only an explicit completed result is labeled Saved.

## System specs text and JSON (alpha.36)

The System page's **Save text** and **Save JSON** use the same one-job export
worker, native Save As picker, temporary-file replacement and 64 MiB limit. The
page clones its published specs snapshot and the current sampler snapshot when
the button is pressed; live values (clocks, temperatures, usage) resolve at that
instant and carry their provider label. Text is UTF-8 with a BOM and CRLF, like
Speccy's "Save as text", with each section's state and read age. JSON is
`kind: trontop_system_specs`, schema 1, with every row's value or its
`unavailable` reason, notes, live keys and section states.

"Private values in saved files" defaults off and resets after every save.
With it off, private rows (serials, UUIDs, MAC/IP addresses, product IDs, user
and computer names, Bluetooth names) are written as excluded. Drive interface
paths never appear in either format, even with private values on. Names of
devices and installed software remain; this is not an anonymous report.

## Verification and remaining gate

Ten new tests exercise serialization, metadata/privacy across sensor/inventory
sections, missing/zero/partial GPU values, nonfinite numbers, maximum u64, Unicode,
an independent quoted-CSV parser, formula-prefix cases, one-job ownership and bounded
drop. Owned temporary-directory tests verify cancellation, size-limit failure and a
Windows sharing-locked destination preserve fixture bytes, then verify replacement
and cleanup on success. They never write a user's export destination.

Headless UI tests use an injected backend: explicit Save captures options and all
rows despite a nonmatching search, idle reopening resets private mode, unavailable
and first-sample states cannot dispatch, and Save/Close/status remain unclipped with
stable geometry at 1040x640 in light and dark themes. The offscreen pass adds
`export`, `export-private-light` and `export-failed-compact` fixture PNGs.

The real Windows Save As path is compiled, not end-to-end validated. Before release,
use an explicitly authorized isolated instance to verify picker cancellation,
overwrite confirmation, Unicode names, rejected/missing/locked destinations, return
to the app, and closing the app with the picker or file write outstanding. Do not
inject global input or manipulate Trent's working desktop to satisfy this gate.
