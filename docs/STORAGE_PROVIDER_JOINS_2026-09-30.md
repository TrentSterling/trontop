# Storage provider joins: alpha.51 checkpoint

Windows Storage Management metadata could attach to the wrong physical disk by
assuming `MSFT_PhysicalDisk.DeviceId` was the OS disk number. Checked disk numbers
and unambiguous UniqueId/UniqueIdFormat joins now preserve identity; physical
capacity disagreement and missing/ambiguous identifiers leave physical facts
unavailable. OS capacity and partition style remain independent. Identifier
values are not included in issue messages.

Unknown partition number and size now remain optional through the report instead
of appearing as Partition 0 / 0 B. Drive letters reject truncation and normalize
real lowercase letters. Access paths and identifiers require nonempty text;
volume paths compare case-insensitively. Volume query failures and ambiguous
matches are reported without dropping known partition facts. Free space above
volume size is unavailable, while known zero free space remains valid.

Production logic lives in `src/specs/storage/management.rs`, invoked by the
native WMI collector and by twelve ordinary provider/report fixtures. Ten valid
before-fix failure logs are preserved beside the review candidate. Two baseline
fixtures already passed and are not counted as reproduced bugs. Removal of two
obsolete WMI array accessors resolves a strict Clippy failure; existing row shape
checks still verify the original array values. The interrupted first coverage
attempt at `20260930-102850-340` has no passing receipt.

Final checks: **589 passed, 0 failed, 58 ignored**; formatting, strict all-target
Clippy and release PASS. The instrumented suite, 24 exact read-only probes and
ten analyzer checks PASS. Coverage: **24,354/26,093 (93.34%)**, 1,739 missed,
zero mixed; run `target/coverage/runs/20260930-103155-387/`. Management joins
execute 145/161 lines; 16 lines remain open. This checkpoint adds 93 production
lines and 87 covered lines versus alpha.50; it does not claim complete storage
branch coverage or universal identity matching.

The native read-only storage probe reports Healthy NVMe/SATA media and their
GPT partitions; NVMe health remains readable (45 C, 4% used, no critical warning).
The USB gadget's absent physical identity is explicit. Collection took 347.327 ms
on this machine; this is not release UI timing or a cross-hardware guarantee.
No pass-through command, driver installation, native UI test or desktop input.
Conservative association may omit physical details on other providers/virtual
disks; preserve that explicit limit rather than guess identity.

Candidate: `target/review/alpha51-provider-joins/trontop.exe`, 17,837,568 bytes,
File/Product version `0.3.0-alpha.51`, SHA-256
`59930255969997BD93B34E36DD34808D46A5358B4396689B3BF29B9CC91FEEC0`.
All 195 source/build inputs, instrumented/release copies and old alpha.42-50
hashes were independently verified. Full receipt and verifier are beside the EXE.
Trent authorized replacing old Trontop instances and opening the latest; the
alpha.51 title/window/PID was confirmed without injected input. Work is paused
at his request; [resume anchor](RESUME_ANCHOR.md) records every remaining gate.

Primary schema references used for the implementation:

- [MSFT_PhysicalDisk: DeviceId, identifiers and physical metadata](https://learn.microsoft.com/en-us/windows-hardware/drivers/storage/msft-physicaldisk)
- [MSFT_Disk: OS number, capacity and VPD identifiers](https://learn.microsoft.com/en-us/windows-hardware/drivers/storage/msft-disk)
- [MSFT_Partition: typed number, letter, size and access paths](https://learn.microsoft.com/en-us/windows-hardware/drivers/storage/msft-partition)
- [MSFT_Volume: path, size and free space](https://learn.microsoft.com/en-us/windows-hardware/drivers/storage/msft-volume)
