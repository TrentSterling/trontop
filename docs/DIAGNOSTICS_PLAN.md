# Next slice: provider health and support reports

This is a code-audit plan, not an implemented feature or a reproduced failure on
Trent's machine. It follows the alpha.4 process-action checkpoint.

## Evidence to address

- `src/sampler.rs`: initial service enumeration uses `unwrap_or_default`, so an
  access/provider failure becomes an empty inventory. Later failures retain the
  previous successful rows without recording freshness or the new error.
- Startup enumeration currently returns rows only, not per-source errors. An empty
  Run key, unreadable key and unavailable Startup folder need distinct diagnostics.
- `ProcessControlInfo::default` represents unavailable inspection, but its native
  error is discarded. Aggregate diagnostic counts would explain coverage without
  retaining every process name or command line in a support report.
- A missing process-user lookup is currently labeled `System`. It should render an
  explicit unknown account instead of attributing inaccessible data to that account.
- CPU frequency is refreshed at sampler initialization only. The current speed
  display needs either periodic refresh or honest static labeling.
- GPU Engine and NVML already carry provider errors. Preserve their different
  scopes: aggregate PDH activity is not an NVML adapter's utilization.
- Per-process GPU defaults to zero when no entry is returned. Provider unavailable,
  counter warming and genuine zero activity need distinct presentation semantics.

## Bounded implementation

1. Model provider status as starting, live, stale or unavailable, with last success,
   last attempt, collection cost and a sanitized error. No additional UI-thread
   collection. Cache timestamps must describe the provider, not just the latest
   overall system snapshot.
2. Add an About/diagnostics dialog with version, build identity, provider rows and
   explicit copy support-report action. Do not automatically copy or upload anything.
3. Keep the default report free of process command lines, executable paths, user and
   host names, GPU UUIDs and adapter addresses. A full snapshot export is a separate,
   explicitly requested operation with a privacy warning.
4. Tests inject missing/failed/recovered providers, verify stale cache labeling and
   report redaction, and render light/dark/compact fixtures offscreen. No desktop
   input, focus changes, native window movement or production process mutations.

Do not expand this slice into service mutations, new hardware drivers or desktop
soak automation. Those have separate safety and release gates. Update the task board
with exact verification evidence when this work actually lands.
