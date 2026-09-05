# Optional Ctrl+Shift+Esc takeover: proposal, not implemented

Requested for discussion by Trent on 2026-09-05. Desired split: Ctrl+Shift+Esc
shows Trontop, while Ctrl+Alt+Del > Task Manager still opens Windows Task Manager.
No hook, hotkey registration, registry redirection or desktop test has been run.

Recommended design:

- Explicit opt-in setting, disabled by default, effective only while Trontop runs.
- Investigate normal RegisterHotKey registration first; do not assume a Windows-owned
  combination is available. Report failure rather than claiming the binding works.
- If necessary, prototype WH_KEYBOARD_LL on an isolated authorized test desktop.
  Suppress only the exact Ctrl+Shift+Esc chord; leave Ctrl+Alt+Del and all other
  input untouched. Do not replace or redirect taskmgr.exe itself.
- Dedicated hook thread with a message loop. Callback only updates minimal modifier
  state and posts a show request; no telemetry, UI rendering, blocking locks or I/O.
  No keyboard logging or persistence. Pass unrelated input through CallNextHookEx.
- Require a healthy/available Trontop UI before suppressing the chord. Explicit
  disable and app exit remove the hook. If the hook is absent, Windows retains its
  normal shortcut. This is not a guarantee against every hung-process scenario.
- Provide an explicit Open Windows Task Manager command as another escape route.
- Optional launch-at-login is a separate user choice, not silently enabled with
  this setting. A stopped portable EXE cannot intercept keyboard input.

Validation before shipping: exact modifiers (including left/right, Alt and Win),
repeat handling, key-down/up pairing, disabling while held, UI heartbeat failure,
teardown, task-manager fallback and hook-thread failure. Test pure chord/state logic
headlessly first. Real Windows 11 behavior, elevated foreground applications, RDP,
secure desktop transitions and Ctrl+Alt+Del fallback remain unverified. Native hook
testing needs fresh permission and isolation from Trent's working desktop.

Microsoft documents that low-level hooks can suppress input, must return quickly
and can be silently removed on timeout. This makes the design plausible, not proof
that interception of this specific OS shortcut works in every Windows configuration.

Sources checked:

- [RegisterHotKey conflicts](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerhotkey)
- [LowLevelKeyboardProc behavior and thread guidance](https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelkeyboardproc)
- [Windows shortcuts and security-screen Task Manager](https://support.microsoft.com/en-us/windows/keyboard-shortcuts-in-windows-dcc61a57-8ff0-cffe-9796-cb9706c75eec)
