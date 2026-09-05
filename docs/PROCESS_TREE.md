# Process hierarchy robustness and scaling (alpha.15)

Historical alpha.15 measurements and implementation notes follow. Alpha.16 changes
resource sorting to the displayed subtree values and adds observed-identity expansion
retention; see `PROCESS_SORTING.md`. The measurements below were not repeated for
alpha.16 and are not claims about its native window performance.

## Display contract

The tree is a snapshot-scoped forest of indices, not an authority for native actions.
The sampler's parent IDs and raw process records remain untouched. Selection and
End Task/priority/affinity identity checks are unchanged. Export still writes the
source records, not a rewritten hierarchy.

- Resolve each PID once. Real snapshots contain unique PIDs; malformed duplicates
  consistently use the last observation once for tree links and resource totals.
- Missing parents and self-parent links become roots.
- Reject a reported parent known to have been created after its child. Compare
  nonzero exact Windows creation identities when both exist, otherwise nonzero
  Unix-second start times. Equal or unknown times retain the reported link; these
  cases do not prove parent identity.
- Remove every edge inside a parent cycle, making each cycle member a root.
  Incoming non-cycle children stay attached. No arbitrary cycle member owns the
  resources of all the others. Original parent IDs remain available on name hover.
- Search includes matching processes and their valid ancestors, revealing context
  without modifying the user's stored expansion set. Unknown search PIDs are ignored.
- Totals cover the full valid subtree even when collapse/search hides descendants.
  Child addition keeps source order, preserving floating-point and incomplete-GPU
  semantics. Sibling/root sort policy is unchanged.

Windows explicitly warns that parent PIDs can refer to an unrelated process after
reuse and recommends checking creation dates. The timestamp rule follows that
guidance; it is not a new native query or a claim to recover an exited parent.
[Microsoft Win32_Process parent-ID guidance](https://learn.microsoft.com/en-us/windows/win32/cimwin32prov/win32-process#properties).

## Implementation

The former implementation walked the same ancestors for every match and used three
recursive traversals. A chain had quadratic ancestor work even when collapsed.
The replacement uses indexed parent/child arrays, a linear cycle walk, iterative
parent-before-child order reversed for totals, one-time ancestor inclusion, and an
explicit display stack. Hierarchy depth no longer consumes a proportional call stack.
Expected hierarchy bookkeeping is O(n), plus display sorting O(n log n) worst case;
temporary storage is O(n). Name comparisons still cost the compared string length.
No whole process/string records are cloned by the tree builder.

The existing-app redesign skill's layout/state audit also identified that unbounded
indentation could leave only blank names or ellipses at deep levels. Indentation now
reserves room for controls, the icon and name, and caps at six visual steps. The full
depth and reported parent PID appear on name hover; compressed indentation is labeled
there. The model keeps the actual depth and all rows. Existing zebra backgrounds,
rounded controls, gradients, text selection policy and numeric alignment are retained.
This is a targeted readability change, not an app-wide redesign/accessibility audit.

## Reproducible measurement

```powershell
cargo test --offline --release process_tree_timing_probe -- --ignored --nocapture --test-threads=1
```

Run only this selected probe, not all ignored native tests. It generates synthetic
wide, balanced and chain hierarchies at 500/1,000 rows with all matches collapsed,
all expanded, or one leaf searched. It measures only tree building, not snapshot
collection, view search-string folding, egui rendering, GPU present, native dragging
or whole-app FPS. Three warmups precede twenty timed builds per combination.
For the A/B probe only, both implementations run on an explicit 8 MiB test thread
stack to accommodate the old recursive code. No native processes are spawned by the
fixture, and there is no window, tray, provider or OS input.

On 2026-09-05, three sequential before/after pairs were run after compilation and the
ordinary suite finished. Other user workloads were not controlled. Values below are
the median of three per-run medians in microseconds, not a universal speed guarantee.

| Processes | Shape | View | Alpha.14 us | Alpha.15 us |
|---:|---|---|---:|---:|
| 500 | wide | collapsed | 154.0 | 53.6 |
| 500 | wide | expanded | 184.6 | 71.8 |
| 500 | wide | leaf-search | 69.3 | 46.3 |
| 500 | balanced | collapsed | 324.2 | 60.0 |
| 500 | balanced | expanded | 350.6 | 79.8 |
| 500 | balanced | leaf-search | 101.1 | 54.5 |
| 500 | chain | collapsed | 4368.4 | 67.5 |
| 500 | chain | expanded | 4357.1 | 80.9 |
| 500 | chain | leaf-search | 254.8 | 76.4 |
| 1,000 | wide | collapsed | 299.7 | 107.9 |
| 1,000 | wide | expanded | 364.9 | 141.2 |
| 1,000 | wide | leaf-search | 149.5 | 88.9 |
| 1,000 | balanced | collapsed | 645.6 | 117.0 |
| 1,000 | balanced | expanded | 739.5 | 143.9 |
| 1,000 | balanced | leaf-search | 198.7 | 100.2 |
| 1,000 | chain | collapsed | 16771.9 | 132.8 |
| 1,000 | chain | expanded | 16958.6 | 159.5 |
| 1,000 | chain | leaf-search | 534.2 | 150.6 |

The 1,000-node collapsed-chain medians ranged 16,707.4-17,086.9 us before and
125.6-143.3 us after. Its representative ratio is about 126x for this adversarial
shape. The 1,000-node wide expanded case is about 2.6x. Neither is a real desktop
drag speedup claim.

Baseline: 5b000f4 alpha.14 plus the identical timing probe, retained at
`target/perf-baseline/alpha14/tree-timing.exe`.
SHA-256 `0790CDA9E6630BB526E6D016D9E3E4D22B2CBC8CAB14FEA19341B4C00E95173D`.
Candidate test EXE: `target/release/deps/trontop-108603f402ed02a4.exe`.
SHA-256 `61984491A32F2E8B7B28688FAD90B8D1C2025413B36CF4833FF59F1DA398F156`.
These are test binaries, not the review app. The pairs used owned Process handles
with redirected stdout/stderr and hidden subprocess startup, avoiding PowerShell's
GUI-test-EXE stdout closure issue. Each test exited successfully before the next ran.

## Verification

Nine new ordinary tests cover:

- 50,000-level chain collapse, full expansion, leaf search and a 50,000-member
  cycle on a **256 KiB** test-thread stack, including resource/count assertions.
- Cycle roots with retained valid branches, source-order reversal and filtered totals.
- Exact reused-parent identities, rounded/unknown timestamp fallback, duplicate PIDs
  and missing/unknown matches.
- 128 deterministic arbitrary parent graphs, requiring unique rows and conserved
  root CPU/memory/process totals.
- 2,880 small valid-forest cases against a separate recursive reference, covering
  all ten sort columns, both directions, match/expansion subsets, source permutations,
  complete/partial/missing GPU values and every aggregate field.
- Production-UI local scroll, full depth tooltip and name-click selection at
  1040x640 and 1280x760 in light/dark. Deep names stay visible and single-line.

Final local gate: **134 passed, 0 failed, 9 opt-in ignored** (25.85 s), formatting
and strict Clippy PASS, optimized review build PASS (32.32 s). Offscreen pass produced
55 PNGs (34.14 s). The two deep-tree variants, normal Processes and the partial-GPU
light tree were actually viewed. This does not claim review of all 55 images.

Review app: `target/review-build/release/trontop.exe`, alpha.15, 13,212,160 bytes.
SHA-256 `8087381610511F7A219182CC5E1308A182CF2EC36B621F29989FC605827CE7C5`.
Dependency inspection shows only Windows imports, not a dynamic MSVC runtime.
Clean-machine portability remains a separate release gate. No alpha.15 app was
launched and no existing window was touched.

## Still open

Native dragging/closing/frame rates, provider overhead/stalls, search metadata
caching, broader hardware coverage, controls and release gates remain open.
The displayed-resource sorting and expanded-PID retention issues identified in
alpha.15 are addressed by alpha.16 (`PROCESS_SORTING.md`), with explicit unknown
identity limitations. No full Task Manager parity is claimed.
