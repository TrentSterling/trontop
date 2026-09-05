# Sensors: proposed next slice

Trent asked about temperatures and additional hardware telemetry on September 4, 2026.
No sensor provider is implemented by the alpha.2 UI/harness checkpoint.

## Confirmed on this machine

A read-only `nvidia-smi` query returned RTX 5070 Ti temperature 49 C, power 26.97 W,
graphics clock 802 MHz, memory clock 405 MHz, fan 0%, and 6976 / 16303 MiB VRAM.
These are a single instantaneous observation, not expected values or test assertions.
The query changed no clocks, power limits, fan settings, driver settings, or processes.

## Implementation order

1. Optional NVIDIA NVML provider: adapter name, GPU temperature, board power,
   graphics/memory clocks, fan percentage and VRAM. Per-field support varies; retain
   `Option`/unavailable states rather than substituting zero. Keep Windows GPU Engine
   PDH as the existing per-process and engine utilization source.
2. GPU Performance cards and histories for temperature/power, peak readings and
   provider status. Optionally expose temperature in the tray tooltip. No fabricated
   CPU temperatures and no claim that missing fan data means a stopped fan.
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
