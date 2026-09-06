# Hardware polling: source audit, 2026-09-06

Scope: A06, A10, A22. Read-only research and probes; no installed software,
drivers, services, power-policy changes, stress workload or desktop automation.

## What the evidence actually says

There is a confirmed measurement-label problem, not evidence of a Rust language
limitation. Our pinned `sysinfo 0.38.4` Windows implementation uses
`CallNtPowerInformation(ProcessorInformation)` and reads `CurrentMhz` in
`src/windows/cpu.rs::get_frequencies`. Trontop selected CPU 0's value and called
it `SPEED`. This machine returns 3700 MHz through that path while Task Manager's
supplied screenshot shows 5.12 GHz.

Microsoft defines CurrentMhz in terms of the specified clock and the current
throttle. It is not documented as an effective hardware-cycle measurement.
[PROCESSOR_POWER_INFORMATION](https://learn.microsoft.com/en-us/windows/win32/power/processor-power-information-str)

Bruce Dawson reported the same field staying at MaxMhz rather than tracking the
clock shown by Task Manager. That historical report supports investigating the
source, but its closed state is not proof of a fix on this machine.
[Windows performance issue 100](https://github.com/microsoft/Windows-Dev-Performance/issues/100)

Alpha.33 labels the existing reading **POWER CLOCK**, names the CurrentMhz/CPU 0
source, and displays `-- GHz` when absent. **Live boost/effective frequency is
still unfinished.** Relabeling the field is not frequency parity.

## Local read-only evidence

The production CPU snapshot helper, exercised using sysinfo on the installed
machine, returned 24 logical / 24 physical processors. Four separate usage reads
contained distinct per-processor values, including idle-ish and heavily loaded
processors. Refresh plus snapshot construction took 162.8, 172.2, 232.0 and
249.0 microseconds in the debug test binary. CurrentMhz stayed at 3700.
This does not measure the rest of the sampler, native dragging, GPU rendering,
shutdown or a whole-app frame budget.

At 2026-09-06 05:17:34 and 05:17:35 America/Chicago, Windows `Get-Counter`
returned valid status 0 for all of these:

| English PDH counter | Sample 1 | Sample 2 |
| --- | ---: | ---: |
| Processor Information(_Total) / % Processor Performance | 144.5575 | 144.5997 |
| Processor Information(_Total) / Processor Frequency | 3366 MHz | 3366 MHz |
| Processor Information(0,0) / Processor Frequency | 3700 MHz | 3700 MHz |
| Processor Information(0,8) / Processor Frequency | 3200 MHz | 3200 MHz |

Inference: this hybrid CPU makes an arbitrary aggregate-frequency multiplier
especially suspect. Multiplying the total or CPU 0 nominal value yields different
answers. Do not pick whichever agrees with a screenshot. Validate aggregation,
reference frequency, processor-group identity and sampling interval first. Busy
time, performance relative to nominal, and effective frequency are distinct.

## What open-source tools use

| Reference | Useful lesson | Boundary for Trontop |
| --- | --- | --- |
| [btop4win](https://github.com/aristocratos/btop4win) | Its extended sensor configuration uses LibreHardwareMonitor via LHM-CppExport; the documented build needs additional libraries and elevation. | Switching language does not supply sensors automatically. Do not silently add those runtime requirements to our portable EXE. |
| [LibreHardwareMonitor](https://github.com/LibreHardwareMonitor/LibreHardwareMonitor) | Broad hardware coverage; some sensors need administrator access. | Capability depends on the device and backend. Current absence is not permission to invent zero or an unrelated temperature. |
| [LHM Intel CPU backend](https://github.com/LibreHardwareMonitor/LibreHardwareMonitor/blob/master/LibreHardwareMonitorLib/Hardware/Cpu/IntelCpu.cs) | The inspected implementation uses Intel MSR access through its PawnIO module for low-level CPU sensors. | CPU package/core temperatures are not another ordinary sysinfo property. No LHM/PawnIO code or driver was added. |
| [System Informer](https://github.com/winsiderss/systeminformer) | Relevant native-monitor reference for deeper process and performance coverage. | Its exact CPU-frequency algorithm was not established in this pass; repository access was incomplete. Do not attribute an unverified formula to it. |

Boxel's owner-authored `crates/boxel/src/color.rs` and `ui/theme_window.rs` were
also inspected for ColorMagic. The six palette-family bands and HSL conversion
inspired the small native adaptation. No new runtime dependency. The expected
SpaceView checkout was absent from `C:/Github` and `C:/trontstack`; do not claim
that its current engine was inspected or fully ported.

## Next finite hardware work, not an endless search

1. Finish the CPU frequency/utility comparison with a measured Windows source,
   clear semantics and per-group identity. Keep the present power clock separate.
2. Measure each existing provider's wall time and worst-case delay on the worker;
   the CPU micro-probe cannot clear GPU PDH, process metadata or storage polling.
3. Decide D01: optional read-only connection to an already-running sensor provider
   versus explicitly approved privileged sensor integration. Keep the ordinary
   process manager portable and operational when the provider is absent.
4. Complete the A06 field-by-field sheet (GPU adapters/VRAM, networking, CPU
   details, NPU), with supported, partial and missing fields, not a parity slogan.

Repeat CPU probe:

```powershell
cargo test --offline native_logical_cpu_read_only_probe -- --ignored --nocapture
```

No third-party monitor code was copied. LHM lists MPL-2.0 with additional notices;
btop4win lists Apache-2.0. Check exact files and integration obligations before
borrowing source, rather than assuming all GitHub code has the same licence.
