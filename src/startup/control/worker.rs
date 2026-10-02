use super::{Failure, Receipt, Request};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, SyncSender},
};
use std::thread::{self, JoinHandle};

#[derive(Clone, Debug)]
pub struct Outcome {
    pub request: Request,
    pub result: Result<Receipt, Failure>,
}

#[derive(Default)]
pub struct Controller {
    requests: Option<SyncSender<Request>>,
    latest: Arc<Mutex<Option<Outcome>>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    active: Option<Request>,
}
impl Controller {
    pub fn spawn(ctx: eframe::egui::Context) -> Self {
        #[cfg(windows)]
        {
            Self::with_backend(ctx, super::native::apply)
        }
        #[cfg(not(windows))]
        {
            let _ = ctx;
            Self::default()
        }
    }
    pub(crate) fn with_backend(
        ctx: eframe::egui::Context,
        mut apply: impl FnMut(&Request) -> Result<Receipt, Failure> + Send + 'static,
    ) -> Self {
        let (tx, rx) = mpsc::sync_channel::<Request>(1);
        let latest = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        let thread_latest = Arc::clone(&latest);
        let thread_stop = Arc::clone(&stop);
        let worker = thread::Builder::new().name("trontop-startup-control".into()).spawn(move || {
            while let Ok(request) = rx.recv() {
                if thread_stop.load(Ordering::Acquire) { break; }
                let execution = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| request.validate().map_err(Failure::unchanged).and_then(|()| apply(&request))));
                let crashed = execution.is_err();
                let result = execution.unwrap_or_else(|_| Err(Failure::uncertain("Startup worker failed. Outcome is unknown; refresh before trying again.")));
                if let Ok(mut slot) = thread_latest.lock() { *slot = Some(Outcome { request, result }); }
                ctx.request_repaint();
                if crashed { break; }
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
    pub fn ready(&self) -> bool {
        self.active.is_none()
            && self.requests.is_some()
            && self
                .worker
                .as_ref()
                .is_some_and(|worker| !worker.is_finished())
    }
    pub fn busy(&self) -> bool {
        self.active.is_some()
    }
    pub fn submit(&mut self, request: Request) -> Result<(), String> {
        request.validate()?;
        if !self.ready() {
            return Err("Startup control worker is busy or unavailable.".into());
        }
        self.requests
            .as_ref()
            .ok_or("Startup control worker is unavailable.")?
            .try_send(request.clone())
            .map_err(|_| "Startup control worker disconnected; no retry was sent.")?;
        self.active = Some(request);
        Ok(())
    }
    pub fn poll(&mut self) -> Option<Outcome> {
        let mut result = match self.latest.try_lock() {
            Ok(mut slot) => slot.take(),
            Err(std::sync::TryLockError::WouldBlock) => return None,
            Err(std::sync::TryLockError::Poisoned(_)) => None,
        };
        if result.is_none()
            && self.worker.as_ref().is_some_and(JoinHandle::is_finished)
            && let Some(request) = self.active.as_ref()
        {
            result = Some(Outcome {
                request: request.clone(),
                result: Err(Failure::uncertain(
                    "Startup worker exited. Outcome is unknown; refresh before trying again.",
                )),
            });
        }
        if result.is_some() {
            self.active = None;
        }
        result
    }
}
impl Drop for Controller {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.requests.take();
        if let Some(worker) = self.worker.take() {
            crate::shutdown::finish(worker, std::time::Duration::ZERO);
        }
    }
}

#[cfg(test)]
mod tests;
