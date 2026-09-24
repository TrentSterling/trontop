"""Collect shipped Windows dependency notices from the locked Cargo sources.

Run after `cargo fetch --locked --target x86_64-pc-windows-msvc`.
No network access, credentials, or machine-specific paths enter the output.
"""
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
metadata = json.loads(subprocess.check_output([
    "cargo", "metadata", "--locked", "--offline", "--format-version", "1",
    "--filter-platform", "x86_64-pc-windows-msvc",
], cwd=ROOT))
sections = ["Trontop third-party notices\n\n"
            "These components retain their upstream licenses. The Trontop Commons\n"
            "Clause does not restrict these components when used independently.\n"
            "For OR alternatives, Trontop uses MIT where available, otherwise\n"
            "Apache-2.0 where available. AND requirements remain cumulative.\n"
            "This inventory includes Windows build and test dependencies.\n"]
missing = []
fallbacks = {
    "accesskit": "accesskit", "clipboard-win": "clipboard-win",
    "ecolor": "egui", "eframe": "egui", "egui": "egui", "egui-winit": "egui",
    "egui_extras": "egui", "emath": "egui", "epaint": "egui",
    "gl_generator": "gl-rs", "khronos_api": "gl-rs",
    "gpu-descriptor": "gpu-descriptor", "gpu-descriptor-types": "gpu-descriptor",
    "hexf-parse": "hexf", "profiling": "profiling", "spirv": "rspirv",
}
for package in sorted(metadata["packages"], key=lambda p: (p["name"], p["version"])):
    if package["name"] == "trontop":
        continue
    root = Path(package["manifest_path"]).parent
    files = sorted(p for p in root.rglob("*") if p.is_file()
                   and (p.name.lower().startswith(("license", "licence", "copying", "notice", "ofl", "ufl"))
                        or any(parent.name.lower() == "licenses" for parent in p.parents)
                        or (package["name"] == "epaint_default_fonts" and p.suffix == ".txt"))
                   and p.suffix.lower() not in (".rs", ".html", ".json"))
    if not files and package["name"] in fallbacks:
        files = [ROOT / "docs/licenses" / (fallbacks[package["name"]] + ".txt")]
    if not files:
        missing.append(package["name"])
    header = f"\n{'=' * 72}\n{package['name']} {package['version']}\nUpstream license: {package.get('license', 'See notices')}\n"
    sections.append(header)
    for path in files:
        if "gpl" in path.name.lower() and "Apache-2.0 OR GPL" in (package.get("license") or ""):
            continue
        label = path.relative_to(root).as_posix() if path.is_relative_to(root) else path.name
        sections.append(f"\n--- {label} ---\n" + path.read_text(encoding="utf-8", errors="replace"))
for name, path in [
    ("Rajdhani font", ROOT / "assets/fonts/OFL.txt"),
    ("Windows CPU ABI reference", ROOT / "docs/SYSTEM_INFORMER_NOTICE.txt"),
]:
    sections.append(f"\n{'=' * 72}\n{name}\n\n" + path.read_text(encoding="utf-8"))
if missing:
    raise SystemExit("Missing license files: " + ", ".join(missing))
text = "\n".join(line.rstrip() for line in "\n".join(sections).splitlines()) + "\n"
(ROOT / "THIRD_PARTY_NOTICES.txt").write_text(text, encoding="utf-8", newline="\n")
print(f"Wrote notices for {len(metadata['packages']) - 1} resolved dependencies and bundled references/fonts.")
