# Alpha.35: Windows GPU adapters (A06/A07/A15/A32)

Performance / GPU adapters now selects by session-local LUID + physical node,
not vector position or vendor name. Each adapter gets separate engine histories,
dedicated/shared/committed memory and source-labelled DXGI capacities. Graphs and
Overview share these timestamped series; no second history or UI polling loop.
The existing aggregate/process activity semantics and NVIDIA sensors remain separate.

## Sources and limits

- DXGI GetDesc1 supplies name, vendor/device IDs and logical-adapter capacities.
  Shared system memory is a limit, not allocated or reserved RAM. Linked-node
  capacities are not duplicated into per-node usage percentages.
  [DXGI adapter description](https://learn.microsoft.com/en-us/windows/win32/api/dxgi/ns-dxgi-dxgi_adapter_desc)
- PDH `GPU Adapter Memory(*)` supplies `Dedicated Usage`, `Shared Usage` and
  `Total Committed`, formatted as 64-bit integers. They are whole physical-adapter
  counters, not the sum of per-process allocations. Committed is a separate
  accounting measure, not an amount to add to the first two.
- `QueryVideoMemoryInfo` was deliberately not used for system-wide usage: its
  budget/usage belongs to the calling process.
  [Microsoft API semantics](https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_4/nf-dxgi1_4-idxgiadapter3-queryvideomemoryinfo)
- Engine counters sum process usage for each physical engine, then take the
  busiest engine for the adapter. Same-kind engines remain distinct by index.
  Missing counters produce partial lower bounds and graph gaps, not zero.
  [Task Manager GPU accounting](https://devblogs.microsoft.com/directx/gpus-in-the-task-manager/)
- Three persistent wildcard memory counters reuse one query on the existing
  background sampler worker. DXGI inventory/retry is 30 seconds. Native array
  buffers use aligned storage, bounded name pointers/counts, fresh sizing on
  retry, four attempts and a 1 MiB cap. Up to 64 identities / 256 engines per
  identity are retained; the existing overall 512-series chart budget still applies.
  [PDH formatted arrays](https://learn.microsoft.com/en-us/windows/win32/api/pdh/nf-pdh-pdhgetformattedcounterarrayw)
- Last usable memory fields survive failures with `~`/Cached and no fake history.
  Software adapters are labelled. Unmatched counter identities retain their LUID
  with unknown hardware/capacity rather than being guessed to be a GPU or NPU.
  DXGI can expose multiple identically named adapters; selector entries include
  the identity. This is not proof of multiple physical RTX cards.
- No driver, service, elevation, GPU device, window or stress workload is created
  by this provider. Windows APIs can still stall the background sampler; this
  pass does not prove a native interaction or shutdown latency budget.

## Read-only native evidence

Production adapter probe, September 6: six DXGI/counter union identities, four
with complete memory readings. RTX 5070 Ti dedicated use changed from
9,512,497,152 to 9,512,566,784 bytes; shared from 335,187,968 to 336,236,544.
Intel Graphics and Microsoft Basic Render Driver were identified. One PDH LUID
had no DXGI description; two additional same-name RTX DXGI identities had no
memory counters. No guessed device join or zero-filled missing values.

First inventory/query took 427.191 ms; subsequent collections took 0.195 and
0.122 ms. These are short debug-provider timings, not whole-app performance.
A preceding independent Get-Counter probe also returned four adapter-memory
identities and byte-scale values. Samples were not synchronized to Task Manager;
exact cross-monitor parity remains unchecked.

## Verification

Ordinary regressions cover hexadecimal identity normalization, distinct nodes and
same-kind engines, partial coverage, retained/zero/missing memory, removed adapter
expiry, source-aware JSON, timestamp/gap isolation, reordered selection and compact
dark/light field geometry. The offscreen review has wide, compact, missing-light
and cached variants. Exact final results and EXE identity: `CURRENT_STATE.md`.

```powershell
cargo test --offline
cargo test --offline native_gpu_adapter_memory_read_only_probe -- --ignored --nocapture
cargo test --offline render_gpu_adapter_visual_pass -- --ignored --nocapture
```

The latter two tests are individually opt-in. Never run all ignored tests.
No native window movement, focus stealing or global input belongs in this pass.
