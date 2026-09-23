//! GetAdaptersAddresses (local configuration only, no traffic) plus SetupDi
//! network-class driver versions.
use super::*;
use crate::specs::native::NativeError;
use crate::specs::native::setupapi::{Filter, Query, devices};
use windows::Win32::Devices::DeviceAndDriverInstallation::GUID_DEVCLASS_NET;
use windows::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER;
use windows::Win32::Foundation::{ERROR_BUFFER_OVERFLOW, ERROR_NO_DATA, ERROR_SUCCESS};
use windows::Win32::NetworkManagement::IpHelper::{
    GAA_FLAG_INCLUDE_GATEWAYS, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_MULTICAST,
    GetAdaptersAddresses, GetExtendedTcpTable, GetExtendedUdpTable, GetIfEntry2,
    IP_ADAPTER_ADDRESSES_LH, MIB_IF_ROW2, TCP_TABLE_OWNER_PID_ALL, UDP_TABLE_OWNER_PID,
};
use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
use windows::Win32::Networking::WinSock::SOCKET_ADDRESS;

const AF_INET: u32 = 2;
const AF_INET6: u32 = 23;
const LARGEST_TABLE: u32 = 16 * 1024 * 1024;

/// WinInet's per-user proxy values (read-only HKCU query).
pub(super) fn proxy() -> Result<Proxy, String> {
    use crate::specs::native::registry::{self, Hive};
    const PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Internet Settings";
    let read = |name| registry::read(Hive::CurrentUser, PATH, name).map_err(|e| e.to_string());
    let text = |name| -> Result<Option<String>, String> {
        Ok(read(name)?
            .and_then(|v| v.text().map(|t| t.trim().to_string()))
            .filter(|t| !t.is_empty()))
    };
    Ok(Proxy {
        // WinInet treats a missing ProxyEnable as 0.
        enabled: read("ProxyEnable")?.and_then(|v| v.as_u64()).unwrap_or(0) != 0,
        server: text("ProxyServer")?,
        bypass: text("ProxyOverride")?,
        script: text("AutoConfigURL")?,
    })
}

/// One GetExtended*Table call, sized by Windows. The bytes are a copy.
fn owner_table(read: impl Fn(*mut std::ffi::c_void, &mut u32) -> u32) -> Result<Vec<u8>, String> {
    let mut size = 0u32;
    for _ in 0..4 {
        let mut buffer = vec![0u64; (size as usize).div_ceil(8).max(1)];
        let mut len = (buffer.len() * 8) as u32;
        let status = read(buffer.as_mut_ptr().cast(), &mut len);
        if status == ERROR_INSUFFICIENT_BUFFER.0 && len <= LARGEST_TABLE {
            size = len;
            continue;
        }
        if status != ERROR_SUCCESS.0 {
            return Err(
                NativeError::win32("GetExtendedTcpTable/GetExtendedUdpTable", status).to_string(),
            );
        }
        return Ok(buffer
            .iter()
            .flat_map(|w| w.to_ne_bytes())
            .take(len as usize)
            .collect());
    }
    Err("the socket table kept changing".to_string())
}

/// The kernel's TCP and UDP socket tables, IPv4 and IPv6. Read-only; nothing
/// is sent on the network.
pub(super) fn connections() -> Result<Connections, String> {
    let mut result = Connections::default();
    for (family, v6) in [(AF_INET, false), (AF_INET6, true)] {
        // SAFETY: `table` points at `size` writable, 8-byte aligned bytes.
        let tcp = owner_table(|table, size| unsafe {
            GetExtendedTcpTable(Some(table), size, false, family, TCP_TABLE_OWNER_PID_ALL, 0)
        })?;
        result
            .tcp
            .extend(parse_tcp(&tcp, v6).ok_or("malformed TCP table")?);
        // SAFETY: as above.
        let udp = owner_table(|table, size| unsafe {
            GetExtendedUdpTable(Some(table), size, false, family, UDP_TABLE_OWNER_PID, 0)
        })?;
        result.udp += udp_count(&udp, v6).ok_or("malformed UDP table")?;
    }
    Ok(result)
}

/// IP_ADAPTER_DHCP_ENABLED in IP_ADAPTER_ADDRESSES::Flags.
const DHCP_ENABLED: u32 = 0x4;
const AF_UNSPEC: u32 = 0;

fn address(socket: &SOCKET_ADDRESS) -> Option<String> {
    let length = usize::try_from(socket.iSockaddrLength).ok()?.min(128);
    if socket.lpSockaddr.is_null() || length < 2 {
        return None;
    }
    // SAFETY: Windows owns `length` readable bytes at lpSockaddr for the
    // lifetime of the adapter buffer the caller keeps alive.
    let bytes = unsafe { std::slice::from_raw_parts(socket.lpSockaddr.cast::<u8>(), length) };
    sockaddr_text(bytes)
}

fn wide(text: windows::core::PWSTR) -> Option<String> {
    if text.is_null() {
        return None;
    }
    // SAFETY: a NUL-terminated string inside the adapter buffer.
    let value = unsafe { text.to_string() }.ok()?;
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}

/// The adapter's physical medium (read-only GetIfEntry2 on its LUID).
fn physical_medium(node: &IP_ADAPTER_ADDRESSES_LH) -> Option<i32> {
    let mut row = MIB_IF_ROW2 {
        InterfaceLuid: node.Luid,
        ..Default::default()
    };
    // SAFETY: a writable row whose InterfaceLuid selects the interface.
    let status = unsafe { GetIfEntry2(&mut row) };
    (status == ERROR_SUCCESS).then_some(row.PhysicalMediumType.0)
}

pub(super) fn adapters(ctx: &Context) -> Result<Vec<Adapter>, String> {
    let flags = GAA_FLAG_INCLUDE_GATEWAYS | GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST;
    let mut size = 16 * 1024u32;
    let mut buffer = Vec::<u64>::new();
    let mut filled = false;
    for _ in 0..4 {
        buffer = vec![0u64; (size as usize).div_ceil(8)];
        // SAFETY: the buffer holds `size` bytes, 8-byte aligned.
        let status = unsafe {
            GetAdaptersAddresses(
                AF_UNSPEC,
                flags,
                None,
                Some(buffer.as_mut_ptr().cast()),
                &mut size,
            )
        };
        if status == ERROR_BUFFER_OVERFLOW.0 && size <= 4 * 1024 * 1024 {
            continue;
        }
        if status == ERROR_NO_DATA.0 {
            return Ok(Vec::new());
        }
        if status != ERROR_SUCCESS.0 {
            return Err(NativeError::win32("GetAdaptersAddresses", status).to_string());
        }
        filled = true;
        break;
    }
    if !filled {
        // Never walk a buffer Windows did not fill.
        return Err("GetAdaptersAddresses: the adapter list kept changing".to_string());
    }
    let drivers = devices(&Query::new(Filter::Class(GUID_DEVCLASS_NET), ctx)).unwrap_or_default();
    let mut adapters = Vec::new();
    let mut current = buffer.as_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
    let mut guard = 0;
    while !current.is_null() && guard < 256 {
        guard += 1;
        // SAFETY: each node lives inside `buffer`; Next is null-terminated.
        let node = unsafe { &*current };
        let mut adapter = Adapter {
            alias: wide(node.FriendlyName).unwrap_or_else(|| "Network adapter".into()),
            description: wide(node.Description),
            kind: node.IfType,
            medium: physical_medium(node),
            up: node.OperStatus == IfOperStatusUp,
            speed_bps: Some(node.TransmitLinkSpeed.max(node.ReceiveLinkSpeed)),
            mtu: Some(node.Mtu),
            mac: mac_text(
                node.PhysicalAddress
                    .get(..(node.PhysicalAddressLength as usize).min(8))
                    .unwrap_or(&[]),
            ),
            // SAFETY: Flags is the documented view of this union.
            dhcp: unsafe { node.Anonymous2.Flags } & DHCP_ENABLED != 0,
            dhcp_server: address(&node.Dhcpv4Server),
            suffix: wide(node.DnsSuffix),
            ..Default::default()
        };
        let mut unicast = node.FirstUnicastAddress;
        while !unicast.is_null() {
            // SAFETY: list node inside `buffer`.
            let item = unsafe { &*unicast };
            if let Some(text) = address(&item.Address) {
                if text.contains(':') {
                    adapter.ipv6.push(text);
                } else {
                    adapter.ipv4.push(text);
                }
            }
            unicast = item.Next;
        }
        let mut gateway = node.FirstGatewayAddress;
        while !gateway.is_null() {
            // SAFETY: list node inside `buffer`.
            let item = unsafe { &*gateway };
            adapter.gateways.extend(address(&item.Address));
            gateway = item.Next;
        }
        let mut dns = node.FirstDnsServerAddress;
        while !dns.is_null() {
            // SAFETY: list node inside `buffer`.
            let item = unsafe { &*dns };
            adapter.dns.extend(address(&item.Address));
            dns = item.Next;
        }
        adapter.driver = adapter.description.as_ref().and_then(|description| {
            // Windows appends " #2" to the description of additional instances.
            let base = match description.rsplit_once(" #") {
                Some((base, n)) if n.bytes().all(|b| b.is_ascii_digit()) => base,
                _ => description.as_str(),
            };
            drivers
                .iter()
                .find(|d| {
                    [d.friendly_name.as_deref(), d.description.as_deref()]
                        .into_iter()
                        .flatten()
                        .any(|n| {
                            n.eq_ignore_ascii_case(description) || n.eq_ignore_ascii_case(base)
                        })
                })
                .and_then(|d| {
                    let version = d.driver_version.clone()?;
                    Some(match &d.driver_date {
                        Some(date) => format!("{version} ({date})"),
                        None => version,
                    })
                })
        });
        adapters.push(adapter);
        current = node.Next;
    }
    Ok(adapters)
}
