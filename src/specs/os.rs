//! Operating System section (lane: os).
//!
//! Scope: edition/version/build/UBR (Windows 11 when build >= 22000, even though
//! the registry ProductName still says "Windows 10"), architecture, install
//! date, boot/uptime (LiveKey::Uptime), locale/time zone, firmware type and
//! Secure Boot, TPM presence/version, virtualization split into CPU capability,
//! firmware-enabled and hypervisor/VBS in use, memory integrity, page file,
//! .NET/DirectX where exposed. Computer/user names and product IDs are private.
use super::{Context, Section, SectionId};

#[cfg(windows)]
mod native;

/// "Microsoft Windows 10 Home" on build 22000+ is Windows 11: the registry
/// ProductName was never updated. Strips the "Microsoft " prefix.
fn windows_name(caption: &str, build: Option<u32>) -> String {
    let name = caption.trim();
    let name = name.strip_prefix("Microsoft ").unwrap_or(name);
    match build {
        Some(build) if build >= 22_000 && name.contains("Windows 10") => {
            name.replacen("Windows 10", "Windows 11", 1)
        }
        _ => name.to_string(),
    }
}

/// .NET Framework 4.x "Release" DWORD to version, per Microsoft's
/// "How to: Determine which .NET Framework versions are installed".
fn dotnet_version(release: u64) -> Option<&'static str> {
    Some(match release {
        533_320.. => "4.8.1",
        528_040.. => "4.8",
        461_808.. => "4.7.2",
        461_308.. => "4.7.1",
        460_798.. => "4.7",
        394_802.. => "4.6.2",
        394_254.. => "4.6.1",
        393_295.. => "4.6",
        379_893.. => "4.5.2",
        378_675.. => "4.5.1",
        378_389.. => "4.5",
        _ => return None,
    })
}

/// Win32_DeviceGuard SecurityServicesRunning / Configured identifiers.
fn security_service(id: u64) -> String {
    match id {
        1 => "Credential Guard".into(),
        2 => "Memory integrity (HVCI)".into(),
        3 => "System Guard Secure Launch".into(),
        4 => "SMM Firmware Measurement".into(),
        5 => "Kernel-mode hardware-enforced stack protection".into(),
        6 => "Hypervisor-enforced paging translation".into(),
        7 => "Hardware-enforced stack protection (kernel, audit)".into(),
        other => format!("Security service {other}"),
    }
}

/// Win32_DeviceGuard VirtualizationBasedSecurityStatus.
fn vbs_status(status: u64) -> &'static str {
    match status {
        0 => "Off",
        1 => "Enabled, not running",
        2 => "Running",
        _ => "Unknown state reported",
    }
}

pub fn collect(ctx: &Context) -> Section {
    #[cfg(windows)]
    {
        native::collect(ctx)
    }
    #[cfg(not(windows))]
    {
        let _ = ctx;
        Section::unavailable(SectionId::OperatingSystem, "read on Windows only")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_eleven_is_derived_from_the_build_not_the_product_name() {
        assert_eq!(
            windows_name("Microsoft Windows 10 Home", Some(26_200)),
            "Windows 11 Home"
        );
        assert_eq!(
            windows_name("Microsoft Windows 11 Pro", Some(26_100)),
            "Windows 11 Pro"
        );
        assert_eq!(
            windows_name("Microsoft Windows 10 Pro", Some(19_045)),
            "Windows 10 Pro"
        );
        assert_eq!(windows_name("Windows 10 Home", None), "Windows 10 Home");
    }

    #[test]
    fn documented_tables_map_exactly() {
        assert_eq!(dotnet_version(533_325), Some("4.8.1"));
        assert_eq!(dotnet_version(528_372), Some("4.8"));
        assert_eq!(dotnet_version(1), None);
        assert_eq!(security_service(2), "Memory integrity (HVCI)");
        assert_eq!(security_service(42), "Security service 42");
        assert_eq!(vbs_status(2), "Running");
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "Read-only Operating System specs probe; no driver, elevation, window or input"]
    fn native_specs_os_read_only_probe() {
        let started = std::time::Instant::now();
        let sections = [collect(&Context::probe())];
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        println!("{}", crate::specs::probe_text(&sections));
        println!("collected in {elapsed:.3} ms (private values masked)");
    }
}
