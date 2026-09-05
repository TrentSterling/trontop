use crate::model::{GpuSnapshot, ServiceRow};
use std::collections::HashMap;

mod startup;
pub use startup::enumerate_startup;

fn counter_reading(api_ok: bool, status_ok: bool, value: f64) -> Option<f32> {
    (api_ok && status_ok && value.is_finite() && value >= 0.0).then(|| value.min(100.0) as f32)
}

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
            let mut valid_counters = 0;
            for counter in &self.counters {
                let mut value = PDH_FMT_COUNTERVALUE::default();
                let status = unsafe {
                    PdhGetFormattedCounterValue(counter.handle, PDH_FMT_DOUBLE, None, &mut value)
                };
                let Some(number) = counter_reading(
                    status == 0,
                    value.CStatus == PDH_CSTATUS_VALID_DATA
                        || value.CStatus == PDH_CSTATUS_NEW_DATA,
                    unsafe { value.Anonymous.doubleValue },
                ) else {
                    continue;
                };
                valid_counters += 1;
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
                    available: valid_counters > 0,
                    valid_counters,
                    total_counters: self.counters.len(),
                    utilization_percent: total,
                    engine_utilization: engines,
                    error: (valid_counters == 0).then(|| "No valid GPU counter readings".into()),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdh_valid_zero_is_not_confused_with_invalid_counter_data() {
        assert_eq!(counter_reading(true, true, 0.0), Some(0.0));
        assert_eq!(counter_reading(false, true, 0.0), None);
        assert_eq!(counter_reading(true, false, 12.0), None);
        assert_eq!(counter_reading(true, true, f64::NAN), None);
        assert_eq!(counter_reading(true, true, -1.0), None);
    }

    #[cfg(windows)]
    #[test]
    fn service_control_manager_returns_inventory() {
        let services = enumerate_services().expect("SCM inventory should be readable");
        assert!(!services.is_empty());
        assert!(services.iter().any(|service| !service.name.is_empty()));
    }
}
