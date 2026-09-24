# Real telemetry and the actual origin story

Trent rejected the smooth synthetic launch graphs because they made a working
application look fake. He explicitly authorized his real CPU/GPU telemetry and
said app names could remain anonymized. All 16 screenshots, the gallery manifest
and the social preview image have been replaced.

The ignored marketing renderer now requires an explicit live-capture opt-in.
It runs Trontop's normal sampler for 125 seconds before rendering, continuing to
accept real samples between shots. It opens no window, sends no desktop input,
and generates no stress workload. Process actions are disabled. Numerical values
are retained; account names, personal paths, command lines and the host identifier
are omitted. Process names and hardware models remain real.

Capture evidence:

- 137 real samples over approximately 139 seconds.
- CPU: 36.68 to 64.11 percent; 137 distinct measured values.
- GPU: 8.33 to 17.32 percent; 135 measured values after provider warm-up.
- 16 images across 12 themes, with real disk/network bursts and sensor histories.
- Each gallery entry records capture time, sample sequence and real-data source.
- The local numeric-only trace is `target/marketing/capture-metrics.json`.
- Capture test: 1 passed, 0 failed, 476 filtered out, 139.82 seconds.

See `MARKETING.md` for the updated opt-in export command. Future captures use the
current workload, so the media pipeline is repeatable but the measurements vary.

The launch article now opens with Trent's own reason for building the app:
Windows 11 Task Manager was lagging on his powerful PC and reporting stale CPU
readings that cleared when reopened. His goal became a functional, pretty
replacement with Speccy-level hardware detail. The article credits Discord as
the theme-system inspiration and presents the Task Manager behavior as his
experience, not a universal diagnosis.

Validation: formatting, 429 ordinary tests (0 failed, 48 ignored), strict
all-target Clippy and release build pass. Browser checks pass at 390/768/1280 in
light/dark, including image loading, theme selection, lightbox, downloads and
no-JavaScript fallback. Representative native UI captures and the OG card were
visually reviewed.

Published revisions:

- Website: `9eb3c11a144f68c19d35fad817c743d537ef6b89`, GitHub Pages build succeeded.
- Blog: [PR 4](https://github.com/TrentSterling/blog/pull/4), merged as
  `0bc2662389d666f25578af5b4379a079ec63973d` after build/SEO checks passed.
- Anonymous checks at 23:47 UTC confirmed the live article's new story and the
  gallery's real-data metadata. All 16 public screenshots and `og.png` matched
  the newly captured files byte for byte.

[Gallery](https://tront.xyz/trontop/) | [Article](https://tront.xyz/blog/posts/trontop/)

The alpha.41 binary release is unchanged; this follow-up changes capture tooling,
documentation and public media, not production application behavior.
