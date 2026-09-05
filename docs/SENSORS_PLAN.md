# Sensors: implementation and next steps

Trent asked about temperatures and additional hardware telemetry on September 4, 2026.
The alpha.2 checkpoint had no provider. Alpha.3 now implements the first NVIDIA slice.

## Implemented in alpha.3

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

## Remaining implementation order

1. Broader vendor/legacy-driver coverage, stronger per-field error explanations and
   provider recovery tests. Validate unsupported-driver startup on another machine.
2. Optional temperature in the tray tooltip, per-adapter alert thresholds and export.
   No fabricated CPU temperatures and no claim that missing fan data means a stopped fan.
3. Storage temperature, wear, power-on hours and error counters through Windows
   storage reliability APIs where the device/controller/permissions expose them.
4. Investigate CPU package/core temperature and power separately. Do not label ACPI
   thermal zones as CPU package temperature. A driver-based provider changes security,
   privilege, distribution and portability requirements; do not silently install one.

Keep all queries on the background sampler, with slower cadence for expensive sensors.
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
