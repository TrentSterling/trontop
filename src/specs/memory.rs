//! RAM section (lane: memory).
//!
//! Scope: installed vs usable capacity, per-DIMM data from SMBIOS type 17
//! (slot/bank locator, size, type DDR4/DDR5, configured and rated speed,
//! manufacturer, part number; serial is private), type 16 array (slots, max
//! capacity), channel/rank where encoded, live usage (LiveKey::MemoryUsed and
//! LiveKey::MemoryCommit). Never "Number of SPD modules: 0": SPD needs a driver,
//! SMBIOS does not.
use super::{Context, Section, SectionId};

pub fn collect(_ctx: &Context) -> Section {
    Section::not_implemented(SectionId::Memory)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Read-only RAM specs probe; no driver, elevation, window or input"]
    fn native_specs_memory_read_only_probe() {
        let started = std::time::Instant::now();
        let sections = [collect(&Context::probe())];
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        println!("{}", crate::specs::probe_text(&sections));
        println!("collected in {elapsed:.3} ms (private values masked)");
    }
}
