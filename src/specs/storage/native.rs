//! Windows reads for Storage and Optical Drives: SetupDi disk interfaces,
//! metadata-only storage property IOCTLs (the handle has no read or write
//! access), Windows Storage Management WMI and the CD-ROM device class.
use super::*;
use crate::specs::native::setupapi::{self, DevNodeStatus, Filter, PropertyValue, Query};
use crate::specs::native::{NativeError, PCIE_LINK_KEYS, pcie_link_text, wmi};
use std::time::Duration;
use windows::Win32::Devices::DeviceAndDriverInstallation::GUID_DEVCLASS_CDROM;
use windows::Win32::Devices::Properties::DEVPKEY_Device_Parent;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    OPEN_EXISTING,
};
use windows::Win32::System::IO::DeviceIoControl;
use windows::Win32::System::Ioctl::{
    GUID_DEVINTERFACE_DISK, IOCTL_STORAGE_GET_DEVICE_NUMBER, IOCTL_STORAGE_QUERY_PROPERTY,
    StorageAccessAlignmentProperty, StorageDeviceProperty, StorageDeviceProtocolSpecificProperty,
    StorageDeviceSeekPenaltyProperty, StorageDeviceTrimProperty,
};
use windows::core::PCWSTR;

const WMI_TIMEOUT: Duration = Duration::from_secs(8);

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: owns one handle from CreateFileW.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// Metadata-only open: desired access 0, so no sector read or write is possible.
fn open(path: &str) -> Result<Handle, NativeError> {
    let wide = path.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    // SAFETY: terminated path; no security attributes or template.
    unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
    }
    .map(Handle)
    .map_err(|e| NativeError::from_windows("CreateFileW", &e))
}

fn ioctl(handle: &Handle, code: u32, input: &[u8], output: usize) -> Result<Vec<u8>, NativeError> {
    let mut buffer = vec![0u8; output];
    let mut returned = 0u32;
    // SAFETY: input and output buffers are live for the call with exact sizes.
    unsafe {
        DeviceIoControl(
            handle.0,
            code,
            (!input.is_empty()).then_some(input.as_ptr().cast()),
            input.len() as u32,
            Some(buffer.as_mut_ptr().cast()),
            buffer.len() as u32,
            Some(&mut returned),
            None,
        )
    }
    .map_err(|e| NativeError::from_windows("DeviceIoControl", &e))?;
    buffer.truncate((returned as usize).min(output));
    Ok(buffer)
}

/// STORAGE_PROPERTY_QUERY with PropertyStandardQuery and no extra parameters.
fn property(handle: &Handle, id: i32, output: usize) -> Result<Vec<u8>, NativeError> {
    let mut query = [0u8; 12];
    query[0..4].copy_from_slice(&id.to_le_bytes());
    ioctl(handle, IOCTL_STORAGE_QUERY_PROPERTY, &query, output)
}

/// The NVMe SMART / Health Information log page through the in-box driver's
/// protocol-specific property query. Read-only, no pass-through command.
fn nvme_health(handle: &Handle) -> Result<NvmeHealth, NativeError> {
    const PROTOCOL_DATA: usize = 40;
    const LOG: usize = 512;
    let mut query = vec![0u8; 8 + PROTOCOL_DATA + LOG];
    query[0..4].copy_from_slice(&StorageDeviceProtocolSpecificProperty.0.to_le_bytes());
    let fields: [u32; 6] = [
        3,                    // ProtocolTypeNvme
        2,                    // NVMeDataTypeLogPage
        2,                    // NVME_LOG_PAGE_HEALTH_INFO
        0,                    // ProtocolDataRequestSubValue
        PROTOCOL_DATA as u32, // ProtocolDataOffset
        LOG as u32,           // ProtocolDataLength
    ];
    for (index, value) in fields.iter().enumerate() {
        query[8 + index * 4..12 + index * 4].copy_from_slice(&value.to_le_bytes());
    }
    let reply = ioctl(handle, IOCTL_STORAGE_QUERY_PROPERTY, &query, query.len())?;
    let dword = |at: usize| {
        reply
            .get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
    };
    // STORAGE_PROTOCOL_DATA_DESCRIPTOR: Version, Size, then the protocol data header.
    let offset = dword(8 + 16).ok_or(NativeError::Malformed("NVMe log reply"))?;
    let length = dword(8 + 20).ok_or(NativeError::Malformed("NVMe log reply"))?;
    let start = 8usize.saturating_add(offset);
    let log = reply
        .get(start..start.saturating_add(length.min(LOG)))
        .ok_or(NativeError::Malformed("NVMe log reply bounds"))?;
    parse_nvme_health(log).ok_or(NativeError::Malformed("NVMe health log is short"))
}

fn read_disk(interface: &str, disk: &mut Disk) -> Result<(), NativeError> {
    let handle = open(interface)?;
    disk.descriptor = property(&handle, StorageDeviceProperty.0, 1024)
        .ok()
        .and_then(|bytes| parse_descriptor(&bytes));
    disk.seek_penalty = property(&handle, StorageDeviceSeekPenaltyProperty.0, 12)
        .ok()
        .and_then(|b| b.get(8).map(|v| *v != 0));
    disk.trim = property(&handle, StorageDeviceTrimProperty.0, 12)
        .ok()
        .and_then(|b| b.get(8).map(|v| *v != 0));
    disk.sectors = property(&handle, StorageAccessAlignmentProperty.0, 28)
        .ok()
        .and_then(|b| {
            let logical = u32::from_le_bytes(b.get(16..20)?.try_into().ok()?);
            let physical = u32::from_le_bytes(b.get(20..24)?.try_into().ok()?);
            (logical > 0 && physical > 0).then_some((logical, physical))
        });
    disk.number = ioctl(&handle, IOCTL_STORAGE_GET_DEVICE_NUMBER, &[], 12)
        .ok()
        .and_then(|b| Some(u32::from_le_bytes(b.get(4..8)?.try_into().ok()?)));
    if disk.descriptor.as_ref().is_some_and(|d| d.bus == 0x11) {
        match nvme_health(&handle) {
            Ok(health) => disk.nvme = Some(health),
            Err(error) => disk.nvme_error = Some(error.to_string()),
        }
    }
    Ok(())
}

pub(super) fn collect(ctx: &Context) -> Section {
    let mut issues = Vec::new();
    let devices = match setupapi::devices(&Query {
        extra: &[DEVPKEY_Device_Parent],
        ..Query::new(Filter::Interface(GUID_DEVINTERFACE_DISK), ctx)
    }) {
        Ok(devices) => devices,
        Err(error) => {
            let reason = error.to_string();
            return build(Err(reason.clone()), Value::unavailable(reason), Vec::new());
        }
    };
    let pci = setupapi::devices(&Query {
        extra: &PCIE_LINK_KEYS,
        ..Query::new(Filter::Enumerator("PCI"), ctx)
    })
    .unwrap_or_default();
    let mut disks = Vec::new();
    for device in &devices {
        let Some(interface) = device.interface_path.clone() else {
            continue;
        };
        let mut disk = Disk {
            interface: interface.clone(),
            name: device.name().map(str::to_string),
            service: device.service.clone(),
            ..Default::default()
        };
        let parent = device.extra.first().and_then(|v| match v {
            Some(PropertyValue::Text(parent)) => Some(parent.clone()),
            _ => None,
        });
        if let Some(controller) = parent
            .as_ref()
            .and_then(|p| pci.iter().find(|d| d.instance_id.eq_ignore_ascii_case(p)))
        {
            disk.controller = controller.name().map(|name| match controller.status {
                Some(status) if !status.started() || status.has_problem() => {
                    format!("{name} ({})", DevNodeStatus::label(status))
                }
                _ => name.to_string(),
            });
            disk.link = pcie_link_text(&controller.extra);
            if controller.service.is_some() {
                disk.service = controller.service.clone();
            }
        }
        if let Err(error) = read_disk(&interface, &mut disk) {
            issues.push(format!("{}: {error}", disk_title(&disk)));
        }
        disks.push(disk);
        if ctx.should_stop() {
            issues.push("Read budget exhausted; remaining disks not read.".into());
            break;
        }
    }
    let reliability = storage_wmi(ctx, &mut disks, &mut issues);
    build(Ok(disks), reliability, issues)
}

/// Joins Windows Storage Management data by disk number. Returns the SMART
/// availability reason for non-NVMe drives.
fn storage_wmi(ctx: &Context, disks: &mut [Disk], issues: &mut Vec<String>) -> Value {
    let connection = match wmi::Wmi::connect(r"ROOT\Microsoft\Windows\Storage") {
        Ok(connection) => connection,
        Err(error) => {
            issues.push(format!("Windows Storage Management: {error}"));
            return Value::unavailable(error.to_string());
        }
    };
    let query = |wql: &str| connection.query(wql, ctx.timeout(WMI_TIMEOUT));
    let number_of = |row: &wmi::WmiRow, name: &str| row.u64(name).map(|n| n as u32);
    match query(
        "SELECT DeviceId, MediaType, SpindleSpeed, HealthStatus, Size FROM MSFT_PhysicalDisk",
    ) {
        Ok(rows) => {
            for row in rows {
                let id = row.text("DeviceId").and_then(|d| d.parse::<u32>().ok());
                if let Some(disk) = disks
                    .iter_mut()
                    .find(|d| d.number.is_some() && d.number == id)
                {
                    disk.media = row.u64("MediaType");
                    disk.spindle = row.u64("SpindleSpeed");
                    disk.health = row.u64("HealthStatus");
                    disk.bytes = row.u64("Size").filter(|s| *s > 0);
                }
            }
        }
        Err(error) => issues.push(format!("MSFT_PhysicalDisk: {error}")),
    }
    match query("SELECT Number, PartitionStyle FROM MSFT_Disk") {
        Ok(rows) => {
            for row in rows {
                let number = number_of(&row, "Number");
                if let Some(disk) = disks
                    .iter_mut()
                    .find(|d| d.number.is_some() && d.number == number)
                {
                    disk.partition_style = row.u64("PartitionStyle");
                }
            }
        }
        Err(error) => issues.push(format!("MSFT_Disk: {error}")),
    }
    let volumes = query("SELECT Path, FileSystem, FileSystemLabel, SizeRemaining FROM MSFT_Volume")
        .unwrap_or_default();
    match query(
        "SELECT DiskNumber, PartitionNumber, DriveLetter, Size, GptType, MbrType, AccessPaths FROM MSFT_Partition",
    ) {
        Ok(rows) => {
            for row in rows {
                let number = number_of(&row, "DiskNumber");
                let Some(disk) = disks
                    .iter_mut()
                    .find(|d| d.number.is_some() && d.number == number)
                else {
                    continue;
                };
                let paths = row.texts("AccessPaths");
                let volume = volumes.iter().find(|v| {
                    v.text("Path")
                        .is_some_and(|p| paths.iter().any(|a| a.eq_ignore_ascii_case(&p)))
                });
                let kind = row
                    .text("GptType")
                    .map(|g| gpt_type(&g).map_or_else(|| format!("GPT type {g}"), str::to_string))
                    .or_else(|| {
                        row.u64("MbrType")
                            .filter(|t| *t != 0)
                            .map(|t| format!("MBR type {t:02X}h"))
                    });
                disk.partitions.push(Partition {
                    number: number_of(&row, "PartitionNumber").unwrap_or(0),
                    letter: row
                        .u64("DriveLetter")
                        .and_then(|c| char::from_u32(c as u32))
                        .filter(char::is_ascii_alphabetic),
                    bytes: row.u64("Size").unwrap_or(0),
                    kind,
                    file_system: volume.and_then(|v| v.text("FileSystem")),
                    label: volume.and_then(|v| v.text("FileSystemLabel")),
                    free: volume.and_then(|v| v.u64("SizeRemaining")),
                });
            }
            for disk in disks.iter_mut() {
                disk.partitions.sort_by_key(|p| p.number);
            }
        }
        Err(error) => issues.push(format!("MSFT_Partition: {error}")),
    }
    match query("SELECT DeviceId FROM MSFT_StorageReliabilityCounter") {
        Ok(_) => Value::unavailable(
            "ATA SMART attribute tables need a pass-through command, which Trontop does not send",
        ),
        Err(error) => Value::unavailable(error.to_string()),
    }
}

pub(super) fn optical(ctx: &Context) -> Result<Vec<(String, Vec<Row>)>, String> {
    let devices = setupapi::devices(&Query::new(Filter::Class(GUID_DEVCLASS_CDROM), ctx))
        .map_err(|e| e.to_string())?;
    Ok(devices
        .into_iter()
        .map(|d| {
            let name = d.name().unwrap_or("Optical drive").to_string();
            let rows = vec![
                Row::new(
                    "Manufacturer",
                    Value::from_option(d.manufacturer.clone(), NOT_REPORTED_DRIVE),
                ),
                Row::new(
                    "Driver",
                    Value::from_option(
                        d.driver_version.clone().map(|v| match &d.driver_date {
                            Some(date) => format!("{v} ({date})"),
                            None => v,
                        }),
                        NOT_REPORTED_DRIVE,
                    ),
                ),
                Row::new(
                    "Status",
                    Value::from_option(d.status.map(DevNodeStatus::label), NOT_REPORTED_DRIVE),
                ),
                Row::known("Device instance", d.instance_id.clone()).private(),
            ];
            (name, rows)
        })
        .collect())
}
