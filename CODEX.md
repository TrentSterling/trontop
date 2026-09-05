# CODEX.md: Trontop handoff

Trontop is an active hand-driven Codex project. The overnight coordination loop
must not modify this repository.

## Current checkpoint

Version 0.3.0-alpha.1 builds on the seven-page native system control deck with a real
process hierarchy, guarded scheduler controls, theme-derived zebra tables, and an
embedded multi-resolution Windows icon and version resource.
Read `docs/CURRENT_STATE.md` for the full handoff and `TASK_BOARD.md` before choosing
new work. The next high-value systems slice is recoverable suspend/resume and service
controls. The private GitHub repository is not connected because the saved GitHub CLI
token is invalid; reauthenticate before creating `TrentSterling/trontop`.

GPU Engine instances are enumerated through PDH and aggregated by PID. Do not
synthesize counter paths or display estimated GPU values. All slow inventory and
live telemetry remain outside the render thread.

## Verification

```powershell
cargo fmt --all --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```
