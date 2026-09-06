//! The UI owns only an asynchronous controller, never the native tray handles.
use super::super::{TrayAction, TraySample, TrayState};
use eframe::egui;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use windows::Win32::Foundation::{HANDLE, WAIT_FAILED};
use windows::Win32::System::Threading::{CreateEventW, INFINITE, SetEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, MSG, MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx, PM_NOREMOVE,
    PM_REMOVE, PeekMessageW, QS_ALLINPUT, TranslateMessage, WM_QUIT,
};

pub(super) trait Backend {
    fn update(&mut self, sample: TraySample) -> Result<(), ()>;
}

struct Shared {
    // A latest-only mailbox and auto-reset event coalesce any startup backlog.
    latest: Mutex<Option<TraySample>>,
    wake: OwnedHandle,
    stop: AtomicBool,
    state: AtomicU8,
    pending: Arc<AtomicU8>,
    #[cfg(test)]
    applied: std::sync::atomic::AtomicU64,
}

impl Shared {
    fn signal(&self) {
        // Shared is kept alive by the worker through every wait. OwnedHandle
        // closes only after the controller, worker and all sampler sinks release it.
        let _ = unsafe { SetEvent(self.handle()) };
    }

    fn handle(&self) -> HANDLE {
        HANDLE(self.wake.as_raw_handle())
    }

    fn set_state(&self, state: TrayState, ctx: &egui::Context) {
        if self.state.swap(state as u8, Ordering::AcqRel) != state as u8 {
            ctx.request_repaint(); // Transitions only, not every tray sample/hover.
        }
    }
}

#[derive(Clone)]
pub struct TraySink(Arc<Shared>);

impl TraySink {
    pub fn publish(&self, sample: TraySample) {
        if self.0.stop.load(Ordering::Acquire) {
            return;
        }
        if let Ok(mut latest) = self.0.latest.lock() {
            *latest = Some(sample);
        }
        self.0.signal();
    }
}

pub struct TrayController {
    sink: TraySink,
    worker: Option<JoinHandle<()>>,
}

impl TrayController {
    pub fn new(ctx: egui::Context) -> Option<Self> {
        Self::spawn(ctx, |ctx, pending| {
            super::create_native_tray(ctx, pending).map(|tray| super::NativeTray {
                tray,
                meter: super::CpuMeter::default(),
            })
        })
    }

    fn spawn<B: Backend + 'static>(
        ctx: egui::Context,
        create: impl FnOnce(egui::Context, Arc<AtomicU8>) -> Option<B> + Send + 'static,
    ) -> Option<Self> {
        // Unnamed, non-inheritable, auto-reset: no global window/thread targeting.
        let handle = unsafe { CreateEventW(None, false, false, None) }.ok()?;
        let shared = Arc::new(Shared {
            latest: Mutex::new(None),
            wake: unsafe { OwnedHandle::from_raw_handle(handle.0) },
            stop: AtomicBool::new(false),
            state: AtomicU8::new(TrayState::Starting as u8),
            pending: Arc::new(AtomicU8::new(0)),
            #[cfg(test)]
            applied: std::sync::atomic::AtomicU64::new(0),
        });
        let worker_shared = Arc::clone(&shared);
        let worker = thread::Builder::new()
            .name("trontop-tray".into())
            .spawn(move || {
                // Declared first, dropped last, including failed creation/unwind.
                let guard = ExitState {
                    shared: worker_shared,
                    ctx,
                };
                if guard.shared.stop.load(Ordering::Acquire) {
                    return;
                }
                let mut message = MSG::default();
                unsafe {
                    let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
                }
                // B need not be Send: all handles are born, used and dropped here.
                let Some(mut backend) =
                    create(guard.ctx.clone(), Arc::clone(&guard.shared.pending))
                else {
                    return;
                };
                // If close raced a slow constructor, destroy its late result now.
                if guard.shared.stop.load(Ordering::Acquire) {
                    return;
                }
                guard.shared.set_state(TrayState::Ready, &guard.ctx);
                run(&mut backend, &guard, &mut message);
            })
            .ok()?;
        // No ready recv and no failed-constructor join on the app thread.
        Some(Self {
            sink: TraySink(shared),
            worker: Some(worker),
        })
    }

    pub fn sink(&self) -> TraySink {
        self.sink.clone()
    }

    pub fn state(&self) -> TrayState {
        match self.sink.0.state.load(Ordering::Acquire) {
            0 => TrayState::Starting,
            1 => TrayState::Ready,
            2 => TrayState::UpdateFailed,
            4 => TrayState::Stopped,
            _ => TrayState::Unavailable,
        }
    }

    pub fn poll(&self) -> Option<TrayAction> {
        match self.sink.0.pending.swap(0, Ordering::AcqRel) {
            1 => Some(TrayAction::Show),
            2 => Some(TrayAction::Quit),
            _ => None,
        }
    }

    #[cfg(test)]
    pub(super) fn applied(&self) -> u64 {
        self.sink.0.applied.load(Ordering::Acquire)
    }
}

impl Drop for TrayController {
    fn drop(&mut self) {
        self.sink.0.stop.store(true, Ordering::Release);
        self.sink.0.signal();
        if let Some(worker) = self.worker.take() {
            crate::shutdown::finish(worker, std::time::Duration::from_millis(100));
        }
    }
}

struct ExitState {
    shared: Arc<Shared>,
    ctx: egui::Context,
}

impl Drop for ExitState {
    fn drop(&mut self) {
        let stopped = self.shared.stop.swap(true, Ordering::AcqRel);
        self.shared.set_state(
            if stopped {
                TrayState::Stopped
            } else {
                TrayState::Unavailable
            },
            &self.ctx,
        );
    }
}

fn run(backend: &mut impl Backend, guard: &ExitState, message: &mut MSG) {
    let shared = &guard.shared;
    loop {
        if shared.stop.load(Ordering::Acquire) {
            return;
        }
        // Native menu/click messages and our own sample/stop event share one sleep.
        // INPUTAVAILABLE avoids stranding messages already observed by PeekMessage.
        let result = unsafe {
            MsgWaitForMultipleObjectsEx(
                Some(&[shared.handle()]),
                INFINITE,
                QS_ALLINPUT,
                MWMO_INPUTAVAILABLE,
            )
        };
        if result == WAIT_FAILED || shared.stop.load(Ordering::Acquire) {
            return;
        }
        let sample = shared
            .latest
            .lock()
            .ok()
            .and_then(|mut latest| latest.take());
        if let Some(sample) = sample {
            let succeeded = backend.update(sample).is_ok();
            #[cfg(test)]
            if succeeded {
                shared.applied.fetch_add(1, Ordering::Release);
            }
            shared.set_state(
                if succeeded {
                    TrayState::Ready
                } else {
                    TrayState::UpdateFailed
                },
                &guard.ctx,
            );
        }
        // Bound one drain so a busy menu queue cannot starve stop/sample handling.
        for _ in 0..64 {
            if shared.stop.load(Ordering::Acquire) {
                return;
            }
            if !unsafe { PeekMessageW(message, None, 0, 0, PM_REMOVE) }.as_bool() {
                break;
            }
            if message.message == WM_QUIT {
                return;
            }
            unsafe {
                let _ = TranslateMessage(message);
                DispatchMessageW(message);
            }
        }
    }
}

#[cfg(test)]
mod tests;
