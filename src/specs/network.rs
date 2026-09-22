//! Network section (lane: network).
//!
//! Scope: adapters from GetAdaptersAddresses (alias, description, type, link
//! speed, status, MTU, DHCP, DNS, gateway), driver details via SetupDi, Wi-Fi
//! details from WLAN API where the service runs (SSID, BSSID, signal, band are
//! private or dimmed accordingly), live throughput via
//! LiveKey::NetworkThroughput { interface } matching the sampler's interface
//! name. MAC and IP addresses are private. No internet requests.
use super::{Context, Section, SectionId};

pub fn collect(_ctx: &Context) -> Section {
    Section::not_implemented(SectionId::Network)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Read-only Network specs probe; no driver, elevation, window or input"]
    fn native_specs_network_read_only_probe() {
        let started = std::time::Instant::now();
        let sections = [collect(&Context::probe())];
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        println!("{}", crate::specs::probe_text(&sections));
        println!("collected in {elapsed:.3} ms (private values masked)");
    }
}
