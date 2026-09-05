//! Explicit, local-only snapshot exports. No provider calls or automatic captures.
use crate::model::SystemSnapshot;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread::JoinHandle;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

mod encode;
mod file;
#[cfg(windows)]
mod native;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum Format {
    #[default]
    Json,
    Csv,
}
impl Format {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Csv => "csv",
        }
    }
}

#[derive(Clone, Copy, Default)]
pub struct Options {
    pub format: Format,
    pub private_details: bool,
}

pub struct Capture {
    pub snapshot: SystemSnapshot,
    pub options: Options,
    pub at: Instant,
    pub unix_ms: Option<u64>,
}
impl Capture {
    pub fn new(snapshot: SystemSnapshot, options: Options) -> Self {
        Self {
            snapshot,
            options,
            at: Instant::now(),
            unix_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .ok()
                .and_then(|d| u64::try_from(d.as_millis()).ok()),
        }
    }
}

#[derive(Debug)]
pub enum Outcome {
    Saved {
        path: PathBuf,
        bytes: u64,
        sequence: u64,
    },
    Cancelled,
    Failed(String),
}

type Backend = Arc<dyn Fn(Capture, &AtomicBool) -> Outcome + Send + Sync>;

struct Job {
    receive: mpsc::Receiver<Outcome>,
    thread: JoinHandle<()>,
    stop: Arc<AtomicBool>,
}
impl Drop for Job {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        // Dropping a JoinHandle detaches. Do not wait on a save dialog or filesystem.
    }
}

#[derive(Default)]
pub struct Exporter {
    backend: Option<Backend>,
    job: Option<Job>,
}
impl Exporter {
    #[cfg(test)]
    pub(crate) fn with_backend(
        backend: impl Fn(Capture, &AtomicBool) -> Outcome + Send + Sync + 'static,
    ) -> Self {
        Self {
            backend: Some(Arc::new(backend)),
            job: None,
        }
    }
    pub fn native() -> Self {
        #[cfg(windows)]
        {
            Self {
                backend: Some(Arc::new(|capture, stop| {
                    match native::choose_path(capture.options.format) {
                        Ok(Some(path)) if !stop.load(Ordering::Acquire) => {
                            let sequence = capture.snapshot.sequence;
                            match file::save(&path, &capture, stop) {
                                Ok(bytes) => Outcome::Saved {
                                    path,
                                    bytes,
                                    sequence,
                                },
                                Err(error) => Outcome::Failed(error.to_string()),
                            }
                        }
                        Ok(_) => Outcome::Cancelled,
                        Err(error) => Outcome::Failed(error),
                    }
                })),
                job: None,
            }
        }
        #[cfg(not(windows))]
        {
            Self::default()
        }
    }
    pub fn available(&self) -> bool {
        self.backend.is_some()
    }
    pub fn busy(&self) -> bool {
        self.job.is_some()
    }

    pub fn submit(&mut self, capture: Capture, ctx: eframe::egui::Context) -> Result<(), String> {
        if self.busy() {
            return Err("An export is already in progress.".into());
        }
        let backend = self
            .backend
            .clone()
            .ok_or("Export is unavailable in this session.")?;
        let (send, receive) = mpsc::sync_channel(1);
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let thread = std::thread::Builder::new()
            .name("trontop-export".into())
            .spawn(move || {
                let outcome = backend(capture, &worker_stop);
                let _ = send.send(outcome);
                ctx.request_repaint();
            })
            .map_err(|error| format!("Could not start export: {error}"))?;
        self.job = Some(Job {
            receive,
            thread,
            stop,
        });
        Ok(())
    }

    pub fn poll(&mut self) -> Option<Outcome> {
        let job = self.job.as_ref()?;
        let outcome = match job.receive.try_recv() {
            Ok(outcome) => outcome,
            Err(mpsc::TryRecvError::Empty) => {
                // A send can race the first read. Completion alone does not mean
                // there was no result: read the channel again after observing it.
                if !job.thread.is_finished() {
                    return None;
                }
                job.receive.try_recv().unwrap_or_else(|_| {
                    Outcome::Failed("Export worker stopped without a result.".into())
                })
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                Outcome::Failed("Export worker stopped without a result.".into())
            }
        };
        self.job.take();
        Some(outcome)
    }
}

fn seconds(at: Option<Instant>, now: Instant) -> Option<f64> {
    at.map(|at| now.saturating_duration_since(at).as_secs_f64())
}
