# Trontop

**[Download for Windows](https://tront.xyz/trontop/#download)** | **[Theme gallery](https://tront.xyz/trontop/#themes)** | **[Launch post](https://tront.xyz/blog/posts/trontop/)**

![Trontop Overview with real telemetry](https://tront.xyz/trontop/media/electric-overview.png)

Source available under **Apache 2.0 + Commons Clause 1.0**. Free for personal
and workplace use. Attribution and license notices must be retained, and sales
are restricted as defined by [LICENSE](LICENSE). This is not an OSI open-source
license. Third-party components retain their own terms in
[THIRD_PARTY_NOTICES.txt](THIRD_PARTY_NOTICES.txt).

Current public preview: **0.3.0-alpha.41**, Windows 11 x64, unsigned. Read the
[release notes and known limitations](docs/RELEASE_ALPHA41.md) and
[privacy statement](PRIVACY.md). Screenshots use captured telemetry from Trent's PC.

Trontop is a fast, native Windows process and performance manager. It is the tool I
want open when Windows Task Manager has become part of the problem.

Local development is paused at verified **alpha.51**: 589 passing ordinary
tests and 93.34% production line coverage. [Resume anchor and remaining work](docs/RESUME_ANCHOR.md).

Version 0.3 alpha provides:

- live process CPU, GPU, memory, state, user, command, path, and disk I/O telemetry
- searchable process-tree and flat-list views with subtree resource totals,
  displayed-total sorting, iterative deep-tree handling and creation-time-aware links
- snapshot-indexed process tables and cached History ordering, avoiding full-record
  copies on repaint; synthetic before/after timings: `docs/PROCESS_VIEW_PERFORMANCE.md`
- a persistent process inspector
- current Windows priority and CPU affinity telemetry, plus confirmed scheduling controls
- guarded process termination with explicit confirmation
- local alpha.43: confirmed End process tree with a reviewed all-instances option,
  plus recoverable Suspend/Resume; [behavior and evidence](docs/PROCESS_CONTROLS.md)
- local alpha.44: Startup approval states, confirmed Enable/Disable, exact session
  Undo and background Refresh; [behavior and evidence](docs/STARTUP_CONTROLS.md)
- local alpha.45: verified suspension state and STATE sorting, readable compact
  status, and reliable gradient keyboard editing; twenty new interaction checks
  cover daily actions, Theme Studio and System [with evidence](docs/DAILY_CONTROLS_2026-09-30.md)
- local alpha.46: CPU core freshness/recovery, invalid-temperature gaps and guarded
  shared-memory reads; twelve new native/UI checks
  [with evidence](docs/SENSOR_GUARDS_2026-09-30.md)
- local alpha.47: truthful Overview sensor/VRAM states and graph tooltips across
  missing intervals; eleven new Overview/plot regressions
  [with evidence](docs/OVERVIEW_GRAPHS_2026-09-30.md)
- local alpha.48: System clock/commit/GPU freshness and complete private-summary
  masking across the page, Copy and JSON; nine new cross-consumer checks
  [with evidence](docs/SYSTEM_LIVE_2026-09-30.md)
- local alpha.49: CPU group-aware nominal references and truthful missing hardware
  facts, PCI fallback and validated BIOS values;
  [sixteen new checks and evidence](docs/HARDWARE_FALLBACKS_2026-09-30.md)
- local alpha.50: close to the live tray, Show/explicit Quit and failure recovery,
  plus truthful partial RAM/storage reports;
  [behavior and checks](docs/SMART_TRAY_2026-09-30.md)
- local alpha.51: identity-safe physical media/volume joins and explicit unknown
  partition fields; [twelve fixtures and checkpoint evidence](docs/STORAGE_PROVIDER_JOINS_2026-09-30.md)
- native creation-time validation on the same action handle for termination, priority,
  and affinity; reused PIDs and Windows-critical processes are refused
- CPU, memory, disk, network, and GPU performance drill-downs
- a graph-first dashboard with Lines/Bars, category filters and a shared two-minute
  timeline for usage, GPU temperature/power/clocks/fan/VRAM, drive temperatures,
  physical-disk activity and network traffic; details: `docs/GRAPH_WALL.md`
- independent physical-disk active time, response latency, queue depth and read/write
  throughput, with explicit cached/missing fields and gap-aware charts; mounted
  volumes stay separate: `docs/PHYSICAL_DISKS.md`
- optional NVIDIA temperature, board power, clocks, fan target, and VRAM sensors with
  two-minute temperature/power histories; missing sensors stay explicitly unavailable
- History, Startup, Users, Details, and Services pages backed by native data
- independent Startup/Services inventory workers, stable cached rows during slow
  or failed reads, and explicit source freshness instead of disappearing fields
- confirmed service Start/Stop/Restart on an independent worker, with state/PID
  preflight and explicit permission/uncertain-outcome errors; native command
  end-to-end validation remains a release gate in `docs/SERVICE_CONTROLS.md`
- per-service command-state retention across selections and subsequent commands;
  unresolved outcomes stay labeled until a newer read resolves them
- explicit JSON snapshot and CSV process export, with private details excluded by
  default, provider freshness, background writing and a native Save As picker;
  format/limitations and the remaining picker-validation gate: `docs/EXPORTS.md`
- a Speccy-class System page: OS, CPU (full CPUID brand, per-core-type caches,
  VT-x capability vs firmware vs hypervisor), per-DIMM RAM from SMBIOS, board and
  BIOS, graphics with 64-bit VRAM and monitors, storage with NVMe health, audio,
  peripherals, network and read-only sensor sources, live temperatures and clocks
  inline, copy and TXT/JSON save with private values hidden by default; no driver,
  elevation or guessed values: `docs/SYSTEM_SPECS.md`
- a frameless Tront shell and four-peg gradient Theme Studio: draggable stops,
  hex/position edits, eight presets, named saves, import/export, live surfaces and
  legacy-theme migration; remaining acceptance checks: `docs/THEME_STUDIO.md`
- background settings load/save, staged file replacement, retained palettes and
  explicit unsaved-close choices; per-user paths and limits: `docs/SETTINGS_PERSISTENCE.md`
- alternating row and gradient column bands, padded cells, and high-contrast selection
- a live full-width CPU tray meter with scrolling history and a resource tooltip,
  updated on its own native thread even when the main window is hidden
- an embedded multi-resolution Trontop icon and Windows version metadata
- bounded local Rust-panic/native-runner failure records, with raw/private payloads
  excluded and no upload; coverage and limits: `docs/FAILURE_REPORTS.md`
- an event-driven egui UI that does not poll the operating system on the render thread

## Run

```powershell
cargo run --release
```

## Build the portable executable

```powershell
cargo build --release
```

The resulting `target/release/trontop.exe` has no external application assets. The
MSVC C runtime is statically linked by `.cargo/config.toml`.

## Direction

Product completion is tracked in [ASK_LEDGER.md](ASK_LEDGER.md); historical engineering
detail is in `TASK_BOARD.md`, and publishing procedures in `RELEASE_PLAN.md`.
The next systems work includes reversible startup controls and isolated
service-command validation. Trontop remains a development
preview without full Task Manager parity. It should be a focused standalone app, not
a general system-utility suite.

## Public media automation

The product screenshots, downloadable theme gallery and social preview image
are reproducible. See [the media workflow](docs/MARKETING.md).
