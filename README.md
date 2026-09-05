# Trontop

Trontop is a fast, native Windows process and performance manager. It is the tool I
want open when Windows Task Manager has become part of the problem.

Version 0.2 provides:

- live process CPU, GPU, memory, state, user, command, path, and disk I/O telemetry
- a searchable and sortable process table
- a persistent process inspector
- guarded process termination with explicit confirmation
- CPU, memory, disk, network, and GPU performance drill-downs
- History, Startup, Users, Details, and Services pages backed by native data
- a frameless Tront shell and persistent live gradient Theme Studio
- a live CPU tray meter with CPU, memory, GPU, and process-count tooltip
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

Near-term work is tracked in `TASK_BOARD.md`. The next systems slice is true process
trees, priority and affinity controls, suspend/resume, and writable service/startup
controls. Trontop should remain a focused standalone app, not a general system-utility
suite.
