//! Windows reads for Audio and Peripherals: SetupDi snapshots, the MMDevice
//! endpoint enumerator (read-only property stores) and Win32_Printer.
use super::*;
use crate::specs::native::setupapi::{Filter, PropertyValue, Query, devices};
use crate::specs::native::{NativeError, wmi};
use std::time::Duration;
use windows::Win32::Devices::DeviceAndDriverInstallation::GUID_DEVCLASS_MEDIA;
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Devices::Properties::DEVPKEY_Device_Parent;
use windows::Win32::Foundation::{PROPERTYKEY, RPC_E_CHANGED_MODE};
use windows::Win32::Media::Audio::{
    DEVICE_STATE_ACTIVE, EDataFlow, IMMDevice, IMMDeviceEnumerator, IMMEndpoint,
    MMDeviceEnumerator, PKEY_AudioEngine_DeviceFormat, eAll, eCapture, eConsole, eRender,
};
use windows::Win32::System::Com::StructuredStorage::{PROPVARIANT, PropVariantClear};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
    CoUninitialize, STGM_READ,
};
use windows::Win32::System::Variant::{VT_BLOB, VT_LPWSTR};
use windows::core::Interface;

pub(super) fn sound_devices() -> Result<Vec<DeviceInfo>, String> {
    devices(&Query::new(Filter::Class(GUID_DEVCLASS_MEDIA))).map_err(|e| e.to_string())
}

pub(super) fn inventory() -> Result<Inventory, String> {
    let list = devices(&Query {
        limit: 4096,
        extra: &[DEVPKEY_Device_Parent],
        ..Query::new(Filter::All)
    })
    .map_err(|e| e.to_string())?;
    let parents = list
        .iter()
        .map(|d| match d.extra.first() {
            Some(Some(PropertyValue::Text(parent))) => Some(parent.clone()),
            _ => None,
        })
        .collect();
    Ok(Inventory {
        devices: list,
        parents,
    })
}

pub(super) fn printers(ctx: &Context) -> Result<Vec<(String, Option<String>, bool)>, String> {
    let rows = wmi::query_once(
        r"ROOT\CIMV2",
        "SELECT Name, DriverName, Default FROM Win32_Printer",
        ctx.timeout(Duration::from_secs(8)),
    )
    .map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .filter_map(|row| {
            Some((
                row.text("Name")?,
                row.text("DriverName"),
                row.bool("Default").unwrap_or(false),
            ))
        })
        .collect())
}

/// Balances one CoInitializeEx on this worker thread.
struct Com(bool);
impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: balances the successful CoInitializeEx in `endpoints`.
            unsafe { CoUninitialize() };
        }
    }
}

/// Reads one property and clears the PROPVARIANT on every path.
fn property<T>(
    store: &windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore,
    key: &PROPERTYKEY,
    read: impl FnOnce(&PROPVARIANT) -> Option<T>,
) -> Option<T> {
    // SAFETY: live store; the returned PROPVARIANT is owned and cleared below.
    let mut value = unsafe { store.GetValue(key) }.ok()?;
    let result = read(&value);
    // SAFETY: clears the owned PROPVARIANT (frees strings and blobs).
    let _ = unsafe { PropVariantClear(&mut value) };
    result
}

fn id(device: &IMMDevice) -> Option<String> {
    // SAFETY: GetId returns a task-allocated string freed here.
    unsafe {
        let raw = device.GetId().ok()?;
        let text = raw.to_string().ok();
        CoTaskMemFree(Some(raw.0.cast()));
        text
    }
}

pub(super) fn endpoints() -> Result<Vec<Endpoint>, String> {
    // SAFETY: reserved pointer is None; balanced by `Com`.
    let init = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    let _com = Com(init.is_ok());
    if init.is_err() && init != RPC_E_CHANGED_MODE {
        return Err(NativeError::hresult("CoInitializeEx", init.0).to_string());
    }
    // SAFETY: documented in-process enumerator class.
    let enumerator: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER) }.map_err(
            |e| NativeError::from_windows("CoCreateInstance(MMDeviceEnumerator)", &e).to_string(),
        )?;
    let default = |flow: EDataFlow| {
        // SAFETY: live enumerator; no default endpoint is an error, not a crash.
        unsafe { enumerator.GetDefaultAudioEndpoint(flow, eConsole) }
            .ok()
            .and_then(|d| id(&d))
    };
    let defaults = [default(eRender), default(eCapture)];
    // SAFETY: live enumerator; active endpoints only.
    let collection = unsafe { enumerator.EnumAudioEndpoints(eAll, DEVICE_STATE_ACTIVE) }
        .map_err(|e| NativeError::from_windows("EnumAudioEndpoints", &e).to_string())?;
    // SAFETY: live collection.
    let count = unsafe { collection.GetCount() }.unwrap_or(0).min(256);
    let mut endpoints = Vec::new();
    for index in 0..count {
        // SAFETY: index below the reported count.
        let Ok(device) = (unsafe { collection.Item(index) }) else {
            continue;
        };
        let capture = device
            .cast::<IMMEndpoint>()
            .ok()
            // SAFETY: live endpoint interface.
            .and_then(|e| unsafe { e.GetDataFlow() }.ok())
            == Some(eCapture);
        // SAFETY: read-only property store.
        let Ok(store) = (unsafe { device.OpenPropertyStore(STGM_READ) }) else {
            continue;
        };
        let name = property(&store, &PKEY_Device_FriendlyName, |value| {
            // SAFETY: vt selects the active union member.
            unsafe {
                let inner = &value.Anonymous.Anonymous;
                (inner.vt == VT_LPWSTR && !inner.Anonymous.pwszVal.is_null())
                    .then(|| inner.Anonymous.pwszVal.to_string().ok())
                    .flatten()
            }
        });
        let format = property(&store, &PKEY_AudioEngine_DeviceFormat, |value| {
            // SAFETY: vt selects the blob member; the slice stays within cbSize.
            unsafe {
                let inner = &value.Anonymous.Anonymous;
                if inner.vt != VT_BLOB || inner.Anonymous.blob.pBlobData.is_null() {
                    return None;
                }
                let blob = &inner.Anonymous.blob;
                parse_format(std::slice::from_raw_parts(
                    blob.pBlobData,
                    blob.cbSize.min(4096) as usize,
                ))
            }
        });
        let endpoint_id = id(&device);
        endpoints.push(Endpoint {
            name: name.unwrap_or_else(|| "Audio endpoint".into()),
            capture,
            default: endpoint_id.is_some() && defaults[usize::from(capture)] == endpoint_id,
            format,
        });
    }
    Ok(endpoints)
}
