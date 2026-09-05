# Trontop

Trontop is a fast, native Windows process and performance manager. It is the tool I
want open when Windows Task Manager has become part of the problem.

The first milestone provides:

- live process CPU, memory, state, executable path, and disk I/O telemetry
- a searchable and sortable process table
- a persistent process inspector
- guarded process termination with explicit confirmation
- CPU and memory history sampled on a background thread
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

Near-term work includes Windows GPU Engine counters through PDH, per-process network
rates, service and startup control, process trees, priority and affinity controls,
and richer performance drill-downs. Trontop should remain a focused standalone app,
not a general system-utility suite.

