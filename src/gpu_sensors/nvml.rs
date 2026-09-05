//! Minimal bindings to documented, read-only NVML queries. The ABI is described at
//! https://docs.nvidia.com/deploy/nvml-api/group__nvmlDeviceQueries.html
//! No NVIDIA implementation or header is copied into this project.
use super::AdapterSensors;
use std::ffi::{CStr, c_char, c_void};
use windows::Win32::Foundation::{FreeLibrary, HMODULE};
use windows::Win32::System::LibraryLoader::{
    GetProcAddress, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW,
};
use windows::core::{PCSTR, w};

type Device = *mut c_void;
type Status = i32;
type Init = unsafe extern "C" fn() -> Status;
type Count = unsafe extern "C" fn(*mut u32) -> Status;
type Handle = unsafe extern "C" fn(u32, *mut Device) -> Status;
type TextQuery = unsafe extern "C" fn(Device, *mut c_char, u32) -> Status;
type ScalarQuery = unsafe extern "C" fn(Device, *mut u32) -> Status;
type TypedQuery = unsafe extern "C" fn(Device, u32, *mut u32) -> Status;
type MemoryQuery = unsafe extern "C" fn(Device, *mut Memory) -> Status;
type MemoryQueryV2 = unsafe extern "C" fn(Device, *mut MemoryV2) -> Status;

#[repr(C)]
#[derive(Default)]
struct Memory {
    total: u64,
    free: u64,
    used: u64,
}

#[repr(C)]
#[derive(Default)]
struct MemoryV2 {
    version: u32,
    total: u64,
    reserved: u64,
    free: u64,
    used: u64,
}

struct Library(HMODULE);

impl Drop for Library {
    fn drop(&mut self) {
        // SAFETY: owns exactly one successful LoadLibraryExW reference.
        unsafe {
            let _ = FreeLibrary(self.0);
        }
    }
}

pub(super) struct Nvml {
    _library: Library,
    shutdown: Init,
    count: Count,
    handle: Handle,
    queries: Queries,
}

#[derive(Default)]
struct Queries {
    name: Option<TextQuery>,
    uuid: Option<TextQuery>,
    temperature: Option<TypedQuery>,
    power: Option<ScalarQuery>,
    clocks: Option<TypedQuery>,
    fan: Option<ScalarQuery>,
    memory: Option<MemoryQuery>,
    memory_v2: Option<MemoryQueryV2>,
}

impl Nvml {
    pub(super) fn load() -> Result<Self, String> {
        // SAFETY: constant DLL name with System32-only dependency resolution.
        // No PATH, CWD, executable directory, or user-supplied DLL is searched.
        let library = Library(unsafe {
            LoadLibraryExW(w!("nvml.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32)
        }.map_err(|_| "NVIDIA sensors unavailable: nvml.dll is not installed in Windows System32. Other telemetry still works.".to_string())?);
        macro_rules! symbol {
            ($name:literal, $ty:ty) => {
                // SAFETY: documented export name and exact C signature. The
                // Library is owned by Nvml until after shutdown and all queries.
                unsafe {
                    GetProcAddress(library.0, PCSTR(concat!($name, "\0").as_ptr())).map(|address| {
                        std::mem::transmute::<unsafe extern "system" fn() -> isize, $ty>(address)
                    })
                }
            };
        }
        let init = symbol!("nvmlInit_v2", Init).ok_or("NVML initialization export unavailable")?;
        let shutdown = symbol!("nvmlShutdown", Init).ok_or("NVML shutdown export unavailable")?;
        let count =
            symbol!("nvmlDeviceGetCount_v2", Count).ok_or("NVML enumeration export unavailable")?;
        let handle = symbol!("nvmlDeviceGetHandleByIndex_v2", Handle)
            .ok_or("NVML device export unavailable")?;
        let queries = Queries {
            name: symbol!("nvmlDeviceGetName", TextQuery),
            uuid: symbol!("nvmlDeviceGetUUID", TextQuery),
            temperature: symbol!("nvmlDeviceGetTemperature", TypedQuery),
            power: symbol!("nvmlDeviceGetPowerUsage", ScalarQuery),
            clocks: symbol!("nvmlDeviceGetClockInfo", TypedQuery),
            fan: symbol!("nvmlDeviceGetFanSpeed", ScalarQuery),
            memory: symbol!("nvmlDeviceGetMemoryInfo", MemoryQuery),
            memory_v2: symbol!("nvmlDeviceGetMemoryInfo_v2", MemoryQueryV2),
        };
        // SAFETY: live library, resolved C ABI, no arguments or output pointers.
        let status = unsafe { init() };
        check(status, "initialization")?;
        Ok(Self {
            _library: library,
            shutdown,
            count,
            handle,
            queries,
        })
    }

    pub(super) fn sample(&self) -> Result<Vec<AdapterSensors>, String> {
        let mut count = 0;
        // SAFETY: live initialized library and writable u32 output.
        check(unsafe { (self.count)(&mut count) }, "enumeration")?;
        if count == 0 {
            return Err("NVML found no NVIDIA GPUs.".into());
        }
        if count > 64 {
            return Err("NVML returned an unexpected adapter count.".into());
        }
        let mut adapters = Vec::with_capacity(count as usize);
        for index in 0..count {
            let mut device = std::ptr::null_mut();
            // SAFETY: index in enumerated count, writable device output.
            let result = check(
                unsafe { (self.handle)(index, &mut device) },
                "device access",
            );
            if let Err(error) = result {
                adapters.push(AdapterSensors {
                    name: format!("NVIDIA adapter {index}"),
                    error: Some(error),
                    ..Default::default()
                });
            } else if device.is_null() {
                adapters.push(AdapterSensors {
                    name: format!("NVIDIA adapter {index}"),
                    error: Some("NVML returned an empty device handle.".into()),
                    ..Default::default()
                });
            } else {
                adapters.push(self.queries.read(device, index));
            }
        }
        // An entirely lost device set must reinitialize, with the sampler's backoff.
        if adapters.iter().all(|adapter| adapter.error.is_some()) {
            return Err(adapters[0].error.clone().unwrap());
        }
        Ok(adapters)
    }
}

impl Drop for Nvml {
    fn drop(&mut self) {
        // SAFETY: paired with successful init, before Library drops its reference.
        unsafe {
            (self.shutdown)();
        }
    }
}

impl Queries {
    fn read(&self, device: Device, index: u32) -> AdapterSensors {
        let mut failed = false;
        let mut scalar = |query: Option<ScalarQuery>| -> Option<u32> {
            let query = query?;
            let mut value = 0;
            // SAFETY: device came from NVML; output has the documented u32 layout.
            let status = unsafe { query(device, &mut value) };
            failed |= status == 15; // NVML_ERROR_GPU_IS_LOST
            (status == 0).then_some(value)
        };
        let power_w = scalar(self.power).map(|mw| mw as f32 / 1000.0);
        let fan_percent = scalar(self.fan);
        let mut typed = |query: Option<TypedQuery>, kind: u32| -> Option<u32> {
            let query = query?;
            let mut value = 0;
            // SAFETY: documented sensor/clock enums and writable output.
            let status = unsafe { query(device, kind, &mut value) };
            failed |= status == 15;
            (status == 0).then_some(value)
        };
        let temperature_c = typed(self.temperature, 0); // NVML_TEMPERATURE_GPU
        let graphics_clock_mhz = typed(self.clocks, 0); // NVML_CLOCK_GRAPHICS
        let memory_clock_mhz = typed(self.clocks, 2); // NVML_CLOCK_MEM
        let mut memory_includes_reserved = false;
        let memory_v2 = self.memory_v2.and_then(|query| {
            let mut value = MemoryV2 {
                version: std::mem::size_of::<MemoryV2>() as u32 | (2 << 24),
                ..Default::default()
            };
            // SAFETY: versioned v2 repr(C) layout, including reserved bytes.
            let status = unsafe { query(device, &mut value) };
            failed |= status == 15;
            (status == 0 && value.total > 0 && value.used <= value.total)
                .then_some((value.used, value.total))
        });
        let memory = memory_v2.or_else(|| {
            self.memory.and_then(|query| {
                let mut value = Memory::default();
                // SAFETY: exact v1 repr(C) layout for unversioned GetMemoryInfo.
                let status = unsafe { query(device, &mut value) };
                failed |= status == 15;
                memory_includes_reserved = true;
                (status == 0 && value.total > 0 && value.used <= value.total)
                    .then_some((value.used, value.total))
            })
        });
        let name = text(self.name, device).unwrap_or_else(|| format!("NVIDIA adapter {index}"));
        let uuid = text(self.uuid, device);
        if failed {
            return AdapterSensors {
                name,
                uuid,
                error: Some("NVIDIA GPU became inaccessible; readings unavailable.".into()),
                ..Default::default()
            };
        }
        AdapterSensors {
            name,
            uuid,
            temperature_c,
            power_w,
            graphics_clock_mhz,
            memory_clock_mhz,
            fan_percent,
            memory,
            memory_includes_reserved,
            error: None,
        }
    }
}

fn text(query: Option<TextQuery>, device: Device) -> Option<String> {
    let mut buffer = [0_u8; 256];
    // SAFETY: buffer length passed exactly, write pointer valid for that length.
    if unsafe { query?(device, buffer.as_mut_ptr().cast(), buffer.len() as u32) } != 0 {
        return None;
    }
    // Bounded NUL search, even if a broken driver omits the terminator.
    let value = CStr::from_bytes_until_nul(&buffer).ok()?.to_string_lossy();
    (!value.is_empty()).then(|| value.into_owned())
}

fn check(status: Status, operation: &str) -> Result<(), String> {
    if status == 0 {
        Ok(())
    } else {
        Err(format!(
            "NVML {operation} failed (code {status}); retrying in 30 seconds."
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe extern "C" fn unsupported(_: Device, _: *mut u32) -> Status {
        3
    }
    unsafe extern "C" fn stopped(_: Device, value: *mut u32) -> Status {
        unsafe {
            *value = 0;
        }
        0
    }
    unsafe extern "C" fn power(_: Device, value: *mut u32) -> Status {
        unsafe {
            *value = 26_970;
        }
        0
    }
    unsafe extern "C" fn lost(_: Device, _: u32, _: *mut u32) -> Status {
        15
    }
    unsafe extern "C" fn memory_v1(_: Device, value: *mut Memory) -> Status {
        unsafe {
            *value = Memory {
                total: 1000,
                free: 300,
                used: 700,
            };
        }
        0
    }
    unsafe extern "C" fn memory_v2(_: Device, value: *mut MemoryV2) -> Status {
        unsafe {
            if (*value).version != (40 | (2 << 24)) {
                return 25;
            }
            *value = MemoryV2 {
                version: (*value).version,
                total: 1000,
                reserved: 100,
                free: 300,
                used: 600,
            };
        }
        0
    }
    unsafe extern "C" fn unsupported_memory_v2(_: Device, _: *mut MemoryV2) -> Status {
        3
    }

    #[test]
    fn vram_prefers_v2_allocations_and_labels_legacy_fallback() {
        assert_eq!(std::mem::size_of::<MemoryV2>(), 40);
        let mut query = Queries {
            memory: Some(memory_v1),
            memory_v2: Some(memory_v2),
            ..Default::default()
        };
        let current = query.read(std::ptr::null_mut(), 0);
        assert_eq!(current.memory, Some((600, 1000)));
        assert!(!current.memory_includes_reserved);
        query.memory_v2 = Some(unsupported_memory_v2);
        let legacy = query.read(std::ptr::null_mut(), 0);
        assert_eq!(legacy.memory, Some((700, 1000)));
        assert!(legacy.memory_includes_reserved);
        query.memory_v2 = None;
        assert_eq!(
            query.read(std::ptr::null_mut(), 0).memory,
            Some((700, 1000))
        );
    }

    #[test]
    fn partial_support_is_not_zero_and_power_units_are_converted() {
        let query = Queries {
            power: Some(power),
            fan: Some(unsupported),
            ..Default::default()
        };
        let value = query.read(std::ptr::null_mut(), 0); // mock functions never use device
        assert_eq!(value.temperature_c, None);
        assert_eq!(value.fan_percent, None);
        assert_eq!(value.power_w, Some(26.97));
        let query = Queries {
            fan: Some(stopped),
            ..Default::default()
        };
        assert_eq!(query.read(std::ptr::null_mut(), 0).fan_percent, Some(0));
        assert_eq!(std::mem::size_of::<Memory>(), 24);
    }

    #[test]
    fn device_loss_discards_partial_readings() {
        let query = Queries {
            power: Some(power),
            temperature: Some(lost),
            ..Default::default()
        };
        let value = query.read(std::ptr::null_mut(), 0);
        assert!(value.error.is_some());
        assert_eq!(value.power_w, None);
    }

    #[test]
    #[ignore = "Read-only installed NVIDIA driver probe; no app window or input automation"]
    fn native_nvml_read_only_probe() {
        let start = std::time::Instant::now();
        let provider = Nvml::load().expect("this opt-in probe needs an installed NVIDIA driver");
        println!(
            "NVML init: {:.3} ms",
            start.elapsed().as_secs_f64() * 1000.0
        );
        for sample in 0..5 {
            if sample > 0 {
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
            let start = std::time::Instant::now();
            let adapters = provider.sample().expect("NVML device queries");
            assert!(!adapters.is_empty());
            let millis = start.elapsed().as_secs_f64() * 1000.0;
            for adapter in adapters {
                println!(
                    "NVML sample {sample}: {millis:.3} ms | {} | temp {:?} C | power {:?} W | clocks {:?}/{:?} MHz | fan {:?}% | VRAM {:?} MiB | includes reserved: {} | error: {:?}",
                    adapter.name,
                    adapter.temperature_c,
                    adapter.power_w,
                    adapter.graphics_clock_mhz,
                    adapter.memory_clock_mhz,
                    adapter.fan_percent,
                    adapter
                        .memory
                        .map(|(used, total)| (used / 1_048_576, total / 1_048_576)),
                    adapter.memory_includes_reserved,
                    adapter.error
                );
            }
        }
    }
}
