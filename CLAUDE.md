# Trontop

Trontop is Trent's native Windows process and performance manager, implemented in
Rust with eframe/egui.

## Product constraints

- Keep telemetry collection off the UI thread.
- Keep the UI event-driven. A new system sample may request one repaint.
- Never terminate a process without an explicit confirmation step.
- Do not report fake or estimated GPU values. Use Windows GPU Engine PDH counters.
- Preserve the dense charcoal and copper visual language.
- Keep the release executable portable and free of runtime asset dependencies.

## Verification

Run these before claiming a change works:

```powershell
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```

