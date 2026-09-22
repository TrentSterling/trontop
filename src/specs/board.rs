//! Motherboard section (lane: board).
//!
//! Scope: SMBIOS type 2 baseboard (manufacturer, product, version; serial is
//! private), type 1 system (UUID and serial private), type 3 chassis, type 0
//! BIOS (vendor, version, date, ROM size, UEFI flag), chipset from the PCI host
//! bridge / LPC device via SetupDi, PCIe slot inventory (type 9), live board
//! temperature only via LiveKey::MotherboardTemperature (bridge). OEM filler
//! strings (smbios::is_placeholder) are Unavailable("not set by the manufacturer").
use super::{Context, Section, SectionId};

pub fn collect(_ctx: &Context) -> Section {
    Section::not_implemented(SectionId::Motherboard)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Read-only Motherboard specs probe; no driver, elevation, window or input"]
    fn native_specs_board_read_only_probe() {
        let started = std::time::Instant::now();
        let sections = [collect(&Context::probe())];
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        println!("{}", crate::specs::probe_text(&sections));
        println!("collected in {elapsed:.3} ms (private values masked)");
    }
}
