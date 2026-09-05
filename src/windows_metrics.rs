use crate::model::{GpuSnapshot, ServiceRow, StartupRow};
use std::collections::HashMap;
use std::path::Path;

#[cfg(windows)]
mod native {
    use super::*;
    use windows::Win32::System::Performance::{
        PDH_CSTATUS_NEW_DATA, PDH_CSTATUS_VALID_DATA, PDH_FMT_COUNTERVALUE, PDH_FMT_DOUBLE,
        PDH_HCOUNTER, PDH_HQUERY, PDH_MORE_DATA, PERF_DETAIL_WIZARD, PdhAddEnglishCounterW,
        PdhCloseQuery, PdhCollectQueryData, PdhEnumObjectItemsW, PdhGetFormattedCounterValue,
        PdhOpenQueryW,
    };
    use windows::Win32::System::Services::{
        CloseServiceHandle, ENUM_SERVICE_STATUS_PROCESSW, EnumServicesStatusExW, OpenSCManagerW,
        SC_ENUM_PROCESS_INFO, SC_MANAGER_ENUMERATE_SERVICE, SERVICE_CONTINUE_PENDING,
        SERVICE_PAUSE_PENDING, SERVICE_PAUSED, SERVICE_RUNNING, SERVICE_START_PENDING,
        SERVICE_STATE_ALL, SERVICE_STOP_PENDING, SERVICE_STOPPED, SERVICE_WIN32,
    };
    use windows::core::{PCWSTR, PWSTR};

    struct Counter {
        pid: u32,
        engine: String,
        handle: PDH_HCOUNTER,
    }

    pub struct GpuSampler {
        query: Option<PDH_HQUERY>,
        counters: Vec<Counter>,
        samples: u32,
        last_error: Option<String>,
    }

    impl GpuSampler {
        pub fn new() -> Self {
            let mut sampler = Self {
                query: None,
                counters: Vec::new(),
                samples: 0,
                last_error: None,
            };
            sampler.rebuild();
            sampler
        }

        pub fn rebuild(&mut self) {
            self.close_query();
            match enumerate_gpu_instances().and_then(open_gpu_query) {
                Ok((query, counters)) if !counters.is_empty() => {
                    self.query = Some(query);
                    self.counters = counters;
                    self.samples = 0;
                    self.last_error = None;
                    unsafe {
                        let _ = PdhCollectQueryData(query);
                    }
                }
                Ok((query, _)) => {
                    unsafe {
                        let _ = PdhCloseQuery(query);
                    }
                    self.last_error = Some("No active Windows GPU Engine counters".into());
                }
                Err(error) => self.last_error = Some(error),
            }
        }

        pub fn sample(&mut self) -> (GpuSnapshot, HashMap<u32, f32>) {
            let Some(query) = self.query else {
                return (
                    GpuSnapshot {
                        error: self.last_error.clone(),
                        ..Default::default()
                    },
                    HashMap::new(),
                );
            };

            let status = unsafe { PdhCollectQueryData(query) };
            if status != 0 {
                return (
                    GpuSnapshot {
                        error: Some(format!("PDH GPU collection failed: 0x{status:08X}")),
                        ..Default::default()
                    },
                    HashMap::new(),
                );
            }
            self.samples += 1;
            if self.samples < 2 {
                return (GpuSnapshot::default(), HashMap::new());
            }

            let mut by_pid = HashMap::<u32, f32>::new();
            let mut by_engine = HashMap::<String, f32>::new();
            for counter in &self.counters {
                let mut value = PDH_FMT_COUNTERVALUE::default();
                let status = unsafe {
                    PdhGetFormattedCounterValue(counter.handle, PDH_FMT_DOUBLE, None, &mut value)
                };
                if status != 0
                    || (value.CStatus != PDH_CSTATUS_VALID_DATA
                        && value.CStatus != PDH_CSTATUS_NEW_DATA)
                {
                    continue;
                }
                let number = unsafe { value.Anonymous.doubleValue } as f32;
                if !number.is_finite() || number <= 0.0 {
                    continue;
                }
                *by_pid.entry(counter.pid).or_default() += number;
                *by_engine.entry(counter.engine.clone()).or_default() += number;
            }
            for value in by_pid.values_mut() {
                *value = value.clamp(0.0, 100.0);
            }
            let mut engines = by_engine.into_iter().collect::<Vec<_>>();
            engines.sort_by(|a, b| b.1.total_cmp(&a.1));
            for (_, value) in &mut engines {
                *value = value.clamp(0.0, 100.0);
            }
            let total = engines
                .iter()
                .map(|(_, value)| *value)
                .fold(0.0_f32, f32::max);
            (
                GpuSnapshot {
                    available: true,
                    utilization_percent: total,
                    engine_utilization: engines,
                    error: None,
                },
                by_pid,
            )
        }

        fn close_query(&mut self) {
            if let Some(query) = self.query.take() {
                unsafe {
                    let _ = PdhCloseQuery(query);
                }
            }
            self.counters.clear();
        }
    }

    impl Drop for GpuSampler {
        fn drop(&mut self) {
            self.close_query();
        }
    }

    fn enumerate_gpu_instances() -> Result<Vec<String>, String> {
        let mut counter_len = 0_u32;
        let mut instance_len = 0_u32;
        let status = unsafe {
            PdhEnumObjectItemsW(
                PCWSTR::null(),
                PCWSTR::null(),
                windows::core::w!("GPU Engine"),
                None,
                &mut counter_len,
                None,
                &mut instance_len,
                PERF_DETAIL_WIZARD,
                0,
            )
        };
        if status != PDH_MORE_DATA && status != 0 {
            return Err(format!("GPU Engine counters unavailable: 0x{status:08X}"));
        }
        if instance_len == 0 {
            return Ok(Vec::new());
        }

        let mut counters = vec![0_u16; counter_len.max(1) as usize];
        let mut instances = vec![0_u16; instance_len as usize];
        let status = unsafe {
            PdhEnumObjectItemsW(
                PCWSTR::null(),
                PCWSTR::null(),
                windows::core::w!("GPU Engine"),
                Some(PWSTR(counters.as_mut_ptr())),
                &mut counter_len,
                Some(PWSTR(instances.as_mut_ptr())),
                &mut instance_len,
                PERF_DETAIL_WIZARD,
                0,
            )
        };
        if status != 0 {
            return Err(format!("Could not enumerate GPU engines: 0x{status:08X}"));
        }
        Ok(parse_multi_sz(&instances))
    }

    fn open_gpu_query(instances: Vec<String>) -> Result<(PDH_HQUERY, Vec<Counter>), String> {
        let mut query = PDH_HQUERY::default();
        let status = unsafe { PdhOpenQueryW(PCWSTR::null(), 0, &mut query) };
        if status != 0 {
            return Err(format!("Could not open GPU PDH query: 0x{status:08X}"));
        }

        let mut counters = Vec::new();
        for instance in instances {
            let Some((pid, engine)) = parse_gpu_instance(&instance) else {
                continue;
            };
            let path = format!(r"\GPU Engine({instance})\Utilization Percentage");
            let wide = path.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
            let mut handle = PDH_HCOUNTER::default();
            let status =
                unsafe { PdhAddEnglishCounterW(query, PCWSTR(wide.as_ptr()), 0, &mut handle) };
            if status == 0 {
                counters.push(Counter {
                    pid,
                    engine,
                    handle,
                });
            }
        }
        Ok((query, counters))
    }

    fn parse_gpu_instance(instance: &str) -> Option<(u32, String)> {
        let pid_start = instance.find("pid_")? + 4;
        let pid_end = instance[pid_start..].find('_')? + pid_start;
        let pid = instance[pid_start..pid_end].parse().ok()?;
        let engine = instance
            .split("engtype_")
            .nth(1)
            .unwrap_or("GPU")
            .trim()
            .to_string();
        Some((pid, engine))
    }

    fn parse_multi_sz(buffer: &[u16]) -> Vec<String> {
        buffer
            .split(|value| *value == 0)
            .take_while(|part| !part.is_empty())
            .map(String::from_utf16_lossy)
            .collect()
    }

    pub fn enumerate_services() -> Result<Vec<ServiceRow>, String> {
        unsafe {
            let manager =
                OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_ENUMERATE_SERVICE)
                    .map_err(|error| format!("Could not open Service Control Manager: {error}"))?;
            let mut needed = 0_u32;
            let mut returned = 0_u32;
            let _ = EnumServicesStatusExW(
                manager,
                SC_ENUM_PROCESS_INFO,
                SERVICE_WIN32,
                SERVICE_STATE_ALL,
                None,
                &mut needed,
                &mut returned,
                None,
                PCWSTR::null(),
            );
            let mut buffer = vec![0_u8; needed.max(1) as usize];
            let result = EnumServicesStatusExW(
                manager,
                SC_ENUM_PROCESS_INFO,
                SERVICE_WIN32,
                SERVICE_STATE_ALL,
                Some(&mut buffer),
                &mut needed,
                &mut returned,
                None,
                PCWSTR::null(),
            );
            let _ = CloseServiceHandle(manager);
            result.map_err(|error| format!("Could not enumerate services: {error}"))?;

            let entries = std::slice::from_raw_parts(
                buffer.as_ptr().cast::<ENUM_SERVICE_STATUS_PROCESSW>(),
                returned as usize,
            );
            let mut services = entries
                .iter()
                .map(|entry| ServiceRow {
                    name: entry.lpServiceName.to_string().unwrap_or_default(),
                    display_name: entry.lpDisplayName.to_string().unwrap_or_default(),
                    status: service_state(entry.ServiceStatusProcess.dwCurrentState),
                    pid: entry.ServiceStatusProcess.dwProcessId,
                })
                .collect::<Vec<_>>();
            services.sort_by(|a, b| {
                a.display_name
                    .to_lowercase()
                    .cmp(&b.display_name.to_lowercase())
            });
            Ok(services)
        }
    }

    fn service_state(
        state: windows::Win32::System::Services::SERVICE_STATUS_CURRENT_STATE,
    ) -> String {
        match state {
            SERVICE_RUNNING => "Running",
            SERVICE_STOPPED => "Stopped",
            SERVICE_PAUSED => "Paused",
            SERVICE_START_PENDING => "Starting",
            SERVICE_STOP_PENDING => "Stopping",
            SERVICE_PAUSE_PENDING => "Pausing",
            SERVICE_CONTINUE_PENDING => "Resuming",
            _ => "Unknown",
        }
        .into()
    }
}

#[cfg(windows)]
pub use native::{GpuSampler, enumerate_services};

#[cfg(not(windows))]
pub struct GpuSampler;

#[cfg(not(windows))]
impl GpuSampler {
    pub fn new() -> Self {
        Self
    }

    pub fn rebuild(&mut self) {}

    pub fn sample(&mut self) -> (GpuSnapshot, HashMap<u32, f32>) {
        (
            GpuSnapshot {
                error: Some("GPU Engine counters require Windows".into()),
                ..Default::default()
            },
            HashMap::new(),
        )
    }
}

#[cfg(not(windows))]
pub fn enumerate_services() -> Result<Vec<ServiceRow>, String> {
    Err("Service inventory requires Windows".into())
}

pub fn enumerate_startup() -> Vec<StartupRow> {
    let mut rows = Vec::new();
    #[cfg(windows)]
    {
        for (key, source) in [
            (
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                "Current user Run key",
            ),
            (
                r"HKLM\Software\Microsoft\Windows\CurrentVersion\Run",
                "Machine Run key",
            ),
            (
                r"HKLM\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Run",
                "32-bit machine Run key",
            ),
        ] {
            if let Ok(output) = std::process::Command::new("reg.exe")
                .args(["query", key])
                .output()
            {
                let text = String::from_utf8_lossy(&output.stdout);
                for line in text.lines() {
                    if let Some(row) = parse_reg_line(line, source) {
                        rows.push(row);
                    }
                }
            }
        }

        for (variable, source) in [
            ("APPDATA", "Current user Startup folder"),
            ("PROGRAMDATA", "Machine Startup folder"),
        ] {
            if let Some(root) = std::env::var_os(variable) {
                let path = Path::new(&root).join("Microsoft/Windows/Start Menu/Programs/Startup");
                if let Ok(entries) = std::fs::read_dir(path) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        rows.push(StartupRow {
                            name: path
                                .file_stem()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into_owned(),
                            command: path.display().to_string(),
                            source: source.into(),
                        });
                    }
                }
            }
        }
    }
    rows.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    rows.dedup_by(|a, b| a.name.eq_ignore_ascii_case(&b.name) && a.command == b.command);
    rows
}

fn parse_reg_line(line: &str, source: &str) -> Option<StartupRow> {
    let trimmed = line.trim();
    let marker = ["REG_SZ", "REG_EXPAND_SZ"]
        .into_iter()
        .find_map(|kind| trimmed.find(kind).map(|index| (kind, index)))?;
    let name = trimmed[..marker.1].trim();
    let command = trimmed[marker.1 + marker.0.len()..].trim();
    if name.is_empty() || command.is_empty() {
        return None;
    }
    Some(StartupRow {
        name: name.into(),
        command: command.into(),
        source: source.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_registry_startup_row() {
        let row = parse_reg_line(
            "Discord    REG_SZ    C:\\Users\\trent\\Discord.exe --startup",
            "Run key",
        )
        .unwrap();
        assert_eq!(row.name, "Discord");
        assert!(row.command.contains("Discord.exe"));
    }

    #[cfg(windows)]
    #[test]
    fn service_control_manager_returns_inventory() {
        let services = enumerate_services().expect("SCM inventory should be readable");
        assert!(!services.is_empty());
        assert!(services.iter().any(|service| !service.name.is_empty()));
    }
}
