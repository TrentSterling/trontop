# Non-blocking settings (alpha.26)

Scope: A13/A21/A22 in `../ASK_LEDGER.md`. The pinned eframe 0.35 file store
joined save threads during autosave and destruction, loaded synchronously before
app creation, and truncated its destination before serialization. These were
verified code paths, not a measured explanation of Trent's native close delay.

## Application boundary

Eframe's `persistence` feature is disabled. The existing egui memory serialization
feature and pinned RON codec remain enabled explicitly; no framework fork or new
runtime component is involved. `cargo tree -e features -i eframe` confirms that
the blocking file store is not included. App-owned settings use one background
worker for reading, parsing, serialization and filesystem operations.

The main thread captures theme/library values and clones egui memory when needed.
Those are CPU operations, not free or hard-real-time work. The worker has bounded
request/result channels, one operation in flight, and a single latest desired
snapshot on the controller. Edits coalesce for 250 ms; close forces the latest
snapshot immediately. UI memory is captured on the existing 30-second cadence
when the app receives updates, on theme/library changes and on close. Idle frames
do not encode the theme library or serialize memory. Polling uses `try_recv` and
dispatch uses `try_send`. Failure retains pending changes until explicit Retry;
there is no automatic error loop or extra worker after a stall.

Readiness and save state use a fixed-height footer. During initial loading,
telemetry/navigation remain available but theme editing is disabled, preventing
temporary defaults from overwriting saved palettes. Once available, preferences
are restored once in `raw_input_hook`, before egui begins layout. Page/selection
belong to the app and are retained. Saved UI memory restores window/widget layout
and zoom; there can be a one-time appearance/layout change after a slow initial
read. Theme Studio remains opaque/readable during loading. Its Revert session
baseline is taken after load, not from temporary defaults.

## Files and migration

- Current file: `%LOCALAPPDATA%/Trontop/settings-v3.json`. Versioned envelope holds
  the existing theme/library keys and egui memory. The executable needs no asset
  directory; these are per-user preferences, not an executable dependency.
- If the current file is absent, read the existing `state-v2.ron` as a legacy
  key-value map. Current and legacy theme encodings are supported. Preserve named
  palettes, egui memory and unknown keys within the stated limits. Migration is
  written only on a later save; the legacy file is never rewritten or deleted.
- Earlier previews continue to use the legacy file and cannot overwrite new
  settings. Among new instances, a non-waiting OS lock plus comparison against
  the originally loaded/last-saved bytes rejects conflicting writes. A conflict
  does not silently merge or replace another instance's choices. Keep the app
  open or export the desired themes before restarting; Retry does not override
  this conflict guard.
- Require a local absolute drive path. Reads are capped at 2 MiB, with at most
  64 entries, keys up to 256 bytes and values up to 1 MiB. Theme/library limits
  remain 16/128 KiB and twelve named palettes. The same limits apply to writes.
  Invalid/unknown/newer data, invalid UI scale, non-file targets and final-component
  Windows reparse points are refused without overwriting the original. Failure
  to load does not silently create a default replacement. Ancestor-directory races
  are not a security boundary against a malicious same-user actor.
- Writes exclusively create a sibling temporary file, write/flush it, then replace
  the destination by same-directory rename. The original is never truncated first.
  Ordinary errors clean only that operation's own temporary file. A tiny
  `settings-v3.lock` file coordinates cooperating instances; no elevation is used.
  Forced process termination can leave a temporary file. There is no wildcard
  cleanup and no claim of transaction durability through arbitrary power loss.

The file contains appearance choices, user-named palettes and egui's existing
widget-memory state, not system telemetry. No settings are uploaded or automatically
copied. Actual user settings were not read, migrated or modified during testing.

## Close behavior

Titlebar Close, tray Quit and OS close requests share the same app-owned gate.
If settings have loaded, capture the final state, queue it and cancel the native
close until success. The UI continues to process events; no settings thread is
joined. A successful save queues Close without further confirmation. The warning
dialog appears only after 250 ms or an error, avoiding a flash on quick saves.

Keep open cancels the pending close without losing changes. Retry save explicitly
retries the latest state. Close anyway explicitly accepts the risk of losing
pending theme/library/UI changes. It does not claim those changes were saved or
roll back a write already in progress. Worker cancellation is checked before
writing/replacement where possible; process exit can interrupt an in-flight call.
Closing while initial loading is stalled is immediate because theme/library
editing has not been allowed and no save was queued.

This removes settings-induced UI joins, not all possible native teardown delays.
GPU/window/tray destruction and driver calls still require the isolated native
close gate. The separate tray-construction ready wait remains open. No current
preview was launched, replaced, closed, moved or otherwise interacted with.

## Verification

Twelve new tests cover filesystem migration/round trip, untouched legacy data,
unknown keys, named palettes, egui memory/zoom, invalid/newer/oversized/read-only
files, lock contention, external conflicts, cancellation, output limits, bounded
coalescing, explicit retry, blocked read/write drop and disabled fixture mode.
Production UI tests exercise pre-pass restoration, dirty detection, preserved
navigation during loading, correct Revert baseline, synthetic OS close deferral,
success/Keep open/Retry/Close anyway, and compact dark/light status/control layouts
at 1x/1.5x/2x UI scale. Commands are inspected, never executed against a native
window. One app-to-worker-to-file-to-fresh-app test uses only an exclusively owned
fixture directory, not a real preview or user preferences.

```powershell
cargo test --offline preferences -- --nocapture --test-threads=1
cargo test --offline
cargo clippy --offline --all-targets -- -D warnings
cargo build --offline --release
cargo test --offline render_offscreen_visual_pass -- --ignored --nocapture --test-threads=1
```

The visual matrix has 101 cases, including eight new compact settings states.
The existing-app redesign skill's state audit caught the initially transparent
disabled editor and tight warning padding; the patch keeps the frame/explanation
readable and disables only the editing controls. Exact final counts, review scope,
build identity and outstanding release gates are recorded in `CURRENT_STATE.md`.
