# CODEX.md: Trontop handoff

Trontop is an active hand-driven Codex project. The overnight coordination loop
must not modify this repository.

## Current checkpoint

Version 0.2.0 expands the first slice into a seven-page native system control deck.
Read `docs/CURRENT_STATE.md` for the full handoff and `TASK_BOARD.md` before choosing
new work. The next high-value systems slice is process trees, priority, affinity,
suspend/resume, and service controls.

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
