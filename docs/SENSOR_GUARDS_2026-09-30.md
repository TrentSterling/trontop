# Alpha.46 sensor freshness and native reader checks

Trent requested continued complete coverage. This pass advances A07/A10/A16/A25
through the existing sensor bridge and sensor pages. End process tree, all-instance
termination and Startup controls from the preceding candidates remain included.

CPU core chips now use the same live resolver as the System headlines. A bridge
poll older than 15 seconds shows `--` with the reason on hover, retains core-index
ordering, and recovers when a fresh poll arrives. Previously the chips formatted
retained readings directly, so they could keep showing temperatures beside a
stopped-provider status. An ordinary production-UI regression reproduced that
failure before the fix.

Invalid native readings are discarded before provider counts are published.
Temperature validity includes the f32 representation used for coloring: a finite
f64 value that overflows that representation is unavailable. A production-UI
regression reproduced `inf` from f64::MAX before this guard. Core and package gaps
recover on a subsequent valid reading. Other units retain their f64 representation.

The HWiNFO reader checks the queried region's size, committed state and readable,
unguarded protection before copying. It copies only the first readable region,
capped at 16 MiB, releases the view and handle, then parses its owned bytes. It
does not form a Rust reference to memory written by the publisher. The production
name remains `Global\HWiNFO_SENS_SM2`; tests use exclusively owned, uniquely named
`Local\Trontop.Test.Hwinfo.*` mappings.

Windows permits reserved shared mappings whose pages have not been committed.
[CreateFileMappingW](https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-createfilemappingw)
documents SEC_RESERVE, commitment and named-object collisions.
[VirtualQuery](https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-virtualquery)
reports the first region sharing state and protection. The owned reserved fixture
checks MEM_RESERVE before invoking the guarded reader; the partial-commit fixture
puts a table beyond the readable region and verifies refusal. The old unguarded
reader was not deliberately invoked on uncommitted memory.

Eight ordinary native checks cover real read-only mapping reads across publisher
updates, retained-copy independence, handle/view release, inactive signatures,
malformed and out-of-view tables, the copy cap, missing mappings, a collision with
an owned event, invalid numeric readings, reserved and partially committed pages.
The WMI row conversion check covers hardware-name resolution, parent fallback and
incomplete rows through the production converter; it does not claim a live query
to an installed LibreHardwareMonitor or OpenHardwareMonitor provider.

Four ordinary UI checks cover expiry/recovery, invalid core and package values,
unavailable-provider retention, and cached multi-adapter GPU views on both Sensors
and Performance. GPU history includes a missing sample and earlier peaks of
71 degrees C and 91 W. Cached status, missing adapter fields and adapter errors
stay explicit. These tests run the production UI without native workers or windows.

```powershell
cargo test --offline specs::bridge -- --nocapture
cargo test --offline app::ui_smoke::sensors -- --nocapture
cargo test --offline app::ui_smoke::sensors::render_sensor_freshness_review -- --ignored --exact --nocapture
cargo fmt --check
cargo test --offline -- --quiet --test-threads=4
cargo clippy --offline --all-targets -- -D warnings
cargo build --offline --release
./scripts/measure-coverage.ps1 -ReadOnlyProbes -TestThreads 4
```

The ordinary suite passes **508 tests, 0 failures, 55 ignored**. Formatting,
strict all-target Clippy and the optimized release build pass. The selected
offscreen check passes and its three final PNGs were inspected:
`target/ui-smoke/sensor-state-alpha46/fresh-dark.png`, `stopped-dark.png`, and
`stopped-light.png`. They use synthetic fixtures for review. Final coverage and
source identity are recorded in `COVERAGE.md` and `CURRENT_STATE.md`.

The local portable candidate is
`target/review/alpha46-sensor-guards/trontop.exe`, version `0.3.0-alpha.46`,
**17,790,464 bytes**, SHA-256
`F17472411104195BC53F56716005AA9D685BAFD073E1DB7969105E18A17FD832`.
The review copy matches the optimized build. It was not launched or installed.

D01 remains open: no real CPU/board sensor provider was installed or started.
The current machine's absent-provider probe does not establish live provider
acceptance. Optional-provider/native failures, broader Windows acceptance and
the exact-build release checks remain separate work in `ASK_LEDGER.md`.
