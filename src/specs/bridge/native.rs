//! Read-only provider access: the WMI namespaces LibreHardwareMonitor and
//! OpenHardwareMonitor publish while running, and HWiNFO's shared-memory view
//! opened with FILE_MAP_READ. Nothing is started, installed or elevated.
use super::*;
use crate::specs::native::{NativeError, wmi};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
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
    let mapping =
        unsafe { OpenFileMappingW(FILE_MAP_READ.0, false, w!("Global\\HWiNFO_SENS_SM2")) }
            .map(Mapping)
            .map_err(|_| {
                "not running (shared memory not published; enable Shared Memory Support in HWiNFO)"
                    .to_string()
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
    // SAFETY: RegionSize bytes are mapped and readable; copied before unmapping.
    let bytes = unsafe { std::slice::from_raw_parts(view.0.Value.cast::<u8>(), size) }.to_vec();
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
