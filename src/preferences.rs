//! App-owned persistence. The UI never reads, writes, locks files, or joins a worker.
use eframe::egui;
use std::collections::BTreeMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, SyncSender},
};
use std::time::{Duration, Instant};

mod file;
#[cfg(test)]
pub(crate) mod tests;

pub const FILE_NAME: &str = "settings-v3.json";
pub const LEGACY_NAME: &str = "state-v2.ron";
const DEBOUNCE: Duration = Duration::from_millis(250);
const SLOW: Duration = Duration::from_secs(2);

#[derive(Clone)]
pub struct Snapshot {
    pub theme: String,
    pub library: String,
    pub memory: egui::Memory,
}

#[derive(Default)]
pub struct Loaded {
    pub theme: crate::theme::ThemeSettings,
    pub library: Option<String>,
    pub memory: Option<egui::Memory>,
}

pub(crate) trait Backend: Send + 'static {
    fn load(&mut self) -> Result<Loaded, String>;
    fn save(&mut self, snapshot: Snapshot, stop: &AtomicBool) -> Result<(), String>;
}

enum Command {
    Load,
    Save(u64, Box<Snapshot>),
}
enum Event {
    Loaded(Result<Box<Loaded>, String>),
    Saved(u64, Result<(), String>),
}

#[derive(Default)]
pub struct Controller {
    commands: Option<SyncSender<Command>>,
    events: Option<Receiver<Event>>,
    stop: Arc<AtomicBool>,
    enabled: bool,
    ready: bool,
    busy: bool,
    revision: u64,
    desired: Option<Snapshot>,
    changed: Option<Instant>,
    started: Option<Instant>,
    error: Option<String>,
}

impl Controller {
    pub fn native(directory: Option<std::path::PathBuf>, ctx: egui::Context) -> Self {
        let Some(directory) = directory else {
            return Self {
                enabled: true,
                error: Some(
                    "Settings location unavailable. Existing files were not changed.".into(),
                ),
                commands: None,
                events: None,
                stop: Arc::default(),
                ready: false,
                busy: false,
                revision: 0,
                desired: None,
                changed: None,
                started: None,
            };
        };
        Self::with_backend(file::Store::new(directory), ctx)
    }

    pub(crate) fn with_backend(mut backend: impl Backend, ctx: egui::Context) -> Self {
        let (commands, input) = mpsc::sync_channel(1);
        let (output, events) = mpsc::sync_channel(1);
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker = std::thread::Builder::new()
            .name("trontop-preferences".into())
            .spawn(move || {
                let mut command = Command::Load;
                loop {
                    if worker_stop.load(Ordering::Acquire) {
                        break;
                    }
                    let event = match command {
                        Command::Load => Event::Loaded(backend.load().map(Box::new)),
                        Command::Save(revision, snapshot) => {
                            Event::Saved(revision, backend.save(*snapshot, &worker_stop))
                        }
                    };
                    if output.send(event).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                    let Ok(next) = input.recv() else {
                        break;
                    };
                    command = next;
                }
            });
        // No join on the UI thread, including failure/drop. Only this one owned
        // worker may touch the files; failed/blocked operations never respawn it.
        let active = worker.is_ok();
        Self {
            commands: active.then_some(commands),
            events: active.then_some(events),
            stop,
            enabled: true,
            busy: active,
            started: active.then(Instant::now),
            error: (!active)
                .then(|| "Settings worker unavailable. No settings were changed.".into()),
            ready: false,
            revision: 0,
            desired: None,
            changed: None,
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }
    pub fn can_edit(&self) -> bool {
        !self.enabled || self.ready
    }
    pub fn pending(&self) -> bool {
        self.desired.is_some() || (self.ready && self.busy)
    }
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
    pub fn can_retry(&self) -> bool {
        !self.busy && self.commands.is_some()
    }
    pub fn slow(&self) -> bool {
        self.started.is_some_and(|at| at.elapsed() >= SLOW)
    }

    pub fn status(&self) -> &'static str {
        if self.error.is_some() {
            "Settings not saved"
        } else if !self.ready {
            "Loading settings"
        } else if self.pending() {
            "Saving settings"
        } else {
            "Settings saved"
        }
    }

    pub fn update(&mut self, snapshot: Snapshot) {
        if !self.enabled || !self.ready {
            return;
        }
        self.revision = self
            .revision
            .checked_add(1)
            .expect("settings revision overflow");
        self.desired = Some(snapshot);
        self.changed = Some(Instant::now());
    }

    pub fn poll(&mut self) -> Option<Loaded> {
        let event = self.events.as_ref().map(Receiver::try_recv);
        match event {
            Some(Ok(Event::Loaded(result))) => {
                self.busy = false;
                self.started = None;
                match result {
                    Ok(loaded) => {
                        self.ready = true;
                        self.error = None;
                        return Some(*loaded);
                    }
                    Err(error) => self.error = Some(error),
                }
            }
            Some(Ok(Event::Saved(revision, result))) => {
                self.busy = false;
                self.started = None;
                match result {
                    Ok(()) => {
                        if revision == self.revision {
                            self.desired = None;
                        }
                        self.error = None;
                    }
                    Err(error) => self.error = Some(error),
                }
            }
            Some(Err(mpsc::TryRecvError::Disconnected)) => {
                self.busy = false;
                self.events = None;
                self.commands = None;
                self.error = Some(
                    "Settings worker stopped. Pending changes are not confirmed saved.".into(),
                );
            }
            _ => {}
        }
        None
    }

    pub fn dispatch(&mut self, force: bool) {
        if self.busy || self.error.is_some() || !self.ready {
            return;
        }
        if !force && self.changed.is_some_and(|at| at.elapsed() < DEBOUNCE) {
            return;
        }
        let Some(snapshot) = &self.desired else {
            return;
        };
        let result = self.commands.as_ref().map(|sender| {
            sender.try_send(Command::Save(self.revision, Box::new(snapshot.clone())))
        });
        if matches!(result, Some(Ok(()))) {
            self.busy = true;
            self.started = Some(Instant::now());
        } else {
            self.error = Some("Settings worker unavailable. Changes remain unsaved.".into());
        }
    }

    pub fn retry(&mut self) {
        if self.busy || self.commands.is_none() {
            return;
        }
        if self.ready {
            self.error = None;
            self.dispatch(true);
        } else if self
            .commands
            .as_ref()
            .is_some_and(|sender| sender.try_send(Command::Load).is_ok())
        {
            self.error = None;
            self.busy = true;
            self.started = Some(Instant::now());
        }
    }

    pub fn schedule(&self, ctx: &egui::Context) {
        if !self.enabled {
            return;
        }
        if self.busy || (self.desired.is_some() && self.error.is_none()) {
            // Poll/repaint only during real pending work, not a permanent idle loop.
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }
}

impl Drop for Controller {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

fn values_to_loaded(values: &BTreeMap<String, String>) -> Result<Loaded, String> {
    let theme = values
        .get(crate::theme::STORAGE_KEY)
        .or_else(|| values.get(crate::theme::LEGACY_STORAGE_KEY))
        .map(|text| {
            crate::theme::ThemeSettings::decode(text)
                .ok_or("Saved theme is invalid or newer than this build; file preserved.")
        })
        .transpose()?
        .unwrap_or_default();
    let library = values.get(crate::theme_studio::LIBRARY_KEY).cloned();
    if let Some(text) = &library
        && !crate::theme_studio::Studio::default().load_library(text)
    {
        return Err(
            "Saved theme library is invalid or newer than this build; file preserved.".into(),
        );
    }
    let memory: Option<egui::Memory> = values
        .get("egui")
        .map(|text| {
            ron::from_str(text)
                .map_err(|_| "Saved UI memory is invalid; file preserved.".to_owned())
        })
        .transpose()?;
    if memory.as_ref().is_some_and(|memory| {
        !memory.options.zoom_factor.is_finite()
            || !(0.1..=10.0).contains(&memory.options.zoom_factor)
    }) {
        return Err("Saved UI scale is invalid; file preserved.".into());
    }
    Ok(Loaded {
        theme,
        library,
        memory,
    })
}
