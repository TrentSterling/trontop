//! Minimal synchronous WMI queries for specs worker threads.
//!
//! `Wmi::connect` initializes COM (multithreaded) for the calling thread and
//! keeps it initialized while the connection lives. Create, use and drop a
//! `Wmi` on the same worker thread; it is deliberately neither Send nor Sync.
//! Process-wide COM security is never changed: the proxy blanket is set per
//! connection. Queries are forward-only with a caller-supplied deadline.
use super::NativeError;
use std::time::{Duration, Instant};

/// Rows beyond this are dropped and reported as an error, never silently cut.
const MAX_ROWS: usize = 4096;
const MAX_ARRAY_ITEMS: usize = 4096;

#[derive(Clone, Debug, PartialEq)]
pub enum WmiValue {
    Null,
    Bool(bool),
    Int(i64),
    UInt(u64),
    Real(f64),
    /// Also CIM_DATETIME values, as DMTF text; see `dmtf_date`.
    Text(String),
    Array(Vec<WmiValue>),
    /// A VARIANT type this helper does not convert; the raw VT number.
    Unsupported(u16),
}

impl WmiValue {
    /// Trimmed non-empty text, or a decimal rendering of a number.
    pub fn as_text(&self) -> Option<String> {
        match self {
            Self::Text(text) => {
                let text = text.trim();
                (!text.is_empty()).then(|| text.to_string())
            }
            Self::Int(value) => Some(value.to_string()),
            Self::UInt(value) => Some(value.to_string()),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Self::UInt(value) => Some(*value),
            Self::Int(value) => u64::try_from(*value).ok(),
            Self::Text(text) => text.trim().parse().ok(),
            _ => None,
        }
    }

    #[cfg(test)]
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(value) => Some(*value),
            Self::UInt(value) => i64::try_from(*value).ok(),
            Self::Text(text) => text.trim().parse().ok(),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Real(value) => Some(*value),
            Self::Int(value) => Some(*value as f64),
            Self::UInt(value) => Some(*value as f64),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    /// Every non-empty text item of an array, or the single text value.
    pub fn texts(&self) -> Vec<String> {
        match self {
            Self::Array(items) => items.iter().filter_map(Self::as_text).collect(),
            other => other.as_text().into_iter().collect(),
        }
    }
}

/// One result object: its non-system properties in WMI's order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WmiRow {
    pub properties: Vec<(String, WmiValue)>,
}

impl WmiRow {
    /// Property names compare ASCII case-insensitively, like WQL.
    pub fn get(&self, name: &str) -> Option<&WmiValue> {
        self.properties
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value)
    }

    pub fn text(&self, name: &str) -> Option<String> {
        self.get(name)?.as_text()
    }

    pub fn u64(&self, name: &str) -> Option<u64> {
        self.get(name)?.as_u64()
    }

    #[cfg(test)]
    pub fn i64(&self, name: &str) -> Option<i64> {
        self.get(name)?.as_i64()
    }

    pub fn f64(&self, name: &str) -> Option<f64> {
        self.get(name)?.as_f64()
    }

    pub fn bool(&self, name: &str) -> Option<bool> {
        self.get(name)?.as_bool()
    }

    pub fn texts(&self, name: &str) -> Vec<String> {
        self.get(name).map(WmiValue::texts).unwrap_or_default()
    }
}

/// "20240315000000.000000+000" as "2024-03-15". None for malformed text.
pub fn dmtf_date(text: &str) -> Option<String> {
    let digits = text.get(..8)?;
    if !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let (year, month, day) = (&digits[..4], &digits[4..6], &digits[6..8]);
    let valid = matches!(month.parse::<u8>(), Ok(1..=12))
        && matches!(day.parse::<u8>(), Ok(1..=31))
        && year != "0000";
    valid.then(|| format!("{year}-{month}-{day}"))
}

#[cfg(windows)]
pub use native::{Wmi, query_once};

#[cfg(windows)]
mod native {
    use super::*;
    use std::marker::PhantomData;
    use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
        CoSetProxyBlanket, CoUninitialize, EOAC_NONE, RPC_C_AUTHN_LEVEL_CALL,
        RPC_C_IMP_LEVEL_IMPERSONATE, SAFEARRAY,
    };
    use windows::Win32::System::Ole::{
        SafeArrayGetDim, SafeArrayGetElement, SafeArrayGetLBound, SafeArrayGetUBound,
        SafeArrayGetVartype,
    };
    use windows::Win32::System::Variant::{VARIANT, VariantClear};
    use windows::Win32::System::Wmi::{
        CIM_DATETIME, CIM_SINT64, CIM_UINT64, IEnumWbemClassObject, IWbemClassObject, IWbemContext,
        IWbemLocator, IWbemServices, WBEM_E_INVALID_CLASS, WBEM_E_INVALID_NAMESPACE,
        WBEM_E_INVALID_QUERY, WBEM_FLAG_CONNECT_USE_MAX_WAIT, WBEM_FLAG_FORWARD_ONLY,
        WBEM_FLAG_NONSYSTEM_ONLY, WBEM_FLAG_RETURN_IMMEDIATELY, WBEM_S_FALSE, WBEM_S_TIMEDOUT,
        WbemLocator,
    };
    use windows::core::{BSTR, PCWSTR};

    // rpcdce.h; avoids pulling the whole RPC feature for two constants.
    const RPC_C_AUTHN_WINNT: u32 = 10;
    const RPC_C_AUTHZ_NONE: u32 = 0;

    /// Balances one successful CoInitializeEx on this thread.
    struct Com {
        balanced: bool,
        // COM apartment state is per thread.
        _thread_bound: PhantomData<*const ()>,
    }

    impl Com {
        fn init() -> Result<Self, NativeError> {
            // SAFETY: reserved pointer is None; paired with CoUninitialize on drop.
            let result = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            if result == RPC_E_CHANGED_MODE {
                // The thread already has an STA; COM is usable, but not ours to undo.
                return Ok(Self {
                    balanced: false,
                    _thread_bound: PhantomData,
                });
            }
            result
                .ok()
                .map_err(|e| NativeError::from_windows("CoInitializeEx", &e))?;
            Ok(Self {
                balanced: true,
                _thread_bound: PhantomData,
            })
        }
    }

    impl Drop for Com {
        fn drop(&mut self) {
            if self.balanced {
                // SAFETY: balances this guard's successful CoInitializeEx.
                unsafe { CoUninitialize() };
            }
        }
    }

    fn wmi_error(api: &'static str, error: &windows::core::Error) -> NativeError {
        let code = error.code().0;
        if code == WBEM_E_INVALID_NAMESPACE.0 {
            NativeError::Unsupported("WMI namespace not present")
        } else if code == WBEM_E_INVALID_CLASS.0 {
            NativeError::Unsupported("WMI class not present")
        } else if code == WBEM_E_INVALID_QUERY.0 {
            NativeError::Malformed("invalid WQL query")
        } else {
            NativeError::from_windows(api, error)
        }
    }

    /// A connection to one namespace, bound to the creating thread.
    pub struct Wmi {
        // Field order matters: release the proxy before COM uninitializes.
        services: IWbemServices,
        _com: Com,
    }

    impl Wmi {
        /// `namespace` such as `r"ROOT\CIMV2"` or `r"ROOT\WMI"`. A missing
        /// namespace returns `NativeError::Unsupported`.
        pub fn connect(namespace: &str) -> Result<Self, NativeError> {
            let com = Com::init()?;
            // SAFETY: documented in-process WMI locator class.
            let locator: IWbemLocator =
                unsafe { CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER) }
                    .map_err(|e| NativeError::from_windows("CoCreateInstance(WbemLocator)", &e))?;
            let empty = BSTR::new();
            // SAFETY: local connection, current user, no context object.
            let services = unsafe {
                locator.ConnectServer(
                    &BSTR::from(namespace),
                    &empty,
                    &empty,
                    &empty,
                    WBEM_FLAG_CONNECT_USE_MAX_WAIT.0,
                    &empty,
                    None::<&IWbemContext>,
                )
            }
            .map_err(|e| wmi_error("IWbemLocator::ConnectServer", &e))?;
            // SAFETY: live proxy; impersonation for the local call only.
            unsafe {
                CoSetProxyBlanket(
                    &services,
                    RPC_C_AUTHN_WINNT,
                    RPC_C_AUTHZ_NONE,
                    PCWSTR::null(),
                    RPC_C_AUTHN_LEVEL_CALL,
                    RPC_C_IMP_LEVEL_IMPERSONATE,
                    None,
                    EOAC_NONE,
                )
            }
            .map_err(|e| NativeError::from_windows("CoSetProxyBlanket", &e))?;
            Ok(Self {
                services,
                _com: com,
            })
        }

        /// Runs one WQL query. Returns `NativeError::Timeout` when the whole
        /// result set is not delivered within `timeout`; partial rows are
        /// discarded rather than shown as complete.
        pub fn query(&self, wql: &str, timeout: Duration) -> Result<Vec<WmiRow>, NativeError> {
            const API: &str = "IWbemServices::ExecQuery";
            let deadline = Instant::now() + timeout;
            // SAFETY: live service proxy and owned BSTR arguments.
            let enumerator: IEnumWbemClassObject = unsafe {
                self.services.ExecQuery(
                    &BSTR::from("WQL"),
                    &BSTR::from(wql),
                    WBEM_FLAG_FORWARD_ONLY | WBEM_FLAG_RETURN_IMMEDIATELY,
                    None::<&IWbemContext>,
                )
            }
            .map_err(|e| wmi_error(API, &e))?;
            let mut rows = Vec::new();
            loop {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(NativeError::Timeout {
                        api: API,
                        after: timeout,
                    });
                }
                let wait = remaining.as_millis().clamp(1, i32::MAX as u128) as i32;
                let mut objects: [Option<IWbemClassObject>; 16] = Default::default();
                let mut returned = 0u32;
                // SAFETY: writable object slots and count; slots are owned after return.
                let status = unsafe { enumerator.Next(wait, &mut objects, &mut returned) };
                if status.0 == WBEM_S_TIMEDOUT.0 {
                    return Err(NativeError::Timeout {
                        api: API,
                        after: timeout,
                    });
                }
                status
                    .ok()
                    .map_err(|e| wmi_error("IEnumWbemClassObject::Next", &e))?;
                for object in objects.iter().take(returned as usize).flatten() {
                    if rows.len() == MAX_ROWS {
                        return Err(NativeError::Malformed("WMI result exceeds 4096 rows"));
                    }
                    rows.push(row(object)?);
                }
                if status.0 == WBEM_S_FALSE.0 || returned == 0 {
                    return Ok(rows);
                }
            }
        }
    }

    /// Connects, runs one query and disconnects. Prefer one `Wmi` per worker
    /// cycle when a provider issues several queries against one namespace.
    pub fn query_once(
        namespace: &str,
        wql: &str,
        timeout: Duration,
    ) -> Result<Vec<WmiRow>, NativeError> {
        Wmi::connect(namespace)?.query(wql, timeout)
    }

    fn row(object: &IWbemClassObject) -> Result<WmiRow, NativeError> {
        const API: &str = "IWbemClassObject::Next";
        // SAFETY: live object; enumeration is ended below on every path.
        unsafe { object.BeginEnumeration(WBEM_FLAG_NONSYSTEM_ONLY.0) }
            .map_err(|e| NativeError::from_windows("IWbemClassObject::BeginEnumeration", &e))?;
        let mut properties = Vec::new();
        let result = loop {
            let mut name = BSTR::new();
            let mut value = VARIANT::default();
            let mut cim = 0i32;
            // SAFETY: owned output BSTR/VARIANT; flavor is not requested.
            let next =
                unsafe { object.Next(0, &mut name, &mut value, &mut cim, std::ptr::null_mut()) };
            if let Err(error) = next {
                break Err(NativeError::from_windows(API, &error));
            }
            // WBEM_S_NO_MORE_DATA is a success code with no name.
            if name.is_empty() {
                break Ok(());
            }
            let converted = convert(&value, cim);
            // SAFETY: clears the VARIANT this loop owns, freeing BSTR/SAFEARRAY data.
            let _ = unsafe { VariantClear(&mut value) };
            properties.push((name.to_string(), converted));
            if properties.len() > 1024 {
                break Err(NativeError::Malformed("WMI object has too many properties"));
            }
        };
        // SAFETY: matches the BeginEnumeration above.
        let _ = unsafe { object.EndEnumeration() };
        result.map(|()| WmiRow { properties })
    }

    const VT_EMPTY: u16 = 0;
    const VT_NULL: u16 = 1;
    const VT_I2: u16 = 2;
    const VT_I4: u16 = 3;
    const VT_R4: u16 = 4;
    const VT_R8: u16 = 5;
    const VT_BSTR: u16 = 8;
    const VT_BOOL: u16 = 11;
    const VT_I1: u16 = 16;
    const VT_UI1: u16 = 17;
    const VT_UI2: u16 = 18;
    const VT_UI4: u16 = 19;
    const VT_I8: u16 = 20;
    const VT_UI8: u16 = 21;
    const VT_INT: u16 = 22;
    const VT_UINT: u16 = 23;
    const VT_ARRAY: u16 = 0x2000;
    const VT_BYREF: u16 = 0x4000;

    fn convert(value: &VARIANT, cim: i32) -> WmiValue {
        // SAFETY: vt selects the active union member; nothing is taken out of
        // the VARIANT, which the caller clears afterwards.
        unsafe {
            let inner = &*value.Anonymous.Anonymous;
            let vt = inner.vt.0;
            let data = &inner.Anonymous;
            if vt & VT_BYREF != 0 {
                return WmiValue::Unsupported(vt);
            }
            if vt & VT_ARRAY != 0 {
                return array(data.parray, vt & !VT_ARRAY);
            }
            match vt {
                VT_EMPTY | VT_NULL => WmiValue::Null,
                VT_BOOL => WmiValue::Bool(data.boolVal.0 != 0),
                VT_I1 => WmiValue::Int(i64::from(data.cVal)),
                VT_I2 => WmiValue::Int(i64::from(data.iVal)),
                VT_I4 | VT_INT => WmiValue::Int(i64::from(data.lVal)),
                VT_I8 => WmiValue::Int(data.llVal),
                VT_UI1 => WmiValue::UInt(u64::from(data.bVal)),
                VT_UI2 => WmiValue::UInt(u64::from(data.uiVal)),
                VT_UI4 | VT_UINT => WmiValue::UInt(u64::from(data.ulVal)),
                VT_UI8 => WmiValue::UInt(data.ullVal),
                VT_R4 => WmiValue::Real(f64::from(data.fltVal)),
                VT_R8 => WmiValue::Real(data.dblVal),
                VT_BSTR => {
                    let text = (*data.bstrVal).to_string();
                    // WMI transports 64-bit integers as strings.
                    match cim {
                        c if c == CIM_UINT64.0 => {
                            text.parse().map_or(WmiValue::Text(text), WmiValue::UInt)
                        }
                        c if c == CIM_SINT64.0 => {
                            text.parse().map_or(WmiValue::Text(text), WmiValue::Int)
                        }
                        c if c == CIM_DATETIME.0 => WmiValue::Text(text),
                        _ => WmiValue::Text(text),
                    }
                }
                other => WmiValue::Unsupported(other),
            }
        }
    }

    /// One-dimensional arrays of the element types WMI actually uses.
    unsafe fn array(psa: *mut SAFEARRAY, element: u16) -> WmiValue {
        if psa.is_null() {
            return WmiValue::Null;
        }
        // SAFETY (whole fn): psa is the live array owned by the caller's
        // VARIANT; each element is copied into a typed local of matching size.
        unsafe {
            if SafeArrayGetDim(psa) != 1 {
                return WmiValue::Unsupported(VT_ARRAY | element);
            }
            let stored = SafeArrayGetVartype(psa).map_or(element, |v| v.0);
            let (Ok(lower), Ok(upper)) = (SafeArrayGetLBound(psa, 1), SafeArrayGetUBound(psa, 1))
            else {
                return WmiValue::Unsupported(VT_ARRAY | element);
            };
            let mut items = Vec::new();
            for index in lower..=upper {
                if items.len() == MAX_ARRAY_ITEMS {
                    break;
                }
                let at = &index as *const i32;
                macro_rules! element {
                    ($ty:ty, $wrap:expr) => {{
                        let mut item: $ty = Default::default();
                        match SafeArrayGetElement(psa, at, (&mut item as *mut $ty).cast()) {
                            Ok(()) => $wrap(item),
                            Err(_) => WmiValue::Null,
                        }
                    }};
                }
                items.push(match stored {
                    VT_BSTR => element!(BSTR, |b: BSTR| WmiValue::Text(b.to_string())),
                    VT_BOOL => element!(i16, |v: i16| WmiValue::Bool(v != 0)),
                    VT_I1 => element!(i8, |v: i8| WmiValue::Int(i64::from(v))),
                    VT_I2 => element!(i16, |v: i16| WmiValue::Int(i64::from(v))),
                    VT_I4 | VT_INT => element!(i32, |v: i32| WmiValue::Int(i64::from(v))),
                    VT_I8 => element!(i64, WmiValue::Int),
                    VT_UI1 => element!(u8, |v: u8| WmiValue::UInt(u64::from(v))),
                    VT_UI2 => element!(u16, |v: u16| WmiValue::UInt(u64::from(v))),
                    VT_UI4 | VT_UINT => element!(u32, |v: u32| WmiValue::UInt(u64::from(v))),
                    VT_UI8 => element!(u64, WmiValue::UInt),
                    VT_R4 => element!(f32, |v: f32| WmiValue::Real(f64::from(v))),
                    VT_R8 => element!(f64, WmiValue::Real),
                    other => return WmiValue::Unsupported(VT_ARRAY | other),
                });
            }
            WmiValue::Array(items)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        #[ignore = "Read-only WMI queries on this worker thread; no writes, window or input"]
        fn native_specs_wmi_read_only_probe() {
            let started = Instant::now();
            let wmi = Wmi::connect(r"ROOT\CIMV2").expect("CIMV2");
            let connected = started.elapsed();
            for wql in [
                "SELECT Caption, Version, BuildNumber, OSArchitecture FROM Win32_OperatingSystem",
                "SELECT Name, NumberOfCores, NumberOfLogicalProcessors, L2CacheSize, L3CacheSize FROM Win32_Processor",
                "SELECT Name, Manufacturer, Status FROM Win32_SoundDevice",
            ] {
                let at = Instant::now();
                let rows = wmi.query(wql, Duration::from_secs(10));
                println!(
                    "{wql}\n  -> {:.3} ms: {rows:?}",
                    at.elapsed().as_secs_f64() * 1000.0
                );
            }
            println!(
                "connect {:.3} ms; missing namespace -> {:?}; missing class -> {:?}",
                connected.as_secs_f64() * 1000.0,
                Wmi::connect(r"ROOT\TrontopNoSuchNamespace").err(),
                wmi.query("SELECT * FROM Trontop_NoSuchClass", Duration::from_secs(5))
                    .err()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_accessors_convert_wmi_shapes() {
        let row = WmiRow {
            properties: vec![
                ("Capacity".into(), WmiValue::Text("17179869184".into())),
                ("Speed".into(), WmiValue::UInt(6400)),
                ("Name".into(), WmiValue::Text("  Fixture  ".into())),
                ("Blank".into(), WmiValue::Text("   ".into())),
                (
                    "HardwareID".into(),
                    WmiValue::Array(vec![
                        WmiValue::Text("HDAUDIO\\FIXTURE".into()),
                        WmiValue::Null,
                    ]),
                ),
                ("Enabled".into(), WmiValue::Bool(true)),
                ("Offset".into(), WmiValue::Int(-5)),
            ],
        };
        assert_eq!(row.u64("capacity"), Some(17_179_869_184));
        assert_eq!(row.u64("SPEED"), Some(6400));
        assert_eq!(row.text("Name").as_deref(), Some("Fixture"));
        assert_eq!(row.text("Blank"), None);
        assert_eq!(row.texts("HardwareID"), ["HDAUDIO\\FIXTURE"]);
        assert_eq!(row.bool("Enabled"), Some(true));
        assert_eq!(row.u64("Offset"), None);
        assert_eq!(row.i64("Offset"), Some(-5));
        assert_eq!(row.get("Missing"), None);
    }

    #[test]
    fn dmtf_dates_validate_before_formatting() {
        assert_eq!(
            dmtf_date("20240315000000.000000+000").as_deref(),
            Some("2024-03-15")
        );
        assert_eq!(dmtf_date("2024"), None);
        assert_eq!(dmtf_date("20241315000000.000000+000"), None);
        assert_eq!(dmtf_date("00000101000000.000000+000"), None);
        assert_eq!(dmtf_date("2024031X"), None);
    }
}
