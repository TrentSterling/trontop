//! CPU section (lane: cpu).
//!
//! Scope: full CPUID brand string (never a bare "Intel Core"), vendor, family/
//! model/stepping, codename only from a documented table, sockets/cores/threads
//! per core type on hybrid parts (P/E), per-core-type cache sizes (L1d/L1i/L2/
//! L3 from GetLogicalProcessorInformationEx), instruction set flags, base clock
//! and live clocks (LiveKey::CpuClockAverage/CpuClockFastest/CpuCoreClock),
//! virtualization capability vs enabled vs in use, package temperature only via
//! LiveKey::CpuPackageTemperature (bridge). No ACPI thermal zones as CPU temps.
use super::{Context, Section, SectionId};

pub fn collect(_ctx: &Context) -> Section {
    Section::not_implemented(SectionId::Cpu)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Read-only CPU specs probe; no driver, elevation, window or input"]
    fn native_specs_cpu_read_only_probe() {
        let started = std::time::Instant::now();
        let sections = [collect(&Context::probe())];
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        println!("{}", crate::specs::probe_text(&sections));
        println!("collected in {elapsed:.3} ms (private values masked)");
    }
}
