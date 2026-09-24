# Upstream license texts

`scripts/generate-notices.py` uses Cargo's locked Windows dependency sources.
Some crate archives omit the repository-root license. These fallbacks were
retrieved from the upstream repositories on 2026-09-24:

- accesskit.txt: https://raw.githubusercontent.com/AccessKit/accesskit/main/LICENSE-MIT
- clipboard-win.txt: https://raw.githubusercontent.com/DoumanAsh/clipboard-win/master/LICENSE
- egui.txt: https://raw.githubusercontent.com/emilk/egui/0.35.0/LICENSE-MIT
- enum-map.txt: https://raw.githubusercontent.com/xfix/enum-map/master/LICENSES/MIT.txt
- gl-rs.txt: https://raw.githubusercontent.com/brendanzab/gl-rs/master/LICENSE
- gpu-descriptor.txt: https://raw.githubusercontent.com/zakarumych/gpu-descriptor/master/license/MIT
- profiling.txt: https://raw.githubusercontent.com/aclysma/profiling/master/LICENSE-MIT
- rspirv.txt: https://raw.githubusercontent.com/gfx-rs/rspirv/master/LICENSE
- hexf.txt: standard CC0-1.0 legal text, copied from enum-map 2.7.3's
  `LICENSES/CC0-1.0.txt`, matching hexf-parse 0.2.1's declared license.

These texts retain their upstream terms. The Trontop Commons Clause does not
relicense upstream components. Font notices include egui's bundled fonts and
Rajdhani, not just the Rust crate licenses.
