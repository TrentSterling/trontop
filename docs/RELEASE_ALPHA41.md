# Trontop 0.3.0-alpha.41

First public source-available development preview for Windows 11 x64.

This release includes the alpha.39 Theme Studio expansion and alpha.40 daily-use
polish: theme-aware T branding, intensity and separate dark/light frost controls,
contrast/font/scale controls, movable dialogs with bright close-button outlines,
Lines/Bars toolbar controls, pinned inspector End task and random-palette logo
contrast correction.

Alpha.41 adds the Apache 2.0 + Commons Clause 1.0 license, Trent Sterling's
attribution notice, third-party notices in the executable, and reproducible
public media generation. Personal and workplace use are free of charge. Sales
are restricted as defined by LICENSE. This is source available, not OSI open source.

## Install

Extract the Windows ZIP and run `trontop.exe`. No installer or Rust toolchain is
required by the application. Keep LICENSE, NOTICE and THIRD_PARTY_NOTICES.txt when
redistributing. Settings and local failure records live under your Windows user
profile. Use SHA256SUMS.txt to verify the release files.

## Preview limits

- Unsigned executable: Windows may show an unknown-publisher/reputation warning.
- Windows 11 x64 is the development target. Clean-machine and broader hardware
  acceptance remain open; no claim of Windows 10, ARM64 or Linux support.
- CPU temperatures require a supported external sensor source. GPU and storage
  readings depend on drivers and hardware; unavailable values stay explicit.
- Native export dialog edge cases and isolated service-control acceptance remain
  open. Existing fake-backend tests do not establish those native guarantees.
- Extended soak and multi-monitor/DPI/window lifecycle coverage remain open.
- Full Task Manager parity, startup editing and suspend/resume are not included.
- There is no automatic updater. Exit Trontop normally before replacing the EXE.

Publishing this early preview does not mark those acceptance items complete.
The preceding private alpha release remains linked in the repository's Releases
history as a rollback option; it predates the public licensing package.

## Report problems

Include the version and About > Copy support report in a GitHub issue. Review any
screenshots or exports for private information before attaching them.
