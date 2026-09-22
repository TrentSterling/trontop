//! Windows reads for the Graphics section: the existing DXGI inventory,
//! SetupDi display-class devices and QueryDisplayConfig monitor paths.
use super::*;
use crate::specs::native::setupapi::{self, DevNodeStatus, Filter, Query};
use crate::specs::native::{
    NativeError, PCIE_LINK_KEYS, PciId, pcie_link_text, registry, utf16_until_nul,
};
use windows::Win32::Devices::DeviceAndDriverInstallation::GUID_DEVCLASS_DISPLAY;
use windows::Win32::Devices::Display::{
    DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO, DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
    DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_PREFERRED_MODE, DISPLAYCONFIG_DEVICE_INFO_HEADER,
    DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO, DISPLAYCONFIG_MODE_INFO,
    DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE, DISPLAYCONFIG_PATH_INFO, DISPLAYCONFIG_TARGET_DEVICE_NAME,
    DISPLAYCONFIG_TARGET_PREFERRED_MODE, DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes,
    QDC_ONLY_ACTIVE_PATHS, QueryDisplayConfig,
};
use windows::Win32::Devices::Properties::DEVPKEY_Device_Driver;
use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, LUID};

pub(super) fn collect(ctx: &Context) -> Section {
    let mut others = Vec::new();
    let adapters = crate::gpu_adapters::dxgi_inventory().map(|inventory| {
        let extra = [
            PCIE_LINK_KEYS[0],
            PCIE_LINK_KEYS[1],
            PCIE_LINK_KEYS[2],
            PCIE_LINK_KEYS[3],
            DEVPKEY_Device_Driver,
        ];
        let devices = setupapi::devices(&Query {
            extra: &extra,
            ..Query::new(Filter::Class(GUID_DEVCLASS_DISPLAY))
        })
        .unwrap_or_default();
        let mut claimed = vec![false; devices.len()];
        let adapters = inventory
            .into_iter()
            .filter(|(_, d)| !d.software)
            .map(|((high, low), d)| {
                // Pair each DXGI adapter with the first unclaimed PCI function
                // that has the same vendor and device ID.
                let device = devices.iter().enumerate().find_map(|(index, info)| {
                    let id = PciId::parse(&info.hardware_ids)?;
                    let matches = !claimed[index]
                        && u32::from(id.vendor) == d.vendor_id
                        && u32::from(id.device) == d.device_id;
                    matches.then(|| {
                        claimed[index] = true;
                        Device {
                            subsystem: id.subsystem,
                            revision: id.revision,
                            driver_version: info.driver_version.clone(),
                            driver_date: info.driver_date.clone(),
                            driver_provider: info.driver_provider.clone(),
                            location: info.location.clone(),
                            link: pcie_link_text(&info.extra),
                            status: info.status.map(DevNodeStatus::label),
                            memory: hardware_value(&info.extra, "HardwareInformation.qwMemorySize")
                                .and_then(|v| v.as_u64())
                                .filter(|v| *v > 0),
                            bios: hardware_value(&info.extra, "HardwareInformation.BiosString")
                                .and_then(|v| match v {
                                    registry::RegValue::Text(text) => Some(text),
                                    registry::RegValue::Binary(bytes) => {
                                        crate::specs::native::utf16_bytes_until_nul(&bytes)
                                    }
                                    _ => None,
                                })
                                .map(|t| t.trim().trim_start_matches("Version").trim().to_string())
                                .filter(|t| !t.is_empty()),
                        }
                    })
                });
                Adapter {
                    name: d.name.trim().to_string(),
                    luids: vec![(high, low)],
                    vendor_id: d.vendor_id,
                    device_id: d.device_id,
                    dedicated_video: d.dedicated_video,
                    dedicated_system: d.dedicated_system,
                    shared: d.shared_limit,
                    device,
                }
            })
            .collect::<Vec<_>>();
        others = devices
            .iter()
            .zip(&claimed)
            .filter(|(_, claimed)| !**claimed)
            .filter_map(|(d, _)| Some((d.name()?.to_string(), d.driver_version.clone())))
            .collect();
        adapters
    });
    let adapters = adapters.map(fold_duplicates);
    let monitors = if ctx.should_stop() {
        Err("read budget exhausted".to_string())
    } else {
        monitors().map_err(|e| e.to_string())
    };
    build(adapters, monitors, &others)
}

/// DXGI can list one physical GPU under several LUIDs. An entry without its
/// own PCI function folds into the matched adapter with the same IDs.
fn fold_duplicates(adapters: Vec<Adapter>) -> Vec<Adapter> {
    let mut folded = Vec::<Adapter>::new();
    for adapter in adapters {
        if adapter.device.is_none()
            && let Some(owner) = folded.iter_mut().find(|a| {
                a.device.is_some()
                    && a.vendor_id == adapter.vendor_id
                    && a.device_id == adapter.device_id
            })
        {
            owner.luids.extend(adapter.luids);
            continue;
        }
        folded.push(adapter);
    }
    // A matched adapter may come after its duplicates in LUID order.
    let (matched, unmatched): (Vec<_>, Vec<_>) =
        folded.into_iter().partition(|a| a.device.is_some());
    let mut result = matched;
    for adapter in unmatched {
        match result
            .iter_mut()
            .find(|a| a.vendor_id == adapter.vendor_id && a.device_id == adapter.device_id)
        {
            Some(owner) => owner.luids.extend(adapter.luids),
            None => result.push(adapter),
        }
    }
    result
}

/// A value from the adapter's driver key (DEVPKEY_Device_Driver, extra index 4).
fn hardware_value(
    extra: &[Option<setupapi::PropertyValue>],
    name: &str,
) -> Option<registry::RegValue> {
    let driver = extra.get(4)?.as_ref()?.text()?;
    let path = format!(r"SYSTEM\CurrentControlSet\Control\Class\{driver}");
    registry::read(registry::Hive::LocalMachine, &path, name)
        .ok()
        .flatten()
}

fn luid(value: LUID) -> (u32, u32) {
    (value.HighPart as u32, value.LowPart)
}

fn header<T>(
    kind: windows::Win32::Devices::Display::DISPLAYCONFIG_DEVICE_INFO_TYPE,
    adapter: LUID,
    id: u32,
) -> DISPLAYCONFIG_DEVICE_INFO_HEADER {
    DISPLAYCONFIG_DEVICE_INFO_HEADER {
        r#type: kind,
        size: std::mem::size_of::<T>() as u32,
        adapterId: adapter,
        id,
    }
}

fn monitors() -> Result<Vec<Monitor>, NativeError> {
    let mut paths = Vec::<DISPLAYCONFIG_PATH_INFO>::new();
    let mut modes = Vec::<DISPLAYCONFIG_MODE_INFO>::new();
    for _ in 0..3 {
        let (mut path_count, mut mode_count) = (0u32, 0u32);
        // SAFETY: writable counts.
        let status = unsafe {
            GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut path_count, &mut mode_count)
        };
        if status.0 != 0 {
            return Err(NativeError::win32("GetDisplayConfigBufferSizes", status.0));
        }
        paths = vec![DISPLAYCONFIG_PATH_INFO::default(); path_count.min(64) as usize];
        modes = vec![DISPLAYCONFIG_MODE_INFO::default(); mode_count.min(128) as usize];
        let (mut paths_len, mut modes_len) = (paths.len() as u32, modes.len() as u32);
        // SAFETY: arrays sized by the counts passed.
        let status = unsafe {
            QueryDisplayConfig(
                QDC_ONLY_ACTIVE_PATHS,
                &mut paths_len,
                paths.as_mut_ptr(),
                &mut modes_len,
                modes.as_mut_ptr(),
                None,
            )
        };
        if status == ERROR_INSUFFICIENT_BUFFER {
            continue;
        }
        if status.0 != 0 {
            return Err(NativeError::win32("QueryDisplayConfig", status.0));
        }
        paths.truncate(paths_len as usize);
        modes.truncate(modes_len as usize);
        break;
    }
    Ok(paths
        .iter()
        .map(|path| {
            let target = &path.targetInfo;
            let mut monitor = Monitor {
                adapter: luid(target.adapterId),
                ..Default::default()
            };
            let refresh = target.refreshRate;
            if refresh.Denominator != 0 && refresh.Numerator != 0 {
                monitor.refresh =
                    Some(f64::from(refresh.Numerator) / f64::from(refresh.Denominator));
            }
            // SAFETY: modeInfoIdx is the documented union member without
            // QDC_VIRTUAL_MODE_AWARE; the index is bounds-checked.
            let source_index = unsafe { path.sourceInfo.Anonymous.modeInfoIdx } as usize;
            if let Some(mode) = modes.get(source_index)
                && mode.infoType == DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE
            {
                // SAFETY: infoType says the source mode member is active.
                let source = unsafe { mode.Anonymous.sourceMode };
                monitor.current = Some((source.width, source.height));
            }
            let mut name = DISPLAYCONFIG_TARGET_DEVICE_NAME {
                header: header::<DISPLAYCONFIG_TARGET_DEVICE_NAME>(
                    DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
                    target.adapterId,
                    target.id,
                ),
                ..Default::default()
            };
            // SAFETY: the header's size matches the structure passed.
            if unsafe { DisplayConfigGetDeviceInfo(&mut name.header) } == 0 {
                let friendly = utf16_until_nul(&name.monitorFriendlyDeviceName);
                monitor.name = (!friendly.trim().is_empty()).then(|| friendly.trim().to_string());
                monitor.manufacturer = pnp_id(name.edidManufactureId);
                monitor.product = Some(name.edidProductCodeId);
                monitor.connection = connection(name.outputTechnology.0);
            } else {
                monitor.connection = connection(target.outputTechnology.0);
            }
            let mut preferred = DISPLAYCONFIG_TARGET_PREFERRED_MODE {
                header: header::<DISPLAYCONFIG_TARGET_PREFERRED_MODE>(
                    DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_PREFERRED_MODE,
                    target.adapterId,
                    target.id,
                ),
                ..Default::default()
            };
            // SAFETY: the header's size matches the structure passed.
            if unsafe { DisplayConfigGetDeviceInfo(&mut preferred.header) } == 0
                && preferred.width > 0
                && preferred.height > 0
            {
                let sync = preferred.targetMode.targetVideoSignalInfo.vSyncFreq;
                let rate = (sync.Denominator != 0 && sync.Numerator != 0)
                    .then(|| f64::from(sync.Numerator) / f64::from(sync.Denominator));
                monitor.native = Some((preferred.width, preferred.height, rate));
            }
            let mut color = DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO {
                header: header::<DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO>(
                    DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO,
                    target.adapterId,
                    target.id,
                ),
                ..Default::default()
            };
            // SAFETY: the header's size matches the structure passed.
            if unsafe { DisplayConfigGetDeviceInfo(&mut color.header) } == 0 {
                // SAFETY: `value` views the same bitfield as the anonymous struct.
                let bits = unsafe { color.Anonymous.value };
                monitor.hdr = Some((bits & 1 != 0, bits & 2 != 0));
                monitor.bits = Some(color.bitsPerColorChannel);
            }
            monitor
        })
        .collect())
}
