//! Specs collection workers. One thread per section, so a slow section never
//! blocks another, plus one sensor-bridge thread for live readings. Native
//! calls never hold a publication lock; the UI reads with try_lock and keeps
//! its previous copy on contention. Never on the sampler's one-second path.
use super::live::BridgeReadings;
use super::model::{Completeness, Section, SectionHealth, SectionId, SectionState};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, TryLockError, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime};

/// Automatic refresh cadence after each completed read. Refresh is on demand too.
pub const CADENCE: Duration = Duration::from_secs(300);
/// Soft per-read budget providers see through `Context`.
pub const BUDGET: Duration = Duration::from_secs(20);
/// After this long in flight, a read is reported Slow (reporting, not cancellation).
pub const SLOW_AFTER: Duration = Duration::from_secs(10);
/// Bridge poll interval when the bridge gives no hint.
pub const LIVE_INTERVAL: Duration = Duration::from_secs(2);
/// Bridge poll interval while the System page is not visible.
pub const LIVE_IDLE_INTERVAL: Duration = Duration::from_secs(30);

pub type Provider = fn(&Context) -> Section;
pub type LiveProvider = fn(&Context) -> BridgeReadings;
/// Asks the UI for one repaint when a worker publishes something new, so a
/// finished read reaches the screen at once instead of on the next system
/// sample.
pub type Wake = Arc<dyn Fn() + Send + Sync>;

/// What a provider may know about its own read: a soft deadline and shutdown.
/// Check `should_stop()` between sub-queries and return what you have (with an
/// issue) instead of starting new work.
#[derive(Clone)]
pub struct Context {
    started: Instant,
    budget: Duration,
    stop: Arc<AtomicBool>,
}

impl Context {
    pub fn new(budget: Duration, stop: Arc<AtomicBool>) -> Self {
        Self {
            started: Instant::now(),
            budget,
            stop,
        }
    }

    /// For `#[ignore]` read-only probes: a 60 s budget that never stops.
    #[cfg(test)]
    pub fn probe() -> Self {
        Self::new(Duration::from_secs(60), Arc::new(AtomicBool::new(false)))
    }

    pub fn remaining(&self) -> Duration {
        self.budget.saturating_sub(self.started.elapsed())
    }

    pub fn expired(&self) -> bool {
        self.remaining().is_zero()
    }

    /// The app is shutting down.
    pub fn stopped(&self) -> bool {
        self.stop.load(Ordering::Acquire)
    }

    pub fn should_stop(&self) -> bool {
        self.stopped() || self.expired()
    }

    /// A per-call timeout (for example a WMI query) bounded by the remaining
    /// budget, never zero.
    pub fn timeout(&self, max: Duration) -> Duration {
        self.remaining().min(max).max(Duration::from_millis(1))
    }
}

/// The provider for a collected section; None for the derived Summary.
pub fn provider(id: SectionId) -> Option<Provider> {
    use super::{board, bridge, cpu, devices, graphics, memory, network, os, storage};
    Some(match id {
        SectionId::Summary => return None,
        SectionId::OperatingSystem => os::collect,
        SectionId::Cpu => cpu::collect,
        SectionId::Memory => memory::collect,
        SectionId::Motherboard => board::collect,
        SectionId::Graphics => graphics::collect,
        SectionId::Storage => storage::collect,
        SectionId::OpticalDrives => storage::collect_optical,
        SectionId::Audio => devices::collect_audio,
        SectionId::Peripherals => devices::collect_peripherals,
        SectionId::Network => network::collect,
        SectionId::SensorBridge => bridge::collect,
    })
}

#[derive(Clone, Debug, Default)]
struct Slot {
    section: Option<Arc<Section>>,
    health: SectionHealth,
}

/// One published section as the UI sees it.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: SectionId,
    /// The last completed read; kept while a newer read is slow or failed.
    pub section: Option<Arc<Section>>,
    pub health: SectionHealth,
}

/// Everything the System page renders. Cheap to clone (Arcs).
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub entries: Vec<Entry>,
    pub bridge: Arc<BridgeReadings>,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            entries: SectionId::COLLECTED
                .into_iter()
                .map(|id| Entry {
                    id,
                    section: None,
                    health: SectionHealth::default(),
                })
                .collect(),
            bridge: Arc::new(BridgeReadings::default()),
        }
    }
}

impl Snapshot {
    pub fn get(&self, id: SectionId) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.id == id)
    }
}

struct SectionWorker {
    id: SectionId,
    shared: Arc<Mutex<Slot>>,
    cached: Slot,
    requests: Option<mpsc::SyncSender<()>>,
    thread: Option<JoinHandle<()>>,
}

struct LiveWorker {
    shared: Arc<Mutex<Arc<BridgeReadings>>>,
    cached: Arc<BridgeReadings>,
    requests: Option<mpsc::SyncSender<()>>,
    thread: Option<JoinHandle<()>>,
}

pub struct Monitor {
    sections: Vec<SectionWorker>,
    live: LiveWorker,
    live_active: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
}

impl Monitor {
    /// Starts every section worker (each reads immediately) and the bridge.
    pub fn spawn(wake: Wake) -> Self {
        Self::spawn_with(provider, super::bridge::read_live, wake)
    }

    /// Fixture sections and a fixture bridge; no native reads.
    #[cfg(test)]
    pub(crate) fn fixture(providers: impl Fn(SectionId) -> Option<Provider>, wake: Wake) -> Self {
        fn bridge(_: &Context) -> BridgeReadings {
            BridgeReadings {
                retry_after: Some(Duration::from_secs(60)),
                ..Default::default()
            }
        }
        Self::spawn_with(providers, bridge, wake)
    }

    fn spawn_with(
        providers: impl Fn(SectionId) -> Option<Provider>,
        live: LiveProvider,
        wake: Wake,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let live_active = Arc::new(AtomicBool::new(true));
        let sections = SectionId::COLLECTED
            .into_iter()
            .filter_map(|id| providers(id).map(|p| spawn_section(id, p, &stop, wake.clone())))
            .collect();
        let live = spawn_live(live, &stop, &live_active, wake);
        Self {
            sections,
            live,
            live_active,
            stop,
        }
    }

    /// Requests a new read of every section and the bridge. Coalesces with
    /// reads already queued; never starts a second thread for a stuck read.
    pub fn refresh(&self) {
        for worker in &self.sections {
            if let Some(tx) = &worker.requests {
                let _ = tx.try_send(());
            }
        }
        if let Some(tx) = &self.live.requests {
            let _ = tx.try_send(());
        }
    }

    /// Fast bridge polling while the System page is visible, slow otherwise.
    pub fn set_live_active(&self, active: bool) {
        if !self.live_active.swap(active, Ordering::AcqRel)
            && active
            && let Some(tx) = &self.live.requests
        {
            let _ = tx.try_send(());
        }
    }

    /// Latest published state. Never blocks on a worker.
    pub fn snapshot(&mut self, now: Instant) -> Snapshot {
        let entries = self
            .sections
            .iter_mut()
            .map(|worker| {
                let mut poisoned = false;
                match worker.shared.try_lock() {
                    Ok(slot) => worker.cached = slot.clone(),
                    Err(TryLockError::Poisoned(_)) => poisoned = true,
                    Err(TryLockError::WouldBlock) => {}
                }
                let mut health = worker.cached.health.clone();
                let stopped =
                    poisoned || worker.thread.as_ref().is_none_or(JoinHandle::is_finished);
                if stopped {
                    health.state = SectionState::Stopped;
                    health.collecting_since = None;
                    health.issues.push(
                        "Collection worker stopped; previous data (if any) is retained.".into(),
                    );
                } else if let Some(since) = health.collecting_since {
                    let waited = now.saturating_duration_since(since);
                    if waited >= SLOW_AFTER {
                        health.state = SectionState::Slow;
                        health.issues.push(format!(
                            "Read has run for {:.0} s; previous data (if any) is retained.",
                            waited.as_secs_f64()
                        ));
                    }
                }
                Entry {
                    id: worker.id,
                    section: worker.cached.section.clone(),
                    health,
                }
            })
            .collect();
        match self.live.shared.try_lock() {
            Ok(readings) => self.live.cached = Arc::clone(&readings),
            Err(TryLockError::Poisoned(_) | TryLockError::WouldBlock) => {}
        }
        Snapshot {
            entries,
            bridge: Arc::clone(&self.live.cached),
        }
    }
}

impl Drop for Monitor {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let threads = self
            .sections
            .iter_mut()
            .map(|w| {
                w.requests.take();
                w.thread.take()
            })
            .chain(std::iter::once({
                self.live.requests.take();
                self.live.thread.take()
            }))
            .flatten()
            .collect::<Vec<_>>();
        for thread in threads {
            crate::shutdown::finish(thread, Duration::ZERO);
        }
    }
}

fn spawn_section(
    id: SectionId,
    provider: Provider,
    stop: &Arc<AtomicBool>,
    wake: Wake,
) -> SectionWorker {
    let shared = Arc::new(Mutex::new(Slot::default()));
    let (tx, rx) = mpsc::sync_channel(1);
    let thread_shared = Arc::clone(&shared);
    let thread_stop = Arc::clone(stop);
    let thread = thread::Builder::new()
        .name(format!("trontop-specs-{}", id.key()))
        .spawn(move || {
            loop {
                if thread_stop.load(Ordering::Acquire) {
                    break;
                }
                let at = Instant::now();
                let wall = SystemTime::now();
                if let Ok(mut slot) = thread_shared.lock() {
                    slot.health.collecting_since = Some(at);
                    if slot.section.is_none() {
                        slot.health.state = SectionState::Collecting;
                    }
                }
                let context = Context::new(BUDGET, Arc::clone(&thread_stop));
                let mut section = provider(&context);
                let duration = at.elapsed();
                if thread_stop.load(Ordering::Acquire) {
                    break;
                }
                if section.id != id {
                    section.id = id;
                    section.push_issue("Provider returned a different section id.");
                }
                let health = SectionHealth {
                    state: match section.completeness() {
                        Completeness::Complete => SectionState::Complete,
                        Completeness::Partial => SectionState::Partial,
                        Completeness::Unavailable => SectionState::Unavailable,
                    },
                    collected_at: Some(at),
                    collected_wall: Some(wall),
                    duration: Some(duration),
                    collecting_since: None,
                    issues: Vec::new(),
                };
                let section = Arc::new(section);
                if let Ok(mut slot) = thread_shared.lock() {
                    *slot = Slot {
                        section: Some(section),
                        health,
                    };
                }
                // Reads are rare (first visit, refresh, every five minutes).
                wake();
                match rx.recv_timeout(CADENCE) {
                    Ok(()) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        })
        .ok();
    SectionWorker {
        id,
        shared,
        cached: Slot::default(),
        requests: thread.as_ref().map(|_| tx),
        thread,
    }
}

fn spawn_live(
    live: LiveProvider,
    stop: &Arc<AtomicBool>,
    active: &Arc<AtomicBool>,
    wake: Wake,
) -> LiveWorker {
    let initial = Arc::new(BridgeReadings::default());
    let shared = Arc::new(Mutex::new(Arc::clone(&initial)));
    let (tx, rx) = mpsc::sync_channel(1);
    let thread_shared = Arc::clone(&shared);
    let thread_stop = Arc::clone(stop);
    let thread_active = Arc::clone(active);
    let thread = thread::Builder::new()
        .name("trontop-specs-live".into())
        .spawn(move || {
            let mut first = true;
            loop {
                if thread_stop.load(Ordering::Acquire) {
                    break;
                }
                let at = Instant::now();
                let context = Context::new(Duration::from_secs(5), Arc::clone(&thread_stop));
                let mut readings = live(&context);
                if thread_stop.load(Ordering::Acquire) {
                    break;
                }
                readings.collected_at = Some(at);
                let visible = thread_active.load(Ordering::Acquire);
                let wait = if visible {
                    readings
                        .retry_after
                        .unwrap_or(LIVE_INTERVAL)
                        .clamp(Duration::from_secs(1), Duration::from_secs(60))
                } else {
                    LIVE_IDLE_INTERVAL
                };
                if let Ok(mut slot) = thread_shared.lock() {
                    *slot = Arc::new(readings);
                }
                // The first reading replaces "not checked yet"; later ones
                // only while a page shows them (at most every second).
                if std::mem::take(&mut first) || visible {
                    wake();
                }
                match rx.recv_timeout(wait) {
                    Ok(()) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        })
        .ok();
    LiveWorker {
        shared,
        cached: initial,
        requests: thread.as_ref().map(|_| tx),
        thread,
    }
}

#[cfg(test)]
mod tests {
    use super::super::model::{Group, Value};
    use super::*;
    use std::sync::atomic::AtomicUsize;

    static RELEASE: AtomicBool = AtomicBool::new(false);
    static CPU_READS: AtomicUsize = AtomicUsize::new(0);
    static MEMORY_READS: AtomicUsize = AtomicUsize::new(0);

    fn blocked(_: &Context) -> Section {
        while !RELEASE.load(Ordering::Acquire) {
            thread::sleep(Duration::from_millis(2));
        }
        MEMORY_READS.fetch_add(1, Ordering::AcqRel);
        Section::new(SectionId::Memory).group(Group::new("Released").kv("A", Value::known("1")))
    }

    fn quick(_: &Context) -> Section {
        CPU_READS.fetch_add(1, Ordering::AcqRel);
        Section::new(SectionId::Cpu)
            .group(Group::new("Fixture CPU").kv("Cores", Value::known("24")))
            .issue("fixture partial")
    }

    fn wrong_id(_: &Context) -> Section {
        Section::new(SectionId::Audio).group(Group::new("Wrong"))
    }

    fn live(_: &Context) -> BridgeReadings {
        BridgeReadings {
            status: Value::known("Fixture bridge"),
            retry_after: Some(Duration::from_secs(60)),
            ..Default::default()
        }
    }

    /// Polls until `what` holds. The bound is generous because a loaded
    /// parallel test run can delay thread start-up by seconds; timing out is a
    /// failure with the last snapshot, never a silent return.
    fn wait_for(monitor: &mut Monitor, what: impl Fn(&Snapshot) -> bool) -> Snapshot {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let snapshot = monitor.snapshot(Instant::now());
            if what(&snapshot) {
                return snapshot;
            }
            assert!(
                Instant::now() < deadline,
                "condition not reached; last snapshot: {snapshot:?}"
            );
            thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn slow_section_does_not_block_others_and_is_reported_slow() {
        let wakes = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&wakes);
        let mut monitor = Monitor::spawn_with(
            |id| match id {
                SectionId::Cpu => Some(quick as Provider),
                SectionId::Memory => Some(blocked as Provider),
                SectionId::Peripherals => Some(wrong_id as Provider),
                _ => None,
            },
            live,
            Arc::new(move || {
                counter.fetch_add(1, Ordering::AcqRel);
            }),
        );
        // Also wait until the blocked worker has actually begun its first
        // read: thread start order is not guaranteed, and under load the
        // other sections can finish before the Memory thread first runs.
        let snapshot = wait_for(&mut monitor, |s| {
            s.get(SectionId::Cpu).is_some_and(|e| e.section.is_some())
                && s.get(SectionId::Memory)
                    .is_some_and(|e| e.health.state == SectionState::Collecting)
                && s.get(SectionId::Peripherals)
                    .is_some_and(|e| e.section.is_some())
                && s.bridge.collected_at.is_some()
                // Two published sections and the first bridge reading each
                // ask for a repaint; the blocked Memory read has not.
                && wakes.load(Ordering::Acquire) >= 3
        });
        let cpu = snapshot.get(SectionId::Cpu).unwrap();
        assert_eq!(cpu.health.state, SectionState::Partial);
        assert!(cpu.health.duration.is_some() && cpu.health.collected_wall.is_some());
        let memory = snapshot.get(SectionId::Memory).unwrap();
        assert!(memory.section.is_none());
        assert_eq!(memory.health.state, SectionState::Collecting);
        let wrong = snapshot
            .get(SectionId::Peripherals)
            .unwrap()
            .section
            .clone();
        let wrong = wrong.unwrap();
        assert_eq!(wrong.id, SectionId::Peripherals);
        assert!(wrong.issues[0].contains("different section id"));
        assert_eq!(snapshot.bridge.status, Value::known("Fixture bridge"));
        assert_eq!(snapshot.entries.len(), 3);
        // Reporting a slow read uses the consumer's clock; no second thread starts.
        let later = monitor.snapshot(Instant::now() + SLOW_AFTER + Duration::from_secs(1));
        let memory = later.get(SectionId::Memory).unwrap();
        assert_eq!(memory.health.state, SectionState::Slow);
        assert!(memory.health.issues[0].contains("previous data"));
        // Refresh requests coalesce into one queued read per worker. The
        // blocked worker makes this deterministic: twenty requests while its
        // read is in flight queue exactly one more read.
        let before = CPU_READS.load(Ordering::Acquire);
        for _ in 0..20 {
            monitor.refresh();
        }
        let _ = wait_for(&mut monitor, |_| CPU_READS.load(Ordering::Acquire) > before);
        assert_eq!(MEMORY_READS.load(Ordering::Acquire), 0);
        RELEASE.store(true, Ordering::Release);
        let released = wait_for(&mut monitor, |s| {
            s.get(SectionId::Memory)
                .is_some_and(|e| e.section.is_some())
                && MEMORY_READS.load(Ordering::Acquire) >= 2
        });
        thread::sleep(Duration::from_millis(50));
        assert_eq!(MEMORY_READS.load(Ordering::Acquire), 2);
        assert_eq!(
            released.get(SectionId::Memory).unwrap().health.state,
            SectionState::Complete
        );
        let started = Instant::now();
        drop(monitor);
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn every_collected_section_has_a_provider_and_context_deadlines_hold() {
        for id in SectionId::COLLECTED {
            assert!(provider(id).is_some(), "{id:?}");
        }
        assert!(provider(SectionId::Summary).is_none());
        let context = Context::probe();
        assert!(!context.should_stop());
        assert!(context.timeout(Duration::from_secs(5)) <= Duration::from_secs(5));
        let expired = Context::new(Duration::ZERO, Arc::new(AtomicBool::new(false)));
        assert!(expired.should_stop());
        assert_eq!(
            expired.timeout(Duration::from_secs(5)),
            Duration::from_millis(1)
        );
    }
}
