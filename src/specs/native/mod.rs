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
    fn utf16_helpers_stop_at_nul_and_reject_odd_lengths() {
        assert_eq!(utf16_until_nul(&[0x41, 0x42, 0, 0x43]), "AB");
        assert_eq!(
            utf16_bytes_until_nul(&[0x41, 0, 0x42, 0]).as_deref(),
            Some("AB")
        );
        assert_eq!(utf16_bytes_until_nul(&[0x41]), None);
    }
}
