//! SetupDi device inventory: names, IDs, driver details and node status for
//! devices of one setup class, one interface class, one enumerator, or all.
//! Read-only: device information sets are enumerated and destroyed, nothing
//! is installed, enabled, disabled or changed.
//!
//! `instance_id` and `interface_path` can embed serial numbers; show them only
//! as private rows.

/// One device node. Text fields are None when Windows has no value.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeviceInfo {
    /// PnP device instance ID, e.g. `PCI\VEN_10DE&DEV_2C05...`. Private.
    pub instance_id: String,
    /// Device interface path; only for `Filter::Interface` queries. Private.
    pub interface_path: Option<String>,
    pub friendly_name: Option<String>,
    pub description: Option<String>,
    /// The name the bus (for example USB) reports for the device.
    pub bus_description: Option<String>,
    pub manufacturer: Option<String>,
    pub hardware_ids: Vec<String>,
    pub compatible_ids: Vec<String>,
    pub class: Option<String>,
    /// Braced upper-case GUID text.
    pub class_guid: Option<String>,
    pub enumerator: Option<String>,
    pub service: Option<String>,
    pub location: Option<String>,
    pub driver_version: Option<String>,
    /// UTC "YYYY-MM-DD".
    pub driver_date: Option<String>,
    pub driver_provider: Option<String>,
    /// None when the node status could not be queried.
    pub status: Option<DevNodeStatus>,
    /// Values for `Query::extra` keys, in the same order.
    pub extra: Vec<Option<PropertyValue>>,
}

impl DeviceInfo {
    /// Friendly name, then description, then the bus-reported name.
    pub fn name(&self) -> Option<&str> {
        self.friendly_name
            .as_deref()
            .or(self.description.as_deref())
            .or(self.bus_description.as_deref())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DevNodeStatus {
    /// DN_* flags from cfg.h.
    pub flags: u32,
    /// CM_PROB_* code; 0 when there is no problem.
    pub problem: u32,
}

impl DevNodeStatus {
    const DN_STARTED: u32 = 0x0000_0008;
    const DN_HAS_PROBLEM: u32 = 0x0000_0400;

    pub fn started(self) -> bool {
        self.flags & Self::DN_STARTED != 0
    }

    pub fn has_problem(self) -> bool {
        self.flags & Self::DN_HAS_PROBLEM != 0
    }

    /// "Working", "Not started" or "Problem code N" (Device Manager's codes).
    pub fn label(self) -> String {
        if self.has_problem() {
            format!("Problem code {}", self.problem)
        } else if self.started() {
            "Working".into()
        } else {
            "Not started".into()
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PropertyValue {
    Text(String),
    List(Vec<String>),
    U32(u32),
    U64(u64),
    I32(i32),
    Bool(bool),
    /// Braced upper-case GUID text.
    Guid(String),
    /// Raw FILETIME ticks; see `super::filetime_date`.
    FileTime(u64),
    Binary(Vec<u8>),
}

impl PropertyValue {
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }
}

const DEVPROP_TYPE_INT32: u32 = 0x06;
const DEVPROP_TYPE_UINT32: u32 = 0x07;
const DEVPROP_TYPE_UINT64: u32 = 0x09;
const DEVPROP_TYPE_GUID: u32 = 0x0D;
const DEVPROP_TYPE_FILETIME: u32 = 0x10;
const DEVPROP_TYPE_BOOLEAN: u32 = 0x11;
const DEVPROP_TYPE_STRING: u32 = 0x12;
const DEVPROP_TYPE_BINARY: u32 = 0x1003;
const DEVPROP_TYPE_STRING_LIST: u32 = 0x2012;

/// Decodes a unified device property buffer. None for unsupported types or
/// sizes that do not match the declared type.
pub fn decode_property(kind: u32, bytes: &[u8]) -> Option<PropertyValue> {
    match kind {
        DEVPROP_TYPE_STRING => super::utf16_bytes_until_nul(bytes)
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty())
            .map(PropertyValue::Text),
        DEVPROP_TYPE_STRING_LIST => match super::registry::decode(7, bytes)? {
            super::registry::RegValue::MultiText(items) => Some(PropertyValue::List(items)),
            _ => None,
        },
        DEVPROP_TYPE_UINT32 => bytes
            .try_into()
            .ok()
            .map(|b: [u8; 4]| PropertyValue::U32(u32::from_le_bytes(b))),
        DEVPROP_TYPE_INT32 => bytes
            .try_into()
            .ok()
            .map(|b: [u8; 4]| PropertyValue::I32(i32::from_le_bytes(b))),
        DEVPROP_TYPE_UINT64 => bytes
            .try_into()
            .ok()
            .map(|b: [u8; 8]| PropertyValue::U64(u64::from_le_bytes(b))),
        DEVPROP_TYPE_FILETIME => bytes
            .try_into()
            .ok()
            .map(|b: [u8; 8]| PropertyValue::FileTime(u64::from_le_bytes(b))),
        DEVPROP_TYPE_BOOLEAN => match bytes {
            [value] => Some(PropertyValue::Bool(*value != 0)),
            _ => None,
        },
        DEVPROP_TYPE_GUID => guid_text(bytes).map(PropertyValue::Guid),
        DEVPROP_TYPE_BINARY => Some(PropertyValue::Binary(bytes.to_vec())),
        _ => None,
    }
}

/// 16 GUID bytes in memory order as "{XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}".
pub fn guid_text(bytes: &[u8]) -> Option<String> {
    let b: [u8; 16] = bytes.try_into().ok()?;
    Some(format!(
        "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
        u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
        u16::from_le_bytes([b[4], b[5]]),
        u16::from_le_bytes([b[6], b[7]]),
        b[8],
        b[9],
        b[10],
        b[11],
        b[12],
        b[13],
        b[14],
        b[15]
    ))
}

#[cfg(windows)]
pub use native::{Filter, Query, devices};

#[cfg(windows)]
mod native {
    use super::super::NativeError;
    use super::*;
    use crate::specs::Context;
    use std::mem::size_of;
    use windows::Win32::Devices::DeviceAndDriverInstallation::{
        CM_DEVNODE_STATUS_FLAGS, CM_Get_DevNode_Status, CM_PROB, CR_SUCCESS, DIGCF_ALLCLASSES,
        DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, HDEVINFO, SETUP_DI_GET_CLASS_DEVS_FLAGS,
        SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W, SP_DEVINFO_DATA,
        SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInfo, SetupDiEnumDeviceInterfaces,
        SetupDiGetClassDevsW, SetupDiGetDeviceInstanceIdW, SetupDiGetDeviceInterfaceDetailW,
        SetupDiGetDevicePropertyW,
    };
    use windows::Win32::Devices::Properties::{
        DEVPKEY_Device_BusReportedDeviceDesc, DEVPKEY_Device_Class, DEVPKEY_Device_ClassGuid,
        DEVPKEY_Device_CompatibleIds, DEVPKEY_Device_DeviceDesc, DEVPKEY_Device_DriverDate,
        DEVPKEY_Device_DriverProvider, DEVPKEY_Device_DriverVersion, DEVPKEY_Device_EnumeratorName,
        DEVPKEY_Device_FriendlyName, DEVPKEY_Device_HardwareIds, DEVPKEY_Device_LocationInfo,
        DEVPKEY_Device_Manufacturer, DEVPKEY_Device_Service, DEVPROPTYPE,
    };
    use windows::Win32::Foundation::{DEVPROPKEY, ERROR_INSUFFICIENT_BUFFER, ERROR_NO_MORE_ITEMS};
    use windows::core::{GUID, HRESULT, PCWSTR};

    const MAX_PROPERTY_BYTES: usize = 64 * 1024;

    pub enum Filter<'a> {
        /// A device setup class, e.g. GUID_DEVCLASS_MEDIA.
        Class(GUID),
        /// A device interface class, e.g. GUID_DEVINTERFACE_DISK; fills
        /// `interface_path`. One entry per device even with several interfaces.
        Interface(GUID),
        /// A PnP enumerator such as "USB", "HID", "PCI" or "HDAUDIO".
        Enumerator(&'a str),
        All,
    }

    pub struct Query<'a> {
        pub filter: Filter<'a>,
        /// Only devices currently attached (DIGCF_PRESENT).
        pub present_only: bool,
        /// Enumeration stops after this many devices.
        pub limit: usize,
        /// Additional unified properties to read into `DeviceInfo::extra`.
        pub extra: &'a [DEVPROPKEY],
        /// The section's read budget, checked before every device.
        pub ctx: &'a Context,
    }

    impl<'a> Query<'a> {
        pub fn new(filter: Filter<'a>, ctx: &'a Context) -> Self {
            Self {
                filter,
                present_only: true,
                limit: 512,
                extra: &[],
                ctx,
            }
        }
    }

    struct DeviceSet(HDEVINFO);
    impl Drop for DeviceSet {
        fn drop(&mut self) {
            // SAFETY: owns exactly one device information set.
            unsafe {
                let _ = SetupDiDestroyDeviceInfoList(self.0);
            }
        }
    }

    fn is(error: &windows::core::Error, code: windows::Win32::Foundation::WIN32_ERROR) -> bool {
        error.code() == HRESULT::from_win32(code.0)
    }

    /// Enumerates matching devices. Individual missing properties are None and
    /// a device whose interface detail cannot be read is skipped; only a
    /// failure to open or walk the set, or an exhausted read budget, is an error.
    pub fn devices(query: &Query<'_>) -> Result<Vec<DeviceInfo>, NativeError> {
        const API: &str = "SetupDiGetClassDevsW";
        const EXHAUSTED: NativeError =
            NativeError::Unsupported("read budget exhausted during device enumeration");
        if query.ctx.should_stop() {
            return Err(EXHAUSTED);
        }
        let present = if query.present_only {
            DIGCF_PRESENT
        } else {
            SETUP_DI_GET_CLASS_DEVS_FLAGS(0)
        };
        let enumerator;
        // SAFETY: optional GUID pointer and terminated enumerator string live
        // across the call; no parent window.
        let set = DeviceSet(
            unsafe {
                match &query.filter {
                    Filter::Class(guid) => {
                        SetupDiGetClassDevsW(Some(guid), PCWSTR::null(), None, present)
                    }
                    Filter::Interface(guid) => SetupDiGetClassDevsW(
                        Some(guid),
                        PCWSTR::null(),
                        None,
                        present | DIGCF_DEVICEINTERFACE,
                    ),
                    Filter::Enumerator(name) => {
                        enumerator = name.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
                        SetupDiGetClassDevsW(
                            None,
                            PCWSTR(enumerator.as_ptr()),
                            None,
                            present | DIGCF_ALLCLASSES,
                        )
                    }
                    Filter::All => {
                        SetupDiGetClassDevsW(None, PCWSTR::null(), None, present | DIGCF_ALLCLASSES)
                    }
                }
            }
            .map_err(|e| NativeError::from_windows(API, &e))?,
        );
        let mut devices = Vec::new();
        let mut index = 0u32;
        while devices.len() < query.limit {
            if query.ctx.should_stop() {
                return Err(EXHAUSTED);
            }
            let mut data = SP_DEVINFO_DATA {
                cbSize: size_of::<SP_DEVINFO_DATA>() as u32,
                ..Default::default()
            };
            let mut interface_path = None;
            let step = match &query.filter {
                Filter::Interface(guid) => {
                    interface(set.0, guid, index, &mut data).map(|path| interface_path = path)
                }
                _ => {
                    // SAFETY: live set and writable, sized device data.
                    unsafe { SetupDiEnumDeviceInfo(set.0, index, &mut data) }
                        .map_err(|e| NativeError::from_windows("SetupDiEnumDeviceInfo", &e))
                }
            };
            index += 1;
            match step {
                Ok(()) => {}
                Err(NativeError::Windows { code, .. })
                    if code == HRESULT::from_win32(ERROR_NO_MORE_ITEMS.0).0 as u32 =>
                {
                    break;
                }
                Err(error) => return Err(error),
            }
            if matches!(query.filter, Filter::Interface(_)) && interface_path.is_none() {
                continue;
            }
            let mut device = read(set.0, &data, query.extra);
            device.interface_path = interface_path;
            // Several interfaces of one device collapse into its first entry.
            if !devices
                .iter()
                .any(|d: &DeviceInfo| d.instance_id == device.instance_id)
            {
                devices.push(device);
            }
        }
        Ok(devices)
    }

    fn interface(
        set: HDEVINFO,
        guid: &GUID,
        index: u32,
        data: &mut SP_DEVINFO_DATA,
    ) -> Result<Option<String>, NativeError> {
        let mut interface = SP_DEVICE_INTERFACE_DATA {
            cbSize: size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
            ..Default::default()
        };
        // SAFETY: live set, interface GUID and writable, sized interface data.
        unsafe { SetupDiEnumDeviceInterfaces(set, None, guid, index, &mut interface) }
            .map_err(|e| NativeError::from_windows("SetupDiEnumDeviceInterfaces", &e))?;
        let mut required = 0;
        // SAFETY: size query with no output buffer.
        let sizing = unsafe {
            SetupDiGetDeviceInterfaceDetailW(set, &interface, None, 0, Some(&mut required), None)
        };
        if sizing
            .err()
            .is_none_or(|e| !is(&e, ERROR_INSUFFICIENT_BUFFER))
            || !(8..=65536).contains(&required)
        {
            // This one interface is unreadable; the walk goes on.
            return Ok(None);
        }
        // DWORD-aligned storage; cbSize is the fixed part; the path starts at byte 4.
        let mut buffer = vec![0u32; (required as usize).div_ceil(4)];
        buffer[0] = size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
        // SAFETY: buffer holds `required` bytes; device data is sized.
        let detail = unsafe {
            SetupDiGetDeviceInterfaceDetailW(
                set,
                &interface,
                Some(buffer.as_mut_ptr().cast()),
                required,
                None,
                Some(data),
            )
        };
        if detail.is_err() {
            return Ok(None);
        }
        let words = buffer
            .into_iter()
            .flat_map(|v| [v as u16, (v >> 16) as u16])
            .skip(2)
            .take((required as usize - 4) / 2)
            .collect::<Vec<_>>();
        let path = super::super::utf16_until_nul(&words);
        Ok((!path.is_empty()).then_some(path))
    }

    fn property(set: HDEVINFO, data: &SP_DEVINFO_DATA, key: &DEVPROPKEY) -> Option<PropertyValue> {
        let mut buffer = vec![0u8; 512];
        for _ in 0..3 {
            let mut kind = DEVPROPTYPE::default();
            let mut required = 0u32;
            // SAFETY: buffer slice length is the declared capacity.
            let result = unsafe {
                SetupDiGetDevicePropertyW(
                    set,
                    data,
                    key,
                    &mut kind,
                    Some(&mut buffer),
                    Some(&mut required),
                    0,
                )
            };
            match result {
                Ok(()) => return decode_property(kind.0, buffer.get(..required as usize)?),
                Err(e)
                    if is(&e, ERROR_INSUFFICIENT_BUFFER)
                        && (required as usize) <= MAX_PROPERTY_BYTES =>
                {
                    buffer.resize(required as usize, 0);
                }
                Err(_) => return None,
            }
        }
        None
    }

    fn read(set: HDEVINFO, data: &SP_DEVINFO_DATA, extra: &[DEVPROPKEY]) -> DeviceInfo {
        let text = |key| match property(set, data, key) {
            Some(PropertyValue::Text(text)) => Some(text),
            _ => None,
        };
        let list = |key| match property(set, data, key) {
            Some(PropertyValue::List(items)) => items,
            Some(PropertyValue::Text(text)) => vec![text],
            _ => Vec::new(),
        };
        let mut id = [0u16; 512];
        let mut required = 0;
        // SAFETY: slice capacity is passed; the ID fits in MAX_DEVICE_ID_LEN (200).
        let instance_id =
            unsafe { SetupDiGetDeviceInstanceIdW(set, data, Some(&mut id), Some(&mut required)) }
                .map(|()| super::super::utf16_until_nul(&id))
                .unwrap_or_default();
        let mut flags = CM_DEVNODE_STATUS_FLAGS(0);
        let mut problem = CM_PROB(0);
        // SAFETY: writable outputs and a devinst from this live set.
        let status = unsafe { CM_Get_DevNode_Status(&mut flags, &mut problem, data.DevInst, 0) };
        DeviceInfo {
            instance_id,
            interface_path: None,
            friendly_name: text(&DEVPKEY_Device_FriendlyName),
            description: text(&DEVPKEY_Device_DeviceDesc),
            bus_description: text(&DEVPKEY_Device_BusReportedDeviceDesc),
            manufacturer: text(&DEVPKEY_Device_Manufacturer),
            hardware_ids: list(&DEVPKEY_Device_HardwareIds),
            compatible_ids: list(&DEVPKEY_Device_CompatibleIds),
            class: text(&DEVPKEY_Device_Class),
            class_guid: match property(set, data, &DEVPKEY_Device_ClassGuid) {
                Some(PropertyValue::Guid(guid)) => Some(guid),
                _ => None,
            },
            enumerator: text(&DEVPKEY_Device_EnumeratorName),
            service: text(&DEVPKEY_Device_Service),
            location: text(&DEVPKEY_Device_LocationInfo),
            driver_version: text(&DEVPKEY_Device_DriverVersion),
            driver_date: match property(set, data, &DEVPKEY_Device_DriverDate) {
                Some(PropertyValue::FileTime(ticks)) => super::super::filetime_date(ticks),
                _ => None,
            },
            driver_provider: text(&DEVPKEY_Device_DriverProvider),
            status: (status == CR_SUCCESS).then_some(DevNodeStatus {
                flags: flags.0,
                problem: problem.0,
            }),
            extra: extra.iter().map(|key| property(set, data, key)).collect(),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use windows::Win32::Devices::DeviceAndDriverInstallation::GUID_DEVCLASS_MEDIA;
        use windows::Win32::System::Ioctl::GUID_DEVINTERFACE_DISK;

        #[test]
        fn an_exhausted_budget_stops_before_any_setupdi_call() {
            let ctx = Context::new(
                std::time::Duration::ZERO,
                std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            );
            let result = devices(&Query::new(Filter::All, &ctx));
            assert_eq!(
                result.map(|d| d.len()).map_err(|e| e.to_string()),
                Err("read budget exhausted during device enumeration".into())
            );
        }

        #[test]
        #[ignore = "Read-only SetupDi enumeration; no install, enable/disable, window or input"]
        fn native_specs_setupapi_read_only_probe() {
            for (label, filter) in [
                ("media class", Filter::Class(GUID_DEVCLASS_MEDIA)),
                ("disk interface", Filter::Interface(GUID_DEVINTERFACE_DISK)),
                ("PCI enumerator", Filter::Enumerator("PCI")),
            ] {
                let started = std::time::Instant::now();
                let ctx = Context::probe();
                let result = devices(&Query::new(filter, &ctx));
                let elapsed = started.elapsed().as_secs_f64() * 1000.0;
                match result {
                    Ok(list) => {
                        println!("{label}: {} devices in {elapsed:.3} ms", list.len());
                        for device in list.iter().take(6) {
                            println!(
                                "  {:?} / {:?} / driver {:?} {:?} {:?} / {:?} / interface={}",
                                device.name(),
                                device.manufacturer,
                                device.driver_version,
                                device.driver_date,
                                device.driver_provider,
                                device.status.map(DevNodeStatus::label),
                                device.interface_path.is_some()
                            );
                        }
                    }
                    Err(error) => println!("{label}: {error}"),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16(text: &str) -> Vec<u8> {
        text.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    #[test]
    fn property_buffers_decode_by_declared_type() {
        assert_eq!(
            decode_property(DEVPROP_TYPE_STRING, &utf16(" NVIDIA \0")),
            Some(PropertyValue::Text("NVIDIA".into()))
        );
        assert_eq!(decode_property(DEVPROP_TYPE_STRING, &utf16("  \0")), None);
        assert_eq!(
            decode_property(DEVPROP_TYPE_STRING_LIST, &utf16("PCI\\A\0PCI\\B\0\0")),
            Some(PropertyValue::List(vec!["PCI\\A".into(), "PCI\\B".into()]))
        );
        assert_eq!(
            decode_property(DEVPROP_TYPE_UINT32, &7u32.to_le_bytes()),
            Some(PropertyValue::U32(7))
        );
        assert_eq!(decode_property(DEVPROP_TYPE_UINT32, &[1]), None);
        assert_eq!(
            decode_property(DEVPROP_TYPE_BOOLEAN, &[0xff]),
            Some(PropertyValue::Bool(true))
        );
        assert_eq!(
            decode_property(
                DEVPROP_TYPE_FILETIME,
                &116_444_736_000_000_000u64.to_le_bytes()
            ),
            Some(PropertyValue::FileTime(116_444_736_000_000_000))
        );
        let guid = [
            0x6b, 0xa4, 0x70, 0x4d, 0x1a, 0xe3, 0xd0, 0x11, 0xa2, 0x9a, 0x00, 0xa0, 0xc9, 0x06,
            0x29, 0x10,
        ];
        assert_eq!(
            decode_property(DEVPROP_TYPE_GUID, &guid),
            Some(PropertyValue::Guid(
                "{4D70A46B-E31A-11D0-A29A-00A0C9062910}".into()
            ))
        );
        assert_eq!(decode_property(0xffff, &[]), None);
    }

    #[test]
    fn node_status_labels_match_device_manager_terms() {
        let working = DevNodeStatus {
            flags: 0x0000_000A,
            problem: 0,
        };
        assert_eq!(working.label(), "Working");
        let failed = DevNodeStatus {
            flags: 0x0000_0400,
            problem: 28,
        };
        assert_eq!(failed.label(), "Problem code 28");
        let idle = DevNodeStatus {
            flags: 0,
            problem: 0,
        };
        assert_eq!(idle.label(), "Not started");
    }
}
