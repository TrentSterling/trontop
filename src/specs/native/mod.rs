//! Shared read-only native helpers for specs providers. Each helper only
//! queries: no drivers, no elevation, no writes, no network requests. Callers
//! run on specs worker threads, never on the UI thread.
//!
//! Byte/string decoders are portable and unit-tested; the Windows calls that
//! feed them are `#[cfg(windows)]`.
pub mod registry;
pub mod setupapi;
pub mod smbios;
pub mod wmi;

use std::time::Duration;

/// Errors carry only an API name and a numeric code, never native message text,
/// paths or identifiers, so `to_string()` is safe as an Unavailable reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeError {
    /// `code` is the Win32 error or the HRESULT bits.
    Windows {
        api: &'static str,
        code: u32,
    },
    AccessDenied {
        api: &'static str,
    },
    Timeout {
        api: &'static str,
        after: Duration,
    },
    Malformed(&'static str),
    /// A complete, human-readable reason such as "WMI namespace not present".
    Unsupported(&'static str),
}

impl std::fmt::Display for NativeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Windows { api, code } => write!(f, "{api} failed (Windows error 0x{code:08X})"),
            Self::AccessDenied { api } => {
                write!(f, "requires administrator ({api}: access denied)")
            }
            Self::Timeout { api, after } => write!(
                f,
                "{api} did not answer within {:.1} s",
                after.as_secs_f64()
            ),
            Self::Malformed(what) => write!(f, "malformed data: {what}"),
            Self::Unsupported(reason) => f.write_str(reason),
        }
    }
}

impl NativeError {
    const E_ACCESSDENIED: u32 = 0x8007_0005;
    const WBEM_E_ACCESS_DENIED: u32 = 0x8004_1003;
    const ERROR_ACCESS_DENIED: u32 = 5;

    /// From a Win32 error code (for APIs returning WIN32_ERROR / GetLastError).
    pub fn win32(api: &'static str, code: u32) -> Self {
        if code == Self::ERROR_ACCESS_DENIED {
            Self::AccessDenied { api }
        } else {
            Self::Windows { api, code }
        }
    }

    /// From an HRESULT value (for COM and windows::core::Result APIs).
    pub fn hresult(api: &'static str, code: i32) -> Self {
        let code = code as u32;
        if code == Self::E_ACCESSDENIED || code == Self::WBEM_E_ACCESS_DENIED {
            Self::AccessDenied { api }
        } else {
            Self::Windows { api, code }
        }
    }

    #[cfg(windows)]
    pub fn from_windows(api: &'static str, error: &windows::core::Error) -> Self {
        Self::hresult(api, error.code().0)
    }

    /// GetLastError after an API that signals failure through its return value.
    #[cfg(windows)]
    pub fn last_error(api: &'static str) -> Self {
        Self::from_windows(api, &windows::core::Error::from_thread())
    }
}

/// PCI identity parsed from a hardware ID such as
/// `PCI\VEN_10DE&DEV_2C05&SUBSYS_205B196E&REV_A1`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PciId {
    pub vendor: u16,
    pub device: u16,
    /// Subsystem vendor (board partner) and subsystem device.
    pub subsystem: Option<(u16, u16)>,
    pub revision: Option<u8>,
}

impl PciId {
    /// The most specific `PCI\VEN_...` entry of a hardware ID list.
    pub fn parse(hardware_ids: &[String]) -> Option<Self> {
        hardware_ids
            .iter()
            .filter_map(|id| Self::parse_one(id))
            .max_by_key(|id| u8::from(id.subsystem.is_some()) + u8::from(id.revision.is_some()))
    }

    fn parse_one(id: &str) -> Option<Self> {
        let rest = id
            .get(..4)?
            .eq_ignore_ascii_case("PCI\\")
            .then(|| &id[4..])?;
        let mut parsed = Self::default();
        let mut vendor = None;
        let mut device = None;
        for part in rest.split('&') {
            let (key, value) = part.split_at_checked(part.find('_')?)?;
            let value = &value[1..];
            match key.to_ascii_uppercase().as_str() {
                "VEN" => vendor = u16::from_str_radix(value, 16).ok(),
                "DEV" => device = u16::from_str_radix(value, 16).ok(),
                "SUBSYS" if value.len() == 8 => {
                    let device = u16::from_str_radix(&value[..4], 16).ok()?;
                    let vendor = u16::from_str_radix(&value[4..], 16).ok()?;
                    parsed.subsystem = Some((vendor, device));
                }
                "REV" => parsed.revision = u8::from_str_radix(value, 16).ok(),
                _ => {}
            }
        }
        parsed.vendor = vendor?;
        parsed.device = device?;
        Some(parsed)
    }
}

/// A PCI Express link from the DEVPKEY_PciDevice_*LinkSpeed/Width codes
/// (pciprop.h: speed 1 = 2.5 GT/s ... 6 = 64 GT/s; width in lanes).
pub fn pcie_link(speed: Option<u32>, width: Option<u32>) -> Option<String> {
    let generation = match speed? {
        1 => "1.x",
        2 => "2.0",
        3 => "3.0",
        4 => "4.0",
        5 => "5.0",
        6 => "6.0",
        _ => return None,
    };
    Some(match width.filter(|w| (1..=32).contains(w)) {
        Some(width) => format!("PCIe {generation} x{width}"),
        None => format!("PCIe {generation}"),
    })
}

/// DEVPKEY_PciDevice_CurrentLinkSpeed, CurrentLinkWidth, MaxLinkSpeed and
/// MaxLinkWidth, in that order, for `setupapi::Query::extra`.
#[cfg(windows)]
pub const PCIE_LINK_KEYS: [windows::Win32::Foundation::DEVPROPKEY; 4] = [
    windows::Win32::NetworkManagement::WiFi::DEVPKEY_PciDevice_CurrentLinkSpeed,
    windows::Win32::NetworkManagement::WiFi::DEVPKEY_PciDevice_CurrentLinkWidth,
    windows::Win32::NetworkManagement::WiFi::DEVPKEY_PciDevice_MaxLinkSpeed,
    windows::Win32::NetworkManagement::WiFi::DEVPKEY_PciDevice_MaxLinkWidth,
];

/// "PCIe 5.0 x16 (maximum PCIe 5.0 x16)" from four `PCIE_LINK_KEYS` values.
pub fn pcie_link_text(values: &[Option<setupapi::PropertyValue>]) -> Option<String> {
    let number = |index: usize| match values.get(index)? {
        Some(setupapi::PropertyValue::U32(v)) => Some(*v),
        _ => None,
    };
    let current = pcie_link(number(0), number(1));
    let maximum = pcie_link(number(2), number(3));
    match (current, maximum) {
        (Some(current), Some(maximum)) if current == maximum => Some(current),
        (Some(current), Some(maximum)) => Some(format!("{current} (maximum {maximum})")),
        (Some(current), None) => Some(current),
        (None, Some(maximum)) => Some(format!("maximum {maximum}; current link not reported")),
        (None, None) => None,
    }
}

/// PCI-SIG vendor IDs of common PC component and board makers.
pub fn pci_vendor(id: u16) -> Option<&'static str> {
    Some(match id {
        0x8086 => "Intel",
        0x1022 => "AMD",
        0x1002 => "AMD (ATI)",
        0x10DE => "NVIDIA",
        0x1043 => "ASUS",
        0x1458 => "Gigabyte",
        0x1462 => "MSI",
        0x1849 => "ASRock",
        0x196E => "PNY",
        0x19DA => "Zotac",
        0x3842 => "EVGA",
        0x1569 => "Palit",
        0x1B4C => "Galax (KFA2)",
        0x7377 => "Colorful",
        0x1DA2 => "Sapphire",
        0x148C => "PowerColor",
        0x1682 => "XFX",
        0x1028 => "Dell",
        0x103C => "HP",
        0x17AA => "Lenovo",
        0x144D => "Samsung",
        0x15B7 => "Western Digital (SanDisk)",
        0x1C5C => "SK hynix",
        0x1987 => "Phison",
        0x10EC => "Realtek",
        0x14E4 => "Broadcom",
        0x168C => "Qualcomm Atheros",
        0x17CB => "Qualcomm",
        0x1B21 => "ASMedia",
        0x1344 => "Micron",
        0x1E0F => "KIOXIA",
        0x1D97 => "Shenzhen Longsys (Lexar)",
        0x10EE => "Xilinx",
        0x14C3 => "MediaTek",
        _ => return None,
    })
}

/// UTF-16 up to the first NUL (or the whole slice), lossily decoded.
pub fn utf16_until_nul(words: &[u16]) -> String {
    let end = words.iter().position(|w| *w == 0).unwrap_or(words.len());
    String::from_utf16_lossy(&words[..end])
}

/// Little-endian UTF-16 bytes up to the first NUL. None for odd lengths.
pub fn utf16_bytes_until_nul(bytes: &[u8]) -> Option<String> {
    if !bytes.len().is_multiple_of(2) {
        return None;
    }
    let words = bytes
        .chunks_exact(2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .collect::<Vec<_>>();
    Some(utf16_until_nul(&words))
}

/// A Windows FILETIME (100 ns ticks since 1601-01-01 UTC) as "YYYY-MM-DD".
/// None for zero or out-of-range values. Driver dates are UTC midnight.
pub fn filetime_date(filetime: u64) -> Option<String> {
    const TICKS_PER_DAY: u64 = 864_000_000_000;
    const DAYS_1601_TO_1970: i64 = 134_774;
    if filetime == 0 {
        return None;
    }
    let days = (filetime / TICKS_PER_DAY) as i64 - DAYS_1601_TO_1970;
    let (year, month, day) = civil_from_days(days);
    (1601..=9999)
        .contains(&year)
        .then(|| format!("{year:04}-{month:02}-{day:02}"))
}

/// Howard Hinnant's days-from-civil inverse; days relative to 1970-01-01.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filetime_dates_convert_exactly() {
        assert_eq!(
            filetime_date(116_444_736_000_000_000).as_deref(),
            Some("1970-01-01")
        );
        // 2024-02-29 00:00:00 UTC.
        assert_eq!(
            filetime_date(133_536_384_000_000_000).as_deref(),
            Some("2024-02-29")
        );
        assert_eq!(filetime_date(0), None);
        assert_eq!(filetime_date(u64::MAX), None);
    }

    #[test]
    fn errors_map_access_denied_and_never_include_native_text() {
        assert_eq!(
            NativeError::win32("RegOpenKeyExW", 5).to_string(),
            "requires administrator (RegOpenKeyExW: access denied)"
        );
        assert_eq!(
            NativeError::hresult("IWbemServices::ExecQuery", 0x8004_1003_u32 as i32),
            NativeError::AccessDenied {
                api: "IWbemServices::ExecQuery"
            }
        );
        assert_eq!(
            NativeError::hresult("CoCreateInstance", 0x8000_4005_u32 as i32).to_string(),
            "CoCreateInstance failed (Windows error 0x80004005)"
        );
    }

    #[test]
    fn pci_ids_parse_the_most_specific_entry() {
        let ids = [
            r"PCI\VEN_10DE&DEV_2C05&SUBSYS_205B196E&REV_A1".to_string(),
            r"PCI\VEN_10DE&DEV_2C05&SUBSYS_205B196E".to_string(),
            r"PCI\VEN_10DE&DEV_2C05".to_string(),
            r"PCI\CC_030000".to_string(),
        ];
        let id = PciId::parse(&ids).unwrap();
        assert_eq!((id.vendor, id.device), (0x10DE, 0x2C05));
        assert_eq!(id.subsystem, Some((0x196E, 0x205B)));
        assert_eq!(id.revision, Some(0xA1));
        assert_eq!(pci_vendor(0x196E), Some("PNY"));
        use setupapi::PropertyValue::U32;
        assert_eq!(
            pcie_link_text(&[Some(U32(4)), Some(U32(16)), Some(U32(5)), Some(U32(16))]).as_deref(),
            Some("PCIe 4.0 x16 (maximum PCIe 5.0 x16)")
        );
        assert_eq!(
            pcie_link_text(&[Some(U32(5)), Some(U32(4)), Some(U32(5)), Some(U32(4))]).as_deref(),
            Some("PCIe 5.0 x4")
        );
        assert_eq!(pcie_link_text(&[None, None, None, None]), None);
        assert_eq!(pcie_link(Some(9), Some(4)), None);
        assert_eq!(PciId::parse(&[r"USB\VID_046D".to_string()]), None);
        assert_eq!(PciId::parse(&[r"PCI\VEN_ZZZZ&DEV_1".to_string()]), None);
    }

    #[test]
    fn utf16_helpers_stop_at_nul_and_reject_odd_lengths() {
        assert_eq!(utf16_until_nul(&[0x41, 0x42, 0, 0x43]), "AB");
        assert_eq!(
            utf16_bytes_until_nul(&[0x41, 0, 0x42, 0]).as_deref(),
            Some("AB")
        );
        assert_eq!(utf16_bytes_until_nul(&[0x41]), None);
    }
}
