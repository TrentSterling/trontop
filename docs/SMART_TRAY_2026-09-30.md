# Smart tray and hardware report checks (alpha.50)

Trent asked for a smart system tray and close to tray (A39). Window X and OS
close now hide while telemetry and the live CPU notification icon continue.
Show restores the retained view. Explicit Quit saves settings and exits.
Quit is available in the tray, About and the titlebar X context menu.

Final verification: **577 ordinary tests passed, 0 failed, 58 ignored**;
formatting, strict all-target Clippy and optimized release build PASS.
The final instrumented suite also passes all 577 ordinary checks, the exact
24 reviewed read-only probes and ten analyzer checks. Production source-line
coverage is **93.33% (24,267/26,000)**, 1,733 missed, zero mixed. All 193
application/build inputs and prior review EXE hashes were independently checked.
Seventeen hardware failure logs and the development layout failure are retained.

Portable EXE: `target/review/alpha50-smart-tray/trontop.exe`, 17,838,592 bytes,
File/Product version `0.3.0-alpha.50`, SHA-256
`1179784C3B92DFD822BF1F9DB26BC8BB9829DA9613843285F88330329F3CAF1F`.
Release/review hashes match. The verifier and receipts are beside the candidate.
No launch, install, publication or native desktop automation occurred.
Two offscreen 1040x640 fixture-only About images were inspected in
`target/ui-smoke/tray-alpha50/`; Quit, Copy controls and license headings fit
in dark/light. A read-only storage probe retains the real NVMe health log
(4% used, no critical warning, 45 C at read time); RAM reports 64 GiB DDR5 at
6400 MT/s, 2 of 4 slots. These are one local observation, not field parity.
Cleanup audit: zero test processes and zero owned Startup fixture leaves.

If the tray is starting, missing or failing, close keeps the window reachable.
A failure while hidden restores access without requesting focus. A later
successful update enables hiding again. Quit from the tray exposes slow/error
save controls; hiding never authorizes process exit. A pending Quit takes
priority over a subsequent Show click. Hidden sampling retains the freshest
snapshot without normal sample repaint wakes, and hidden UI frames skip page
drawing. See [tray lifecycle](TRAY_LIFECYCLE.md) and
[settings persistence](SETTINGS_PERSISTENCE.md).

Ten new headless app checks exercise the production window commands with a
fake backend on the existing owner-thread tray worker. They cover X/OS close,
repeated hide/Show, retained state, failed startup/icon/worker, recovery, Quit
priority, settings waits/errors and both Quit controls. Compact dark/light
checks include 1x/1.5x/2x. A mailbox check publishes 10,000 hidden samples and
verifies no sample repaint callback and one latest value. No command reaches
a native window. The full harness caught an About overflow at 1000x580 during
development; Quit moved beside Copy support report in its existing top row.
The original layout assertion remains in place.

This candidate also completes the RAM/storage fixture work already in progress:

- Twelve RAM checks preserve unknown-size module facts, avoid treating unknown
  as empty, aggregate system arrays/slots/ECC, reject contradictory slot counts,
  retain partial speed readings and distinguish complete installed totals from
  subtotals. Extended size/speed reserved bits are rejected. Non-system arrays
  do not inflate RAM. Complete SMBIOS fallbacks name their source.
- Ten storage checks validate descriptor headers, terminated strings and NVMe
  reply headers/types/offsets/full lengths. Missing removable-media metadata
  stays unavailable. Wide NVMe counters retain their exact original quantity;
  unknown warning bits stay visible. Enumeration failures retain collection
  issues, and media/partition/health/optical facts preserve privacy.

Seventeen hardware failures were reproduced before fixes in
`target/review/alpha50-memory-storage/`. Two initially incorrect 32 GiB fixtures
were corrected and are not counted as code bugs. Native Windows Storage
Management joins were not rewritten. No field-parity or universal health-data
claim follows from these fixtures.

Final suite, source/binary hashes, coverage and portable candidate are recorded
in [CURRENT_STATE](CURRENT_STATE.md) and [COVERAGE](COVERAGE.md). Trent explicitly
directed: "dont hijack my mouse to test systray tho just fix it and call it done".
A39 is done for the requested implementation/headless scope. No mouse or desktop
automation was performed. Native tray behavior, hidden CPU, teardown and broader
release acceptance have no new verification claim. The alpha.49 desktop instance
was left untouched.

## Primary format references

- [DMTF SMBIOS 3.8.0, Type 16 and 17](https://www.dmtf.org/sites/default/files/standards/documents/DSP0134_3.8.0.pdf)
- [Windows storage descriptor](https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ns-winioctl-storage_device_descriptor)
- [Storage bus enumeration](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntddstor/ne-ntddstor-storage_bus_type)
- [Protocol reply descriptor](https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ns-winioctl-storage_protocol_data_descriptor)
- [Protocol-specific data and relative offset](https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ns-winioctl-storage_protocol_specific_data)
- [Windows NVMe health counters](https://learn.microsoft.com/en-us/windows/win32/api/nvme/ns-nvme-nvme_health_info_log)
- [NVMe Base Specification 2.1](https://nvmexpress.org/wp-content/uploads/NVM-Express-Base-Specification-Revision-2.1-2024.08.05-Ratified.pdf)
