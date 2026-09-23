//! Network section (lane: network).
//!
//! Scope: adapters from GetAdaptersAddresses (alias, description, type, link
//! speed, status, MTU, DHCP, DNS, gateway), driver details via SetupDi, Wi-Fi
//! details from WLAN API where the service runs (SSID, BSSID, signal, band are
//! private or dimmed accordingly), live throughput via
//! LiveKey::NetworkThroughput { interface } matching the sampler's interface
//! name. MAC and IP addresses are private. No internet requests.
//!
//! Also, as Speccy does: WinInet proxy settings (HKCU Internet Settings,
//! addresses private) and the local TCP/UDP socket tables
//! (GetExtendedTcpTable / GetExtendedUdpTable: a read of the kernel's own
//! table, no packets sent; endpoints private).
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
    /// NDIS_PHYSICAL_MEDIUM from GetIfEntry2, when it could be read.
    medium: Option<i32>,
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

/// NdisPhysicalMediumBluetooth: a Bluetooth PAN adapter emulates Ethernet
/// (ifType 6), so the physical medium is the truthful type.
const MEDIUM_BLUETOOTH: i32 = 10;

fn adapter_type(adapter: &Adapter) -> String {
    match adapter.medium {
        Some(MEDIUM_BLUETOOTH) => "Bluetooth (PAN)".into(),
        _ => if_type(adapter.kind),
    }
}

/// WinInet proxy settings for the current user. Server names are private.
#[derive(Clone, Debug, Default, PartialEq)]
struct Proxy {
    enabled: bool,
    server: Option<String>,
    bypass: Option<String>,
    script: Option<String>,
}

/// One row of the kernel's TCP table.
#[derive(Clone, Debug, PartialEq)]
struct Connection {
    /// MIB_TCP_STATE (2 LISTEN, 5 ESTABLISHED, ...).
    state: u32,
    local: String,
    remote: String,
    pid: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Connections {
    tcp: Vec<Connection>,
    udp: usize,
}

const TCP_LISTEN: u32 = 2;
const TCP_ESTABLISHED: u32 = 5;
/// Connection rows listed by name; the counts always cover every row.
const LISTED_CONNECTIONS: usize = 40;

/// The rows of a MIB_*TABLE_OWNER_PID buffer: a u32 count, then fixed-size
/// rows from byte 4. None when the count does not fit the buffer.
fn table_rows(bytes: &[u8], size: usize) -> Option<std::slice::ChunksExact<'_, u8>> {
    let count = u32::from_le_bytes(bytes.get(..4)?.try_into().ok()?) as usize;
    let end = count.checked_mul(size)?.checked_add(4)?;
    Some(bytes.get(4..end)?.chunks_exact(size))
}

fn dword(row: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(row.get(at..at + 4)?.try_into().ok()?))
}

/// Ports sit in network byte order in the low word of their DWORD.
fn port(row: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(row.get(at..at + 2)?.try_into().ok()?))
}

/// MIB_TCPTABLE_OWNER_PID (24-byte rows) or MIB_TCP6TABLE_OWNER_PID (56).
fn parse_tcp(bytes: &[u8], v6: bool) -> Option<Vec<Connection>> {
    let rows = table_rows(bytes, if v6 { 56 } else { 24 })?;
    rows.map(|row| {
        if v6 {
            let ip = |at: usize| -> Option<std::net::Ipv6Addr> {
                let octets: [u8; 16] = row.get(at..at + 16)?.try_into().ok()?;
                Some(octets.into())
            };
            Some(Connection {
                local: format!("[{}]:{}", ip(0)?, port(row, 20)?),
                remote: format!("[{}]:{}", ip(24)?, port(row, 44)?),
                state: dword(row, 48)?,
                pid: dword(row, 52)?,
            })
        } else {
            let ip = |at: usize| -> Option<std::net::Ipv4Addr> {
                let octets: [u8; 4] = row.get(at..at + 4)?.try_into().ok()?;
                Some(octets.into())
            };
            Some(Connection {
                state: dword(row, 0)?,
                local: format!("{}:{}", ip(4)?, port(row, 8)?),
                remote: format!("{}:{}", ip(12)?, port(row, 16)?),
                pid: dword(row, 20)?,
            })
        }
    })
    .collect()
}

/// The row count of MIB_UDPTABLE_OWNER_PID (12-byte rows) or
/// MIB_UDP6TABLE_OWNER_PID (28).
fn udp_count(bytes: &[u8], v6: bool) -> Option<usize> {
    Some(table_rows(bytes, if v6 { 28 } else { 12 })?.len())
}

fn push_internet_groups(
    section: &mut Section,
    proxy: Result<Proxy, String>,
    connections: Result<Connections, String>,
) {
    let mut group = Group::new("Internet options (WinInet)").collapsed();
    match proxy {
        Ok(proxy) => {
            group.push_row(Row::known(
                "Proxy",
                if proxy.enabled { "Enabled" } else { "Disabled" },
            ));
            group.push_row(
                Row::new(
                    "Proxy server",
                    Value::from_option(proxy.server, NONE_CONFIGURED),
                )
                .private(),
            );
            group.push_row(
                Row::new(
                    "Proxy bypass",
                    Value::from_option(proxy.bypass, NONE_CONFIGURED),
                )
                .private(),
            );
            group.push_row(
                Row::new(
                    "Automatic configuration script",
                    Value::from_option(proxy.script, NONE_CONFIGURED),
                )
                .private(),
            );
        }
        Err(reason) => group.push_row(Row::unavailable("Proxy", reason)),
    }
    section.push_group(group);

    let mut group = Group::new("Connections").collapsed();
    match connections {
        Ok(mut connections) => {
            let count = |state| connections.tcp.iter().filter(|c| c.state == state).count();
            let (established, listening) = (count(TCP_ESTABLISHED), count(TCP_LISTEN));
            group.push_row(Row::known(
                "TCP connections",
                format!(
                    "{} total: {established} established, {listening} listening, {} other",
                    connections.tcp.len(),
                    connections.tcp.len() - established - listening
                ),
            ));
            group.push_row(Row::known("UDP endpoints", connections.udp.to_string()));
            connections.tcp.retain(|c| c.state == TCP_ESTABLISHED);
            connections.tcp.sort_by_key(|c| c.pid);
            for connection in connections.tcp.iter().take(LISTED_CONNECTIONS) {
                group.push_row(
                    Row::known(
                        format!("PID {}", connection.pid),
                        format!("{} to {}", connection.local, connection.remote),
                    )
                    .private(),
                );
            }
            if connections.tcp.len() > LISTED_CONNECTIONS {
                group.push_row(Row::known(
                    "Listed",
                    format!(
                        "first {LISTED_CONNECTIONS} of {} established connections",
                        connections.tcp.len()
                    ),
                ));
            }
        }
        Err(reason) => group.push_row(Row::unavailable("Connections", reason)),
    }
    section.push_group(group);
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
        b if b >= 1_000_000 && b.is_multiple_of(1_000_000) => format!("{} Mbps", b / 1_000_000),
        b if b >= 1_000_000 => format!("{:.1} Mbps", b as f64 / 1e6),
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
        group.push_row(Row::known("Type", adapter_type(adapter)));
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
            Row::live("Throughput", live).note("Download / upload over the sampler interval"),
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
    #[cfg(windows)]
    {
        let mut section = build(native::adapters(ctx));
        if !section.groups.is_empty() {
            let proxy = native::proxy();
            let connections = if ctx.should_stop() {
                Err("read budget exhausted".to_string())
            } else {
                native::connections()
            };
            push_internet_groups(&mut section, proxy, connections);
        }
        section
    }
    #[cfg(not(windows))]
    {
        let _ = ctx;
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
        assert_eq!(speed_text(866_700_000), "866.7 Mbps");
        assert_eq!(speed_text(100_000_000), "100 Mbps");
        let pan = Adapter {
            kind: 6,
            medium: Some(MEDIUM_BLUETOOTH),
            ..Default::default()
        };
        assert_eq!(adapter_type(&pan), "Bluetooth (PAN)");
        let wired = Adapter {
            kind: 6,
            medium: Some(0),
            ..Default::default()
        };
        assert_eq!(adapter_type(&wired), "Ethernet");
    }

    #[test]
    fn socket_tables_parse_and_endpoints_stay_private() {
        // MIB_TCPTABLE_OWNER_PID: count, then state, local addr/port,
        // remote addr/port, PID (ports big-endian in their low word).
        let mut v4 = 2u32.to_le_bytes().to_vec();
        for (state, pid) in [(5u32, 4242u32), (2, 4)] {
            v4.extend(state.to_le_bytes());
            v4.extend([192, 0, 2, 44, 0x1F, 0x90, 0, 0]);
            v4.extend([198, 51, 100, 7, 0x01, 0xBB, 0, 0]);
            v4.extend(pid.to_le_bytes());
        }
        let tcp = parse_tcp(&v4, false).unwrap();
        assert_eq!(tcp[0].local, "192.0.2.44:8080");
        assert_eq!(tcp[0].remote, "198.51.100.7:443");
        assert_eq!((tcp[0].state, tcp[0].pid), (5, 4242));
        let mut v6 = 1u32.to_le_bytes().to_vec();
        let mut row = vec![0u8; 56];
        row[15] = 1;
        row[20..22].copy_from_slice(&[0x13, 0x88]);
        row[48..52].copy_from_slice(&2u32.to_le_bytes());
        row[52..56].copy_from_slice(&9u32.to_le_bytes());
        v6.extend(row);
        let tcp6 = parse_tcp(&v6, true).unwrap();
        assert_eq!(tcp6[0].local, "[::1]:5000");
        assert_eq!((tcp6[0].state, tcp6[0].pid), (2, 9));
        // A count larger than the buffer is rejected, never over-read.
        assert_eq!(parse_tcp(&v4[..30], false), None);
        assert_eq!(parse_tcp(&u32::MAX.to_le_bytes(), true), None);
        assert_eq!(udp_count(&[3, 0, 0, 0], false), None);
        assert_eq!(udp_count(&[0, 0, 0, 0], true), Some(0));

        let mut section = Section::new(SectionId::Network);
        push_internet_groups(
            &mut section,
            Ok(Proxy {
                enabled: true,
                server: Some("proxy.fixture.example:3128".into()),
                ..Default::default()
            }),
            Ok(Connections {
                tcp: tcp.into_iter().chain(tcp6).collect(),
                udp: 7,
            }),
        );
        let text = crate::specs::probe_text(std::slice::from_ref(&section));
        assert!(text.contains("Proxy: Enabled"), "{text}");
        assert!(
            text.contains("TCP connections: 3 total: 1 established, 2 listening, 0 other"),
            "{text}"
        );
        assert!(text.contains("UDP endpoints: 7"), "{text}");
        assert!(text.contains("PID 4242"), "{text}");
        for secret in ["proxy.fixture", "192.0.2.44", "198.51.100.7"] {
            assert!(!text.contains(secret), "{secret} leaked: {text}");
        }
        let mut failed = Section::new(SectionId::Network);
        push_internet_groups(&mut failed, Err("fixture".into()), Err("fixture".into()));
        let text = crate::specs::probe_text(std::slice::from_ref(&failed));
        assert!(
            text.contains("Connections: Unavailable (fixture)"),
            "{text}"
        );
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
