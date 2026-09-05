# Executable icons

Alpha.9 adds real embedded executable artwork to Processes, Details and the selected
process inspector. Navigation and fallback symbols remain original code-drawn vectors.
No emoji, icon font, downloaded icon service or runtime asset directory is required.

## UI and cache

The real app starts one `trontop-icons` worker. Headless `with_services` fixtures do
not start it, even when their synthetic rows contain executable paths.

- The UI performs only lexical path filtering, cache lookup and bounded texture
  uploads. It never stats, canonicalizes or opens executable files.
- At most 256 cached entries, including pending requests and negative results;
  at most 32 outstanding requests/results; at most eight uploads per UI frame.
  Each result is a 32x32 straight-alpha RGBA image. Full cached pixel payload is
  about 1 MiB, excluding allocator, texture metadata and backend overhead.
- Requests deduplicate by lexical executable path. Completed entries not used in
  the current frame are evicted oldest-first. No current process list is scanned
  just to prefetch icons; visible rows and the inspector request what they draw.
- Unavailable/failed icons retry after 60 seconds. Successful artwork refreshes
  after ten minutes, only when requested again. A failed refresh retains prior art.
- Loading, unavailable and denied paths use the same fixed-size vector fallback.
  Arrival of an icon cannot move the adjacent name. Table icons select their row;
  inspector icons are hover-only decoration with an identity disclaimer.
- Closing the cache signals stop and drops channels without joining the worker.
  A stuck native call keeps its one worker and owned resources, not replacement
  threads. This isolates icon loading, not an end-to-end native close benchmark.

## Native reads and scope

Only absolute drive-letter `.exe` paths are considered. The native worker checks
that the drive is fixed, then checks each ancestor before descending. Reparse,
offline and recall-marked components, inaccessible files, directory targets, UNC,
mapped network drives, removable media, relative paths, device paths and alternate
streams fall back without extraction. This gate is best-effort, not a security
boundary against concurrent path changes.

`ExtractIconExW` reads embedded resources; Trontop does not execute the target,
load its application DLLs, invoke shell association handlers or open Explorer.
The icon is drawn into two private DIBs' background states using one owned surface.
Black/white compositing recovers transparency without trusting legacy GDI alpha
bytes. Every HICON, bitmap and DC is RAII-owned; the previous selected object is
restored before deletion. GDI is flushed before CPU reads of DIB memory.

Icons do not establish process identity, publisher trust or signature validity.
PID/creation-time action validation remains separate. An updated file can leave
old decorative art in the cache. Case aliases can occupy separate bounded entries.
UWP-specific artwork, shell-associated icons, junction-backed app installs and
remote/removable applications are not covered by this first provider. Legacy XOR
inversion is approximated as opaque artwork because an RGBA texture cannot invert
arbitrary backgrounds.

## Verification

Unit tests cover lexical rejection, ancestor short-circuiting, negative caching,
duplicate requests, refresh retention, queue and cache limits, upload budgets,
eviction, disconnected workers and nonblocking drop under a stuck mock loader.
Pixel tests preserve opaque black, transparent and antialiased colored pixels.
The native path-gate tests use injected callbacks, not real network/cloud paths.

Headless production-UI tests require identical process-name geometry before/after
icon arrival in dark/light modes and verify local icon clicks select the right row.
Three extra offscreen fixtures cover mixed loaded/fallback rows and the inspector.
Their colored sample icons are synthetic test-only art, not bundled application art.

Explicit read-only native probe, never a blanket ignored-test run:

```powershell
cargo test native_executable_icon_read_only_probe --offline -- --ignored --nocapture
```

It extracts this test EXE's embedded icon only, writes a review PNG under
`target/ui-smoke`, then performs 40 repeat extractions and compares GDI/USER resource
counts. It creates no windows and sends no OS input. On 2026-09-05 the final run
passed: first extraction 2.4387 ms, resource counts (4, 2) to (4, 2). The initial run
was 2.9378 ms with identical counts. This is a small
read-only resource-lifetime check, not a guarantee for every executable or a measure
of sustained app overhead. See `CURRENT_STATE.md` for the final suite/build gate.

## Primary API references

- [ExtractIconExW and HICON ownership](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-extracticonexw)
- [DrawIconEx alpha and mask behavior](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-drawiconex)
- [DIB section lifetime and GDI synchronization](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/nf-wingdi-createdibsection)
- [Drive types](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getdrivetypew)
- [File attributes](https://learn.microsoft.com/en-us/windows/win32/fileio/file-attribute-constants)
