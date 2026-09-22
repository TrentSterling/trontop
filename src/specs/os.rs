//! Operating System section (lane: os).
//!
//! Scope: edition/version/build/UBR (Windows 11 when build >= 22000, even though
//! the registry ProductName still says "Windows 10"), architecture, install
//! date, boot/uptime (LiveKey::Uptime), locale/time zone, firmware type and
//! Secure Boot, TPM presence/version, virtualization split into CPU capability,
//! firmware-enabled and hypervisor/VBS in use, memory integrity, page file,
//! .NET/DirectX where exposed. Computer/user names and product IDs are private.
use super::{Context, Section, SectionId};

pub fn collect(_ctx: &Context) -> Section {
    Section::not_implemented(SectionId::OperatingSystem)
}

#[cfg(test)]
mod tests {
    use super::*;

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
