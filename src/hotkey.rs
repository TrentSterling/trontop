//! Global show/hide hotkey: the choices, the registration API and its worker.
//!
//! `RegisterHotKey` without a window is per thread, so one dedicated thread
//! registers, receives `WM_HOTKEY` and unregisters. It sleeps in a message-aware
//! wait on a private event (zero idle wake-ups, no timer) and asks the UI for a
//! repaint only for a real key press or a registration result. That matters: a
//! hidden eframe window must never be fed periodic repaints.

/// Settings-file key. Absent or unrecognized values load as the default, so
/// settings written before this feature (or by a newer build) still open.
pub const STORAGE_KEY: &str = "trontop.hotkey.v1";

/// Windows hotkey modifier bits (`MOD_*`), kept free of the `windows` crate so
/// the choices and their tests are plain data.
pub const MOD_ALT: u32 = 0x0001;
pub const MOD_CONTROL: u32 = 0x0002;
pub const MOD_SHIFT: u32 = 0x0004;
pub const MOD_WIN: u32 = 0x0008;
pub const MOD_NOREPEAT: u32 = 0x4000;
const VK_ESCAPE: u32 = 0x1B;
/// `VK_OEM_3`: the grave / backtick key on a US layout.
const VK_OEM_3: u32 = 0xC0;
/// One hotkey per process; the thread-level registration id.
const HOTKEY_ID: i32 = 1;

/// The user-selectable chords.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Choice {
    #[default]
    CtrlShiftBacktick,
    CtrlAltEsc,
    WinShiftEsc,
    Off,
}

impl Choice {
    pub const ALL: [Self; 4] = [
        Self::CtrlShiftBacktick,
        Self::CtrlAltEsc,
        Self::WinShiftEsc,
        Self::Off,
    ];

    /// Shown in the UI.
    pub fn label(self) -> &'static str {
        match self {
            Self::CtrlShiftBacktick => "Ctrl+Shift+`",
            Self::CtrlAltEsc => "Ctrl+Alt+Esc",
            Self::WinShiftEsc => "Win+Shift+Esc",
            Self::Off => "Off",
        }
    }

    /// Stable settings-file text. Never reuse a retired value for a new chord.
    pub fn encode(self) -> &'static str {
        match self {
            Self::CtrlShiftBacktick => "ctrl+shift+backtick",
            Self::CtrlAltEsc => "ctrl+alt+esc",
            Self::WinShiftEsc => "win+shift+esc",
            Self::Off => "off",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        Self::ALL
            .into_iter()
            .find(|choice| choice.encode().eq_ignore_ascii_case(text))
    }

    /// `(modifiers, virtual key)`, or `None` when the hotkey is off.
    pub fn chord(self) -> Option<(u32, u32)> {
        match self {
            Self::CtrlShiftBacktick => Some((MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT, VK_OEM_3)),
            Self::CtrlAltEsc => Some((MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, VK_ESCAPE)),
            Self::WinShiftEsc => Some((MOD_WIN | MOD_SHIFT | MOD_NOREPEAT, VK_ESCAPE)),
            Self::Off => None,
        }
    }
}

/// How the last registration attempt for a choice ended.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Status {
    /// Requested; the worker has not answered yet.
    Pending,
    Off,
    Active,
    /// Another application already owns the chord.
    InUse,
    /// Windows refused the registration for another reason.
    Failed,
}

/// A status together with the choice it belongs to, so a stale answer for an
/// earlier choice is never shown against the current one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Report {
    pub choice: Choice,
    pub status: Status,
}

impl Report {
    /// One short sentence when the chord is not working, otherwise nothing.
    pub fn notice(self) -> Option<String> {
        match self.status {
            Status::InUse => Some(format!("{} is in use by another app", self.choice.label())),
            Status::Failed => Some(format!(
                "Windows would not register {}",
                self.choice.label()
            )),
            Status::Pending | Status::Off | Status::Active => None,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum RegisterError {
    InUse,
    Other(String),
}

/// What toggling does, given the window's state when the key is pressed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    /// Un-hide, restore and focus.
    Show,
    /// Hide the way the close button does.
    Hide,
}

/// Only a visible, restored window that already has the foreground hides; every
/// other state brings it to the front, so the key always means "show me".
pub fn action(visible: bool, minimized: bool, foreground: bool) -> Action {
    if visible && !minimized && foreground {
        Action::Hide
    } else {
        Action::Show
    }
}

/// Thread-level hotkey calls. A fake stands in for it in tests.
pub(crate) trait HotkeyApi {
    fn register(&self, id: i32, modifiers: u32, key: u32) -> Result<(), RegisterError>;
    fn unregister(&self, id: i32) -> Result<(), String>;
    /// Next queued hotkey id on this thread, discarding other messages.
    fn receive(&self) -> Option<i32>;
}

/// A held registration, released on drop (including unwinding).
struct Registration<'a> {
    api: &'a dyn HotkeyApi,
    id: i32,
    // Thread registrations must be released on the thread that owns them.
    _thread: std::marker::PhantomData<std::rc::Rc<()>>,
}

impl<'a> Registration<'a> {
    fn new(
        api: &'a dyn HotkeyApi,
        id: i32,
        modifiers: u32,
        key: u32,
    ) -> Result<Self, RegisterError> {
        api.register(id, modifiers, key)?;
        Ok(Self {
            api,
            id,
            _thread: std::marker::PhantomData,
        })
    }
}

impl Drop for Registration<'_> {
    fn drop(&mut self) {
        if let Err(error) = self.api.unregister(self.id) {
            eprintln!("Trontop: could not release hotkey {}: {error}", self.id);
        }
    }
}

/// The worker's state between wake-ups: at most one live registration.
struct Slot<'a> {
    api: &'a dyn HotkeyApi,
    held: Option<Registration<'a>>,
}

impl<'a> Slot<'a> {
    fn new(api: &'a dyn HotkeyApi) -> Self {
        Self { api, held: None }
    }

    /// Release the old chord first (so re-selecting the same one can succeed),
    /// then register the new one.
    fn apply(&mut self, choice: Choice) -> Status {
        self.held = None;
        // A press queued for the chord just released must not toggle the new one.
        self.drain();
        let Some((modifiers, key)) = choice.chord() else {
            return Status::Off;
        };
        match Registration::new(self.api, HOTKEY_ID, modifiers, key) {
            Ok(held) => {
                self.held = Some(held);
                Status::Active
            }
            Err(RegisterError::InUse) => {
                eprintln!(
                    "Trontop: hotkey {} is in use by another app",
                    choice.label()
                );
                Status::InUse
            }
            Err(RegisterError::Other(error)) => {
                eprintln!(
                    "Trontop: could not register hotkey {}: {error}",
                    choice.label()
                );
                Status::Failed
            }
        }
    }

    fn drain(&self) {
        for _ in 0..64 {
            if self.api.receive().is_none() {
                break;
            }
        }
    }

    /// Whether the held chord was pressed since the last call. Always drains the
    /// queue so a stray message can never keep the wait loop awake. Repeats
    /// coalesce into one press.
    fn pressed(&self) -> bool {
        let mut pressed = false;
        for _ in 0..64 {
            let Some(id) = self.api.receive() else {
                break;
            };
            pressed |= self.held.is_some() && id == HOTKEY_ID;
        }
        pressed
    }
}

#[cfg(windows)]
mod native {
    use super::{Choice, HotkeyApi, RegisterError, Report, Slot, Status};
    use eframe::egui;
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread::{self, JoinHandle};
    use windows::Win32::Foundation::{ERROR_HOTKEY_ALREADY_REGISTERED, HANDLE, HWND, WAIT_FAILED};
    use windows::Win32::System::Threading::{CreateEventW, INFINITE, SetEvent};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        HOT_KEY_MODIFIERS, RegisterHotKey, UnregisterHotKey,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        MSG, MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx, PM_NOREMOVE, PM_REMOVE,
        PeekMessageW, QS_ALLINPUT, WM_HOTKEY,
    };
    use windows::core::HRESULT;

    pub(crate) struct NativeHotkeys;

    impl HotkeyApi for NativeHotkeys {
        fn register(&self, id: i32, modifiers: u32, key: u32) -> Result<(), RegisterError> {
            unsafe { RegisterHotKey(None::<HWND>, id, HOT_KEY_MODIFIERS(modifiers), key) }.map_err(
                |error| {
                    if error.code() == HRESULT::from_win32(ERROR_HOTKEY_ALREADY_REGISTERED.0) {
                        RegisterError::InUse
                    } else {
                        RegisterError::Other(error.to_string())
                    }
                },
            )
        }

        fn unregister(&self, id: i32) -> Result<(), String> {
            unsafe { UnregisterHotKey(None::<HWND>, id) }.map_err(|error| error.to_string())
        }

        fn receive(&self) -> Option<i32> {
            // This thread owns no windows: any other message is noise to discard.
            let mut message = MSG::default();
            for _ in 0..64 {
                if !unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
                    return None;
                }
                if message.message == WM_HOTKEY {
                    return Some(message.wParam.0 as i32);
                }
            }
            None
        }
    }

    struct Shared {
        // Auto-reset event: rebind and stop requests wake the worker's wait.
        wake: OwnedHandle,
        stop: AtomicBool,
        pressed: AtomicBool,
        desired: Mutex<Option<Choice>>,
        report: Mutex<Report>,
    }

    impl Shared {
        fn signal(&self) {
            // The worker and the controller both hold an Arc, so the handle
            // outlives every signal.
            let _ = unsafe { SetEvent(HANDLE(self.wake.as_raw_handle())) };
        }

        fn publish(&self, choice: Choice, status: Status, ctx: &egui::Context) {
            if let Ok(mut report) = self.report.lock() {
                *report = Report { choice, status };
            }
            // One repaint per registration result, never on a timer.
            ctx.request_repaint();
        }
    }

    /// Owns the hotkey thread. The UI polls it; it never touches the OS itself.
    pub struct Controller {
        shared: Arc<Shared>,
        worker: Option<JoinHandle<()>>,
    }

    impl Controller {
        pub fn new(ctx: egui::Context, initial: Choice) -> Option<Self> {
            Self::spawn(ctx, initial, || NativeHotkeys)
        }

        /// `make` runs on the worker thread, so the API need not be `Send`.
        pub(crate) fn spawn<A: HotkeyApi + 'static>(
            ctx: egui::Context,
            initial: Choice,
            make: impl FnOnce() -> A + Send + 'static,
        ) -> Option<Self> {
            // Unnamed, non-inheritable, auto-reset: no global window/thread targeting.
            let handle = unsafe { CreateEventW(None, false, false, None) }.ok()?;
            let shared = Arc::new(Shared {
                wake: unsafe { OwnedHandle::from_raw_handle(handle.0) },
                stop: AtomicBool::new(false),
                pressed: AtomicBool::new(false),
                desired: Mutex::new(Some(initial)),
                report: Mutex::new(Report {
                    choice: initial,
                    status: Status::Pending,
                }),
            });
            let worker_shared = Arc::clone(&shared);
            let worker = thread::Builder::new()
                .name("trontop-hotkey".into())
                .spawn(move || run(&make(), &worker_shared, &ctx, initial))
                .ok()?;
            Some(Self {
                shared,
                worker: Some(worker),
            })
        }

        /// Release the current chord and register `choice`. Re-selecting the
        /// current choice retries it, which recovers from "in use".
        pub fn set(&self, choice: Choice) {
            if let Ok(mut report) = self.shared.report.lock() {
                *report = Report {
                    choice,
                    status: Status::Pending,
                };
            }
            if let Ok(mut desired) = self.shared.desired.lock() {
                *desired = Some(choice);
            }
            self.shared.signal();
        }

        pub fn report(&self) -> Report {
            self.shared.report.lock().map_or(
                Report {
                    choice: Choice::Off,
                    status: Status::Failed,
                },
                |report| *report,
            )
        }

        /// True once per press (repeats coalesce). Call from the UI thread.
        pub fn take_press(&self) -> bool {
            self.shared.pressed.swap(false, Ordering::AcqRel)
        }

        /// Wake the worker so it re-checks its fake API; tests stand in for the
        /// system's own hotkey message this way.
        #[cfg(test)]
        pub(crate) fn nudge(&self) {
            self.shared.signal();
        }

        /// Whether a press is waiting, without consuming it.
        #[cfg(test)]
        pub(crate) fn press_waiting(&self) -> bool {
            self.shared.pressed.load(Ordering::Acquire)
        }
    }

    impl Drop for Controller {
        fn drop(&mut self) {
            self.shared.stop.store(true, Ordering::Release);
            self.shared.signal();
            if let Some(worker) = self.worker.take() {
                crate::shutdown::finish(worker, std::time::Duration::from_millis(100));
            }
        }
    }

    fn run(api: &dyn HotkeyApi, shared: &Shared, ctx: &egui::Context, initial: Choice) {
        // Create this thread's message queue before the first registration.
        let mut message = MSG::default();
        unsafe {
            let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
        }
        let mut slot = Slot::new(api);
        let mut current = initial;
        loop {
            if shared.stop.load(Ordering::Acquire) {
                break;
            }
            let wanted = shared.desired.lock().ok().and_then(|mut d| d.take());
            if let Some(choice) = wanted {
                current = choice;
                shared.publish(choice, slot.apply(choice), ctx);
            }
            if slot.pressed() {
                shared.pressed.store(true, Ordering::Release);
                ctx.request_repaint();
            }
            let result = unsafe {
                MsgWaitForMultipleObjectsEx(
                    Some(&[HANDLE(shared.wake.as_raw_handle())]),
                    INFINITE,
                    QS_ALLINPUT,
                    MWMO_INPUTAVAILABLE,
                )
            };
            if result == WAIT_FAILED {
                // Never spin: give up the chord and say so.
                slot.held = None;
                shared.publish(current, Status::Failed, ctx);
                break;
            }
        }
        // `slot` drops here, unregistering on the thread that registered.
    }
}

#[cfg(windows)]
pub use native::Controller;

#[cfg(not(windows))]
pub struct Controller;

#[cfg(not(windows))]
impl Controller {
    pub fn new(_ctx: eframe::egui::Context, _initial: Choice) -> Option<Self> {
        None
    }

    pub fn set(&self, _choice: Choice) {}

    pub fn report(&self) -> Report {
        Report {
            choice: Choice::Off,
            status: Status::Off,
        }
    }

    pub fn take_press(&self) -> bool {
        false
    }
}

#[cfg(all(test, windows))]
pub(crate) mod tests;
