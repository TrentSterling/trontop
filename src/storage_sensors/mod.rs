//! Read-only drive sensors, isolated from the normal telemetry/render threads.
//! One owned worker per interface, bounded slots, no replacement for stuck I/O.
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[cfg(windows)]
mod native;

const MAX_DRIVES: usize = 32;
const MAX_SENSORS: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Windows(u32),
    Malformed,
    Timeout,
    Stopped,
    WorkerLost,
    NoReadings,
    Capacity,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // ERROR_INVALID_FUNCTION / ERROR_NOT_SUPPORTED: the storage driver
            // does not implement the temperature property for this drive.
            Self::Windows(code @ (1 | 50)) => write!(
                f,
                "Not reported by the drive's storage driver (code {code}); SMART temperature requires administrator"
            ),
            Self::Windows(5) => f.write_str("Requires administrator (code 5: access denied)"),
            Self::Windows(code) => write!(f, "Windows query unavailable (code {code})"),
            Self::Malformed => f.write_str("Invalid storage descriptor"),
            Self::Timeout => f.write_str("Query timed out; cancellation requested"),
            Self::Stopped => f.write_str("Device disconnected or monitoring stopped"),
            Self::WorkerLost => f.write_str("Sensor worker unavailable"),
            Self::NoReadings => f.write_str("No temperature readings reported"),
            Self::Capacity => f.write_str("Storage worker limit reached"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Device {
    // Opaque interface identity; no drive-index/mount-letter history association.
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Temperature {
    pub index: u16,
    pub celsius: Option<i16>,
    pub over_threshold: Option<i16>,
    pub under_threshold: Option<i16>,
    pub event: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Temperatures {
    pub sensors: Vec<Temperature>,
    pub warning: Option<i16>,
    pub critical: Option<i16>,
}

/// Parse bytes, never reinterpret untrusted variable-size driver data as a struct.
fn parse(bytes: &[u8]) -> Result<Temperatures, Error> {
    if bytes.len() < 24 {
        return Err(Error::Malformed);
    }
    let dword = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
    let word = |at| u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap());
    let temperature =
        |at| optional_temperature(i16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()));
    let version = dword(0);
    let size = dword(4);
    let count = word(12) as usize;
    if version < 40
        || version > size
        || size > bytes.len()
        || count > MAX_SENSORS
        || size < 24 + count * 16
    {
        return Err(Error::Malformed);
    }
    let mut sensors = Vec::with_capacity(count);
    for n in 0..count {
        let offset = 24 + n * 16;
        let index = word(offset);
        if sensors
            .iter()
            .any(|sensor: &Temperature| sensor.index == index)
        {
            return Err(Error::Malformed);
        }
        sensors.push(Temperature {
            index,
            celsius: temperature(offset + 2),
            over_threshold: temperature(offset + 4),
            under_threshold: temperature(offset + 6),
            event: bytes[offset + 10] != 0,
        });
    }
    Ok(Temperatures {
        sensors,
        warning: temperature(10),
        critical: temperature(8),
    })
}

fn optional_temperature(value: i16) -> Option<i16> {
    // The SDK sentinel is i16::MIN. This SSD also returns -274 for unset
    // per-sensor thresholds; no Celsius temperature below absolute zero is valid.
    (value >= -273).then_some(value)
}

#[derive(Clone, Debug)]
pub struct DriveReading {
    pub device: Device,
    pub temperatures: Temperatures,
    pub last_attempt: Option<Instant>,
    pub last_success: Option<Instant>,
    pub query_millis: Option<f64>,
    pub error: Option<Error>,
    pub present: bool,
}

impl DriveReading {
    fn new(device: Device) -> Self {
        Self {
            device,
            temperatures: Temperatures::default(),
            last_attempt: None,
            last_success: None,
            query_millis: None,
            error: None,
            present: true,
        }
    }

    pub fn live(&self, now: Instant) -> bool {
        self.present
            && self.error.is_none()
            && self
                .last_success
                .is_some_and(|at| now.saturating_duration_since(at) <= Duration::from_secs(15))
    }

    pub fn status(&self, now: Instant) -> &'static str {
        if !self.present {
            "Disconnected"
        } else if self.live(now) {
            "Live"
        } else if self.last_success.is_some() {
            "Cached"
        } else if self.error.is_some() {
            "Unavailable"
        } else {
            "Starting"
        }
    }

    fn complete(&mut self, result: Result<Temperatures, Error>, at: Instant, elapsed: Duration) {
        self.query_millis = Some(elapsed.as_secs_f64() * 1000.0);
        match result {
            Ok(values) => {
                let usable = values.sensors.iter().any(|sensor| sensor.celsius.is_some());
                if usable {
                    self.temperatures = values;
                    self.last_success = Some(at);
                    self.error = None;
                } else {
                    // Keep sensor indices even before the first usable reading.
                    if self.last_success.is_none() {
                        self.temperatures = values;
                    }
                    self.error = Some(Error::NoReadings);
                }
            }
            Err(error) => self.error = Some(error),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub drives: Vec<DriveReading>,
    pub inventory_at: Option<Instant>,
    pub inventory_error: Option<Error>,
}

impl Snapshot {
    pub fn health(&self, now: Instant) -> crate::diagnostics::Health {
        use crate::diagnostics::{Health, Issue, State};
        let mut health = Health::default();
        let at = self
            .drives
            .iter()
            .filter_map(|d| d.last_attempt)
            .chain(self.inventory_at)
            .max();
        if let Some(at) = at {
            let live = self.drives.iter().filter(|d| d.live(now)).count();
            let last_success = self.drives.iter().filter_map(|d| d.last_success).max();
            let state = if live > 0 {
                if live == self.drives.len() && self.inventory_error.is_none() {
                    State::Live
                } else {
                    State::Partial
                }
            } else if last_success.is_some() {
                State::Stale
            } else {
                State::Unavailable
            };
            health.record(
                at,
                Duration::ZERO,
                state,
                Some((live, self.drives.len())),
                (state != State::Live).then_some(Issue::StorageSensors),
            );
            health.last_success = last_success;
            // Per-drive query costs are shown on their cards, not added as though
            // independent concurrent requests were one synchronous operation.
            health.query_millis = None;
        }
        health
    }
}

trait Backend: Send + Sync + 'static {
    fn enumerate(&self) -> Result<Vec<Device>, Error>;
    fn read(
        &self,
        device: &Device,
        cancel: &AtomicBool,
        stop: &AtomicBool,
    ) -> Result<Temperatures, Error>;
    fn cancel(&self, worker: &JoinHandle<()>);
}

#[derive(Clone, Copy)]
struct Timing {
    poll: Duration,
    sample: Duration,
    timeout: Duration,
    retry: Duration,
    inventory: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            poll: Duration::from_millis(100),
            sample: Duration::from_secs(5),
            timeout: Duration::from_secs(1),
            retry: Duration::from_secs(60),
            inventory: Duration::from_secs(30),
        }
    }
}

struct Completed {
    result: Result<Temperatures, Error>,
    at: Instant,
    elapsed: Duration,
}

struct Slot {
    reading: DriveReading,
    worker: Option<JoinHandle<()>>,
    command: mpsc::SyncSender<()>,
    results: mpsc::Receiver<Completed>,
    cancel: Arc<AtomicBool>,
    pending: Option<Instant>,
    next: Instant,
    timed_out: bool,
    retired: bool,
    worker_stop: Arc<AtomicBool>,
    backend: Arc<dyn Backend>,
}

impl Slot {
    fn spawn(
        device: Device,
        backend: Arc<dyn Backend>,
        stop: Arc<AtomicBool>,
    ) -> Result<Self, Error> {
        let (command, commands) = mpsc::sync_channel(1);
        let (results_sender, results) = mpsc::sync_channel(1);
        let cancel = Arc::new(AtomicBool::new(false));
        let thread_cancel = cancel.clone();
        let worker_stop = Arc::new(AtomicBool::new(false));
        let thread_stop = worker_stop.clone();
        let reader = backend.clone();
        let target = device.clone();
        let worker = thread::Builder::new()
            .name("trontop-drive-sensor".into())
            .spawn(move || {
                while !stop.load(Ordering::Acquire) && !thread_stop.load(Ordering::Acquire) {
                    match commands.recv_timeout(Duration::from_millis(100)) {
                        Ok(()) => {}
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                    if stop.load(Ordering::Acquire) || thread_stop.load(Ordering::Acquire) {
                        break;
                    }
                    let started = Instant::now();
                    let result = reader.read(&target, &thread_cancel, &stop);
                    let at = Instant::now();
                    if results_sender
                        .send(Completed {
                            result,
                            at,
                            elapsed: at - started,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .map_err(|_| Error::WorkerLost)?;
        Ok(Self {
            reading: DriveReading::new(device),
            worker: Some(worker),
            command,
            results,
            cancel,
            pending: None,
            next: Instant::now(),
            timed_out: false,
            retired: false,
            worker_stop,
            backend,
        })
    }

    fn retire(&mut self) {
        if self.retired {
            return;
        }
        self.retired = true;
        self.worker_stop.store(true, Ordering::Release);
        self.cancel.store(true, Ordering::Release);
        if let Some(worker) = &self.worker {
            self.backend.cancel(worker);
        }
    }

    fn update(&mut self, now: Instant, timing: Timing) {
        match self.results.try_recv() {
            Ok(done) => {
                self.pending = None;
                self.reading.complete(done.result, done.at, done.elapsed);
                // A slow but eventually successful query still receives failure backoff.
                self.next = now
                    + if self.timed_out || self.reading.error.is_some() {
                        timing.retry
                    } else {
                        timing.sample
                    };
                self.timed_out = false;
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.pending = None;
                self.reading.error = Some(Error::WorkerLost);
                return;
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
        if self.retired {
            return;
        }
        if self
            .pending
            .is_some_and(|at| now.saturating_duration_since(at) >= timing.timeout)
            && !self.timed_out
        {
            self.timed_out = true;
            self.reading.error = Some(Error::Timeout);
            self.cancel.store(true, Ordering::Release);
            if let Some(worker) = &self.worker {
                self.backend.cancel(worker);
            }
            // Do NOT free its buffers, remove its slot, or spawn replacement I/O.
        }
        if self.pending.is_none() && self.reading.present && now >= self.next {
            if self.worker.as_ref().is_none_or(JoinHandle::is_finished) {
                self.reading.error = Some(Error::WorkerLost);
                return;
            }
            self.cancel.store(false, Ordering::Release);
            if self.command.try_send(()).is_ok() {
                self.pending = Some(now);
                self.reading.last_attempt = Some(now);
            } else {
                self.reading.error = Some(Error::WorkerLost);
            }
        }
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            self.backend.cancel(&worker);
            // Worker owns all in-flight buffers/handles until the native call returns.
            crate::shutdown::finish(worker, Duration::ZERO);
        }
    }
}

pub struct Monitor {
    latest: Arc<RwLock<Arc<Snapshot>>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Monitor {
    #[cfg(windows)]
    pub fn spawn() -> Self {
        Self::with_backend(Arc::new(native::Native), Timing::default())
    }

    #[cfg(not(windows))]
    pub fn spawn() -> Self {
        Self {
            latest: Arc::new(RwLock::new(Arc::new(Snapshot::default()))),
            stop: Arc::new(AtomicBool::new(false)),
            worker: None,
        }
    }

    pub fn latest(&self) -> Arc<Snapshot> {
        self.latest
            .read()
            .map(|value| value.clone())
            .unwrap_or_default()
    }

    fn with_backend(backend: Arc<dyn Backend>, timing: Timing) -> Self {
        let latest = Arc::new(RwLock::new(Arc::new(Snapshot::default())));
        let stop = Arc::new(AtomicBool::new(false));
        let worker_latest = latest.clone();
        let worker_stop = stop.clone();
        let worker = thread::Builder::new()
            .name("trontop-storage".into())
            .spawn(move || {
                let mut slots: Vec<Slot> = Vec::new();
                let mut next_inventory = Instant::now();
                let mut snapshot = Snapshot::default();
                while !worker_stop.load(Ordering::Acquire) {
                    let now = Instant::now();
                    if now >= next_inventory {
                        snapshot.inventory_at = Some(now);
                        match backend.enumerate() {
                            Ok(devices) => {
                                snapshot.inventory_error =
                                    (devices.len() > MAX_DRIVES).then_some(Error::Capacity);
                                for slot in &mut slots {
                                    slot.reading.present =
                                        devices.iter().any(|d| d.id == slot.reading.device.id);
                                    if !slot.reading.present {
                                        slot.retire();
                                    }
                                }
                                // Retiring threads count toward the cap until truly finished.
                                // An uncooperative driver can never cause replacement-thread growth.
                                slots.retain(|s| {
                                    !s.retired
                                        || s.worker.as_ref().is_some_and(|w| !w.is_finished())
                                });
                                for device in devices.into_iter().take(MAX_DRIVES) {
                                    if slots.iter().any(|s| s.reading.device.id == device.id) {
                                        continue;
                                    }
                                    if slots.len() == MAX_DRIVES {
                                        snapshot.inventory_error = Some(Error::Capacity);
                                        break;
                                    }
                                    match Slot::spawn(device, backend.clone(), worker_stop.clone())
                                    {
                                        Ok(slot) => slots.push(slot),
                                        Err(error) => snapshot.inventory_error = Some(error),
                                    }
                                }
                            }
                            Err(error) => snapshot.inventory_error = Some(error),
                        }
                        next_inventory = Instant::now() + timing.inventory;
                    }
                    if worker_stop.load(Ordering::Acquire) {
                        break;
                    }
                    for slot in &mut slots {
                        slot.update(Instant::now(), timing);
                    }
                    snapshot.drives = slots.iter().map(|s| s.reading.clone()).collect();
                    if let Ok(mut shared) = worker_latest.write() {
                        *shared = Arc::new(snapshot.clone());
                    }
                    thread::sleep(timing.poll);
                }
            })
            .ok();
        if worker.is_none() {
            *latest.write().unwrap() = Arc::new(Snapshot {
                inventory_error: Some(Error::WorkerLost),
                inventory_at: Some(Instant::now()),
                ..Default::default()
            });
        }
        Self {
            latest,
            stop,
            worker,
        }
    }
}

impl Drop for Monitor {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            crate::shutdown::finish(worker, Duration::ZERO);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::sync::{Condvar, Mutex};

    fn device(id: &str) -> Device {
        Device {
            id: id.into(),
            name: format!("Fixture {id}"),
        }
    }
    #[test]
    fn windows_errors_read_as_reasons() {
        let unsupported = Error::Windows(1).to_string();
        assert!(unsupported.starts_with("Not reported by the drive's storage driver"));
        assert!(unsupported.contains("requires administrator"));
        assert_eq!(
            Error::Windows(50).to_string().replace("50", "1"),
            unsupported
        );
        assert!(
            Error::Windows(5)
                .to_string()
                .starts_with("Requires administrator")
        );
        assert_eq!(
            Error::Windows(21).to_string(),
            "Windows query unavailable (code 21)"
        );
    }

    fn values() -> Temperatures {
        Temperatures {
            sensors: vec![Temperature {
                index: 0,
                celsius: Some(44),
                over_threshold: Some(90),
                under_threshold: None,
                event: false,
            }],
            warning: Some(90),
            critical: Some(95),
        }
    }

    fn descriptor() -> Vec<u8> {
        let mut bytes = vec![0; 24 + 3 * 16];
        bytes[..4].copy_from_slice(&40u32.to_le_bytes());
        bytes[4..8].copy_from_slice(&72u32.to_le_bytes());
        bytes[8..10].copy_from_slice(&95i16.to_le_bytes());
        bytes[10..12].copy_from_slice(&90i16.to_le_bytes());
        bytes[12..14].copy_from_slice(&3u16.to_le_bytes());
        for (index, value) in [0i16, -12, i16::MIN].into_iter().enumerate() {
            let offset = 24 + index * 16;
            bytes[offset..offset + 2].copy_from_slice(&(index as u16).to_le_bytes());
            bytes[offset + 2..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn descriptor_keeps_indices_zero_negative_and_unavailable_distinct() {
        let data = parse(&descriptor()).unwrap();
        assert_eq!(
            data.sensors.iter().map(|s| s.celsius).collect::<Vec<_>>(),
            [Some(0), Some(-12), None]
        );
        assert_eq!(
            data.sensors.iter().map(|s| s.index).collect::<Vec<_>>(),
            [0, 1, 2]
        );
        assert_eq!((data.warning, data.critical), (Some(90), Some(95)));
        assert_eq!(optional_temperature(-274), None);
        assert_eq!(optional_temperature(-273), Some(-273));
    }

    #[test]
    fn malformed_descriptors_never_read_outside_returned_bytes() {
        let valid = descriptor();
        for length in 0..valid.len() {
            assert_eq!(parse(&valid[..length]), Err(Error::Malformed));
        }
        for (offset, value) in [(0, 8u32), (0, 99), (4, 2000)] {
            let mut bytes = valid.clone();
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            assert_eq!(parse(&bytes), Err(Error::Malformed));
        }
        let mut bytes = valid.clone();
        bytes[12..14].copy_from_slice(&129u16.to_le_bytes());
        assert_eq!(parse(&bytes), Err(Error::Malformed));
        let mut bytes = valid;
        bytes[40..42].copy_from_slice(&0u16.to_le_bytes());
        assert_eq!(parse(&bytes), Err(Error::Malformed));
    }

    #[test]
    fn failed_or_removed_drive_keeps_cached_values_without_refreshing_their_age() {
        let now = Instant::now();
        let mut reading = DriveReading::new(device("stable-interface"));
        reading.complete(Ok(values()), now, Duration::from_millis(3));
        assert!(reading.live(now));
        reading.last_attempt = Some(now + Duration::from_secs(5));
        reading.complete(
            Err(Error::Windows(5)),
            now + Duration::from_secs(5),
            Duration::from_millis(1),
        );
        assert_eq!(reading.last_success, Some(now));
        assert_eq!(reading.temperatures, values());
        assert_eq!(reading.status(now), "Cached");
        reading.present = false;
        assert_eq!(reading.status(now), "Disconnected");
        let snapshot = Snapshot {
            drives: vec![reading],
            inventory_at: Some(now),
            inventory_error: None,
        };
        let health = snapshot.health(now);
        assert_eq!(health.last_success, Some(now));
        assert_eq!(health.coverage, Some((0, 1)));
        assert_eq!(
            health.state(crate::diagnostics::Provider::StorageSensors, now),
            crate::diagnostics::State::Stale
        );
        assert!(health.query_millis.is_none());
    }

    #[derive(Default)]
    struct Fake {
        release: (Mutex<bool>, Condvar),
        fast_reads: AtomicUsize,
        blocked_reads: AtomicUsize,
        active: AtomicUsize,
        // Which worker threads were cancelled, and which one runs the blocked
        // read, so assertions about the stuck drive ignore an unrelated drive.
        cancelled: Mutex<Vec<thread::ThreadId>>,
        blocked_worker: Mutex<Option<thread::ThreadId>>,
        inventory: Mutex<Vec<Device>>,
    }

    impl Fake {
        fn blocked_cancels(&self) -> usize {
            let Some(blocked) = *self.blocked_worker.lock().unwrap() else {
                return 0;
            };
            self.cancelled
                .lock()
                .unwrap()
                .iter()
                .filter(|id| **id == blocked)
                .count()
        }
    }

    impl Backend for Fake {
        fn enumerate(&self) -> Result<Vec<Device>, Error> {
            Ok(self.inventory.lock().unwrap().clone())
        }
        fn read(
            &self,
            device: &Device,
            _: &AtomicBool,
            _: &AtomicBool,
        ) -> Result<Temperatures, Error> {
            self.active.fetch_add(1, Ordering::SeqCst);
            if device.id == "blocked" {
                *self.blocked_worker.lock().unwrap() = Some(thread::current().id());
                self.blocked_reads.fetch_add(1, Ordering::SeqCst);
                let (lock, event) = &self.release;
                let guard = lock.lock().unwrap();
                // Fake an uncooperative driver, even after cancellation. A bounded
                // test safety timeout prevents a failing assertion leaking a thread
                // forever; it is far longer than the test so it can never release
                // the read (and permit a legitimate retry) mid-test.
                let _ = event
                    .wait_timeout_while(guard, Duration::from_secs(120), |released| !*released)
                    .unwrap();
            } else {
                self.fast_reads.fetch_add(1, Ordering::SeqCst);
            }
            self.active.fetch_sub(1, Ordering::SeqCst);
            if device.id == "failing" {
                Err(Error::Windows(5))
            } else {
                Ok(values())
            }
        }
        fn cancel(&self, worker: &JoinHandle<()>) {
            self.cancelled.lock().unwrap().push(worker.thread().id());
        }
    }

    fn wait_until(mut check: impl FnMut() -> bool) {
        let start = Instant::now();
        while !check() {
            assert!(
                start.elapsed() < Duration::from_secs(20),
                "test condition timed out"
            );
            thread::sleep(Duration::from_millis(2));
        }
    }

    #[test]
    fn failed_query_waits_for_retry_deadline_and_lost_worker_is_explicit() {
        let fake = Arc::new(Fake::default());
        let stop = Arc::new(AtomicBool::new(false));
        let mut slot = Slot::spawn(device("failing"), fake.clone(), stop.clone()).unwrap();
        let timing = Timing::default();
        slot.update(Instant::now(), timing);
        wait_until(|| {
            slot.update(Instant::now(), timing);
            slot.reading.error == Some(Error::Windows(5))
        });
        assert_eq!(fake.fast_reads.load(Ordering::SeqCst), 1);
        let deadline = slot.next;
        for _ in 0..20 {
            slot.update(deadline - Duration::from_millis(1), timing);
        }
        assert_eq!(fake.fast_reads.load(Ordering::SeqCst), 1);
        assert!(slot.pending.is_none());
        slot.update(deadline, timing);
        assert!(slot.pending.is_some());
        wait_until(|| fake.fast_reads.load(Ordering::SeqCst) == 2);
        stop.store(true, Ordering::Release);
        wait_until(|| slot.worker.as_ref().unwrap().is_finished());
        // Drain the completed request, then observe the disconnected result channel.
        slot.update(deadline, timing);
        slot.update(deadline, timing);
        assert_eq!(slot.reading.error, Some(Error::WorkerLost));
        assert!(slot.pending.is_none());
    }

    #[test]
    fn stuck_drive_cannot_block_another_drive_grow_workers_or_hold_up_drop() {
        let fake = Arc::new(Fake::default());
        *fake.inventory.lock().unwrap() = vec![device("blocked"), device("fast")];
        let monitor = Monitor::with_backend(
            fake.clone(),
            Timing {
                poll: Duration::from_millis(2),
                sample: Duration::from_millis(10),
                timeout: Duration::from_millis(25),
                retry: Duration::from_millis(100),
                inventory: Duration::from_millis(10),
            },
        );
        // Wait for the stuck drive's own timeout. Under a loaded parallel test
        // run the fast drive's worker can also be starved past the 25 ms
        // deadline; that is a real (and correctly reported) timeout of that
        // drive, so it may add its own cancel, but never a second stuck read.
        wait_until(|| {
            fake.fast_reads.load(Ordering::SeqCst) >= 3
                && fake.blocked_reads.load(Ordering::SeqCst) >= 1
                && monitor
                    .latest()
                    .drives
                    .iter()
                    .any(|d| d.device.id == "blocked" && d.error == Some(Error::Timeout))
        });
        assert_eq!(fake.blocked_reads.load(Ordering::SeqCst), 1);
        assert_eq!(fake.blocked_cancels(), 1);
        // Hot unplug/replug cannot start a replacement while the first query lives.
        *fake.inventory.lock().unwrap() = vec![device("fast")];
        wait_until(|| {
            monitor
                .latest()
                .drives
                .iter()
                .any(|d| d.device.id == "blocked" && !d.present)
        });
        *fake.inventory.lock().unwrap() = vec![device("fast"), device("blocked")];
        wait_until(|| {
            monitor
                .latest()
                .drives
                .iter()
                .any(|d| d.device.id == "blocked" && d.present)
        });
        assert_eq!(fake.blocked_reads.load(Ordering::SeqCst), 1);
        assert_eq!(monitor.latest().drives.len(), 2);
        let started = Instant::now();
        drop(monitor);
        // The read is still blocked (for up to 120 s), so returning at all
        // proves drop does not wait on it; 2 s only absorbs scheduler stalls.
        assert!(started.elapsed() < Duration::from_secs(2));
        *fake.release.0.lock().unwrap() = true;
        fake.release.1.notify_all();
        wait_until(|| fake.active.load(Ordering::SeqCst) == 0);
    }

    #[test]
    fn device_cap_is_reported_without_spawning_unbounded_queries() {
        let fake = Arc::new(Fake::default());
        *fake.inventory.lock().unwrap() = (0..MAX_DRIVES + 8)
            .map(|i| device(&format!("drive-{i}")))
            .collect();
        let monitor = Monitor::with_backend(fake, Timing::default());
        wait_until(|| monitor.latest().inventory_at.is_some());
        assert_eq!(monitor.latest().drives.len(), MAX_DRIVES);
        assert_eq!(monitor.latest().inventory_error, Some(Error::Capacity));
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "Read-only native probe of the previously identified TEAM SSD only; no app windows or disk writes"]
    fn native_storage_ssd_probe() {
        struct OnlySsd;
        impl Backend for OnlySsd {
            fn enumerate(&self) -> Result<Vec<Device>, Error> {
                let devices: Vec<_> = native::Native
                    .enumerate()?
                    .into_iter()
                    .filter(|device| device.name == "TEAM TM8FP6002T")
                    .collect();
                if devices.len() != 1 {
                    return Err(Error::NoReadings);
                }
                Ok(devices)
            }
            fn read(
                &self,
                device: &Device,
                cancel: &AtomicBool,
                stop: &AtomicBool,
            ) -> Result<Temperatures, Error> {
                assert_eq!(device.name, "TEAM TM8FP6002T");
                native::Native.read(device, cancel, stop)
            }
            fn cancel(&self, worker: &JoinHandle<()>) {
                native::Native.cancel(worker);
            }
        }
        let monitor = Monitor::with_backend(Arc::new(OnlySsd), Timing::default());
        wait_until(|| {
            monitor.latest().inventory_error.is_some()
                || monitor
                    .latest()
                    .drives
                    .iter()
                    .any(|d| d.last_success.is_some() || d.error.is_some())
        });
        let snapshot = monitor.latest();
        assert!(
            snapshot.inventory_error.is_none(),
            "inventory: {:?}",
            snapshot.inventory_error
        );
        assert_eq!(snapshot.drives.len(), 1);
        let drive = &snapshot.drives[0];
        assert!(drive.error.is_none(), "query: {:?}", drive.error);
        assert!(drive.live(Instant::now()));
        eprintln!(
            "Read-only native storage: {} / {:?} ms / {:?}",
            drive.device.name, drive.query_millis, drive.temperatures
        );
    }
}
