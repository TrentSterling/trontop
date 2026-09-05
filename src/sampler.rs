use crate::diagnostics::{Diagnostics, Issue, Provider, State};
use crate::gpu_sensors::SensorSampler;
use crate::model::{
    CpuInfo, DiskRow, NetworkRow, ProcessControlInfo, ProcessRow, SystemSnapshot, UserSummary,
};
use crate::platform;
use crate::tray::{TraySample, TraySink};
use crate::windows_metrics::GpuSampler;
use eframe::egui;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use sysinfo::{Disks, Networks, ProcessesToUpdate, System, Users};

const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);
const GPU_INVENTORY_INTERVAL: u64 = 30;
const CONTROL_REFRESH_INTERVAL: u64 = 5;

pub struct Sampler {
    latest: Arc<RwLock<SystemSnapshot>>,
    generation: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
    service_refresh: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Sampler {
    pub fn spawn(ctx: egui::Context, tray: Option<TraySink>) -> Self {
        let latest = Arc::new(RwLock::new(SystemSnapshot::default()));
        let generation = Arc::new(AtomicU64::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let service_refresh = Arc::new(AtomicBool::new(false));
        let worker_service_refresh = Arc::clone(&service_refresh);

        let worker_latest = Arc::clone(&latest);
        let worker_generation = Arc::clone(&generation);
        let worker_stop = Arc::clone(&stop);
        let worker = thread::Builder::new()
            .name("trontop-sampler".into())
            .spawn(move || {
                sample_loop(
                    worker_latest,
                    worker_generation,
                    worker_stop,
                    worker_service_refresh,
                    ctx,
                    tray,
                )
            })
            .expect("failed to start process sampler");

        Self {
            latest,
            generation,
            stop,
            service_refresh,
            worker: Some(worker),
        }
    }

    pub fn latest_after(&self, seen_generation: u64) -> Option<SystemSnapshot> {
        if self.generation.load(Ordering::Acquire) == seen_generation {
            return None;
        }
        self.latest.read().ok().map(|snapshot| snapshot.clone())
    }

    pub fn request_service_refresh(&self) {
        self.service_refresh.store(true, Ordering::Release);
    }
}

impl Drop for Sampler {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            crate::shutdown::finish(worker, Duration::ZERO);
        }
    }
}

fn sample_loop(
    latest: Arc<RwLock<SystemSnapshot>>,
    generation: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
    service_refresh: Arc<AtomicBool>,
    ctx: egui::Context,
    tray: Option<TraySink>,
) {
    // These workers start without waiting for either native inventory. Their
    // immutable snapshots can be read even while a provider call is stuck.
    let mut inventories = crate::inventory::Inventories::spawn();
    let mut system = System::new_all();
    let mut disks = Disks::new_with_refreshed_list();
    let mut networks = Networks::new_with_refreshed_list();
    let users = Users::new_with_refreshed_list();
    let mut gpu_sampler = GpuSampler::new();
    // The optional driver library and every sensor call stay on this worker.
    let mut sensor_sampler = SensorSampler::default();
    // Storage enumeration and every drive query have isolated, bounded workers.
    let storage_monitor = crate::storage_sensors::Monitor::spawn();
    let mut sequence = 0_u64;
    let mut previous_sample = Instant::now();
    let mut diagnostics = Diagnostics::default();
    let mut process_controls = HashMap::<(u32, u64), ProcessControlInfo>::new();

    system.refresh_cpu_frequency();

    if wait_for_stop(&stop, sysinfo::MINIMUM_CPU_UPDATE_INTERVAL) {
        return;
    }

    loop {
        if stop.load(Ordering::Acquire) {
            return;
        }
        let cycle_started = Instant::now();
        let sample_seconds = previous_sample.elapsed().as_secs_f64().max(0.001);
        previous_sample = Instant::now();

        system.refresh_cpu_usage();
        system.refresh_memory();
        system.refresh_processes(ProcessesToUpdate::All, true);
        disks.refresh(true);
        networks.refresh(true);
        if sequence.is_multiple_of(CONTROL_REFRESH_INTERVAL) {
            system.refresh_cpu_frequency();
        }
        let system_ok = !system.cpus().is_empty() && system.total_memory() > 0;
        diagnostics.get_mut(Provider::System).record(
            cycle_started,
            cycle_started.elapsed(),
            if system_ok {
                State::Live
            } else if system.cpus().is_empty() && system.total_memory() == 0 {
                State::Unavailable
            } else {
                State::Partial
            },
            None,
            (!system_ok).then_some(Issue::MissingSystemData),
        );

        let control_started = Instant::now();
        let active_process_keys = system
            .processes()
            .values()
            .map(|process| (process.pid().as_u32(), process.start_time()))
            .collect::<HashSet<_>>();
        process_controls.retain(|key, _| active_process_keys.contains(key));
        for &(pid, started_at) in &active_process_keys {
            if sequence.is_multiple_of(CONTROL_REFRESH_INTERVAL)
                || !process_controls.contains_key(&(pid, started_at))
            {
                process_controls.insert(
                    (pid, started_at),
                    platform::query_process_control(pid)
                        .unwrap_or_default()
                        .for_observed_start(started_at),
                );
            }
        }
        if sequence.is_multiple_of(CONTROL_REFRESH_INTERVAL) {
            let readable = process_controls
                .values()
                .filter(|info| info.accessible && info.created_at_100ns.is_some())
                .count();
            let total = active_process_keys.len();
            diagnostics.get_mut(Provider::ProcessControls).record(
                control_started,
                control_started.elapsed(),
                if readable == total {
                    State::Live
                } else if readable > 0 {
                    State::Partial
                } else {
                    State::Unavailable
                },
                Some((readable, total)),
                (readable < total).then_some(Issue::ProcessAccess),
            );
        }

        let gpu_started = Instant::now();
        if sequence > 0 && sequence.is_multiple_of(GPU_INVENTORY_INTERVAL) {
            gpu_sampler.refresh_instances();
        }
        let (gpu, gpu_by_pid) = gpu_sampler.sample();
        let gpu_state = if gpu.available {
            if gpu.valid_counters == gpu.total_counters && gpu.error.is_none() {
                State::Live
            } else {
                State::Partial
            }
        } else if gpu.error.is_none() {
            State::Starting
        } else {
            State::Unavailable
        };
        diagnostics.get_mut(Provider::GpuActivity).record(
            gpu_started,
            gpu_started.elapsed(),
            gpu_state,
            Some((gpu.valid_counters, gpu.total_counters)),
            matches!(gpu_state, State::Partial | State::Unavailable).then_some(Issue::GpuCounters),
        );
        if service_refresh.swap(false, Ordering::AcqRel) {
            inventories.request_services();
        }
        let (startup, startup_health) = inventories.startup();
        let (services, service_health) = inventories.services();
        *diagnostics.get_mut(Provider::Startup) = startup_health;
        *diagnostics.get_mut(Provider::Services) = service_health;

        sequence += 1;
        let logical_cpu_count = system.cpus().len().max(1) as f32;
        let processes = system
            .processes()
            .values()
            .map(|process| {
                let disk = process.disk_usage();
                let user = process
                    .user_id()
                    .and_then(|id| users.get_user_by_id(id))
                    .map(|user| user.name().to_string())
                    .unwrap_or_else(|| "Unknown account".into());
                ProcessRow {
                    pid: process.pid().as_u32(),
                    parent_pid: process.parent().map(|pid| pid.as_u32()),
                    name: process.name().to_string_lossy().into_owned(),
                    status: format!("{:?}", process.status()),
                    user,
                    cpu_percent: (process.cpu_usage() / logical_cpu_count).clamp(0.0, 100.0),
                    gpu_percent: gpu.for_process(&gpu_by_pid, process.pid().as_u32()),
                    memory_bytes: process.memory(),
                    virtual_memory_bytes: process.virtual_memory(),
                    read_bytes_per_sec: disk.read_bytes as f64 / sample_seconds,
                    write_bytes_per_sec: disk.written_bytes as f64 / sample_seconds,
                    total_read_bytes: disk.total_read_bytes,
                    total_write_bytes: disk.total_written_bytes,
                    accumulated_cpu_millis: process.accumulated_cpu_time(),
                    started_at_unix: process.start_time(),
                    executable: process.exe().map(ToOwned::to_owned),
                    command: process
                        .cmd()
                        .iter()
                        .map(|part| part.to_string_lossy())
                        .collect::<Vec<_>>()
                        .join(" "),
                    cwd: process.cwd().map(ToOwned::to_owned),
                    control: process_controls
                        .get(&(process.pid().as_u32(), process.start_time()))
                        .copied()
                        .unwrap_or_default(),
                }
            })
            .collect::<Vec<_>>();

        let disk_rows = disks
            .list()
            .iter()
            .map(|disk| {
                let usage = disk.usage();
                DiskRow {
                    name: disk.name().to_string_lossy().into_owned(),
                    mount: disk.mount_point().display().to_string(),
                    file_system: disk.file_system().to_string_lossy().into_owned(),
                    kind: format!("{:?}", disk.kind()),
                    total_bytes: disk.total_space(),
                    available_bytes: disk.available_space(),
                    read_bytes_per_sec: usage.read_bytes as f64 / sample_seconds,
                    write_bytes_per_sec: usage.written_bytes as f64 / sample_seconds,
                    removable: disk.is_removable(),
                }
            })
            .collect();
        let network_rows = networks
            .iter()
            .map(|(name, network)| NetworkRow {
                name: name.clone(),
                received_bytes_per_sec: network.received() as f64 / sample_seconds,
                transmitted_bytes_per_sec: network.transmitted() as f64 / sample_seconds,
                total_received_bytes: network.total_received(),
                total_transmitted_bytes: network.total_transmitted(),
            })
            .collect();
        let user_rows = aggregate_users(&processes);
        let cpu = CpuInfo {
            brand: system
                .cpus()
                .first()
                .map(|cpu| cpu.brand().trim().to_string())
                .unwrap_or_default(),
            frequency_mhz: system.cpus().first().map_or(0, sysinfo::Cpu::frequency),
            physical_cores: System::physical_core_count().unwrap_or_default(),
            logical_cores: system.cpus().len(),
        };

        let sensors = sensor_sampler.sample();
        if let Some(at) = sensors.attempted_at {
            let health = diagnostics.get_mut(Provider::GpuSensors);
            if health.last_attempt != Some(at) {
                let total = sensors.adapters.len() * 6;
                let available = sensors
                    .adapters
                    .iter()
                    .map(|a| {
                        [
                            a.temperature_c.is_some(),
                            a.power_w.is_some(),
                            a.graphics_clock_mhz.is_some(),
                            a.memory_clock_mhz.is_some(),
                            a.fan_percent.is_some(),
                            a.memory.is_some(),
                        ]
                        .into_iter()
                        .filter(|present| *present)
                        .count()
                    })
                    .sum::<usize>();
                let state = if sensors.error.is_some() || available == 0 {
                    State::Unavailable
                } else if available == total {
                    State::Live
                } else {
                    State::Partial
                };
                health.record(
                    at,
                    Duration::from_secs_f64(sensors.query_millis / 1000.0),
                    state,
                    Some((
                        if sensors.error.is_some() {
                            0
                        } else {
                            available
                        },
                        total,
                    )),
                    match state {
                        State::Unavailable => Some(Issue::GpuDriver),
                        State::Partial => Some(Issue::PartialSensors),
                        _ => None,
                    },
                );
            }
        }
        if stop.load(Ordering::Acquire) {
            return;
        }
        let storage_sensors = storage_monitor.latest();
        *diagnostics.get_mut(Provider::StorageSensors) = storage_sensors.health(Instant::now());
        let snapshot = SystemSnapshot {
            diagnostics: diagnostics.clone(),
            sequence,
            cpu_percent: system.global_cpu_usage().clamp(0.0, 100.0),
            memory_used_bytes: system.used_memory(),
            memory_total_bytes: system.total_memory(),
            memory_available_bytes: system.available_memory(),
            swap_used_bytes: system.used_swap(),
            swap_total_bytes: system.total_swap(),
            process_count: processes.len(),
            uptime_seconds: System::uptime(),
            sample_seconds,
            host_name: System::host_name().unwrap_or_else(|| "Windows PC".into()),
            os_name: System::long_os_version().unwrap_or_else(|| "Windows".into()),
            cpu,
            processes,
            disks: disk_rows,
            networks: network_rows,
            gpu,
            gpu_sensors: sensors,
            storage_sensors,
            users: user_rows,
            startup: Arc::clone(&startup),
            services: Arc::clone(&services),
        };

        if let Some(tray) = &tray {
            tray.publish(TraySample {
                cpu_percent: snapshot.cpu_percent,
                memory_percent: if snapshot.memory_total_bytes == 0 {
                    0.0
                } else {
                    snapshot.memory_used_bytes as f32 / snapshot.memory_total_bytes as f32 * 100.0
                },
                gpu_percent: snapshot.gpu.reading().exact(),
                process_count: snapshot.process_count,
            });
        }
        if let Ok(mut slot) = latest.write() {
            *slot = snapshot;
            generation.store(sequence, Ordering::Release);
        }
        ctx.request_repaint();

        let remaining = SAMPLE_INTERVAL.saturating_sub(cycle_started.elapsed());
        if wait_for_stop(&stop, remaining) {
            break;
        }
    }
}

fn aggregate_users(processes: &[ProcessRow]) -> Vec<UserSummary> {
    let mut users = HashMap::<String, UserSummary>::new();
    for process in processes {
        let row = users
            .entry(process.user.clone())
            .or_insert_with(|| UserSummary {
                name: process.user.clone(),
                ..Default::default()
            });
        row.gpu_percent = if row.process_count == 0 {
            process.gpu_percent
        } else {
            row.gpu_percent.sum(process.gpu_percent)
        };
        row.process_count += 1;
        row.cpu_percent += process.cpu_percent;
        row.memory_bytes += process.memory_bytes;
        row.disk_bytes_per_sec += process.read_bytes_per_sec + process.write_bytes_per_sec;
    }
    let mut rows = users.into_values().collect::<Vec<_>>();
    rows.sort_by(|a, b| b.memory_bytes.cmp(&a.memory_bytes));
    rows
}

fn wait_for_stop(stop: &AtomicBool, duration: Duration) -> bool {
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline {
        if stop.load(Ordering::Acquire) {
            return true;
        }
        thread::sleep(
            Duration::from_millis(25).min(deadline.saturating_duration_since(Instant::now())),
        );
    }
    stop.load(Ordering::Acquire)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn user_gpu_totals_do_not_turn_missing_process_data_into_zero() {
        use crate::gpu_activity::Usage;
        let processes: Vec<_> = [
            ("a", Usage::Measured(7.0)),
            ("a", Usage::Unavailable),
            ("b", Usage::Measured(0.0)),
            ("c", Usage::Unreported),
        ]
        .into_iter()
        .map(|(user, gpu_percent)| ProcessRow {
            user: user.into(),
            gpu_percent,
            ..Default::default()
        })
        .collect();
        let users = aggregate_users(&processes);
        let usage = |name: &str| users.iter().find(|u| u.name == name).unwrap().gpu_percent;
        assert_eq!(usage("a"), Usage::Partial(7.0));
        assert_eq!(usage("b"), Usage::Measured(0.0));
        assert_eq!(usage("c"), Usage::Unreported);
    }
}
