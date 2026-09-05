//! Read-only physical-disk PDH telemetry, independent of mounted-volume totals.
use crate::diagnostics::{Health, Issue, State};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[cfg(windows)]
mod native;
#[cfg(test)]
mod tests;

pub const METRICS: usize = 5;
pub const MAX_DEVICES: usize = 128;
pub const STALE_AFTER: Duration = Duration::from_secs(3);

#[derive(Clone, Copy, Debug)]
pub enum Metric {
    Active,
    Response,
    Queue,
    Read,
    Write,
}

impl Metric {
    pub const ALL: [Self; METRICS] = [
        Self::Active,
        Self::Response,
        Self::Queue,
        Self::Read,
        Self::Write,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Active => "Active time",
            Self::Response => "Response time",
            Self::Queue => "Queue depth",
            Self::Read => "Read speed",
            Self::Write => "Write speed",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::Active => "active_percent",
            Self::Response => "response_millis",
            Self::Queue => "outstanding_requests",
            Self::Read => "read_bytes_per_sec",
            Self::Write => "write_bytes_per_sec",
        }
    }
    pub fn format(self, value: Option<f64>) -> String {
        value.map_or_else(
            || "--".into(),
            |v| match self {
                Self::Active => format!("{v:.1}%"),
                Self::Response => format!("{v:.2} ms"),
                Self::Queue => format!("{v:.0}"),
                Self::Read | Self::Write => crate::format::rate(v),
            },
        )
    }
    pub fn explanation(self) -> &'static str {
        match self {
            Self::Active => {
                "100 minus Windows % Idle Time, bounded to 0-100%. Not the queue-weighted % Disk Time counter."
            }
            Self::Response => {
                "Average elapsed time per completed transfer, including queueing; Windows seconds converted to milliseconds. Idle intervals can legitimately report zero."
            }
            Self::Queue => {
                "Requests outstanding at collection time, including requests in service. Not only waiting requests or an interval average."
            }
            Self::Read => {
                "Physical-device bytes read per second; not summed across mounted volumes."
            }
            Self::Write => {
                "Physical-device bytes written per second; not summed across mounted volumes."
            }
        }
    }
    fn convert(self, raw: f64) -> Option<f64> {
        if !raw.is_finite() || raw < 0.0 {
            return None;
        }
        let value = match self {
            Self::Active => (100.0 - raw).clamp(0.0, 100.0),
            Self::Response => raw * 1000.0,
            _ => raw,
        };
        value.is_finite().then_some(value)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Reading {
    pub value: Option<f64>,
    pub at: Option<Instant>,
}

impl Reading {
    pub fn live(self, snapshot: &Snapshot, now: Instant) -> Option<f64> {
        (self.at.is_some() && self.at == snapshot.at && snapshot.fresh(now))
            .then_some(self.value)
            .flatten()
    }
    pub fn state(self, snapshot: &Snapshot, now: Instant) -> &'static str {
        if self.live(snapshot, now).is_some() {
            "Live"
        } else if self.value.is_some() {
            "Cached"
        } else if snapshot.generation < 2
            && snapshot.error.is_none()
            && (snapshot.at.is_none() || snapshot.fresh(now))
        {
            "Warming"
        } else {
            "Unavailable"
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Device {
    /// PDH instance identity, not a persistent hardware serial or volume mapping.
    pub instance: String,
    pub number: u32,
    pub readings: [Reading; METRICS],
}

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub at: Option<Instant>,
    pub generation: u64,
    pub devices: Vec<Device>,
    pub query_millis: f64,
    pub error: Option<&'static str>,
}

impl Snapshot {
    pub fn fresh(&self, now: Instant) -> bool {
        self.at
            .is_some_and(|at| now.saturating_duration_since(at) <= STALE_AFTER)
    }
    pub fn state(&self, now: Instant) -> State {
        let any_cached = self
            .devices
            .iter()
            .any(|d| d.readings.iter().any(|r| r.value.is_some()));
        if self.at.is_some() && !self.fresh(now) {
            return if any_cached {
                State::Stale
            } else {
                State::Unavailable
            };
        }
        let valid = self
            .devices
            .iter()
            .flat_map(|d| d.readings)
            .filter(|r| r.live(self, now).is_some())
            .count();
        if valid > 0 {
            if valid == self.devices.len() * METRICS && self.error.is_none() {
                State::Live
            } else {
                State::Partial
            }
        } else if any_cached {
            State::Stale
        } else if self.error.is_none() && self.generation < 2 {
            State::Starting
        } else {
            State::Unavailable
        }
    }
    pub fn health(&self, now: Instant) -> Health {
        let state = self.state(now);
        let mut health = Health::default();
        health.record(
            self.at.unwrap_or(now),
            Duration::from_secs_f64(self.query_millis / 1000.0),
            state,
            Some((
                self.devices
                    .iter()
                    .flat_map(|d| d.readings)
                    .filter(|r| r.live(self, now).is_some())
                    .count(),
                self.devices.len() * METRICS,
            )),
            (!matches!(state, State::Live | State::Starting)).then_some(Issue::DiskCounters),
        );
        health.last_success = self
            .devices
            .iter()
            .flat_map(|d| d.readings)
            .filter_map(|r| r.at)
            .max();
        health
    }
}

pub(super) type Column = Result<Vec<(String, Option<f64>)>, &'static str>;
pub(super) type Batch = [Column; METRICS];

fn disk_number(instance: &str) -> Option<u32> {
    if instance.len() > 512 || instance.chars().any(char::is_control) {
        return None;
    }
    let number = instance.split(' ').next()?;
    if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    number.parse().ok()
}

impl Snapshot {
    fn apply(&mut self, batch: Batch, now: Instant, elapsed: Duration) {
        self.generation += 1;
        self.at = Some(now);
        self.query_millis = elapsed.as_secs_f64() * 1000.0;
        self.error = batch
            .iter()
            .find_map(|column| column.as_ref().err().copied());
        let names: BTreeSet<_> = batch
            .iter()
            .filter_map(|c| c.as_ref().ok())
            .flat_map(|c| c.iter().map(|(name, _)| name))
            .filter(|name| disk_number(name).is_some())
            .cloned()
            .collect();
        // Only a complete array read can confirm device removal. Partial failure
        // retains identity/last good readings, never promotes them to fresh data.
        if batch.iter().all(Result::is_ok) {
            self.devices.retain(|d| names.contains(&d.instance));
        }
        for name in &names {
            if self.devices.iter().any(|d| &d.instance == name) {
                continue;
            }
            if self.devices.len() >= MAX_DEVICES {
                self.error = Some("Physical disk device limit reached.");
                break;
            }
            self.devices.push(Device {
                instance: name.clone(),
                number: disk_number(name).unwrap(),
                ..Default::default()
            });
        }
        for (metric, column) in Metric::ALL.into_iter().zip(batch) {
            let Ok(values) = column else {
                continue;
            };
            let mut unique = BTreeMap::new();
            for (name, value) in values {
                // Ambiguous duplicate identities are missing, never summed.
                unique
                    .entry(name)
                    .and_modify(|v| *v = None)
                    .or_insert(value);
            }
            for device in &mut self.devices {
                if let Some(value) = unique
                    .get(&device.instance)
                    .copied()
                    .flatten()
                    .and_then(|v| metric.convert(v))
                {
                    device.readings[metric as usize] = Reading {
                        value: Some(value),
                        at: Some(now),
                    };
                }
            }
        }
        self.devices.sort_by(|a, b| {
            a.number
                .cmp(&b.number)
                .then_with(|| a.instance.cmp(&b.instance))
        });
    }
}

/// One fixed worker, a single pending immutable snapshot, no native work under
/// a lock. A stalled PDH call cannot block snapshot reads or spawn replacements.
pub struct Monitor {
    pending: Arc<Mutex<Option<Arc<Snapshot>>>>,
    latest: Arc<Snapshot>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Monitor {
    pub fn spawn() -> Self {
        Self::spawn_with(|| {
            #[cfg(windows)]
            {
                let mut sampler = native::Sampler::default();
                Box::new(move || sampler.sample())
            }
            #[cfg(not(windows))]
            {
                Box::new(|| std::array::from_fn(|_| Err("Physical disk counters require Windows.")))
            }
        })
    }
    fn spawn_with<F>(factory: F) -> Self
    where
        F: FnOnce() -> Box<dyn FnMut() -> Batch> + Send + 'static,
    {
        let pending = Arc::new(Mutex::new(None));
        let worker_pending = pending.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let worker = thread::Builder::new()
            .name("trontop-disk-activity".into())
            .spawn(move || {
                let mut read = factory();
                let mut snapshot = Snapshot::default();
                while !worker_stop.load(Ordering::Acquire) {
                    let start = Instant::now();
                    let batch = read();
                    snapshot.apply(batch, Instant::now(), start.elapsed());
                    let next = Arc::new(snapshot.clone());
                    if let Ok(mut slot) = worker_pending.lock() {
                        *slot = Some(next);
                    }
                    // park/unpark makes normal drop prompt; no busy wake timer.
                    thread::park_timeout(Duration::from_secs(1).saturating_sub(start.elapsed()));
                }
            })
            .ok();
        Self {
            pending,
            latest: Arc::new(Snapshot {
                at: Some(Instant::now()),
                ..Default::default()
            }),
            stop,
            worker,
        }
    }
    pub fn snapshot(&mut self) -> Arc<Snapshot> {
        if let Ok(mut slot) = self.pending.try_lock()
            && let Some(snapshot) = slot.take()
        {
            self.latest = snapshot;
        }
        if self.worker.as_ref().is_none_or(JoinHandle::is_finished)
            && self.latest.error != Some("Physical disk worker unavailable.")
        {
            let mut last = (*self.latest).clone();
            last.error = Some("Physical disk worker unavailable.");
            // Expire every current reading without discarding retained values.
            last.at = Some(
                Instant::now()
                    .checked_sub(STALE_AFTER + Duration::from_secs(1))
                    .unwrap(),
            );
            self.latest = Arc::new(last);
        }
        self.latest.clone()
    }
}

impl Drop for Monitor {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            worker.thread().unpark();
            crate::shutdown::finish(worker, Duration::ZERO);
        }
    }
}
