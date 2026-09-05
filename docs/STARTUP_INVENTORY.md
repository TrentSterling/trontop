# Stable inventory snapshots

Alpha.10 separates Startup source completeness from the rows currently displayed.
A failed read cannot prove that a previously observed entry has been removed.
Services similarly exposes when its retained list is no longer a live result.

## Startup cache

`src/startup.rs` owns five independent sources: current-user Run, machine Run,
32-bit machine Run, current-user Startup folder and machine Startup folder.
`src/windows_metrics/startup.rs` supplies read-only results. The sampler applies
them to an immutable UI snapshot; native collection remains outside rendering.

| Read result | Cache update | Visible result |
| --- | --- | --- |
| Complete readable source | Replace that source, including confirmed removals | Live or Empty |
| Confirmed missing key/folder | Clear that source | Absent |
| Failed read with some observed entries | Merge observations by identity, retain unseen entries | Partial; individual rows Observed or Cached |
| Failed read without observations | Preserve prior entries and timestamps | Cached, or Unavailable if nothing was ever known |
| No attempt yet | Keep the source field present | Starting |
| Last attempt older than 75 seconds | Preserve data, do not imply freshness | Cached or Unavailable |

Last-complete timestamps advance only for complete or confirmed-absent results.
Entry timestamps advance only when the entry was actually observed. An explicit
per-attempt flag distinguishes observations even if two attempts share a timestamp.
If a provider omits a source result, it is treated as failed, not removed.

Registry identities use the value name. Startup-folder identities use the full
filename, not the display stem, so similarly named `.lnk` and `.cmd` files remain
distinct. Sources are separate namespaces. This is inventory only: it does not
resolve shortcuts, launch commands, infer enabled/disabled state, enumerate every
Windows autostart mechanism, or persist a new startup configuration.

Each source retains at most 4,096 entries and 2 MiB of row string bytes (key, name,
command). This text bound excludes allocator/metadata and temporary merge/index
storage. Native reads enforce the same budgets; defensive cache checks downgrade
over-limit results to incomplete before they can erase old entries. A visible
retention warning remains until a complete read succeeds.

The sampler builds globally alphabetized source/entry indices only on inventory
refresh. UI filtering scans references; table virtualization formats strings only
for visible rows. This avoids per-frame sorting/full-table formatting, not all
per-frame work. Startup and service native inventory calls still share the sampler;
isolating potentially slow registry/folder/SCM calls is future work.

## Services and layout

The existing service cache keeps the last complete list after a failed read.
`src/app/inventory.rs` now labels that list and its rows Cached, explaining that
reported service states may have changed. An unavailable initial read is not an
empty successful inventory and does not imply that services are stopped.

Both pages use fixed-height hover status surfaces and four-column zebra tables.
Startup source fields exist from the first frame. Failures and recovery change
content/state, not the positions of the table headers. Long values truncate with
hover detail; ordinary labels remain non-selectable. No native service actions,
clipboard writes, keyboard hooks or desktop automation are added here.

## Verification

The full local alpha.10 gate passed 84 tests (six opt-in ignored), strict Clippy,
formatting and the optimized release build. Cache tests cover partial merging,
independent recovery/removal, known-empty versus unavailable, same-name identity,
same-time attempts, row/text bounds and refreshed alphabetical order.

Thirty-two synthetic inventory page/theme/state cases check stable field/header
geometry. A 20,000-entry table fixture formats fewer than 100 rows in one viewport.
The separate GPU offscreen harness generates 42 PNGs total, including six new
Startup/Services variants; selected images were visually inspected. Fixtures never
become fake runtime telemetry. See `HEADLESS_QA.md` and `CURRENT_STATE.md` for commands,
build identity and review scope. No real drag/close latency claim follows from these
tests, and no app window was opened for this slice.
