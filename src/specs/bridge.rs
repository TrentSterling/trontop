//! Sensor bridge (lane: bridge): read-only connections to sensor providers the
//! user already runs (for example LibreHardwareMonitor's WMI namespace or
//! HWiNFO shared memory). Never install, start or elevate anything.
//!
//! `collect` lists the providers checked and what each exposes (Sensor Sources
//! section). `read_live` runs on its own fast worker and maps readings to
//! LiveKey values: CpuPackageTemperature, CpuCoreTemperature, MotherboardTemperature
//! and Sensor { id } for everything else.
use super::{BridgeReadings, Context, Section, SectionId, Value};
use std::time::Duration;

pub fn collect(_ctx: &Context) -> Section {
    Section::not_implemented(SectionId::SensorBridge)
}

/// One fast poll of every running provider. The worker stamps `collected_at`.
pub fn read_live(_ctx: &Context) -> BridgeReadings {
    BridgeReadings {
        status: Value::not_implemented(),
        retry_after: Some(Duration::from_secs(60)),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Read-only Sensor Sources specs probe; no driver, elevation, window or input"]
    fn native_specs_bridge_read_only_probe() {
        let started = std::time::Instant::now();
        let sections = [collect(&Context::probe())];
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        println!("{}", crate::specs::probe_text(&sections));
        let live = read_live(&Context::probe());
        println!(
            "live status {:?}; {} readings",
            live.status,
            live.readings.len()
        );
        println!("collected in {elapsed:.3} ms (private values masked)");
    }
}
