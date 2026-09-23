use crate::gpu_activity::{self, EngineInstance, Usage};
use crate::model::{GpuSnapshot, ServiceRow};
use std::collections::HashMap;

mod startup;
pub use startup::enumerate_startup;

fn counter_reading(api_ok: bool, status_ok: bool, value: f64) -> Option<f32> {
    (api_ok && status_ok && value.is_finite() && value >= 0.0).then(|| value.min(100.0) as f32)
}

/// One engine counter's contribution to a sample's coverage.
#[derive(Clone, Copy, Debug, PartialEq)]
enum CounterState {
    /// The engine instance no longer exists (its process exited). It is not a
    /// coverage gap for the live system; the handle is dropped at the next inventory.
    Ended,
    Reading(Usage),
}

fn classify_counter(instance_gone: bool, reading: Option<f32>) -> CounterState {
    if instance_gone {
        CounterState::Ended
    } else {
        CounterState::Reading(reading.map_or(Usage::Unavailable, Usage::Measured))
    }
}

#[cfg(windows)]
mod native {
    use super::*;
    use windows::Win32::System::Performance::{
        PDH_CSTATUS_NEW_DATA, PDH_CSTATUS_NO_INSTANCE, PDH_CSTATUS_VALID_DATA,
        PDH_FMT_COUNTERVALUE, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY, PDH_MORE_DATA,
        PERF_DETAIL_WIZARD, PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData,
        PdhEnumObjectItemsW, PdhGetFormattedCounterValue, PdhOpenQueryW, PdhRemoveCounter,
    };
    use windows::Win32::System::Services::{
        CloseServiceHandle, ENUM_SERVICE_STATUS_PROCESSW, EnumServicesStatusExW, OpenSCManagerW,
        SC_ENUM_PROCESS_INFO, SC_MANAGER_ENUMERATE_SERVICE, SERVICE_STATE_ALL, SERVICE_WIN32,
    };
    use windows::core::{PCWSTR, PWSTR};

    struct Counter {
        instance: String,
        identity: EngineInstance,
        handle: PDH_HCOUNTER,
        samples: u8,
    }

    pub struct GpuSampler {
        adapters: crate::gpu_adapters::Sampler,
        query: Option<PDH_HQUERY>,
        counters: Vec<Counter>,
        last_error: Option<String>,
    }

    impl GpuSampler {
        pub fn new() -> Self {
            let mut sampler = Self {
                adapters: crate::gpu_adapters::Sampler::default(),
                query: None,
                counters: Vec::new(),
                last_error: None,
            };
            sampler.refresh_instances();
            sampler
        }

        pub fn refresh_instances(&mut self) {
            // Keep existing handles and their previous samples. An inventory refresh
            // must not force every live rate through another warm-up interval.
            let instances = match enumerate_gpu_instances() {
                Ok(instances) => instances
                    .into_iter()
                    .collect::<std::collections::HashSet<_>>(),
                Err(error) => {
                    self.last_error = Some(error);
                    return;
                }
            };
            if self.query.is_none() {
                let mut query = PDH_HQUERY::default();
                let status = unsafe { PdhOpenQueryW(PCWSTR::null(), 0, &mut query) };
                if status != 0 {
                    self.last_error = Some(format!("Could not open GPU PDH query: 0x{status:08X}"));
                    return;
                }
                self.query = Some(query);
            }
            let query = self.query.unwrap();
            self.last_error = None;
            self.counters.retain(|counter| {
                if instances.contains(&counter.instance) {
                    return true;
                }
                let status = unsafe { PdhRemoveCounter(counter.handle) };
                if status != 0 {
                    self.last_error = Some(format!("Could not remove GPU counter: 0x{status:08X}"));
                    return true; // Query still owns it; retry at the next inventory.
                }
                false
            });
            let retained: std::collections::HashSet<_> = self
                .counters
                .iter()
                .map(|counter| counter.instance.clone())
                .collect();
            for instance in instances {
                if retained.contains(&instance) {
                    continue;
                }
                let Some(identity) = EngineInstance::parse(&instance) else {
                    self.last_error = Some("Unrecognized GPU engine identity".into());
                    continue;
                };
                let path = format!(r"\GPU Engine({instance})\Utilization Percentage");
                let wide: Vec<_> = path.encode_utf16().chain(Some(0)).collect();
                let mut handle = PDH_HCOUNTER::default();
                let status =
                    unsafe { PdhAddEnglishCounterW(query, PCWSTR(wide.as_ptr()), 0, &mut handle) };
                if status == 0 {
                    self.counters.push(Counter {
                        instance,
                        identity,
                        handle,
                        samples: 0,
                    });
                } else {
                    self.last_error = Some(format!("Could not add GPU counter: 0x{status:08X}"));
                }
            }
            if self.counters.is_empty() {
                self.last_error = Some("No active Windows GPU Engine counters".into());
            }
        }

        pub fn sample(&mut self) -> (GpuSnapshot, HashMap<u32, Usage>) {
            let (mut snapshot, by_pid) = self.sample_engines();
            snapshot.adapters = self.adapters.sample(
                std::mem::take(&mut snapshot.adapters),
                std::time::Instant::now(),
            );
            (snapshot, by_pid)
        }

        fn sample_engines(&mut self) -> (GpuSnapshot, HashMap<u32, Usage>) {
            let Some(query) = self.query else {
                return (
                    GpuSnapshot {
                        error: self.last_error.clone(),
                        ..Default::default()
                    },
                    HashMap::new(),
                );
            };

            if self.counters.is_empty() {
                return (
                    GpuSnapshot {
                        error: self.last_error.clone(),
                        ..Default::default()
                    },
                    HashMap::new(),
                );
            }
            let status = unsafe { PdhCollectQueryData(query) };
            if status != 0 {
                // A failed collection breaks the interval. Re-prime counters before
                // publishing a rate after recovery, never interpolate across it.
                for counter in &mut self.counters {
                    counter.samples = 0;
                }
                return (
                    GpuSnapshot {
                        error: Some(format!("PDH GPU collection failed: 0x{status:08X}")),
                        ..Default::default()
                    },
                    HashMap::new(),
                );
            }
            let mut readings = Vec::with_capacity(self.counters.len());
            let mut valid_counters = 0;
            let mut ended_counters = 0;
            for counter in &mut self.counters {
                counter.samples = counter.samples.saturating_add(1);
                if counter.samples < 2 {
                    readings.push((&counter.identity, Usage::Warming));
                    continue;
                }
                let mut value = PDH_FMT_COUNTERVALUE::default();
                let status = unsafe {
                    PdhGetFormattedCounterValue(counter.handle, PDH_FMT_DOUBLE, None, &mut value)
                };
                let number = counter_reading(
                    status == 0,
                    value.CStatus == PDH_CSTATUS_VALID_DATA
                        || value.CStatus == PDH_CSTATUS_NEW_DATA,
                    unsafe { value.Anonymous.doubleValue },
                );
                match classify_counter(value.CStatus == PDH_CSTATUS_NO_INSTANCE, number) {
                    CounterState::Ended => ended_counters += 1,
                    CounterState::Reading(usage) => {
                        if number.is_some() {
                            valid_counters += 1;
                        }
                        readings.push((&counter.identity, usage));
                    }
                }
            }
            let adapters =
                crate::gpu_adapters::aggregate(readings.iter().copied(), self.last_error.is_some());
            let (total, mut by_pid, engines) = gpu_activity::aggregate(readings);
            // Incomplete enumeration may have omitted an engine for any process.
            if self.last_error.is_some() {
                for reading in by_pid.values_mut() {
                    if let Some(value) = reading.value() {
                        *reading = Usage::Partial(value);
                    }
                }
            }
            (
                GpuSnapshot {
                    adapters,
                    available: valid_counters > 0,
                    valid_counters,
                    total_counters: self.counters.len() - ended_counters,
                    utilization_percent: total.value().unwrap_or_default(),
                    engine_utilization: engines,
                    error: self.last_error.clone().or_else(|| {
                        (total == Usage::Unavailable)
                            .then(|| "No valid GPU counter readings".into())
                    }),
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
                    status: crate::service_control::status_from_native(entry.ServiceStatusProcess),
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

    #[cfg(test)]
    mod gpu_tests {
        use super::*;
        use std::time::{Duration, Instant};

        #[test]
        #[ignore = "Read-only native GPU PDH query; no app window, tray or input"]
        fn native_gpu_refresh_preserves_warm_counters() {
            let mut sampler = GpuSampler::new();
            assert!(sampler.query.is_some(), "{:?}", sampler.last_error);
            assert!(!sampler.counters.is_empty(), "{:?}", sampler.last_error);
            let first = sampler.sample();
            assert!(!first.0.available);
            std::thread::sleep(Duration::from_millis(250));
            let second = sampler.sample();
            assert!(second.0.available, "{:?}", second.0.error);
            let before: HashMap<_, _> = sampler
                .counters
                .iter()
                .map(|c| (c.instance.clone(), (c.handle.0, c.samples)))
                .collect();
            let started = Instant::now();
            sampler.refresh_instances();
            let refresh_ms = started.elapsed().as_secs_f64() * 1000.0;
            let mut retained = 0;
            for counter in &sampler.counters {
                if let Some(&(handle, samples)) = before.get(&counter.instance) {
                    assert_eq!(counter.handle.0, handle);
                    assert_eq!(counter.samples, samples);
                    retained += 1;
                }
            }
            assert!(retained > 0);
            std::thread::sleep(Duration::from_millis(250));
            let (after, processes) = sampler.sample();
            assert!(after.available, "{:?}", after.error);
            assert!(
                processes
                    .values()
                    .filter_map(|v| v.value())
                    .all(|v| v.is_finite() && (0.0..=100.0).contains(&v))
            );
            // Every engine this box reports must parse; an unrecognized identity
            // used to pin the whole GPU reading at Partial forever.
            assert!(sampler.last_error.is_none(), "{:?}", sampler.last_error);
            eprintln!(
                "Native GPU refresh: {retained} handles retained, {refresh_ms:.3} ms inventory, {}/{} valid counters, {} PID readings; no UI/input",
                after.valid_counters,
                after.total_counters,
                processes.len()
            );
        }
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

    pub fn refresh_instances(&mut self) {}

    pub fn sample(&mut self) -> (GpuSnapshot, HashMap<u32, Usage>) {
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

    #[test]
    fn exited_engine_instances_do_not_mark_live_coverage_partial() {
        // A vanished instance is dropped, not reported as a missing live reading.
        assert_eq!(classify_counter(true, None), CounterState::Ended);
        assert_eq!(
            classify_counter(false, None),
            CounterState::Reading(Usage::Unavailable)
        );
        assert_eq!(
            classify_counter(false, Some(3.5)),
            CounterState::Reading(Usage::Measured(3.5))
        );
    }

    #[cfg(windows)]
    #[test]
    fn service_control_manager_returns_inventory() {
        let services = enumerate_services().expect("SCM inventory should be readable");
        assert!(!services.is_empty());
        assert!(services.iter().any(|service| !service.name.is_empty()));
    }
}
