use super::*;
use std::mem::size_of;
use std::os::windows::io::AsRawHandle;
use windows::Win32::Devices::DeviceAndDriverInstallation::*;
use windows::Win32::Foundation::{
    CloseHandle, ERROR_INSUFFICIENT_BUFFER, ERROR_NO_MORE_ITEMS, HANDLE,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    OPEN_EXISTING,
};
use windows::Win32::System::IO::{CancelSynchronousIo, DeviceIoControl};
use windows::Win32::System::Ioctl::{
    GUID_DEVINTERFACE_DISK, IOCTL_STORAGE_QUERY_PROPERTY, StorageDeviceTemperatureProperty,
};
use windows::core::PCWSTR;

pub(super) struct Native;

fn windows_error(error: windows::core::Error) -> Error {
    Error::Windows(error.code().0 as u32 & 0xffff)
}

struct DeviceSet(HDEVINFO);
impl Drop for DeviceSet {
    fn drop(&mut self) {
        unsafe {
            let _ = SetupDiDestroyDeviceInfoList(self.0);
        }
    }
}
struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

fn name(
    set: HDEVINFO,
    data: &SP_DEVINFO_DATA,
    property: SETUP_DI_REGISTRY_PROPERTY,
) -> Option<String> {
    let mut bytes = [0u8; 1024];
    let mut length = 0;
    let mut kind = 0;
    unsafe {
        SetupDiGetDeviceRegistryPropertyW(
            set,
            data,
            property,
            Some(&mut kind),
            Some(&mut bytes),
            Some(&mut length),
        )
    }
    .ok()?;
    if kind != 1 || length as usize > bytes.len() || length % 2 != 0 {
        return None;
    }
    let words: Vec<_> = bytes[..length as usize]
        .chunks_exact(2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .take_while(|c| *c != 0)
        .collect();
    let text = String::from_utf16(&words).ok()?;
    (!text.trim().is_empty()).then(|| text.trim().to_string())
}

impl Backend for Native {
    fn enumerate(&self) -> Result<Vec<Device>, Error> {
        let set = DeviceSet(
            unsafe {
                SetupDiGetClassDevsW(
                    Some(&GUID_DEVINTERFACE_DISK),
                    PCWSTR::null(),
                    None,
                    DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
                )
            }
            .map_err(windows_error)?,
        );
        let mut devices = Vec::new();
        // One overflow entry lets the monitor report limited coverage explicitly.
        for index in 0..=MAX_DRIVES as u32 {
            let mut interface = SP_DEVICE_INTERFACE_DATA {
                cbSize: size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
                ..Default::default()
            };
            match unsafe {
                SetupDiEnumDeviceInterfaces(
                    set.0,
                    None,
                    &GUID_DEVINTERFACE_DISK,
                    index,
                    &mut interface,
                )
            } {
                Ok(()) => {}
                Err(error)
                    if error.code()
                        == windows::core::HRESULT::from_win32(ERROR_NO_MORE_ITEMS.0) =>
                {
                    break;
                }
                Err(error) => return Err(windows_error(error)),
            }
            let mut required = 0;
            let result = unsafe {
                SetupDiGetDeviceInterfaceDetailW(
                    set.0,
                    &interface,
                    None,
                    0,
                    Some(&mut required),
                    None,
                )
            };
            if result.err().is_none_or(|e| {
                e.code() != windows::core::HRESULT::from_win32(ERROR_INSUFFICIENT_BUFFER.0)
            }) || !(8..=65536).contains(&required)
            {
                return Err(Error::Malformed);
            }
            // DWORD-aligned storage, actual allocation rounded up, cbSize includes
            // the platform-specific fixed portion. The UTF-16 path starts at byte 4.
            let mut buffer = vec![0u32; (required as usize).div_ceil(4)];
            buffer[0] = size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
            let mut data = SP_DEVINFO_DATA {
                cbSize: size_of::<SP_DEVINFO_DATA>() as u32,
                ..Default::default()
            };
            unsafe {
                SetupDiGetDeviceInterfaceDetailW(
                    set.0,
                    &interface,
                    Some(buffer.as_mut_ptr().cast()),
                    required,
                    None,
                    Some(&mut data),
                )
            }
            .map_err(windows_error)?;
            let words: Vec<_> = buffer
                .into_iter()
                .flat_map(|v| [v as u16, (v >> 16) as u16])
                .skip(2)
                .take((required as usize - 4) / 2)
                .collect();
            let end = words.iter().position(|c| *c == 0).ok_or(Error::Malformed)?;
            let id = String::from_utf16(&words[..end]).map_err(|_| Error::Malformed)?;
            if id.is_empty() {
                return Err(Error::Malformed);
            }
            let label = name(set.0, &data, SPDRP_FRIENDLYNAME)
                .or_else(|| name(set.0, &data, SPDRP_DEVICEDESC))
                .unwrap_or_else(|| "Storage device".into());
            if !devices.iter().any(|d: &Device| d.id == id) {
                devices.push(Device { id, name: label });
            }
        }
        Ok(devices)
    }

    fn read(
        &self,
        device: &Device,
        cancel: &AtomicBool,
        stop: &AtomicBool,
    ) -> Result<Temperatures, Error> {
        let canceled = || cancel.load(Ordering::Acquire) || stop.load(Ordering::Acquire);
        if canceled() {
            return Err(Error::Stopped);
        }
        let path: Vec<_> = device.id.encode_utf16().chain(Some(0)).collect();
        // Metadata-only access: no sectors, SMART pass-through, settings or writes.
        let handle = Handle(
            unsafe {
                CreateFileW(
                    PCWSTR(path.as_ptr()),
                    0,
                    FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                    None,
                    OPEN_EXISTING,
                    FILE_ATTRIBUTE_NORMAL,
                    None,
                )
            }
            .map_err(windows_error)?,
        );
        if canceled() {
            return Err(Error::Stopped);
        }
        let input = [StorageDeviceTemperatureProperty.0 as u32, 0u32, 0u32];
        let mut output = [0u8; 24 + MAX_SENSORS * 16];
        let mut returned = 0;
        unsafe {
            DeviceIoControl(
                handle.0,
                IOCTL_STORAGE_QUERY_PROPERTY,
                Some(input.as_ptr().cast()),
                size_of::<[u32; 3]>() as u32,
                Some(output.as_mut_ptr().cast()),
                output.len() as u32,
                Some(&mut returned),
                None,
            )
        }
        .map_err(windows_error)?;
        if canceled() {
            return Err(Error::Timeout);
        }
        if returned as usize > output.len() {
            return Err(Error::Malformed);
        }
        parse(&output[..returned as usize])
    }

    fn cancel(&self, worker: &JoinHandle<()>) {
        // An owned Rust thread handle, never an arbitrary process/thread ID. This
        // thread cannot begin another job until the monitor drains its old result.
        unsafe {
            let _ = CancelSynchronousIo(HANDLE(worker.as_raw_handle()));
        }
    }
}
