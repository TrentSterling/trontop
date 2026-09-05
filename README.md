# Trontop

Trontop is a fast, native Windows process and performance manager. It is the tool I
want open when Windows Task Manager has become part of the problem.

Version 0.3 alpha provides:

- live process CPU, GPU, memory, state, user, command, path, and disk I/O telemetry
- searchable process-tree and flat-list views with subtree resource totals,
  displayed-total sorting, iterative deep-tree handling and creation-time-aware links
- snapshot-indexed process tables and cached History ordering, avoiding full-record
  copies on repaint; synthetic before/after timings: `docs/PROCESS_VIEW_PERFORMANCE.md`
- a persistent process inspector
- current Windows priority and CPU affinity telemetry, plus confirmed scheduling controls
- guarded process termination with explicit confirmation
- native creation-time validation on the same action handle for termination, priority,
  and affinity; reused PIDs and Windows-critical processes are refused
- CPU, memory, disk, network, and GPU performance drill-downs
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
The next systems work includes recoverable suspend/resume,
startup controls and isolated service-command validation. Trontop remains a development
preview without full Task Manager parity. It should be a focused standalone app, not
a general system-utility suite.
