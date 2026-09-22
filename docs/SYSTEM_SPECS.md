# System specs page (Speccy-style)

The System page shows Speccy-style sections (Summary, Operating System, CPU,
RAM, Motherboard, Graphics, Storage, Optical Drives, Audio, Peripherals,
Network, Sensor Sources). Each section is a tree of collapsible groups of
label/value rows. Live values (clocks, temperatures, usage) are keys that the
UI resolves every frame from the existing sampler snapshot and the sensor
bridge, never by a native call on the UI thread.

## Layout

| Path | Owner | What |
| --- | --- | --- |
| `src/specs.rs` | integrator | module root, re-exports |
| `src/specs/model.rs` | integrator | `SectionId`, `Value`, `Row`, `Group`, `Item`, `SummaryLine`, `Section`, `SectionHealth` |
| `src/specs/live.rs` | integrator | `LiveKey`, `GpuRef`, `GpuMetric`, bridge types, `resolve`, `band` |
| `src/specs/worker.rs` | integrator | `Context`, `Monitor`, per-section workers, bridge worker |
| `src/specs/report.rs` | integrator | text rendering for Copy and probes |
| `src/specs/native/` | integrator | SMBIOS, WMI, SetupDi, registry helpers |
| `src/specs/fixtures.rs` | integrator | cfg(test) synthetic TEST DATA |
| `src/app/system.rs` | integrator | the page |
| `src/specs/<lane>.rs`, `src/specs/<lane>/`, `docs/specs/<lane>.md` | lane | providers |

Lanes: `os`, `cpu`, `memory`, `board`, `graphics`, `storage` (Storage and
Optical Drives), `devices` (Audio and Peripherals), `network`, `bridge`.

## Provider contract

```rust
pub fn collect(ctx: &Context) -> Section                 // os, cpu, memory, board, graphics, network, bridge
pub fn collect(ctx: &Context) -> Section                 // storage: Storage
pub fn collect_optical(ctx: &Context) -> Section         // storage: OpticalDrives
pub fn collect_audio(ctx: &Context) -> Section           // devices: Audio
pub fn collect_peripherals(ctx: &Context) -> Section     // devices: Peripherals
pub fn read_live(ctx: &Context) -> BridgeReadings        // bridge: fast live poll
```

- Runs on its own worker thread: immediately, then 5 minutes after each read,
  and on Refresh. A read over 10 s is shown as Slow with previous data kept.
  Budget 20 s (`ctx.remaining()`, `ctx.should_stop()`, `ctx.timeout(max)`).
- Return the section for your own `SectionId`. Anything unobtainable is
  `Value::Unavailable(reason)` with a real reason. Never estimate, default or
  substitute an unrelated reading. Blank known text becomes Unavailable.
- Partial failures go in `Section::issues` (no private data in issues or
  group titles). A section with no groups is Unavailable.
- Private values (serials, MAC/IP, product IDs/keys, user/computer names,
  SSIDs, GUIDs/UUIDs, EDID serials, instance IDs, interface paths): `.private()`.
- No panics, no `unwrap` on native data (release builds abort on panic).
- Read-only: no drivers, elevation, registry writes, system changes or network.
- Windows calls live under `#[cfg(windows)]` (usually `src/specs/<lane>/native.rs`).

## Live keys

`Row::live(label, key)` shows only the resolved live value. `Group::live` and
`SummaryLine::live` show it beside the title or headline. Sampler-owned keys:
`CpuUsage`, `CpuClockAverage`, `CpuClockFastest`, `CpuCoreClock { group, number }`,
`MemoryUsed`, `MemoryCommit`, `Uptime`, `Gpu { adapter: GpuRef { name, ordinal }, metric }`,
`DriveTemperature { interface }` (GUID_DEVINTERFACE_DISK path),
`NetworkThroughput { interface }` (sampler interface name). Bridge-only keys:
`CpuPackageTemperature`, `CpuCoreTemperature { index }`, `MotherboardTemperature`,
`Sensor { id }`. An unavailable sampler value falls back to a bridge reading
with the identical key.

## Verification

```powershell
cargo test specs:: -- --nocapture
cargo test app::ui_smoke::system -- --nocapture
cargo test native_specs_<lane>_read_only_probe -- --ignored --nocapture
cargo test render_system_specs_visual_pass -- --ignored --nocapture
```

Helper probes (read-only, selected by exact name): `native_specs_smbios_read_only_probe`,
`native_specs_wmi_read_only_probe`, `native_specs_setupapi_read_only_probe`,
`native_specs_registry_read_only_probe`. On the reference machine (2026-09-22):
SMBIOS 3.7 copy 3.6 ms with four type 17 entries (two populated, size word
0x7FFF so the extended size field applies); WMI connect 13 ms, queries 4 to 70 ms;
SetupDi media class 12 devices in 68 ms. Probe output masks private values.
