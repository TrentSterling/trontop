# AGENTS.md

Read the root `C:/trontstack/AGENTS.md`, root `C:/trontstack/CLAUDE.md`, and this
project's `CLAUDE.md` before changing Trontop.

Trontop is a Windows-first Rust/egui application. Keep sampling work off the render
thread and verify changes with the commands documented in `CLAUDE.md`.

## Desktop safety (Trent's explicit instruction)

Do not inject global mouse/keyboard input, steal focus, or minimize/restore/move
windows for tests. A prior automated drag almost closed a day-job Claude session.
Use headless egui tests and code-side checks. Any future desktop interaction needs
fresh explicit permission and isolation from the working desktop. Other Claude
sessions and day-job windows are off-limits. A synthetic drag that looks smoother
is not proof of a dragging fix; no cross-project drag patch is validated yet.
