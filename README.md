# Trontop

**[Download for Windows](https://tront.xyz/trontop/#download)** | **[Theme gallery](https://tront.xyz/trontop/#themes)** | **[Launch post](https://tront.xyz/blog/posts/trontop/)**

![Trontop Overview with real telemetry](https://tront.xyz/trontop/media/electric-overview.png)

Source available under **Apache 2.0 + Commons Clause 1.0**. Free for personal
and workplace use. Attribution and license notices must be retained, and sales
are restricted as defined by [LICENSE](LICENSE). This is not an OSI open-source
license. Third-party components retain their own terms in
[THIRD_PARTY_NOTICES.txt](THIRD_PARTY_NOTICES.txt).

Current public preview: **0.3.0-alpha.41**, Windows 11 x64, unsigned. Read the
[release notes and known limitations](docs/RELEASE_ALPHA41.md) and
[privacy statement](PRIVACY.md). Screenshots use captured telemetry from Trent's PC.

Trontop is a fast, native Windows process and performance manager. It is the tool I
want open when Windows Task Manager has become part of the problem.

The local **alpha.57** build adds desktop app and developer cache cleanup and
opt-in sleep rules for minimized processes to the portable Trontop executable.

| Tool | What you can do |
| --- | --- |
| SpaceView | Select a mounted volume or enter a folder. Explore the size map, the 50 largest files, totals by extension, and content-verified duplicates of 1 MiB or larger. Scan the same folder again to see which entries grew or shrank. |
| Build caches | Locate Unity `Library`, Rust `target` and Node `node_modules` under projects with matching markers. Select a cache to map its contents. |
| Cleaner | Preview temporary files, crash dumps, browser and desktop app caches, developer downloads and diagnostic logs. Select by category or file and review the exact paths before permanent deletion. |
| Installed Apps | Search desktop registrations by app or publisher, sort by reported size, copy the filtered list, and map the install folder. Review the registered uninstall, MSI repair or vendor modify action. List current-user Windows packages separately and remove eligible packages. |
| Maintenance | Check reclaimable space and Recycle Bin totals, update a WinGet package by exact ID, audit browser extensions, archive stale App Paths, manage restore points, and configure scheduled cleanup. See the [Windows maintenance features](#windows-maintenance-features). |
| Drive watch | See available space, open a volume in SpaceView, and review the last 128 mount or removal observations from this session. |
| File users | Enter a file path or select a file in SpaceView. Windows Restart Manager lists the applications it can identify. Open a matching live process in the Trontop inspector. |

Cleaner checks the standard local locations. Temporary files (top-level `.tmp`),
standalone Chrome, Edge, Brave and Vivaldi disk or compiled-code cache entries, Firefox
disk cache entries and NuGet HTTP cache files qualify after seven days. Application
crash dumps qualify after 30 days. Chromium scans cover Default and numbered
profiles. The scan skips packed browser databases, cookies, sign-ins and saved
sessions. Browser cache cleanup skips browsers observed running when the job
starts. Windows Disk Cleanup opens the system dialog for additional choices.

The seven-day cache rule also recognizes individually stored HTTP or compiled
code entries for VS Code, Code Insiders, VSCodium, Cursor, Discord (stable,
PTB and Canary), Slack, GitHub Desktop, Unity Hub, Obsidian, Signal, Notion,
Postman and Steam's web interface. Applying cleanup skips selected app caches
whose owner process was running at the start of the job.

The 30-day rules recognize dated npm and OBS diagnostic logs, built wheel
archives in pip's hashed wheel cache, and stable Windows runtime ZIPs in
Electron's download cache. The scheduler exposes desktop app caches, logs and
developer downloads as three additional opt-in categories. A schedule created
with alpha.56 keeps its original selection.

SpaceView reports logical file sizes. Links and junctions are skipped. Hard-linked
names can contribute multiple times to the size map and are excluded from the
Windows duplicate check. The build cache view is read-only. Scan results,
comparisons and mount events live in memory for the session.

The size map uses SpaceView's squarified layout. Build cache detection draws on
Tronthub's Unity project scanner. Drive watch adapts Device History's snapshot
comparison to mounted volumes.

### Windows maintenance features

| Page | Available actions |
| --- | --- |
| Health check | Count eligible cleaner files, desktop registrations, free space and Recycle Bin contents. Open the individual cleanup review or Windows Recycle Bin. |
| Schedule | Register, inspect or remove a current-user Windows task. Choose daily or Monday cleanup, local start time, minimum total MiB and categories. The task runs while signed in and writes aggregate results to the Trontop settings folder. Moving the EXE requires updating the schedule. |
| Updates and Drivers | List updates from the WinGet community source and review one exact package ID for interactive upgrade. Inventory signed-driver records, check for drivers offered by Windows Update, and open its optional-driver installation page. Store updates open in Microsoft Store. |
| Browsers | Inventory extension versions from Chromium manifests and Firefox profiles. Open each supported browser's own privacy or extension settings to manage cookies, history and add-ons. |
| Registry | Find stale current-user **App Paths** entries for missing executables on mounted fixed drives. Archive an individual registration with all its values, or restore it later. |
| Restore points | List restore points and remove an older point after review. Create points in System Protection or open the Windows recovery wizard. Windows manages access to these operations. |
| Drive tools | Run Windows Cipher on free NTFS space. A separate full-format option uses a single zero pass on an eligible data volume. It verifies volume identity and Windows roles, requires a typed confirmation, and opens the interactive Windows tool. SSD firmware can retain remapped blocks. |
| Performance | Filter by name or PID and rank by CPU or memory. Choose **Sleep when minimized** on a windowed process and confirm. Trontop pauses that process after all its visible windows stay minimized for 30 seconds. Restoring a window or choosing **Wake and remove rule** releases the pause. |
| Cloud storage | Open Google or Microsoft storage management in your browser. Explore a locally synced folder in SpaceView. Provider accounts and online files stay with the provider. |
| SpaceView duplicates | Review the exact file to remove and the copy to keep. Trontop reopens both, checks their identities, compares their full contents under lock, then deletes only the selected copy. |

Desktop installer actions use the standard-user session. Apps that expose a
shell wrapper or unsupported registration can be managed through **Windows
Installed Apps**. The Windows package list uses the OS Appx provider. WinGet
requires Microsoft's App Installer. Scheduled cleaning starts disabled until
you confirm a schedule. Cleanup rules cover the categories documented above.

Sleep rules apply to the selected process lifetime. They expire on process exit
or when Trontop closes. Background work in that process pauses along with the UI.
Trontop checks windows twice a second while rules are active. A process with no
observable window stays awake. The inspector also lets you remove a sleep rule.

The preceding **alpha.53** added **End all instances** as the primary
process action, a counted confirmation, and single-process/branch controls under
**More process actions**. The ordinary suite passes 622 tests.
[Process controls](docs/PROCESS_CONTROLS.md). The broader coverage and product
goal retains its [earlier resume scope](docs/RESUME_ANCHOR.md).

Version 0.3 alpha provides:

- live process CPU, GPU, memory, state, user, command, path, and disk I/O telemetry
- searchable process-tree and flat-list views with subtree resource totals,
  displayed-total sorting, iterative deep-tree handling and creation-time-aware links
- snapshot-indexed process tables and cached History ordering, avoiding full-record
  copies on repaint; synthetic before/after timings: `docs/PROCESS_VIEW_PERFORMANCE.md`
- a persistent process inspector
- current Windows priority and CPU affinity telemetry, plus confirmed scheduling controls
- guarded process termination with explicit confirmation
- local alpha.52: direct End all instances with confirmed single-process/branch alternatives,
  plus recoverable Suspend/Resume; [behavior and evidence](docs/PROCESS_CONTROLS.md)
- local alpha.44: Startup approval states, confirmed Enable/Disable, exact session
  Undo and background Refresh; [behavior and evidence](docs/STARTUP_CONTROLS.md)
- local alpha.45: verified suspension state and STATE sorting, readable compact
  status, and reliable gradient keyboard editing; twenty new interaction checks
  cover daily actions, Theme Studio and System [with evidence](docs/DAILY_CONTROLS_2026-09-30.md)
- local alpha.46: CPU core freshness/recovery, invalid-temperature gaps and guarded
  shared-memory reads; twelve new native/UI checks
  [with evidence](docs/SENSOR_GUARDS_2026-09-30.md)
- local alpha.47: truthful Overview sensor/VRAM states and graph tooltips across
  missing intervals; eleven new Overview/plot regressions
  [with evidence](docs/OVERVIEW_GRAPHS_2026-09-30.md)
- local alpha.48: System clock/commit/GPU freshness and complete private-summary
  masking across the page, Copy and JSON; nine new cross-consumer checks
  [with evidence](docs/SYSTEM_LIVE_2026-09-30.md)
- local alpha.49: CPU group-aware nominal references and truthful missing hardware
  facts, PCI fallback and validated BIOS values;
  [sixteen new checks and evidence](docs/HARDWARE_FALLBACKS_2026-09-30.md)
- local alpha.50: close to the live tray, Show/explicit Quit and failure recovery,
  plus truthful partial RAM/storage reports;
  [behavior and checks](docs/SMART_TRAY_2026-09-30.md)
- local alpha.51: identity-safe physical media/volume joins and explicit unknown
  partition fields; [twelve fixtures and checkpoint evidence](docs/STORAGE_PROVIDER_JOINS_2026-09-30.md)
- native creation-time validation on the same action handle for termination, priority,
  and affinity; reused PIDs and Windows-critical processes are refused
- CPU, memory, disk, network, and GPU performance drill-downs
- a graph-first dashboard with Lines/Bars, category filters and a shared two-minute
  timeline for usage, GPU temperature/power/clocks/fan/VRAM, drive temperatures,
  physical-disk activity and network traffic; details: `docs/GRAPH_WALL.md`
- independent physical-disk active time, response latency, queue depth and read/write
  throughput, with explicit cached/missing fields and gap-aware charts; mounted
  volumes stay separate: `docs/PHYSICAL_DISKS.md`
- optional NVIDIA temperature, board power, clocks, fan target, and VRAM sensors with
  two-minute temperature/power histories; missing sensors stay explicitly unavailable
- History, Startup, Users, Details, and Services pages backed by native data
- independent Startup/Services inventory workers, stable cached rows during slow
  or failed reads, and explicit source freshness instead of disappearing fields
- confirmed service Start/Stop/Restart on an independent worker, with state/PID
  preflight and explicit permission/uncertain-outcome errors; native command
  end-to-end validation remains a release gate in `docs/SERVICE_CONTROLS.md`
- per-service command-state retention across selections and subsequent commands;
  unresolved outcomes stay labeled until a newer read resolves them
- explicit JSON snapshot and CSV process export, with private details excluded by
  default, provider freshness, background writing and a native Save As picker;
  format/limitations and the remaining picker-validation gate: `docs/EXPORTS.md`
- a Speccy-class System page: OS, CPU (full CPUID brand, per-core-type caches,
  VT-x capability vs firmware vs hypervisor), per-DIMM RAM from SMBIOS, board and
  BIOS, graphics with 64-bit VRAM and monitors, storage with NVMe health, audio,
  peripherals, network and read-only sensor sources, live temperatures and clocks
  inline, copy and TXT/JSON save with private values hidden by default; no driver,
  elevation or guessed values: `docs/SYSTEM_SPECS.md`
- a frameless Tront shell and four-peg gradient Theme Studio: draggable stops,
  hex/position edits, eight presets, named saves, import/export, live surfaces and
  legacy-theme migration; remaining acceptance checks: `docs/THEME_STUDIO.md`
- background settings load/save, staged file replacement, retained palettes and
  explicit unsaved-close choices; per-user paths and limits: `docs/SETTINGS_PERSISTENCE.md`
- alternating row and gradient column bands, padded cells, and high-contrast selection
- a live full-width CPU tray meter with scrolling history and a resource tooltip,
  updated on its own native thread even when the main window is hidden
- an embedded multi-resolution Trontop icon and Windows version metadata
- bounded local Rust-panic/native-runner failure records, with raw/private payloads
  excluded and no upload; coverage and limits: `docs/FAILURE_REPORTS.md`
- an event-driven egui UI that does not poll the operating system on the render thread

## Run

```powershell
cargo run --release
```

## Build the portable executable

```powershell
cargo build --release
```

The resulting `target/release/trontop.exe` has no external application assets. The
MSVC C runtime is statically linked by `.cargo/config.toml`.

## Direction

Product completion is tracked in [ASK_LEDGER.md](ASK_LEDGER.md); historical engineering
detail is in `TASK_BOARD.md`, and publishing procedures in `RELEASE_PLAN.md`.
The next systems work includes reversible startup controls and isolated
service-command validation. Trontop remains a development
preview without full Task Manager parity. It should be a focused standalone app, not
a general system-utility suite.

## Public media automation

The product screenshots, downloadable theme gallery and social preview image
are reproducible. See [the media workflow](docs/MARKETING.md).
