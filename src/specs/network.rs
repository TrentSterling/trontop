//! Network section (lane: network).
//!
//! Scope: adapters from GetAdaptersAddresses (alias, description, type, link
//! speed, status, MTU, DHCP, DNS, gateway), driver details via SetupDi, Wi-Fi
//! details from WLAN API where the service runs (SSID, BSSID, signal, band are
//! private or dimmed accordingly), live throughput via
//! LiveKey::NetworkThroughput { interface } matching the sampler's interface
//! name. MAC and IP addresses are private. No internet requests.
use super::{Context, Group, LiveKey, Row, Section, SectionId, SummaryLine, Value};

#[cfg(windows)]
mod native;

const NONE_CONFIGURED: &str = "none configured";

/// One adapter from GetAdaptersAddresses, already converted to text.
#[derive(Clone, Debug, Default, PartialEq)]
struct Adapter {
    alias: String,
    description: Option<String>,
    kind: u32,
    up: bool,
    speed_bps: Option<u64>,
    mtu: Option<u32>,
    mac: Option<String>,
    ipv4: Vec<String>,
    ipv6: Vec<String>,
    gateways: Vec<String>,
    dns: Vec<String>,
    dhcp: bool,
    dhcp_server: Option<String>,
    suffix: Option<String>,
    driver: Option<String>,
}

/// IANA ifType names for the types Windows adapters use.
fn if_type(kind: u32) -> String {
    match kind {
        6 => "Ethernet".into(),
        71 => "Wi-Fi (802.11)".into(),
        23 => "PPP".into(),
        24 => "Loopback".into(),
        131 => "Tunnel".into(),
        144 => "IEEE 1394".into(),
        243 | 244 => "Mobile broadband".into(),
        other => format!("Interface type {other}"),
    }
}

fn speed_text(bps: u64) -> String {
    match bps {
        b if b >= 1_000_000_000 && b.is_multiple_of(1_000_000_000) => {
            format!("{} Gbps", b / 1_000_000_000)
        }
        b if b >= 1_000_000_000 => format!("{:.1} Gbps", b as f64 / 1e9),
        b if b >= 1_000_000 => format!("{} Mbps", b / 1_000_000),
        b => format!("{} kbps", b / 1000),
    }
}

/// SOCKADDR bytes (AF_INET or AF_INET6) as text.
fn sockaddr_text(bytes: &[u8]) -> Option<String> {
    let family = u16::from_le_bytes([*bytes.first()?, *bytes.get(1)?]);
    match family {
        2 => {
            let octets: [u8; 4] = bytes.get(4..8)?.try_into().ok()?;
            Some(std::net::Ipv4Addr::from(octets).to_string())
        }
        23 => {
            let octets: [u8; 16] = bytes.get(8..24)?.try_into().ok()?;
            Some(std::net::Ipv6Addr::from(octets).to_string())
        }
        _ => None,
    }
}

fn mac_text(bytes: &[u8]) -> Option<String> {
    (!bytes.is_empty() && bytes.iter().any(|b| *b != 0)).then(|| {
        bytes
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join("-")
    })
}

fn list(values: &[String]) -> Value {
    if values.is_empty() {
        Value::unavailable(NONE_CONFIGURED)
    } else {
        Value::known(values.join(", "))
    }
}

fn build(adapters: Result<Vec<Adapter>, String>) -> Section {
    let adapters = match adapters {
        Ok(adapters) => adapters,
        Err(reason) => return Section::unavailable(SectionId::Network, reason),
    };
    let mut section = Section::new(SectionId::Network);
    let mut shown = adapters
        .into_iter()
        .filter(|a| a.kind != 24)
        .collect::<Vec<_>>();
    // Connected adapters first, then by name.
    shown.sort_by(|a, b| b.up.cmp(&a.up).then_with(|| a.alias.cmp(&b.alias)));
    for adapter in &shown {
        let live = LiveKey::NetworkThroughput {
            interface: adapter.alias.clone(),
        };
        if adapter.up {
            let mut line = adapter
                .description
                .clone()
                .unwrap_or_else(|| adapter.alias.clone());
            if let Some(speed) = adapter.speed_bps.filter(|s| *s > 0 && *s != u64::MAX) {
                line.push_str(&format!(" ({})", speed_text(speed)));
            }
            section.push_summary(SummaryLine::known(line).live(live.clone()));
        }
        let mut group = Group::new(adapter.alias.clone());
        if adapter.up {
            group = group.live(live.clone());
        } else {
            group = group.collapsed();
        }
        group.push_row(Row::new(
            "Adapter",
            Value::from_option(adapter.description.clone(), "not reported"),
        ));
        group.push_row(Row::known("Type", if_type(adapter.kind)));
        group.push_row(Row::known(
            "Status",
            if adapter.up {
                "Connected"
            } else {
                "Not connected"
            },
        ));
        group.push_row(Row::new(
            "Link speed",
            Value::from_option(
                adapter
                    .speed_bps
                    .filter(|s| *s > 0 && *s != u64::MAX && adapter.up)
                    .map(speed_text),
                "no link",
            ),
        ));
        group.push_row(
            Row::new(
                "MAC address",
                Value::from_option(adapter.mac.clone(), "none"),
            )
            .private(),
        );
        group.push_row(Row::new(
            "Driver",
            Value::from_option(
                adapter.driver.clone(),
                "not matched to a network-class device",
            ),
        ));
        if !adapter.up {
            section.push_group(group);
            continue;
        }
        group.push_row(
            Row::live("Throughput", live).note("Receive / transmit over the sampler interval"),
        );
        group.push_row(Row::new(
            "MTU",
            Value::from_option(
                adapter
                    .mtu
                    .filter(|m| *m > 0 && *m != u32::MAX)
                    .map(|m| format!("{m} bytes")),
                "not reported",
            ),
        ));
        group.push_row(Row::new("IPv4", list(&adapter.ipv4)).private());
        group.push_row(Row::new("IPv6", list(&adapter.ipv6)).private());
        group.push_row(Row::new("Gateway", list(&adapter.gateways)).private());
        group.push_row(Row::new("DNS servers", list(&adapter.dns)).private());
        group.push_row(Row::known(
            "DHCP",
            if adapter.dhcp { "Enabled" } else { "Disabled" },
        ));
        if adapter.dhcp {
            group.push_row(
                Row::new(
                    "DHCP server",
                    Value::from_option(adapter.dhcp_server.clone(), "not reported"),
                )
                .private(),
            );
        }
        if let Some(suffix) = &adapter.suffix {
            group.push_row(Row::known("DNS suffix", suffix.clone()).private());
        }
        if adapter.kind == 71 {
            group.push_row(
                Row::unavailable(
                    "Wi-Fi network details",
                    "not read: Windows treats SSID and BSSID as location data and may prompt for location access",
                )
                .note("Trontop never triggers location prompts"),
            );
        }
        section.push_group(group);
    }
    if section.groups.is_empty() {
        return Section::unavailable(SectionId::Network, "Windows reports no network adapters");
    }
    if section.summary.is_empty() {
        section.push_summary(SummaryLine::known("No connected network adapters"));
    }
    section
}

pub fn collect(ctx: &Context) -> Section {
    let _ = ctx;
    #[cfg(windows)]
    {
        build(native::adapters())
    }
    #[cfg(not(windows))]
    {
        build(Err("read on Windows only".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sockaddrs_and_macs_format_exactly() {
        let mut v4 = vec![2u8, 0, 0, 0, 192, 168, 0, 12];
        v4.extend([0; 8]);
        assert_eq!(sockaddr_text(&v4).as_deref(), Some("192.168.0.12"));
        let mut v6 = vec![23u8, 0, 0, 0, 0, 0, 0, 0, 0xFE, 0x80];
        v6.extend([0u8; 13]);
        v6.push(1);
        v6.extend([0; 4]);
        assert_eq!(sockaddr_text(&v6).as_deref(), Some("fe80::1"));
        assert_eq!(sockaddr_text(&[9, 0]), None);
        assert_eq!(
            mac_text(&[0xAA, 0xBB, 0, 1, 2, 3]).as_deref(),
            Some("AA-BB-00-01-02-03")
        );
        assert_eq!(mac_text(&[0; 6]), None);
        assert_eq!(speed_text(1_000_000_000), "1 Gbps");
        assert_eq!(speed_text(2_500_000_000), "2.5 Gbps");
    }

    #[test]
    fn addresses_are_private_and_live_keys_use_the_alias() {
        let section = build(Ok(vec![
            Adapter {
                alias: "Ethernet".into(),
                description: Some("Fixture Ethernet".into()),
                kind: 6,
                up: true,
                speed_bps: Some(1_000_000_000),
                ipv4: vec!["192.0.2.44".into()],
                mac: Some("AA-BB-CC-DD-EE-FF".into()),
                dhcp: true,
                ..Default::default()
            },
            Adapter {
                alias: "Wi-Fi".into(),
                kind: 71,
                up: true,
                ..Default::default()
            },
            Adapter {
                alias: "Ethernet 2".into(),
                kind: 6,
                ..Default::default()
            },
        ]));
        let text = crate::specs::probe_text(std::slice::from_ref(&section));
        assert!(
            text.contains("  Fixture Ethernet (1 Gbps)    [live NetworkThroughput(Ethernet)]"),
            "{text}"
        );
        assert!(
            !text.contains("192.0.2.44") && !text.contains("AA-BB-CC"),
            "{text}"
        );
        assert!(
            text.contains("Wi-Fi network details: Unavailable (not read"),
            "{text}"
        );
        assert!(!section.groups[2].expanded);
        assert_eq!(
            section.groups[2].row_count(),
            6,
            "disconnected adapters stay short"
        );
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "Read-only Network specs probe; no driver, elevation, window, input or network traffic"]
    fn native_specs_network_read_only_probe() {
        let started = std::time::Instant::now();
        let sections = [collect(&Context::probe())];
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        println!("{}", crate::specs::probe_text(&sections));
        println!("collected in {elapsed:.3} ms (private values masked)");
        let names = sysinfo::Networks::new_with_refreshed_list()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        for group in &sections[0].groups {
            println!(
                "alias {:?} matches a sysinfo network name: {}",
                group.title,
                names.contains(&group.title)
            );
        }
    }
}
