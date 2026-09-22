# System specs page (Speccy-class, alpha.36)

Trent, 2026-09-22: "clone speccy, IN trontop, make trontOP OP". The System page
shows Speccy's sections (Summary, Operating System, CPU, RAM, Motherboard,
Graphics, Storage, Optical Drives, Audio, Peripherals, Network) plus Sensor
Sources. Each section is a tree of collapsible groups of label/value rows. Live
values (clocks, temperatures, usage, throughput) are keys that the UI resolves
every frame from the existing sampler snapshot and the sensor bridge, never by a
native call on the UI thread.

Everything is read-only: no driver (no WinRing0, PawnIO or inpoutx64), no
elevation, no registry writes, no system changes, no network traffic. Anything
Windows does not expose is an explicit Unavailable value with the reason.

## Using the page

- **Sub-navigation** with a section icon and a status dot per section (green
  complete, amber partial or slow, red unavailable or stopped, grey waiting).
- **Summary**: one Speccy-style headline block per section, live values right
  aligned and colored by temperature band (drives 50/60 C, everything else
  70/85 C). A missing live value is a muted `--` with the reason on hover.
- **Sections**: header with state pill, freshness ("Read 12 s ago in 35 ms"),
  every issue, Expand all / Collapse all and Copy section. Groups remember their
  open state. Long values (instruction sets, partitions) wrap instead of cutting.
- **Copy**: Copy all (whole page), Copy section, right-click a group title
  (Copy group) or a row (Copy value, Copy row). Private rows never copy while
  hidden. egui clipboard only; no OS input.
- **Save text / Save JSON**: Speccy's "Save as text" equivalent through the
  existing export worker and native Save As picker (`docs/EXPORTS.md`). Private
  values are excluded unless "Private values in saved files" is on; it resets
  after every save.
- **Reveal private values** (off by default) unmasks serials, UUIDs, MAC/IP
  addresses, product IDs, user and computer names, Bluetooth device names and
  device instance IDs on screen only. Drive interface paths (they embed the
  serial) are never shown, copied or saved; they only key live temperatures.
- **Refresh** re-reads every section; reads also repeat every 5 minutes.
- Sections are never blank: Waiting, Reading, Slow (previous data kept),
  Stopped, Partial (issues listed) and Unavailable (reason) all render text.

## Beating Speccy's bugs on the reference PC

| Speccy (v1.33) on this box | Trontop alpha.36 |
| --- | --- |
| GPU memory 4014 MB (WMI AdapterRAM is 32-bit) | Video memory 15.9 GB (driver `HardwareInformation.qwMemorySize`, 64-bit) plus DXGI dedicated 15.6 GB, labelled separately |
| Invented "shader clock" equal to the memory clock | No shader clock row; core and memory clocks are live NVML readings |
| RAM "Number of SPD modules: 0" | Per-DIMM SMBIOS type 17: slot, size, DDR5, maximum and configured MT/s, Corsair CMH64GX5M2Y6400C32, ranks, voltage; serials private |
| TEAM NVMe interface "Unknown", no temperature | "NVMe (PCIe 3.0 x4)", live 44 C, plus the NVMe health log (wear, data written, power-on hours, unsafe shutdowns, media errors) without admin |
| "Virtualization: Not supported" | "Intel VT-x: supported (CPUID hides it while Hyper-V runs)", firmware enabled, SLAT, hypervisor in use, VBS and memory integrity separately |
| Caches describe P-cores only | L1/L2/L3 per core type: 8 x 48 KB (P) and 16 x 32 KB (E) L1d, 8 x 3 MB (P) and 4 x 4 MB (E cluster) L2, 36 MB shared L3 |
| CPU name "Intel Core" | Full CPUID brand "Intel(R) Core(TM) Ultra 9 285K", Arrow Lake-S, LGA1851, microcode 117h |

## Sections, sources and what stays Unavailable

Probe timings are from the reference PC (Core Ultra 9 285K, ASRock Z890-C,
Windows 11 Home 25H2 26200.9457), 2026-09-22, debug build.

**Operating System** (52 ms). Win32_OperatingSystem, CurrentVersion registry
values, GetFirmwareType, TBS device info, GetUserDefaultLocaleName,
GetDynamicTimeZoneInformation, PowerGetActiveScheme, .NET NDP release, Win32_DeviceGuard,
SecurityCenter2 antivirus, Win32_PageFileUsage. "Windows 11" comes from build
>= 22000 (the registry ProductName still says 10). Computer name, registered
user and product ID are private. TPM manufacturer is not read: Win32_Tpm needs
administrator and took about 6 s to refuse, so it is omitted rather than slowing
the section.

**CPU** (3 ms). CPUID (brand, family/model/stepping, instruction sets, leaf 16h,
hypervisor leaves), GetLogicalProcessorInformationEx (packages, cores per
efficiency class, caches per core type), CallNtPowerInformation (nominal clock
per core type: P 3700 MHz, E 3200 MHz), microcode from the registry, SMBIOS
type 4. Codename and desktop socket come only from a documented model table.
Live: usage, average and fastest clock, one clock row per logical processor.
Unavailable: package temperature, package power and core voltage unless a sensor
provider runs (see Sensor Sources); TDP ("not exposed by Windows without a kernel
driver"). Never an ACPI thermal zone.

**RAM** (1 ms). GetPhysicallyInstalledSystemMemory (64.0 GB), GlobalMemoryStatusEx
(63.5 GB usable, 554 MB reserved), SMBIOS type 16 (4 slots, 128 GB maximum, no
ECC) and type 17 per DIMM. Channels are read from the firmware's slot labels
(Dual: A, B). Live: in use, commit charge. Unavailable: timings (memory
controller registers and SPD need a kernel driver).

**Motherboard** (144 ms). SMBIOS types 0, 1, 2, 3 and 9 (nine PCIe/M.2 slots),
PCI class 0601/0600 bridges via SetupDi with vendor/device IDs (Windows names
them generically, so no chipset model is claimed). Serials and the system UUID
are private. OEM filler strings are "not set by the manufacturer". Slot usage is
what the firmware recorded (it lists PCIE1 as available). Live board temperature
only through the bridge.

**Graphics** (100 ms). The existing DXGI inventory, SetupDi display class
(subsystem vendor = board partner PNY, driver, PCIe 5.0 x16 link from
DEVPKEY_PciDevice_*), the driver key (physical VRAM, video BIOS 98.3.58.0.23),
and QueryDisplayConfig monitors (Beyond TV, 3840 x 2160 at 60 Hz current and
native, HDR supported/off, 8 bpc, EDID PNP ID). DXGI lists the RTX 5070 Ti under
three LUIDs (indirect displays render on it); they fold into one adapter.
Parsec, Meta and Virtual Desktop display drivers are listed separately. Live
NVML: temperature, core/memory clock, board power, fan target, VRAM in use.
Intel iGPU live sensors are Unavailable (NVML is NVIDIA only).

**Storage** (330 ms). SetupDi disk interfaces, metadata-only
IOCTL_STORAGE_QUERY_PROPERTY (descriptor, seek penalty, TRIM, sector sizes,
NVMe health log page 02h), IOCTL_STORAGE_GET_DEVICE_NUMBER, Windows Storage
Management WMI (media type, health, partitions, volumes). The disk handle is
opened with desired access 0. Live temperature per drive from the existing
drive sensor worker. The WD HDD reports no temperature. ATA SMART attributes are
"requires administrator" (MSFT_StorageReliabilityCounter). Interface paths are
never displayed and never appear in probes, copies or exports.

**Optical Drives**. SetupDi CD-ROM class; this PC has none ("No optical disk
drives", shown as a complete result, not an error).

**Audio** (with Peripherals 870 ms). SetupDi media class (12 devices with driver
versions) and MMDevice endpoints with the shared-mode engine format
(PKEY_AudioEngine_DeviceFormat) and defaults.

**Peripherals**. One SetupDi snapshot of present devices with parent links:
keyboards and pointing devices with bus-reported product names, game
controllers, USB host controllers and attached devices, the Bluetooth radio and
paired devices (names private), cameras, Win32_Printer.

**Network** (34 ms). GetAdaptersAddresses (no traffic), SetupDi network drivers.
Connected adapters first with link speed and live throughput keyed by the alias
(verified equal to the `sysinfo::Networks` name for the connected adapter).
MAC, IP, gateway, DNS and DHCP server are private. Wi-Fi SSID/BSSID are not read:
Windows treats them as location data and may prompt, and Trontop never triggers
that prompt.

**Sensor Sources** (10 ms, bridge poll every 2 s while visible, 10 s when no
provider, 30 s when hidden). Read-only access to providers the user already runs:
LibreHardwareMonitor and OpenHardwareMonitor WMI namespaces and HWiNFO shared
memory (`Global\HWiNFO_SENS_SM2`, FILE_MAP_READ). Readings map to
CpuPackageTemperature, CpuCoreTemperature, MotherboardTemperature, package power
and core voltage, plus a Sensor key per reading, each with a source label shown
on hover. On the reference PC none runs, so those values are Unavailable with
that reason. The same readings feed the Hardware sensors page.

## Layout and contract

| Path | What |
| --- | --- |
| `src/specs.rs` | module root, re-exports |
| `src/specs/model.rs` | `SectionId`, `Value`, `Row`, `Group`, `Item`, `SummaryLine`, `Section`, `SectionHealth` |
| `src/specs/live.rs` | `LiveKey`, `GpuRef`, `GpuMetric`, bridge types, `resolve` (with source labels), `band` |
| `src/specs/worker.rs` | `Context`, `Monitor`, per-section workers, bridge worker |
| `src/specs/report.rs` | text and JSON rendering for Copy, Save and probes |
| `src/specs/native/` | SMBIOS, WMI, SetupDi, registry, PCI ID and PCIe link helpers |
| `src/specs/<section>.rs` and `<section>/native.rs` | providers: portable mapping plus Windows reads |
| `src/specs/fixtures.rs` | cfg(test) synthetic data and the real-data collector for visual QA |
| `src/app/system.rs` | the page |

Providers return the section for their own `SectionId`, use
`Value::Unavailable(reason)` for anything unobtainable, put partial failures in
`Section::issues`, mark private rows, never panic on native data, and keep
Windows calls under `#[cfg(windows)]`. Workers: one thread per section, read on
first view of System or Hardware sensors, every 5 minutes after, and on Refresh
(coalesced). A read over 10 s shows Slow with previous data kept.

## Verification

```powershell
cargo test specs:: -- --nocapture
cargo test app::ui_smoke::system -- --nocapture
cargo test export::tests::system_specs -- --nocapture
$env:TRONTOP_SPECS_SHOTS = "<folder>"; cargo test render_system_specs_visual_pass -- --ignored --nocapture
cargo test render_system_specs_fixture_visual_pass -- --ignored --nocapture
cargo test native_specs_<section>_read_only_probe -- --ignored --nocapture
```

Section probes: `native_specs_os_read_only_probe`, `_cpu_`, `_memory_`, `_board_`,
`_graphics_`, `_storage_`, `_devices_`, `_network_`, `_bridge_`. Helper probes:
`native_specs_smbios_read_only_probe`, `_wmi_`, `_setupapi_`, `_registry_`.
Probe output masks private values and prints live values as keys (drive keys
without their interface path). The real-data visual pass collects every section
once on the test thread plus one short-lived sampler (read-only), renders ten
offscreen PNGs and never opens a window.
