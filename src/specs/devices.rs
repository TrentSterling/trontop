//! Audio and Peripherals sections (lane: devices).
//!
//! Scope, Audio: sound devices (SetupDi media class) with driver details, and
//! playback/recording endpoints (MMDevice API, default device, format).
//! Peripherals: keyboards, mice/HID, USB controllers and attached USB devices
//! (bus-reported names), Bluetooth radios and paired devices, printers,
//! cameras. Instance IDs and serials are private.
use super::{Context, Section, SectionId};

pub fn collect_audio(_ctx: &Context) -> Section {
    Section::not_implemented(SectionId::Audio)
}

pub fn collect_peripherals(_ctx: &Context) -> Section {
    Section::not_implemented(SectionId::Peripherals)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Read-only Audio and Peripherals specs probe; no driver, elevation, window or input"]
    fn native_specs_devices_read_only_probe() {
        let started = std::time::Instant::now();
        let sections = [
            collect_audio(&Context::probe()),
            collect_peripherals(&Context::probe()),
        ];
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        println!("{}", crate::specs::probe_text(&sections));
        println!("collected in {elapsed:.3} ms (private values masked)");
    }
}
