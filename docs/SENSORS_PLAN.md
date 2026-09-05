# Sensors: implementation and next steps

Trent asked about temperatures and additional hardware telemetry on September 4, 2026.
The alpha.2 checkpoint had no provider. Alpha.3 added NVIDIA; alpha.7 adds storage.

## Implemented in alpha.7: isolated native drive temperatures

`src/storage_sensors/` enumerates GUID_DEVINTERFACE_DISK through SetupAPI and opens
opaque device interfaces with access 0. Only the device temperature property (52)
is queried, avoiding duplicate adapter/device copies. No external assets, vendor
DLLs, installed service, additional driver, elevation or hardware-setting writes.

The coordinator inventories every 30 s. Each interface has one owned worker and
one request in flight. Maximum 32 drive workers; retiring workers count toward the
cap until actually finished. Requests sample after 5 s on success, back off 60 s
after errors/slow completion, and request cancellation after 1 s. No replacement
thread is created for stuck I/O, even after unplug/replug. Buffers/handles stay owned
by their worker until return. The normal system sampler/UI only reads an Arc snapshot.
Native cancellation is not guaranteed; blocked inventory can stall this provider's
coordinator, but not the normal sampler/UI. Monitor drop signals stop without waiting
for stuck calls. Whole-app native close latency remains unmeasured.

UI: scoped GPU/drive/CPU health, compact zebra/hover sensor rows, stable numeric
alignment and fields, true original freshness on cached readings, query cost and
threshold details. Overview shows the hottest reported sensor per drive. Sensor
indices are not CPU cores or mount letters. Threshold-notification-enabled is not
an overheating event. Readings below absolute zero (including the SSD's unset -274 C
threshold) are unavailable; valid zero and reasonable negative Celsius remain valid.

Verification: 55 passing tests, strict Clippy and optimized build pass, 28 offscreen
PNGs. Storage tests cover descriptor truncation/version/count/duplicate indices,
missing values/cache ages, bounded slots, blocked-query isolation, hotplug duplicate
prevention, retry deadlines, lost workers and fast monitor drop. Eight new sensor
layout/state combinations pass. Real native probe, explicitly selected:

```powershell
cargo test native_storage_ssd_probe --offline -- --ignored --nocapture
```

It uses the actual monitor/native backend, restricted to exactly one known
TEAM TM8FP6002T SSD. Latest: 45/45/43 C, 7.2355 ms query, warning 90 C and critical
95 C. Earlier: 44/44/41 C, 7.2946 ms. Both passed without an app window, disk writes
or elevated execution. No HDD query was repeated; unsupported controllers remain
explicitly unavailable. These brief measurements are not a sustained performance
or clean-machine permission matrix. CPU/motherboard sensors are still unconnected.

Primary API references checked before implementation:

- [Device-interface details and opaque paths](https://learn.microsoft.com/en-us/windows/win32/api/setupapi/nf-setupapi-setupdigetdeviceinterfacedetailw)
- [CancelSynchronousIo](https://learn.microsoft.com/en-us/windows/win32/api/ioapiset/nf-ioapiset-cancelsynchronousio)
- [Cancellation ownership and completion limits](https://learn.microsoft.com/en-us/windows/win32/fileio/canceling-pending-i-o-operations)
- [Temperature descriptor](https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ns-winioctl-storage_temperature_data_descriptor)
- [Sensor fields and notification flag](https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ns-winioctl-storage_temperature_info)

## Implemented in alpha.3

This section is the historical first sensor implementation. Alpha.6 adds an
Overview and Hardware sensors route, explicit provider freshness and stable fields
during initial/missing data. On whole-provider failure it retains the last complete
NVML adapter snapshot with a prominent Cached label and original success timestamp.
It does not append cached values to live history. Individual unsupported fields
remain unavailable. CPU/motherboard and drive temperature fields are placeholders
until their real providers are connected, not simulated telemetry.

- Optional NVML library, loaded using `LoadLibraryExW` and `LOAD_LIBRARY_SEARCH_SYSTEM32`.
  No PATH/current-directory search, static NVML import, bundled vendor DLL, driver
  installation, elevation, or hardware-setting writes. Initial support is Windows
  with the DCH driver's System32 library; legacy NVSMI-only installations, AMD and
  Intel sensor providers remain future work. Missing libraries leave other pages intact.
- Adapter identity, GPU die temperature, board watts, graphics/memory clocks, fan
  target percentage, and VRAM used/total. Each unsupported field remains unavailable.
  Fan percentage is an intended speed, not proof that a physical fan is rotating.
- Prefer memory-info v2, which separates driver reservations from allocated memory.
  The v1 fallback is explicitly labeled as including reservations. The initial probe's
  roughly 307 MiB discrepancy with nvidia-smi led to this change.
- Every query runs in the existing background sampler, once per sample. Library or
  enumeration failures unload/retry after 30 seconds. Lost devices do not retain
  partial stale readings. Individual query failures affect their own fields.
- Performance > GPU Sensors has rounded, hoverable, theme-aware cards, two-minute
  temperature/power graphs, rolling peaks and collection timing. GPU Engine PDH
  utilization remains separate, not attributed to an NVML adapter by index.
- History follows UUIDs through enumeration reordering; no ID means no merged history.
  Missing readings and gaps longer than three seconds break traces, never draw zeros.
  Retention is bounded by 120 samples and 120 seconds; disconnected identities expire.

## Verification on Trent's machine

Opt-in, read-only probe (no app window, tray icon, or OS input):

```powershell
cargo test native_nvml_read_only_probe --offline -- --ignored --nocapture
```

Final five-sample run at one-second intervals: initialization 13.854 ms, first query
12.956 ms, warm queries 0.324 / 0.349 / 0.344 / 0.359 ms. A prior run observed about
0.8-1.0 ms warm and 22 ms first-query cost. These are brief query timings, not an
application CPU/drag benchmark or a promise across drivers.

Final native sample: RTX 5070 Ti, 60 C, 90.895 W, 2865/14001 MHz, fan target 0%,
8565/16303 MiB allocated/total VRAM. The immediately following nvidia-smi query gave
60 C, 90.89 W, 2865/14001 MHz, 0%, 8566/16303 MiB. Queries are sequential, not atomic;
live memory and power can change between observations.

Unit/headless checks cover missing fields, real zero, milliwatt conversion, device
loss, v2/v1 memory semantics, retry backoff, UUID ordering, history expiration and
16 sensor layout/state cases. The explicitly selected offscreen visual pass now
produces 15 PNGs, including sensors in dark/light/compact/unavailable states.
No desktop manipulation or running-app replacement was performed.

## Confirmed on this machine

A read-only `nvidia-smi` query returned RTX 5070 Ti temperature 49 C, power 26.97 W,
graphics clock 802 MHz, memory clock 405 MHz, fan 0%, and 6976 / 16303 MiB VRAM.
These are a single instantaneous observation, not expected values or test assertions.
The query changed no clocks, power limits, fan settings, driver settings, or processes.

## Historical storage capability probe (before alpha.7 integration)

A separate one-shot metadata probe used the documented temperature property query
on the three disks identified by read-only Windows inventory. `CreateFileW` used
access 0, shared access and `OPEN_EXISTING`; the only IOCTL was
`IOCTL_STORAGE_QUERY_PROPERTY` (0x002D1400), with property 51/52 and standard query.
No sector reads/writes, threshold changes, privilege enabling or driver installs.
The sandbox denied WMI inventory, so this diagnostic ran outside that sandbox.
A follow-up token-role check in the same tool context returned administrator=False.
This is evidence on one non-elevated machine context, not a clean-profile permission
matrix across controllers and Windows configurations.

Observed results, not an app feature or universal device-support claim:

| Device | Temperature query result | Elapsed for both properties |
| --- | --- | --- |
| TEAM TM8FP6002T (disk 1) | Both returned sensors 0/1/2 at 44/44/41 C; warning 90 C, critical 95 C | 14.756 ms |
| WDC WD60EZAX-00C8VB0 (disk 0) | Adapter error 1117; device error 1 | 4251.274 ms |
| WD My Passport 2626 USB (disk 2) | Both returned error 87 | 0.078 ms |

Only one pass ran. Sensor indices are not package/core labels; index 0 may be a
composite reading. Driver thresholds are not Trontop's recommended operating values.
The signed 16-bit sentinel 0x8000 means not reported, not an actual temperature.
The parser checked returned lengths, descriptor version/size and sensor count before
reading variable-length records. The local diagnostic is saved, untracked, at
`target/diagnostics/read-storage-temperature.ps1`; it is not shipped in the EXE.

Implementation inference from the 4.25-second HDD query: do not put storage queries
on the existing one-second system sampler. Use a dedicated, bounded worker/cache,
slow cadence, failure backoff and stale/error timestamps. A timeout must not spawn
unbounded replacement threads or leave abandoned I/O buffers. Research cancellable
I/O and shutdown semantics before integration. Deduplicate adapter/device readings
with verified physical-device identity; do not assume disk number or mount letter
is stable, and do not count the two returned copies as six sensors.

Primary references:

- [Windows NVMe temperature queries](https://learn.microsoft.com/en-us/windows/win32/fileio/working-with-nvme-devices)
- [Temperature descriptor layout](https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ns-winioctl-storage_temperature_data_descriptor)
- [Sensor indices and Celsius units](https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ns-winioctl-storage_temperature_info)
- [Metadata-only device handles](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew)
- Local Windows SDK 10.0.26100.0 `winioctl.h`: property IDs, IOCTL and unavailable sentinel.

## Remaining implementation order

### GitHub research requested by Trent (2026-09-05)

Checked primary source, not download sites or inferred screenshots:

- [RIGStats](https://github.com/dvalfrid/rigstats#architecture) is a Windows Rust/egui
  dashboard. Its telemetry design includes a .NET service embedding LibreHardwareMonitor
  and a bundled signed PawnIO driver. Its documented CPU package temperature source
  is LHM, not `sysinfo`. This is one concrete design example, not permission to install
  its service/driver or a recommendation to copy its distribution architecture.
- [LibreHardwareMonitor Intel MSR access](https://github.com/LibreHardwareMonitor/LibreHardwareMonitor/blob/master/LibreHardwareMonitorLib/PawnIo/IntelMsr.cs)
  loads a PawnIO module and invokes an MSR-read operation. Its [repository](https://github.com/LibreHardwareMonitor/LibreHardwareMonitor)
  documents broad hardware coverage and some administrator requirements. Source was
  studied only; no third-party code was copied and no driver was installed.
- [sysinfo's Windows component source](https://github.com/GuillaumeGomez/sysinfo/blob/main/src/windows/component.rs)
  queries `root\\WMI` / `MSAcpi_ThermalZoneTemperature` and labels the result Computer.
  It is not an all-core CPU thermal backend. Do not label its generic thermal-zone
  reading CPU Package without verified hardware attribution.

Read-only capability checks in this tool context returned Invalid namespace for
`root/LibreHardwareMonitor` and `root/OpenHardwareMonitor`, and Not supported for
`MSAcpi_ThermalZoneTemperature`. No matching HWiNFO/LHM/OHM/FanControl process was
observed. This only describes those checked interfaces in the current context; it
does not prove the hardware lacks sensors or rule out every existing provider.

Decision: retain portable base EXE with optional installed NVIDIA driver telemetry;
the verified Windows storage query is now integrated in isolated workers. Further
CPU provider research should check existing vendor interfaces and authorized existing
monitoring providers before proposing an optional low-level backend. Any driver/service
installation, elevation or security-setting change needs explicit approval. Trent has
not approved it. No driver-free all-CPU-temperature solution has been verified here.

### Pending runtime work

1. Broader vendor/legacy-driver coverage, stronger per-field error explanations and
   provider recovery tests. Validate unsupported-driver startup on another machine.
2. Optional temperature in the tray tooltip, per-adapter alert thresholds and export.
   No fabricated CPU temperatures and no claim that missing fan data means a stopped fan.
3. Broader storage-controller temperature coverage, wear, power-on hours and error
   counters through Windows APIs where exposed. Alpha.7 integrates temperature only;
   this SSD passed the native runtime probe, not every device/controller.
4. Investigate CPU package/core temperature and power separately. Do not label ACPI
   thermal zones as CPU package temperature. A driver-based provider changes security,
   privilege, distribution and portability requirements; do not silently install one.

Keep all queries off the UI thread, with isolated slow workers for storage sensors.
Load NVML only from trusted system/driver locations with safe dependency resolution,
never from an arbitrary working directory. If the driver library or a symbol is
missing, Trontop must still start as one portable EXE. Do not bundle vendor DLLs or
introduce a runtime asset directory. The CLI query is diagnostic evidence only; do
not spawn nvidia-smi every UI frame as the production integration.

## Primary references checked

- [NVIDIA NVML overview and Windows driver library locations](https://docs.nvidia.com/deploy/nvml-api/nvml-api-reference.html)
- [NVIDIA device queries](https://docs.nvidia.com/deploy/nvml-api/group__nvmlDeviceQueries.html)
- [Microsoft storage reliability counters](https://learn.microsoft.com/en-us/windows-hardware/drivers/storage/msft-storagereliabilitycounter)
- [Microsoft Win32_TemperatureProbe limitations](https://learn.microsoft.com/en-us/windows/win32/cimwin32prov/win32-temperatureprobe)
