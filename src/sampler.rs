use crate::model::{ProcessRow, SystemSnapshot};
use eframe::egui;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use sysinfo::{ProcessesToUpdate, System};

const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);

pub struct Sampler {
    latest: Arc<RwLock<SystemSnapshot>>,
    generation: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Sampler {
    pub fn spawn(ctx: egui::Context) -> Self {
        let latest = Arc::new(RwLock::new(SystemSnapshot::default()));
        let generation = Arc::new(AtomicU64::new(0));
        let stop = Arc::new(AtomicBool::new(false));

        let worker_latest = Arc::clone(&latest);
        let worker_generation = Arc::clone(&generation);
        let worker_stop = Arc::clone(&stop);
        let worker = thread::Builder::new()
            .name("trontop-sampler".into())
            .spawn(move || sample_loop(worker_latest, worker_generation, worker_stop, ctx))
            .expect("failed to start process sampler");

        Self {
            latest,
            generation,
            stop,
            worker: Some(worker),
        }
    }

    pub fn latest_after(&self, seen_generation: u64) -> Option<SystemSnapshot> {
        if self.generation.load(Ordering::Acquire) == seen_generation {
            return None;
        }
        self.latest.read().ok().map(|snapshot| snapshot.clone())
    }
}

impl Drop for Sampler {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn sample_loop(
    latest: Arc<RwLock<SystemSnapshot>>,
    generation: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
    ctx: egui::Context,
) {
    let mut system = System::new_all();
    let logical_cpu_count = system.cpus().len().max(1) as f32;
    let mut sequence = 0_u64;
    let mut previous_sample = Instant::now();

    // CPU deltas need two observations. This short warmup avoids a misleading first frame.
    if wait_for_stop(&stop, sysinfo::MINIMUM_CPU_UPDATE_INTERVAL) {
        return;
    }

    loop {
        let cycle_started = Instant::now();
        let sample_seconds = previous_sample.elapsed().as_secs_f64().max(0.001);
        previous_sample = Instant::now();

        system.refresh_cpu_usage();
        system.refresh_memory();
        system.refresh_processes(ProcessesToUpdate::All, true);

        sequence += 1;
        let processes = system
            .processes()
            .values()
            .map(|process| {
                let disk = process.disk_usage();
                ProcessRow {
                    pid: process.pid().as_u32(),
                    parent_pid: process.parent().map(|pid| pid.as_u32()),
                    name: process.name().to_string_lossy().into_owned(),
                    status: format!("{:?}", process.status()),
                    // sysinfo reports percent of one logical CPU. Task Manager reports a
                    // process as a share of total machine capacity.
                    cpu_percent: (process.cpu_usage() / logical_cpu_count).clamp(0.0, 100.0),
                    memory_bytes: process.memory(),
                    virtual_memory_bytes: process.virtual_memory(),
                    read_bytes_per_sec: disk.read_bytes as f64 / sample_seconds,
                    write_bytes_per_sec: disk.written_bytes as f64 / sample_seconds,
                    started_at_unix: process.start_time(),
                    executable: process.exe().map(ToOwned::to_owned),
                }
            })
            .collect::<Vec<_>>();

        let snapshot = SystemSnapshot {
            sequence,
            cpu_percent: system.global_cpu_usage().clamp(0.0, 100.0),
            memory_used_bytes: system.used_memory(),
            memory_total_bytes: system.total_memory(),
            process_count: processes.len(),
            uptime_seconds: System::uptime(),
            sample_seconds,
            processes,
        };

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
