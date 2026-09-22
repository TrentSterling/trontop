//! Graphics section (lane: graphics).
//!
//! Scope: every adapter (DXGI identity, crate::gpu_adapters::dxgi_inventory),
//! 64-bit dedicated VRAM (never WMI's 32-bit AdapterRAM, which caps at 4 GB),
//! vendor/device/subsystem IDs and board partner, driver version/date, PCIe link
//! where exposed, NVML static data for NVIDIA, monitors (name, native and
//! current resolution, refresh rate; EDID serials private), live values via
//! LiveKey::Gpu { adapter: GpuRef, metric }. Never invent a shader clock.
use super::{Context, Section, SectionId};

pub fn collect(_ctx: &Context) -> Section {
    Section::not_implemented(SectionId::Graphics)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Read-only Graphics specs probe; no driver, elevation, window or input"]
    fn native_specs_graphics_read_only_probe() {
        let started = std::time::Instant::now();
        let sections = [collect(&Context::probe())];
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        println!("{}", crate::specs::probe_text(&sections));
        println!("collected in {elapsed:.3} ms (private values masked)");
    }
}
