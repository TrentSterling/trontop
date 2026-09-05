# CODEX.md: Trontop handoff

Trontop is an active hand-driven Codex project. The overnight coordination loop
must not modify this repository.

## Current checkpoint

Version 0.1.0 is the first native vertical slice. It has a live process table,
search and sorting, process inspection, confirmed process termination, CPU and memory
history, background sampling, and a standalone release executable.

The next substantial slice is native Windows GPU telemetry. Read
`docs/TELEMETRY.md` before implementing it. GPU Engine counter instances must be
enumerated through PDH and aggregated by PID. Do not synthesize counter paths or
display estimated GPU values.

## Verification

```powershell
cargo fmt --all --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```

