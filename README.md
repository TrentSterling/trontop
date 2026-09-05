# Trontop

Trontop is a fast, native Windows process and performance manager. It is the tool I
want open when Windows Task Manager has become part of the problem.

Version 0.3 alpha provides:

- live process CPU, GPU, memory, state, user, command, path, and disk I/O telemetry
- searchable process-tree and flat-list views with subtree resource totals
- a persistent process inspector
- current Windows priority and CPU affinity telemetry, plus confirmed scheduling controls
- guarded process termination with explicit confirmation
- CPU, memory, disk, network, and GPU performance drill-downs
- History, Startup, Users, Details, and Services pages backed by native data
- a frameless Tront shell and persistent live gradient Theme Studio
- theme-derived alternating table rows and high-contrast dual-signal selection states
- a live CPU tray meter with CPU, memory, GPU, and process-count tooltip
- an embedded multi-resolution Trontop icon and Windows version metadata
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

Near-term work is tracked in `TASK_BOARD.md`; product gates and publishing stages are
defined in `RELEASE_PLAN.md`. The next systems work is recoverable suspend/resume and
writable service/startup controls. Trontop should remain a focused standalone app, not
a general system-utility suite.
