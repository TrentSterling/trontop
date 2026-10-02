# CPU and motherboard fallback checks (alpha.49)

This continues A06/A07/A25/A33. The portable candidate retains End process tree,
End all instances, Startup controls and the previous live-value/privacy fixes.

## Behavior

Missing CPUID data leaves virtualization capability and hypervisor detection
unavailable with a reason. Independently supplied Windows firmware and SLAT flags
remain visible. Unknown x86 vendors use generic instruction/virtualization labels;
they no longer inherit AMD labels. Unrecognized Intel brand suffixes do not imply
a desktop socket or mobile codename. Missing processor-package records no longer
invent one package while independently readable core/thread facts remain visible.

Nominal CPU clocks now use the same per-processor branded-frequency query as the
dynamic clock sampler. References are keyed by processor group and number instead
of indexing a group-zero vector. Each distinct identity is read once; invalid,
zero and failed readings are excluded. Successful readings survive partial query
failure or the inventory read budget expiring. Rows mark partial data and retain
source/count notes; missing per-core-type references stay unavailable. CPUID rated
frequencies remain separate, and a missing CPUID bus clock can use SMBIOS even
when other CPUID frequency fields exist. Sampling remains off the render thread.

This reuses the existing bounded, query-only `NtPowerInformation` level 87 /
internal selector 43 / version 1 interface documented in `CPU_CLOCK.md`. It is a
private native API; unsupported status/layout/version/frequency values fail
explicitly. No driver, affinity change or power-policy mutation was added. The
old `GetSystemInfo` / `PROCESSOR_POWER_INFORMATION` vector did not establish
identities for every processor group. Microsoft's descriptions of
[SYSTEM_INFO](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/ns-sysinfoapi-system_info)
and [PROCESSOR_POWER_INFORMATION](https://learn.microsoft.com/en-us/windows/win32/power/processor-power-information-str)
describe the legacy fields; the shared query declarations are linked in
`CPU_CLOCK.md`.

Motherboard inventory preserves separately readable PCI LPC/eSPI and host-bridge
rows when SMBIOS is unavailable, along with every collection issue. BIOS release
bytes marked unknown no longer become revision 255. ROM units are KiB/MiB/GiB,
and reserved extended units remain unavailable. Date normalization requires a
valid numeric Gregorian date, including century leap rules; malformed firmware
text stays available as the original string instead of becoming an invented ISO
date. Legacy two-digit firmware years retain the standard's 19yy interpretation.
Expectations for BIOS units/markers, chassis lock/type and slot width/usage use
[DMTF DSP0134 3.8.0, dated 2024-08-05](https://www.dmtf.org/sites/default/files/standards/documents/DSP0134_3.8.0.pdf).
Private serials and device paths remain out of default reports.

## Evidence

Sixteen new ordinary checks exercise nine CPU fact/collection cases, six board
firmware/PCI cases and one shared native-query decoder. They cover missing and
unknown providers, zero/invalid data, multi-group identity, duplicate references,
partial failures, budget expiry, homogeneous SMT, middle efficiency classes,
cache notes, chassis/slot encodings, private paths and preserved error reasons.
Nine before-fix failure logs are preserved beside the review executable. Those
failures reproduced missing-CPUID AMD labels, unknown-vendor labels, lost bus
fallback, zero nominal values, unknown model suffix inference, lost PCI rows,
BIOS revision markers, ROM units and invalid-date normalization.

Final ordinary and instrumented suites: **544 passed, 0 failed, 57 ignored**.
Formatting, strict all-target Clippy and the optimized release build PASS. All
24 exact read-only probes and 10 coverage-analyzer checks pass. Final production
unique file lines: **23,947/25,782 (92.88%)**, 1,835 uncovered, zero mixed,
24,062 test-only lines and 40 raw profiles. Final isolated run:
`target/coverage/runs/20260930-081614-796/`. All 189 application/build inputs,
copied instrumented object, measurement tools and exact test/probe logs are
independently verified by the candidate's `verify-receipt.py`.

The final-source read-only CPU inventory probe returned 24 cores (8P + 16E),
24 threads, Performance nominal 3700 MHz and Efficient nominal 3200 MHz in
2.162 ms. That is one instrumented provider observation on this machine, not
release UI performance, Task Manager parity or a cross-hardware guarantee.
The earlier standalone reference probe also passed before the final minor
date/package checks; its log is preserved separately. UI layout is unchanged;
the ordinary production-UI checks passed. No new offscreen images were generated
or inspected for alpha.49; alpha.48's images remain historical evidence.

Useful commands:

```powershell
cargo test --offline specs::cpu::fixture_tests
cargo test --offline specs::board::fixture_tests
cargo test --offline cpu_clock::tests::nominal_query_output_validates_version_frequency_and_native_status -- --exact
cargo test --offline -- --quiet --test-threads=4
cargo fmt --all --check
cargo clippy --offline --all-targets -- -D warnings
cargo build --offline --release
./scripts/measure-coverage.ps1 -ReadOnlyProbes -TestThreads 4
python target/review/alpha49-hardware-fallbacks/verify-receipt.py
```

Review EXE: `target/review/alpha49-hardware-fallbacks/trontop.exe`, File/Product
version `0.3.0-alpha.49`, **17,815,552 bytes**, SHA-256
`AE57293D520090869FE27DDF9B432B8726F9AB03B071B727658240F7A1D8F67A`.
Release/review hashes match. Alpha.42 through alpha.48 review hashes remain
verified and preserved. `receipt.json`, `candidate-gates.json` and the verifier
sit beside the candidate. Post-run audit: zero test processes and zero owned
Startup registry fixture leaves. Trent has the verified candidate link and then
explicitly requested it be opened. The first sandboxed launch (PID 165360) was
not visible to Trent despite a window handle. At his repeated request, alpha.49
was relaunched outside the execution sandbox from that exact review path:
PID 164012, normal native window titled `Trontop`. No input, focus or window
manipulation was used. This launch is not native acceptance.

The full goal remains ACTIVE. CPU/motherboard temperature provider decisions,
field parity, isolated native acceptance, soak, clean-machine and exact-build
release acceptance remain open in `ASK_LEDGER.md`. This pass used only owned
test fixtures and the fixed read-only probe list. No native desktop automation,
service command, real Startup change, installation or publication.
