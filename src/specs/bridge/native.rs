//! Read-only provider access: the WMI namespaces LibreHardwareMonitor and
//! OpenHardwareMonitor publish while running, and HWiNFO's shared-memory view
//! opened with FILE_MAP_READ. Nothing is started, installed or elevated.
use super::*;
use crate::specs::native::{NativeError, wmi};
use windows::Win32::Foundation::{CloseHandle, ERROR_FILE_NOT_FOUND, HANDLE};
use windows::Win32::System::Memory::{
    FILE_MAP_READ, MEMORY_BASIC_INFORMATION, MEMORY_MAPPED_VIEW_ADDRESS, MapViewOfFile,
    OpenFileMappingW, UnmapViewOfFile, VirtualQuery,
};
use windows::core::w;

const LARGEST_VIEW: usize = 16 * 1024 * 1024;

fn hardware_monitor(ctx: &Context, namespace: &str) -> Result<Vec<Raw>, String> {
    let connection = wmi::Wmi::connect(namespace).map_err(|error| match error {
        NativeError::Unsupported(_) => {
            format!("not running (WMI namespace {namespace} not present)")
        }
        other => other.to_string(),
    })?;
    let timeout = ctx.timeout(Duration::from_secs(2));
    let hardware = connection
        .query("SELECT Identifier, Name FROM Hardware", timeout)
        .map_err(|e| e.to_string())?;
    let names = hardware
        .iter()
        .filter_map(|row| Some((row.text("Identifier")?, row.text("Name")?)))
        .collect::<BTreeMap<_, _>>();
    let rows = connection
        .query(
            "SELECT Identifier, Name, SensorType, Value, Parent FROM Sensor",
            ctx.timeout(Duration::from_secs(2)),
        )
        .map_err(|e| e.to_string())?;
    let rows = rows
        .iter()
        .filter_map(|row| {
            let parent = row.text("Parent").unwrap_or_default();
            Some((
                row.text("Identifier")?,
                names.get(&parent).cloned().unwrap_or(parent),
                row.text("SensorType")?,
                row.text("Name")?,
                row.f64("Value")?,
            ))
        })
        .collect::<Vec<_>>();
    Ok(from_wmi(&rows))
}

struct Mapping(HANDLE);
impl Drop for Mapping {
    fn drop(&mut self) {
        // SAFETY: owns one handle from OpenFileMappingW.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

struct View(MEMORY_MAPPED_VIEW_ADDRESS);
impl Drop for View {
    fn drop(&mut self) {
        // SAFETY: unmaps the view MapViewOfFile returned.
        unsafe {
            let _ = UnmapViewOfFile(self.0);
        }
    }
}

fn hwinfo() -> Result<Vec<Raw>, String> {
    // SAFETY: read-only open of a named section; no inheritance.
    let mapping = unsafe {
        OpenFileMappingW(FILE_MAP_READ.0, false, w!("Global\\HWiNFO_SENS_SM2"))
    }
    .map(Mapping)
    .map_err(|error| {
        if error.code() == ERROR_FILE_NOT_FOUND.to_hresult() {
            "not running (shared memory not published; enable Shared Memory Support in HWiNFO)"
                .to_string()
        } else {
            NativeError::from_windows("OpenFileMappingW", &error).to_string()
        }
    })?;
    // SAFETY: maps the whole section read-only.
    let view = View(unsafe { MapViewOfFile(mapping.0, FILE_MAP_READ, 0, 0, 0) });
    if view.0.Value.is_null() {
        return Err(NativeError::last_error("MapViewOfFile").to_string());
    }
    let mut info = MEMORY_BASIC_INFORMATION::default();
    // SAFETY: queries the region of our own mapped view.
    let written = unsafe {
        VirtualQuery(
            Some(view.0.Value.cast_const()),
            &mut info,
            std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
        )
    };
    if written == 0 {
        return Err(NativeError::last_error("VirtualQuery").to_string());
    }
    let size = info.RegionSize.min(LARGEST_VIEW);
    // HWiNFO writes this memory from its own process, so no Rust reference to
    // it is ever formed: the bytes are copied raw and parsed from the copy
    // (bounds-checked; a torn copy can only mix two polls, never read out of
    // bounds).
    let mut bytes = Vec::<u8>::with_capacity(size);
    // SAFETY: RegionSize bytes are mapped and readable, `bytes` has `size`
    // bytes of capacity, the ranges cannot overlap, and u8 has no invalid
    // bit patterns; copied before unmapping.
    unsafe {
        std::ptr::copy_nonoverlapping(view.0.Value.cast::<u8>(), bytes.as_mut_ptr(), size);
        bytes.set_len(size);
    }
    drop(view);
    drop(mapping);
    parse_hwinfo(&bytes)
}

pub(super) fn poll(ctx: &Context) -> Poll {
    Poll {
        providers: vec![
            (LIBRE, hardware_monitor(ctx, r"ROOT\LibreHardwareMonitor")),
            (OPEN, hardware_monitor(ctx, r"ROOT\OpenHardwareMonitor")),
            (HWINFO, hwinfo()),
        ],
    }
}
