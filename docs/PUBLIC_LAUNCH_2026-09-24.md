# Alpha.41 public preview

The original synthetic screenshot gallery described below was replaced with real
telemetry at Trent's request later the same day. See `LIVE_MEDIA_2026-09-24.md`.
The exact released executable and its validation remain as recorded here.

Published on 2026-09-24 at Trent's request, including source, Windows download,
product page, blog post, themed screenshot automation and social preview image.

- [Product page and gallery](https://tront.xyz/trontop/)
- [Launch article](https://tront.xyz/blog/posts/trontop/)
- [Windows release](https://github.com/TrentSterling/trontop/releases/tag/v0.3.0-alpha.41)
- [Source](https://github.com/TrentSterling/trontop)

## License and scope

The agreed license is Apache 2.0 with Commons Clause 1.0. Personal and workplace
use is free, attribution must be retained, and sales are restricted as defined
in `LICENSE`. This is source-available software, not an OSI open-source license.
Third-party components retain their own licenses. The archive includes LICENSE,
NOTICE, THIRD_PARTY_NOTICES.txt, PRIVACY.md and preview limitations in README.md.

This is an unsigned development preview. The broader native, clean-machine,
hardware and extended-soak acceptance checks remain open in ASK_LEDGER.md and
RELEASE_ALPHA41.md. A36 is complete; the whole product ledger is not.

## Exact published build

- Version/tag: `0.3.0-alpha.41` / `v0.3.0-alpha.41`.
- Source commit: `92a6d4cc5d136592699904482b11808bd1189ca2`.
- [Passing Windows CI run](https://github.com/TrentSterling/trontop/actions/runs/36068499326).
- Published executable comes directly from that CI artifact.
- Embedded ProductVersion and PrivateBuild match the version and source commit.
- Build metadata reports a clean source checkout and `x86_64-pc-windows-msvc`.
- Executable: 17,669,632 bytes; Authenticode status `NotSigned`.
- ZIP: 7,358,751 bytes. Its executable, metadata and notices match the CI files.

SHA-256:

```text
4dfc426985668def728a1ce851b45d35ffd4458e2f023dbee3a00aa7d261fd24  trontop.exe
620daf30663b355811a52d177774dff61361ae3906a24ddb9e53b53a9ef4bed2  trontop-0.3.0-alpha.41-windows-x64.zip
```

The anonymous public ZIP download was checked against these same hashes after
publication. No authentication was used for the public source/download checks.

## Validation

Local and CI checks passed: formatting, 429 ordinary tests (0 failures, 48
explicitly ignored), strict all-target Clippy and release compilation. CI also
checks formatting and Clippy for the local egui-wgpu patch and runs the package
identity/license/checksum gate.

Marketing renders use the production UI with clearly labeled synthetic demo
data, without a sampler, tray, native window or desktop input. There are 16
screenshots and 12 downloadable version-4 themes. The OG image is 1200 by 630.
See MARKETING.md for repeatable export commands.

Pre-publication browser checks passed at 390, 768 and 1280 pixels in dark/light
mode, including image loading, interactions, no-JavaScript fallback and reduced
motion. Anonymous live checks at 23:16 UTC verified:

- Product page and article return HTTP 200, with the expected OG metadata.
- Portfolio homepage and sitemap link to Trontop.
- All 35 media/style/script assets match the prepared files; text comparison
  allows Git's normal CRLF/LF conversion.
- Desktop/mobile layouts, theme switching, image lightbox/Escape, persisted page
  mode, all 12 theme download links and the alpha.41 release link work.
- No JavaScript errors occurred in the isolated headless browser.

Deployment revisions:

- Website: `TrentSterling/trentsterling.github.io`, master commit
  `2df360f5e6f59093a731bf712626b49cd30ec227`; GitHub Pages build succeeded.
- Blog: [PR 3](https://github.com/TrentSterling/blog/pull/3), merged as
  `d354b224a4dfa31eda20074181d6ae9e7cdae789`;
  [deployment succeeded](https://github.com/TrentSterling/blog/actions/runs/36071465875).

The pre-publication credential-pattern scan examined 1,089 reachable Git blobs
and found no matches in the scanned categories. This was a pattern scan, not a
security audit or a guarantee that every sensitive string was identified.
