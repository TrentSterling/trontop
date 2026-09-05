//! Fixed read-only inventory workers. Native calls never hold the publication lock.
use crate::diagnostics::{Health, Issue, State};
use crate::{model::ServiceRow, startup, windows_metrics};
use std::sync::{
    Arc, Mutex, TryLockError,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const CADENCE: Duration = Duration::from_secs(30);
const SLOW_AFTER: Duration = Duration::from_secs(5);

struct Published<T> {
    data: Arc<T>,
    health: Health,
    active: Option<Instant>,
}
impl<T> Clone for Published<T> {
    fn clone(&self) -> Self {
        Self {
            data: Arc::clone(&self.data),
            health: self.health.clone(),
            active: self.active,
        }
    }
}

/// At most one running call and one coalesced follow-up request per provider.
struct Worker<T> {
    shared: Arc<Mutex<Published<T>>>,
    cached: Published<T>,
    failed_view: Option<(Instant, Arc<T>, Arc<T>)>,
    invalidate: fn(&Arc<T>, Instant) -> Arc<T>,
    requests: Option<mpsc::SyncSender<()>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    born: Instant,
}

impl<T: Send + Sync + 'static> Worker<T> {
    fn spawn(
        name: &str,
        initial: T,
        invalidate: fn(&Arc<T>, Instant) -> Arc<T>,
        mut collect: impl FnMut(&mut Arc<T>, &mut Health, Instant) + Send + 'static,
    ) -> Self {
        let born = Instant::now();
        let cached = Published {
            data: Arc::new(initial),
            health: Health::default(),
            active: None,
        };
        let shared = Arc::new(Mutex::new(cached.clone()));
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::sync_channel(1);
        let thread_shared = Arc::clone(&shared);
        let thread_stop = Arc::clone(&stop);
        let mut data = Arc::clone(&cached.data);
        let thread = thread::Builder::new()
            .name(name.into())
            .spawn(move || {
                let mut health = Health::default();
                loop {
                    if thread_stop.load(Ordering::Acquire) {
                        break;
                    }
                    let at = Instant::now();
                    if let Ok(mut slot) = thread_shared.lock() {
                        slot.active = Some(at);
                    }
                    collect(&mut data, &mut health, at);
                    if thread_stop.load(Ordering::Acquire) {
                        break;
                    }
                    if let Ok(mut slot) = thread_shared.lock() {
                        *slot = Published {
                            data: Arc::clone(&data),
                            health: health.clone(),
                            active: None,
                        };
                    }
                    // A request received during a slow read starts another read only
                    // after that call returns, with a new observation start timestamp.
                    match rx.recv_timeout(CADENCE) {
                        Ok(()) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
            })
            .ok();
        Self {
            shared,
            cached,
            failed_view: None,
            invalidate,
            requests: thread.as_ref().map(|_| tx),
            stop,
            thread,
            born,
        }
    }

    fn request(&self) {
        if let Some(tx) = &self.requests {
            let _ = tx.try_send(());
        }
    }

    fn latest(&mut self, now: Instant) -> (Arc<T>, Health) {
        // Even publication contention cannot park the live sampler.
        let mut poisoned = false;
        match self.shared.try_lock() {
            Ok(slot) => self.cached = slot.clone(),
            Err(TryLockError::Poisoned(_)) => poisoned = true,
            Err(TryLockError::WouldBlock) => {}
        }
        let mut health = self.cached.health.clone();
        let unavailable = poisoned || self.thread.as_ref().is_none_or(JoinHandle::is_finished);
        let slow = self
            .cached
            .active
            .is_some_and(|at| now.saturating_duration_since(at) >= SLOW_AFTER);
        if unavailable || slow {
            let at = self
                .cached
                .active
                .or(health.last_attempt)
                .unwrap_or(self.born);
            health.record(
                at,
                Duration::ZERO,
                State::Unavailable,
                health.coverage.map(|(_, total)| (0, total)),
                Some(if unavailable {
                    Issue::InventoryWorker
                } else {
                    Issue::InventoryTimeout
                }),
            );
            // The call has not completed: elapsed wait is not a completed query duration.
            health.query_millis = None;
            if self.failed_view.as_ref().is_none_or(|(old, _, source)| {
                *old != at || !Arc::ptr_eq(source, &self.cached.data)
            }) {
                self.failed_view = Some((
                    at,
                    (self.invalidate)(&self.cached.data, at),
                    Arc::clone(&self.cached.data),
                ));
            }
            return (Arc::clone(&self.failed_view.as_ref().unwrap().1), health);
        }
        self.failed_view = None;
        if health.last_attempt.is_none()
            && let Some(at) = self.cached.active
        {
            health.record(at, Duration::ZERO, State::Starting, None, None);
            health.query_millis = None;
        }
        (Arc::clone(&self.cached.data), health)
    }
}

impl<T> Drop for Worker<T> {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.requests.take();
        if let Some(thread) = self.thread.take() {
            crate::shutdown::finish(thread, Duration::ZERO);
        }
    }
}

pub struct Inventories {
    startup: Worker<startup::Snapshot>,
    services: Worker<Vec<ServiceRow>>,
}

impl Inventories {
    pub fn spawn() -> Self {
        Self {
            startup: Worker::spawn(
                "trontop-startup-inventory",
                startup::Snapshot::default(),
                invalidate_startup,
                |cache, health, at| {
                    apply_startup(
                        cache,
                        health,
                        windows_metrics::enumerate_startup().sources,
                        at,
                    )
                },
            ),
            services: Worker::spawn(
                "trontop-service-inventory",
                Vec::new(),
                |cache, _| Arc::clone(cache),
                |cache, health, at| {
                    apply_services(cache, health, windows_metrics::enumerate_services(), at)
                },
            ),
        }
    }

    pub fn startup(&mut self) -> (Arc<startup::Snapshot>, Health) {
        self.startup.latest(Instant::now())
    }
    pub fn services(&mut self) -> (Arc<Vec<ServiceRow>>, Health) {
        self.services.latest(Instant::now())
    }
    pub fn request_services(&self) {
        self.services.request();
    }
}

fn invalidate_startup(cache: &Arc<startup::Snapshot>, at: Instant) -> Arc<startup::Snapshot> {
    let mut stale = (**cache).clone();
    stale.apply(Vec::new(), at); // Missing source results retain entries and mark them cached.
    Arc::new(stale)
}

fn apply_startup(
    cache: &mut Arc<startup::Snapshot>,
    health: &mut Health,
    reads: Vec<startup::Read>,
    at: Instant,
) {
    let cached = Arc::make_mut(cache);
    cached.apply(reads, at);
    let (resolved, total) = cached.coverage();
    let state = if resolved == total {
        State::Live
    } else if resolved > 0
        || cached
            .sources
            .iter()
            .any(|s| s.entries.iter().any(|e| e.observed_in_attempt))
    {
        State::Partial
    } else {
        State::Unavailable
    };
    health.record(
        at,
        at.elapsed(),
        state,
        Some((resolved, total)),
        (resolved < total).then_some(Issue::StartupSources),
    );
}

fn apply_services<T>(
    cache: &mut Arc<Vec<T>>,
    health: &mut Health,
    result: Result<Vec<T>, String>,
    at: Instant,
) {
    match result {
        Ok(rows) => {
            *cache = Arc::new(rows);
            health.record(at, at.elapsed(), State::Live, None, None);
        }
        Err(_) => health.record(
            at,
            at.elapsed(),
            State::Unavailable,
            None,
            Some(Issue::ServiceQuery),
        ),
    }
}

#[cfg(test)]
mod tests;
