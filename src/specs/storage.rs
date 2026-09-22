//! Storage and Optical Drives sections (lane: storage).
//!
//! Scope: physical disks with model, capacity, bus/interface (NVMe, SATA, USB;
//! never "Unknown" when STORAGE_ADAPTER_DESCRIPTOR/bus type says otherwise),
//! media type (SSD/HDD by seek penalty), firmware, partitions and volumes,
//! SMART/health where Windows exposes it without pass-through, and a live
//! LiveKey::DriveTemperature { interface } using the GUID_DEVINTERFACE_DISK
//! path (serials private). Optical: drives, media capabilities, loaded media.
use super::{Context, Section, SectionId};

pub fn collect(_ctx: &Context) -> Section {
    Section::not_implemented(SectionId::Storage)
}

pub fn collect_optical(_ctx: &Context) -> Section {
    Section::not_implemented(SectionId::OpticalDrives)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Read-only Storage and Optical Drives specs probe; no driver, elevation, window or input"]
    fn native_specs_storage_read_only_probe() {
        let started = std::time::Instant::now();
        let sections = [
            collect(&Context::probe()),
            collect_optical(&Context::probe()),
        ];
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        println!("{}", crate::specs::probe_text(&sections));
        println!("collected in {elapsed:.3} ms (private values masked)");
    }
}
