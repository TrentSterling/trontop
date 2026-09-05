# Alpha.4 verification checkpoint

Verified September 5, 2026. This is a private development checkpoint, not a tagged
release or a claim that the broader release plan is complete.

## Exact code and remote gate

- Repository: `TrentSterling/trontop`, confirmed PRIVATE after the push.
- Source: `f2f725c95c77cdd1394e508a55659984142923f2`.
- [Windows CI run 33946298909](https://github.com/TrentSterling/trontop/actions/runs/33946298909): success.
- Windows job took 20m38s on the clean runner, using pinned Rust 1.93.0.
- Formatting PASS; tests **35 passed, 0 failed, 3 ignored**; strict Clippy PASS;
  optimized release build PASS; portable artifact upload PASS.
- Ignored tests were not run remotely. Native sensor probing and offscreen rendering
  have separate local evidence below. Interactive tray testing remains opt-in and
  was not performed in this continuation.

The documentation-only follow-up records this exact source result without changing
Rust source, manifest, lockfile, build script, compiler flags or CI workflow. It does
not represent a second independently built binary.

## CI executable

[Private artifact 9963697811](https://github.com/TrentSterling/trontop/actions/runs/33946298909/artifacts/9963697811)
is named `trontop-windows-f2f725c95c77cdd1394e508a55659984142923f2`.
The workflow retains it for 14 days, not as a permanent release download.

Downloaded for inspection, not executed:
`target/ci-artifacts/33946298909/trontop.exe`.

- Size: 12,790,272 bytes.
- PE product version: `0.3.0-alpha.4`; company: `Tront`.
- SHA-256: `4BF2BB4166234315EE28D9B57B1E4C8B2FC05E15ECF4B5AF502650DC314C624E`.
- Authenticode: `NotSigned`.
- `dumpbin /dependents`: Windows imports only; no NVML or dynamic MSVC runtime DLL.

This is not the same hash as the local review build. No cross-toolchain or
cross-machine bit-for-bit reproducibility claim is made.

## Local evidence

- The local optimized review binary is `target/review-build/release/trontop.exe`,
  12,784,128 bytes, version `0.3.0-alpha.4`, unsigned.
- SHA-256: `D4FCD918DBC411A2967A3088F53023F3418F8FA808CFFEC205D1FAB91A34CCC3`.
- Formatting, 35 tests, strict Clippy and optimized build passed locally.
- Explicit offscreen GPU pass: 17 PNGs, no native window or OS input. Confirmation
  button alignment and disabled-state appearance were visually reviewed after the fix.
- Explicit native read-only NVML probe: passed, five samples. Initialization 46.298 ms,
  first query 24.980 ms, warm queries 0.354/0.345/0.327/0.346 ms. These are short probe
  timings, not app CPU, drag or sustained-performance measurements.
- Process-control mutation tests exclusively used their own hidden disposable children.
- The old running release-path PID 62220 was inspected read-only and left untouched.

## Open gates

No tagged alpha or public release was published. Clean-machine launch, administrator
action paths, mixed-load/workday soak and the remaining alpha controls/diagnostics/
export features are still open. Do not run desktop automation or restart Trent's
working app without fresh permission. See `RELEASE_PLAN.md` and `TASK_BOARD.md`.

CI also warned that the v4 checkout/upload actions target deprecated Node 20 and
were forced onto Node 24 by the runner. The run still passed; updating those actions
and improving cold-build cache performance are separate workflow work.
