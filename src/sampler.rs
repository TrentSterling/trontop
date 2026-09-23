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
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use sysinfo::{Disks, Networks, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind, Users};

const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);
// A GPU engine inventory costs about 4 ms on a 650-instance box. Every 5 s keeps
// new GPU clients from staying unreported (and ended ones lingering) for long.
const GPU_INVENTORY_INTERVAL: u64 = 5;
const CONTROL_REFRESH_INTERVAL: u64 = 5;

/// Everything the process table shows except CPU time. Per-process CPU comes
/// from `process_cpu` instead: sysinfo's CPU path calls `GetSystemTimes` once
/// per process, which measured 100 to 670 ms per call on a busy 24-thread box
/// (a 40 to 65 s refresh). Static strings are read once per process instance,
/// and environment blocks are never read.
fn process_refresh_kind() -> ProcessRefreshKind {
    ProcessRefreshKind::nothing()
        .with_memory()
        .with_disk_usage()
        .with_exe(UpdateKind::OnlyIfNotSet)
        .with_user(UpdateKind::OnlyIfNotSet)
        .with_cmd(UpdateKind::OnlyIfNotSet)
        .with_cwd(UpdateKind::OnlyIfNotSet)
}

/// Process refresh plus native CPU times: the work the sampler does per sample
/// for the process table. Shared with the read-only timing probe.
fn refresh_process_table(system: &mut System, cpu_times: &mut crate::process_cpu::Tracker) -> bool {
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, process_refresh_kind());
    let reading = crate::process_cpu::read();
    let ok = reading.is_ok();
    cpu_times.update(reading.ok());
    ok
}

/// CPU share and cumulative CPU time for one sysinfo process, joined to the
/// native snapshot by PID and verified by creation time (same instance).
fn process_cpu(
    cpu_times: &crate::process_cpu::Tracker,
    pid: u32,
    started_at_unix: u64,
) -> (f32, u64) {
    cpu_times
        .times(pid)
        .filter(|times| times.started_at_unix() == Some(started_at_unix))
        .map_or((0.0, 0), |times| {
            (
                cpu_times.percent(pid).unwrap_or(0.0),
                times.accumulated_millis(),
            )
        })
}

fn cpu_info(system: &System, physical_cores: usize) -> CpuInfo {
    CpuInfo {
        brand: system
            .cpus()
            .first()
            .map(|cpu| cpu.brand().trim().to_string())
            .unwrap_or_default(),
        frequency_mhz: system.cpus().first().map_or(0, sysinfo::Cpu::frequency),
        clocks: None,
        physical_cores,
        logical_cores: system.cpus().len(),
        logical_usage: system
            .cpus()
            .iter()
            .map(|cpu| {
                let usage = cpu.cpu_usage();
                usage.is_finite().then(|| usage.clamp(0.0, 100.0))
            })
            .collect(),
    }
}

pub struct Sampler {
    latest: Arc<SnapshotMailbox>,
    stop: Arc<AtomicBool>,
    service_refresh: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

/// Single producer, single UI consumer. Transfer ownership instead of cloning
/// every process name/path/command while holding a lock on the render thread.
#[derive(Default)]
struct SnapshotMailbox {
    pending: Mutex<Option<SystemSnapshot>>,
    generation: AtomicU64,
}

impl SnapshotMailbox {
    fn publish(&self, snapshot: SystemSnapshot) {
        let sequence = snapshot.sequence;
        let retired = if let Ok(mut slot) = self.pending.lock() {
            let retired = slot.replace(snapshot);
            self.generation.store(sequence, Ordering::Release);
            retired
        } else {
            return;
        };
        // A large superseded snapshot is destroyed on the worker, outside the
        // publication lock. Slow readers never cause an unbounded backlog.
        drop(retired);
    }

    fn take_after(&self, seen_generation: u64) -> Option<SystemSnapshot> {
        if self.generation.load(Ordering::Acquire) == seen_generation {
            return None;
        }
        // Never park the UI behind a descheduled publisher. It can keep drawing
        // the accepted snapshot; the publisher requests a repaint after unlock.
        let mut slot = self.pending.try_lock().ok()?;
        if slot.as_ref()?.sequence == seen_generation {
            return None;
        }
        slot.take()
    }
}

impl Sampler {
    pub fn spawn(ctx: egui::Context, tray: Option<TraySink>) -> Self {
        let latest = Arc::new(SnapshotMailbox::default());
        let stop = Arc::new(AtomicBool::new(false));
        let service_refresh = Arc::new(AtomicBool::new(false));
        let worker_service_refresh = Arc::clone(&service_refresh);

        let worker_latest = Arc::clone(&latest);
        let worker_stop = Arc::clone(&stop);
        let worker = thread::Builder::new()
            .name("trontop-sampler".into())
            .spawn(move || {
                sample_loop(
                    worker_latest,
                    worker_stop,
                    worker_service_refresh,
                    ctx,
                    tray,
                )
            })
            .expect("failed to start process sampler");

        Self {
            latest,
            stop,
            service_refresh,
            worker: Some(worker),
        }
    }

    pub fn latest_after(&self, seen_generation: u64) -> Option<SystemSnapshot> {
        self.latest.take_after(seen_generation)
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
    latest: Arc<SnapshotMailbox>,
    stop: Arc<AtomicBool>,
    service_refresh: Arc<AtomicBool>,
    ctx: egui::Context,
    tray: Option<TraySink>,
) {
    // These workers start without waiting for either native inventory. Their
    // immutable snapshots can be read even while a provider call is stuck.
    let mut inventories = crate::inventory::Inventories::spawn();
    let mut disk_activity = crate::disk_activity::Monitor::spawn();
    let mut system = System::new();
    system.refresh_cpu_all();
    system.refresh_memory();
    let mut cpu_times = crate::process_cpu::Tracker::default();
    refresh_process_table(&mut system, &mut cpu_times);
    let mut disks = Disks::new_with_refreshed_list();
    let mut networks = Networks::new_with_refreshed_list();
    let mut network_filters = crate::network_identity::FilterAliases::default();
    let users = Users::new_with_refreshed_list();
    let mut gpu_sampler = GpuSampler::new();
    // The optional driver library and every sensor call stay on this worker.
    let mut sensor_sampler = SensorSampler::default();
    // Storage enumeration and every drive query have isolated, bounded workers.
    let storage_monitor = crate::storage_sensors::Monitor::spawn();
    let mut sequence = 0_u64;
    let mut previous_sample = Instant::now();
    let mut diagnostics = Diagnostics::default();
    let mut memory = crate::memory_metrics::Sampler::default();
    let mut cpu_clock = crate::cpu_clock::Sampler::default();
    let mut process_controls = HashMap::<(u32, u64), ProcessControlInfo>::new();

    system.refresh_cpu_frequency();
    // Physical topology is inventory, not a per-second usage counter.
    let physical_cores = System::physical_core_count().unwrap_or_default();

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
        // sysinfo's Windows swap fields are commit-minus-physical estimates, not
        // page-file occupancy. Query the original counters once and keep errors.
        system.refresh_memory_specifics(sysinfo::MemoryRefreshKind::nothing().with_ram());
        memory.refresh();
        *diagnostics.get_mut(Provider::MemoryCounters) = memory.health.clone();
        cpu_clock.refresh();
        *diagnostics.get_mut(Provider::CpuClock) = cpu_clock.health.clone();
        let process_times_ok = refresh_process_table(&mut system, &mut cpu_times);
        disks.refresh(true);
        networks.refresh(true);
        network_filters.refresh(networks.keys());
        if sequence.is_multiple_of(CONTROL_REFRESH_INTERVAL) {
            system.refresh_cpu_frequency();
        }
        let system_ok = !system.cpus().is_empty() && system.total_memory() > 0 && process_times_ok;
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
                let (cpu_percent, accumulated_cpu_millis) =
                    process_cpu(&cpu_times, process.pid().as_u32(), process.start_time());
                ProcessRow {
                    pid: process.pid().as_u32(),
                    parent_pid: process.parent().map(|pid| pid.as_u32()),
                    name: process.name().to_string_lossy().into_owned(),
                    status: format!("{:?}", process.status()),
                    user,
                    cpu_percent,
                    gpu_percent: gpu.for_process(&gpu_by_pid, process.pid().as_u32()),
                    memory_bytes: process.memory(),
                    virtual_memory_bytes: process.virtual_memory(),
                    read_bytes_per_sec: disk.read_bytes as f64 / sample_seconds,
                    write_bytes_per_sec: disk.written_bytes as f64 / sample_seconds,
                    total_read_bytes: disk.total_read_bytes,
                    total_write_bytes: disk.total_written_bytes,
                    accumulated_cpu_millis,
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
            // NDIS filter modules repeat their adapter's traffic under another alias.
            .filter(|(name, _)| !network_filters.is_filter(name))
            .map(|(name, network)| NetworkRow {
                name: name.clone(),
                received_bytes_per_sec: network.received() as f64 / sample_seconds,
                transmitted_bytes_per_sec: network.transmitted() as f64 / sample_seconds,
                total_received_bytes: network.total_received(),
                total_transmitted_bytes: network.total_transmitted(),
            })
            .collect();
        let user_rows = aggregate_users(&processes);
        let mut cpu = cpu_info(&system, physical_cores);
        cpu.clocks = cpu_clock.values.clone();

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
        let physical_disks = disk_activity.snapshot();
        *diagnostics.get_mut(Provider::DiskActivity) = physical_disks.health(Instant::now());
        let snapshot = SystemSnapshot {
            diagnostics: diagnostics.clone(),
            sequence,
            cpu_percent: system.global_cpu_usage().clamp(0.0, 100.0),
            memory_used_bytes: system.used_memory(),
            memory_total_bytes: system.total_memory(),
            memory_available_bytes: system.available_memory(),
            memory_details: memory.values,
            process_count: processes.len(),
            uptime_seconds: System::uptime(),
            sample_seconds,
            host_name: System::host_name().unwrap_or_else(|| "Windows PC".into()),
            os_name: System::long_os_version().unwrap_or_else(|| "Windows".into()),
            cpu,
            processes,
            disks: disk_rows,
            physical_disks,
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
        latest.publish(snapshot);
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
    #[test]
    #[ignore = "Read-only sysinfo CPU probe; no window, stress workload, tray or process control"]
    fn native_logical_cpu_read_only_probe() {
        let mut system = sysinfo::System::new();
        system.refresh_cpu_all();
        let physical = sysinfo::System::physical_core_count().unwrap_or_default();
        let mut durations = Vec::new();
        for sample in 0..4 {
            std::thread::sleep(std::time::Duration::from_secs(1));
            let at = std::time::Instant::now();
            system.refresh_cpu_usage();
            let cpu = super::cpu_info(&system, physical);
            durations.push(at.elapsed().as_secs_f64() * 1e6);
            assert!(cpu.logical_cores > 0);
            assert_eq!(cpu.logical_usage.len(), cpu.logical_cores);
            assert!(
                cpu.logical_usage
                    .iter()
                    .all(|v| v.is_some_and(|v| (0.0..=100.0).contains(&v)))
            );
            println!(
                "CPU_READ_ONLY sample={sample} logical={} physical={} power_clock_mhz={} busy={:?}",
                cpu.logical_cores, cpu.physical_cores, cpu.frequency_mhz, cpu.logical_usage
            );
        }
        durations.sort_by(f64::total_cmp);
        println!(
            "CPU_READ_ONLY refresh_and_snapshot_us={durations:?}; no claim of Task Manager utility equivalence or native frame timing"
        );
    }
    use super::*;

    fn summary(label: &str, mut seconds: Vec<f64>) {
        seconds.sort_by(f64::total_cmp);
        let at = |q: f64| {
            seconds
                .get(((seconds.len() as f64 - 1.0) * q).round() as usize)
                .copied()
                .unwrap_or(f64::NAN)
        };
        println!(
            "{label}: n={} p50={:.3}s p95={:.3}s max={:.3}s",
            seconds.len(),
            at(0.5),
            at(0.95),
            seconds.last().copied().unwrap_or(f64::NAN)
        );
    }

    fn probe_seconds() -> Duration {
        Duration::from_secs(
            std::env::var("TRONTOP_PROBE_SECONDS")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(60),
        )
    }

    #[test]
    #[ignore = "Read-only native timing probe (>= 60 s): real sampler worker, no window, tray, input or process control"]
    fn native_sampler_cadence_read_only_probe() {
        let budget = probe_seconds();
        // Before: the pre-fix per-sample process refresh (sysinfo's default kind,
        // whose CPU path calls GetSystemTimes once per process).
        if std::env::var_os("TRONTOP_PROBE_SKIP_LEGACY").is_none() {
            let mut system = System::new_all();
            let started = Instant::now();
            let mut legacy = Vec::new();
            while started.elapsed() < budget || legacy.len() < 2 {
                thread::sleep(Duration::from_secs(1));
                let at = Instant::now();
                system.refresh_processes(ProcessesToUpdate::All, true);
                legacy.push(at.elapsed().as_secs_f64());
                println!(
                    "SAMPLER_PROBE before refresh[{}] {:.3}s processes={}",
                    legacy.len() - 1,
                    legacy.last().unwrap(),
                    system.processes().len()
                );
            }
            summary("SAMPLER_PROBE before refresh_processes(All)", legacy);
        }
        // After: the real sampler thread, exactly as the app runs it.
        let sampler = Sampler::spawn(egui::Context::default(), None);
        let started = Instant::now();
        let mut seen = 0;
        let mut last = None;
        let mut gaps = Vec::new();
        let mut intervals = Vec::new();
        let mut system_query = Vec::new();
        let mut processes = 0;
        while started.elapsed() < budget {
            if let Some(snapshot) = sampler.latest_after(seen) {
                let now = Instant::now();
                if let Some(previous) = last.replace(now) {
                    gaps.push(now.duration_since(previous).as_secs_f64());
                }
                if let Some(millis) = snapshot.diagnostics.get(Provider::System).query_millis {
                    system_query.push(millis / 1000.0);
                }
                // Measured on the worker between cycle starts; the most direct
                // cadence figure (observer polling jitter is not included).
                if snapshot.sequence > 1 {
                    intervals.push(snapshot.sample_seconds);
                }
                seen = snapshot.sequence;
                processes = snapshot.process_count;
            }
            thread::sleep(Duration::from_millis(10));
        }
        drop(sampler);
        println!(
            "SAMPLER_PROBE after: {seen} samples in {:.1}s, {processes} processes",
            budget.as_secs_f64()
        );
        summary(
            "SAMPLER_PROBE after worker interval between samples",
            intervals,
        );
        summary("SAMPLER_PROBE after observed gap between samples", gaps);
        summary("SAMPLER_PROBE after system+process refresh", system_query);
        assert!(
            seen >= budget.as_secs() / 2,
            "sampler produced only {seen} samples"
        );
    }

    #[test]
    #[ignore = "Read-only native CPU parity probe: spins one thread in this test process only"]
    fn native_process_cpu_matches_sysinfo_read_only_probe() {
        let stop = Arc::new(AtomicBool::new(false));
        let spinner = {
            let stop = Arc::clone(&stop);
            thread::spawn(move || {
                let mut value = 0_u64;
                while !stop.load(Ordering::Relaxed) {
                    value = std::hint::black_box(value.wrapping_add(1));
                }
            })
        };
        let own = std::process::id();
        let pid = sysinfo::Pid::from_u32(own);
        let mut system = System::new();
        system.refresh_cpu_all();
        let kind = ProcessRefreshKind::nothing().with_cpu();
        let mut tracker = crate::process_cpu::Tracker::default();
        let cpus = system.cpus().len().max(1) as f32;
        let mut worst = 0.0_f32;
        for round in 0..8 {
            system.refresh_processes_specifics(ProcessesToUpdate::Some(&[pid]), true, kind);
            tracker.update(crate::process_cpu::read().ok());
            // sysinfo needs three refreshes of a new PID before its first real
            // delta (the first stores no baseline); the native tracker needs two.
            if round > 1 {
                let process = system.process(pid).unwrap();
                let old = (process.cpu_usage() / cpus).clamp(0.0, 100.0);
                let (new, millis) = process_cpu(&tracker, own, process.start_time());
                worst = worst.max((old - new).abs());
                println!(
                    "CPU_PARITY round={round} sysinfo={old:.3}% native={new:.3}% accumulated sysinfo={}ms native={millis}ms",
                    process.accumulated_cpu_time()
                );
                assert!(new > 0.0);
            }
            thread::sleep(Duration::from_secs(1));
        }
        stop.store(true, Ordering::Relaxed);
        spinner.join().unwrap();
        println!("CPU_PARITY worst difference {worst:.3} percentage points ({cpus} logical CPUs)");
        assert!(worst < 1.0, "native CPU differs from sysinfo by {worst}");
    }

    fn snapshot(sequence: u64) -> SystemSnapshot {
        SystemSnapshot {
            sequence,
            processes: vec![ProcessRow {
                name: format!("Fixture process {sequence}"),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[test]
    fn snapshot_mailbox_transfers_allocations_without_cloning() {
        let mailbox = SnapshotMailbox::default();
        assert!(mailbox.take_after(0).is_none());
        let sample = snapshot(1);
        let processes_address = sample.processes.as_ptr();
        let name_address = sample.processes[0].name.as_ptr();
        mailbox.publish(sample);
        let accepted = mailbox.take_after(0).unwrap();
        assert_eq!(accepted.processes.as_ptr(), processes_address);
        assert_eq!(accepted.processes[0].name.as_ptr(), name_address);
        assert_eq!(accepted.sequence, 1);
        assert!(mailbox.take_after(0).is_none());
        assert!(mailbox.take_after(1).is_none());
    }

    #[test]
    fn snapshot_mailbox_coalesces_to_latest_without_replaying_old_data() {
        let mailbox = SnapshotMailbox::default();
        for sequence in 1..=100 {
            mailbox.publish(snapshot(sequence));
        }
        assert_eq!(mailbox.take_after(0).unwrap().sequence, 100);
        assert!(mailbox.take_after(100).is_none());
        mailbox.publish(snapshot(101));
        assert_eq!(mailbox.take_after(100).unwrap().sequence, 101);
    }

    #[test]
    fn snapshot_mailbox_ui_does_not_wait_for_a_stalled_publisher() {
        let mailbox = Arc::new(SnapshotMailbox::default());
        mailbox.publish(snapshot(1));
        let guard = mailbox.pending.lock().unwrap();
        let reader = Arc::clone(&mailbox);
        let (sent, received) = std::sync::mpsc::sync_channel(1);
        let thread = thread::spawn(move || {
            sent.send(reader.take_after(0).is_none()).unwrap();
        });
        let result = received.recv_timeout(Duration::from_secs(2));
        // Release before assertions/join, even on failure, so a regressed blocking
        // implementation fails the test instead of hanging the entire suite.
        drop(guard);
        thread.join().unwrap();
        assert_eq!(result, Ok(true));
        assert_eq!(mailbox.take_after(0).unwrap().sequence, 1);
    }

    #[test]
    fn snapshot_mailbox_concurrent_updates_are_monotonic_and_complete() {
        let mailbox = Arc::new(SnapshotMailbox::default());
        let publisher = Arc::clone(&mailbox);
        let thread = thread::spawn(move || {
            for sequence in 1..=2000 {
                publisher.publish(snapshot(sequence));
            }
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut seen = 0;
        while seen < 2000 && Instant::now() < deadline {
            if let Some(sample) = mailbox.take_after(seen) {
                assert!(sample.sequence > seen);
                assert_eq!(
                    sample.processes[0].name,
                    format!("Fixture process {}", sample.sequence)
                );
                seen = sample.sequence;
            }
            thread::yield_now();
        }
        thread.join().unwrap();
        assert_eq!(seen, 2000);
    }

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
