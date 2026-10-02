//! Explicitly requested process/shell actions, never executed by the render thread.
//! A single in-flight request is retained until a terminal result; timeouts do not
//! retry or cancel a native call. Default construction cannot execute OS actions.
use crate::model::{PriorityClass, ProcessIdentity};
use crate::platform;
use eframe::egui::Context;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, SyncSender, TryRecvError},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub const SLOW_AFTER: Duration = Duration::from_secs(5);
const START_DEADLINE: Duration = Duration::from_secs(30);
pub mod tree;

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    End(ProcessIdentity),
    EndTree(tree::Plan),
    Suspend(ProcessIdentity),
    Resume(ProcessIdentity),
    Priority(ProcessIdentity, PriorityClass),
    Affinity(ProcessIdentity, usize),
    Reveal(PathBuf),
    Launch(String),
}

impl Action {
    fn validate(&self) -> Result<(), String> {
        match self {
            Self::End(identity) => platform::can_terminate(identity.pid),
            Self::EndTree(plan) => plan.validate(),
            Self::Suspend(identity) | Self::Resume(identity) => platform::can_control(identity.pid),
            Self::Priority(identity, priority) => {
                platform::can_control(identity.pid)?;
                if !PriorityClass::EDITABLE.contains(priority) {
                    return Err("That priority class cannot be applied.".into());
                }
                Ok(())
            }
            Self::Affinity(identity, mask) => {
                platform::can_control(identity.pid)?;
                if *mask == 0 {
                    return Err("At least one logical processor must remain selected.".into());
                }
                Ok(())
            }
            Self::Launch(command) if command.trim().is_empty() || command.contains('\0') => {
                Err("Enter a non-empty command without null characters.".into())
            }
            Self::Reveal(path) if path.as_os_str().is_empty() => {
                Err("The executable path is unavailable.".into())
            }
            _ => Ok(()),
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::End(_) => "End task",
            Self::EndTree(_) => "End process tree",
            Self::Suspend(_) => "Suspend process",
            Self::Resume(_) => "Resume process",
            Self::Priority(..) => "Set priority",
            Self::Affinity(..) => "Set affinity",
            Self::Reveal(_) => "Reveal in Explorer",
            Self::Launch(_) => "Run task",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Request {
    pub action: Action,
    /// Frozen display context, never resolved from the current UI selection.
    pub target: String,
    pub confirmed_at: Instant,
}

impl Request {
    pub fn new(action: Action, target: String) -> Self {
        Self {
            action,
            target,
            confirmed_at: Instant::now(),
        }
    }

    fn validate(&self) -> Result<(), String> {
        if self.confirmed_at.elapsed() >= START_DEADLINE {
            return Err(
                "The action was not started within 30 seconds. Review and confirm again.".into(),
            );
        }
        self.action.validate()
    }

    pub fn description(&self) -> String {
        format!("{}: {}", self.action.label(), self.target)
    }
}

#[derive(Debug)]
pub struct Outcome {
    pub request: Request,
    pub result: Result<(), String>,
    /// Worker-owned references, not inferred CPU activity or another tool's state.
    held_suspensions: Vec<ProcessIdentity>,
}

impl Outcome {
    pub fn message(self) -> (String, bool) {
        match self.result {
            // Native API success acknowledges the request, not process exit or
            // successful initialization of the program launched by the shell.
            Ok(()) => (
                format!("Request accepted. {}", self.request.description()),
                false,
            ),
            Err(error) => (format!("{}: {error}", self.request.description()), true),
        }
    }
}

fn native(action: &Action, suspensions: &mut platform::Suspensions) -> Result<(), String> {
    match action {
        Action::End(identity) => platform::terminate_process(*identity),
        Action::EndTree(plan) => platform::terminate_tree(plan),
        Action::Suspend(identity) => suspensions.suspend(*identity),
        Action::Resume(identity) => suspensions.resume(*identity),
        Action::Priority(identity, priority) => {
            platform::set_process_priority(*identity, *priority)
        }
        Action::Affinity(identity, mask) => platform::set_process_affinity(*identity, *mask),
        Action::Reveal(path) => platform::reveal_in_explorer(path),
        Action::Launch(command) => platform::launch_command(command),
    }
}

#[derive(Default)]
pub struct Controller {
    requests: Option<SyncSender<Request>>,
    results: Option<Receiver<Outcome>>,
    worker: Option<JoinHandle<()>>,
    stop: Arc<AtomicBool>,
    active: Option<Request>,
    held_suspensions: Vec<ProcessIdentity>,
}

impl Controller {
    pub fn spawn(ctx: Context) -> Self {
        // Native handles are created inside their owning worker. No handle
        // crosses threads and no unsafe Send implementation is needed.
        Self::with_factory(ctx, || {
            let mut suspensions = platform::Suspensions::default();
            move |action: &Action| {
                suspensions.prune_exited();
                let result = native(action, &mut suspensions);
                suspensions.prune_exited();
                (result, suspensions.held())
            }
        })
    }

    #[cfg(test)]
    pub(crate) fn with_backend(
        ctx: Context,
        mut backend: impl FnMut(&Action) -> Result<(), String> + Send + 'static,
    ) -> Self {
        Self::with_factory(ctx, move || {
            let mut held = Vec::new();
            move |action: &Action| {
                let result = backend(action);
                if result.is_ok() {
                    match action {
                        Action::Suspend(identity) if !held.contains(identity) => {
                            held.push(*identity)
                        }
                        Action::Resume(identity) | Action::End(identity) => {
                            held.retain(|held| held != identity)
                        }
                        Action::EndTree(plan) => held.retain(|held| {
                            !plan.targets().iter().any(|target| target.identity == *held)
                        }),
                        _ => {}
                    }
                }
                (result, held.clone())
            }
        })
    }

    fn with_factory<F, B>(ctx: Context, factory: F) -> Self
    where
        F: FnOnce() -> B + Send + 'static,
        B: FnMut(&Action) -> (Result<(), String>, Vec<ProcessIdentity>) + 'static,
    {
        let (requests, incoming) = mpsc::sync_channel::<Request>(1);
        let (completed, results) = mpsc::sync_channel(1);
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker = thread::Builder::new()
            .name("trontop-process-actions".into())
            .spawn(move || {
                let mut backend = factory();
                let mut held_suspensions = Vec::new();
                while let Ok(request) = incoming.recv() {
                    if worker_stop.load(Ordering::Acquire) {
                        break;
                    }
                    let result = request.validate().and_then(|()| {
                        if worker_stop.load(Ordering::Acquire) {
                            Err("Trontop is closing. The action was not started.".into())
                        } else {
                            // Existing native functions validate creation time and
                            // critical-process policy on the SAME action handle.
                            let (result, held) = backend(&request.action);
                            held_suspensions = held;
                            result
                        }
                    });
                    if completed
                        .try_send(Outcome {
                            request,
                            result,
                            held_suspensions: held_suspensions.clone(),
                        })
                        .is_err()
                    {
                        break;
                    }
                    ctx.request_repaint();
                }
            });
        match worker {
            Ok(worker) => Self {
                requests: Some(requests),
                results: Some(results),
                worker: Some(worker),
                stop,
                active: None,
                held_suspensions: Vec::new(),
            },
            Err(_) => Self::default(),
        }
    }

    pub fn busy(&self) -> bool {
        self.active.is_some()
    }

    pub fn ready(&self) -> bool {
        !self.busy()
            && self.requests.is_some()
            && self
                .worker
                .as_ref()
                .is_some_and(|worker| !worker.is_finished())
    }

    pub fn active(&self) -> Option<&Request> {
        self.active.as_ref()
    }

    pub fn holds_suspension(&self, identity: ProcessIdentity) -> bool {
        self.held_suspensions.contains(&identity)
    }

    pub fn submit(&mut self, request: Request) -> Result<(), String> {
        request.validate()?;
        if let Action::Suspend(identity) = request.action
            && self.holds_suspension(identity)
        {
            return Err(
                "Trontop already holds a suspension for this process. Resume it first.".into(),
            );
        }
        if self.busy() {
            return Err(
                "Another process or launch action is pending. No additional request was sent."
                    .into(),
            );
        }
        let sender = self
            .requests
            .as_ref()
            .ok_or("The process action worker is unavailable.")?;
        sender
            .try_send(request.clone())
            .map_err(|_| "The process action worker is unavailable. No request was sent.")?;
        self.active = Some(request);
        Ok(())
    }

    pub fn poll(&mut self) -> Option<Outcome> {
        let result = self.results.as_ref()?.try_recv();
        match result {
            Ok(outcome) => {
                self.active = None;
                self.held_suspensions = outcome.held_suspensions.clone();
                Some(outcome)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.requests = None;
                self.held_suspensions.clear();
                self.active.take().map(|request| Outcome {
                    request,
                    result: Err("Action worker disconnected. Outcome is unknown; verify the target before retrying. No automatic retry was sent.".into()),
                    held_suspensions: Vec::new(),
                })
            }
        }
    }
}

impl Drop for Controller {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.requests.take();
        // Never join or kill a blocked native call. Resources remain owned by the
        // worker; a call already in flight can finish until the app process exits.
        self.worker.take();
    }
}

#[cfg(test)]
mod tests;
