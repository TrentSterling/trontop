//! Confirmed, single-flight service commands. No service is controlled by inventory.
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, SyncSender},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
mod observations;
pub use observations::Observations;

#[cfg(windows)]
mod native;
#[cfg(windows)]
pub(crate) use native::status_from_native;
#[cfg(test)]
mod tests;
#[cfg(test)]
pub(crate) use tests::fixture_controller;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum State {
    Stopped,
    Starting,
    Stopping,
    Running,
    Resuming,
    Pausing,
    Paused,
    #[default]
    Unknown,
}

impl State {
    pub fn label(self) -> &'static str {
        match self {
            Self::Stopped => "Stopped",
            Self::Starting => "Starting",
            Self::Stopping => "Stopping",
            Self::Running => "Running",
            Self::Resuming => "Resuming",
            Self::Pausing => "Pausing",
            Self::Paused => "Paused",
            Self::Unknown => "Unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Status {
    pub state: State,
    pub pid: u32,
    pub accepts_stop: bool,
    pub win32_service: bool,
    pub system_process: bool,
    pub exit_code: u32,
    pub service_exit_code: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Start,
    Stop,
    Restart,
}

impl Action {
    pub const ALL: [Self; 3] = [Self::Start, Self::Stop, Self::Restart];
    pub fn label(self) -> &'static str {
        match self {
            Self::Start => "Start",
            Self::Stop => "Stop",
            Self::Restart => "Restart",
        }
    }
    pub fn refusal(self, status: Status) -> Option<&'static str> {
        if !status.win32_service {
            return Some("Only Windows application services can be controlled here.");
        }
        if status.system_process {
            return Some("Services hosted in a Windows system process are protected here.");
        }
        match self {
            Self::Start if status.state == State::Stopped => None,
            Self::Stop | Self::Restart
                if matches!(status.state, State::Running | State::Paused)
                    && status.accepts_stop
                    && status.pid != 0 =>
            {
                None
            }
            Self::Start => Some("Start requires a reported Stopped state."),
            _ => Some("Stop and Restart require a running/paused service that accepts Stop."),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Request {
    pub name: String,
    pub display_name: String,
    pub action: Action,
    pub expected: Status,
    pub staged_at: Instant,
}

impl Request {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.is_empty()
            || self.name.encode_utf16().count() > 256
            || self.name.chars().any(|c| matches!(c, '\0' | '/' | '\\'))
        {
            return Err("Invalid service name; select the service again.".into());
        }
        if self.staged_at.elapsed() > Duration::from_secs(30) {
            return Err("Confirmation expired after 30 seconds; select the action again.".into());
        }
        self.action
            .refusal(self.expected)
            .map_or(Ok(()), |why| Err(why.into()))
    }
}

#[derive(Clone, Debug)]
pub struct Event {
    pub name: String,
    pub action: Action,
    pub phase: &'static str,
    /// A command may have changed state after the last successful observation.
    pub command_at: Option<Instant>,
    pub observed: Option<(Instant, Status)>,
    pub done: bool,
    pub error: Option<String>,
}

trait Service {
    fn query(&mut self) -> Result<Status, String>;
    fn start(&mut self) -> Result<(), String>;
    fn stop(&mut self) -> Result<(), String>;
}
trait Backend {
    fn open(&mut self, request: &Request) -> Result<Box<dyn Service>, String>;
}

fn cancelled(stop: &AtomicBool) -> Result<(), String> {
    if stop.load(Ordering::Acquire) {
        Err("Trontop is closing; remaining work was cancelled. An in-flight command may still complete.".into())
    } else {
        Ok(())
    }
}

// Waiting has a hard observation deadline. It does not cancel an accepted SCM command.
fn wait_for(
    service: &mut dyn Service,
    target: State,
    stop: &AtomicBool,
    timeout: Duration,
    emit: &mut impl FnMut(Status),
) -> Result<Status, String> {
    let started = Instant::now();
    loop {
        cancelled(stop)?;
        let status = service.query()?;
        emit(status);
        if status.state == target {
            return Ok(status);
        }
        if target == State::Running && status.state == State::Stopped {
            return Err(format!(
                "Service returned to Stopped (Windows {}, service {}).",
                status.exit_code, status.service_exit_code
            ));
        }
        if started.elapsed() >= timeout {
            return Err(format!(
                "Timed out observing {}. Last reported state: {}. The accepted command may still complete; check current status before retrying.",
                target.label(),
                status.state.label()
            ));
        }
        // Wake promptly on app shutdown without keeping the render thread waiting.
        for _ in 0..5 {
            cancelled(stop)?;
            thread::sleep(Duration::from_millis(50));
        }
    }
}

fn execute(
    backend: &mut dyn Backend,
    request: &Request,
    stop: &AtomicBool,
    timeout: Duration,
    publish: &mut impl FnMut(Event),
) {
    let mut event = Event {
        name: request.name.clone(),
        action: request.action,
        phase: "Checking",
        command_at: None,
        observed: None,
        done: false,
        error: None,
    };
    publish(event.clone());
    let result = (|| {
        cancelled(stop)?;
        request.validate()?;
        // One handle spans validation, Stop, wait, and Start. Never reopen by name
        // between the two halves of a restart.
        let mut service = backend.open(request)?;
        cancelled(stop)?;
        request.validate()?;
        let status = service.query()?;
        event.observed = Some((Instant::now(), status));
        publish(event.clone());
        if status.state != request.expected.state || status.pid != request.expected.pid {
            return Err("Service state or host PID changed since confirmation. Review the new state and select again.".into());
        }
        if let Some(reason) = request.action.refusal(status) {
            return Err(reason.into());
        }
        if request.action != Action::Start {
            cancelled(stop)?;
            request.validate()?;
            event.phase = "Sending Stop";
            event.command_at = Some(Instant::now());
            publish(event.clone());
            cancelled(stop)?;
            service.stop()?;
            event.phase = "Stopping";
            wait_for(
                service.as_mut(),
                State::Stopped,
                stop,
                timeout,
                &mut |status| {
                    event.observed = Some((Instant::now(), status));
                    publish(event.clone());
                },
            )?;
        }
        if request.action != Action::Stop {
            cancelled(stop)?;
            // Another controller may have started the service after we observed
            // Stopped. Requery on this same handle and do not send duplicate Start.
            let status = service.query()?;
            event.observed = Some((Instant::now(), status));
            if let Some(reason) = Action::Start.refusal(status) {
                return Err(reason.into());
            }
            event.phase = "Sending Start";
            event.command_at = Some(Instant::now());
            publish(event.clone());
            cancelled(stop)?;
            if request.action == Action::Start {
                request.validate()?;
            }
            service.start()?;
            event.phase = "Starting";
            wait_for(
                service.as_mut(),
                State::Running,
                stop,
                timeout,
                &mut |status| {
                    event.observed = Some((Instant::now(), status));
                    publish(event.clone());
                },
            )?;
        }
        Ok(())
    })();
    event.done = true;
    event.phase = if result.is_ok() {
        "Completed"
    } else {
        "Not completed"
    };
    event.error = result.err().map(|error: String| {
        if request.action == Action::Restart {
            format!("{error} Restart is not atomic; the service may remain stopped.")
        } else {
            error
        }
    });
    publish(event);
}

#[derive(Default)]
pub struct Controller {
    requests: Option<SyncSender<Request>>,
    latest: Arc<Mutex<Option<Event>>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    active: Option<Request>,
}

impl Controller {
    pub fn spawn(ctx: eframe::egui::Context) -> Self {
        #[cfg(windows)]
        {
            Self::with_backend(ctx, native::Native)
        }
        #[cfg(not(windows))]
        {
            let _ = ctx;
            Self::default()
        }
    }
    fn with_backend(
        ctx: eframe::egui::Context,
        mut backend: impl Backend + Send + 'static,
    ) -> Self {
        let (tx, rx) = mpsc::sync_channel::<Request>(1);
        let latest = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        let thread_latest = Arc::clone(&latest);
        let thread_stop = Arc::clone(&stop);
        let worker = thread::Builder::new()
            .name("trontop-service-control".into())
            .spawn(move || {
                while let Ok(request) = rx.recv() {
                    if thread_stop.load(Ordering::Acquire) {
                        break;
                    }
                    execute(
                        &mut backend,
                        &request,
                        &thread_stop,
                        Duration::from_secs(30),
                        &mut |event| {
                            if let Ok(mut latest) = thread_latest.lock() {
                                *latest = Some(event);
                            }
                            ctx.request_repaint();
                        },
                    );
                }
            });
        match worker {
            Ok(worker) => Self {
                requests: Some(tx),
                latest,
                stop,
                worker: Some(worker),
                active: None,
            },
            Err(_) => Self::default(),
        }
    }
    pub fn available(&self) -> bool {
        self.requests.is_some() && self.worker.as_ref().is_some_and(|w| !w.is_finished())
    }
    pub fn busy(&self) -> bool {
        self.active.is_some()
    }
    pub fn submit(&mut self, request: Request) -> Result<(), String> {
        request.validate()?;
        if self.busy() {
            return Err("Another service command is still in progress.".into());
        }
        let tx = self
            .requests
            .as_ref()
            .ok_or("Service command worker is unavailable.")?;
        tx.try_send(request.clone())
            .map_err(|_| "Service command worker is busy or disconnected.")?;
        self.active = Some(request);
        Ok(())
    }
    pub fn poll(&mut self) -> Option<Event> {
        // A worker descheduled while publishing must never park the UI thread.
        // Leave the result in place for a later frame if it is currently held.
        let mut event = self.latest.try_lock().ok().and_then(|mut slot| slot.take());
        if event.is_none()
            && self.worker.as_ref().is_some_and(JoinHandle::is_finished)
            && let Some(request) = &self.active
        {
            event = Some(Event {
                name: request.name.clone(),
                action: request.action,
                phase: "Worker disconnected",
                command_at: Some(Instant::now()),
                observed: None,
                done: true,
                error: Some(
                    "Service worker exited. Outcome is unknown; refresh status before retrying."
                        .into(),
                ),
            });
        }
        if event.as_ref().is_some_and(|event| event.done) {
            self.active = None;
        }
        event
    }
}
impl Drop for Controller {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.requests.take();
        if let Some(worker) = self.worker.take() {
            crate::shutdown::finish(worker, Duration::ZERO);
        }
    }
}
