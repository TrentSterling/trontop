//! Fake-API checks only. Nothing here presses a key, touches a window or the
//! foreground; the ignored native audit registers one unreachable private chord.
use super::*;
use eframe::egui;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const DEFAULT_CHORD: (u32, u32) = (MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT, 0xC0);

#[derive(Default)]
pub(crate) struct State {
    /// Chords another application already owns: registering one is "in use".
    pub(crate) taken: Vec<(u32, u32)>,
    /// Refuse every registration for a reason other than "in use".
    pub(crate) refuse: bool,
    /// Currently registered `(id, modifiers, key)`.
    held: Vec<(i32, u32, u32)>,
    /// Every call, in order, so release-before-register is checkable.
    events: Vec<String>,
    messages: VecDeque<i32>,
}

/// Shared with the test thread; the worker owns its own clone.
#[derive(Clone, Default)]
pub(crate) struct Fake(Arc<Mutex<State>>);

impl Fake {
    pub(crate) fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.0.lock().unwrap()
    }
    /// Queue a hotkey message; wake the worker with `Controller::nudge`.
    pub(crate) fn press(&self, id: i32) {
        self.state().messages.push_back(id);
    }
    pub(crate) fn events(&self) -> Vec<String> {
        self.state().events.clone()
    }
    pub(crate) fn held(&self) -> Vec<(i32, u32, u32)> {
        self.state().held.clone()
    }
}

impl HotkeyApi for Fake {
    fn register(&self, id: i32, modifiers: u32, key: u32) -> Result<(), RegisterError> {
        let mut state = self.state();
        state
            .events
            .push(format!("register {id} {modifiers:#x} {key:#x}"));
        if state.refuse {
            return Err(RegisterError::Other("refused".into()));
        }
        if state.taken.contains(&(modifiers, key)) {
            return Err(RegisterError::InUse);
        }
        state.held.push((id, modifiers, key));
        Ok(())
    }
    fn unregister(&self, id: i32) -> Result<(), String> {
        let mut state = self.state();
        state.events.push(format!("unregister {id}"));
        state.held.retain(|(held, _, _)| *held != id);
        Ok(())
    }
    fn receive(&self) -> Option<i32> {
        self.state().messages.pop_front()
    }
}

pub(crate) fn until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !condition() {
        assert!(Instant::now() < deadline, "owned fake hotkey timed out");
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// An egui context that counts repaint requests, standing in for the window.
pub(crate) fn counting_context() -> (egui::Context, Arc<AtomicUsize>) {
    let ctx = egui::Context::default();
    let repaints = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&repaints);
    ctx.set_request_repaint_callback(move |_| {
        counter.fetch_add(1, Ordering::SeqCst);
    });
    (ctx, repaints)
}

fn spawn(initial: Choice) -> (Controller, Fake, Arc<AtomicUsize>) {
    let (controller, fake, _, repaints) = spawn_with_context(initial);
    (controller, fake, repaints)
}

fn spawn_with_context(initial: Choice) -> (Controller, Fake, egui::Context, Arc<AtomicUsize>) {
    let (ctx, repaints) = counting_context();
    let fake = Fake::default();
    let worker_fake = fake.clone();
    let controller = Controller::spawn(ctx.clone(), initial, move || worker_fake).unwrap();
    (controller, fake, ctx, repaints)
}

/// egui only calls the repaint callback for the first request after a pass, so
/// let the worker finish its last request, then run the passes that reset it.
/// Any later request from the worker is then counted. Returns once the count has
/// stopped moving, so a slow machine cannot blame the worker's last request on
/// the idle window that follows.
fn quiesce(ctx: &egui::Context, repaints: &AtomicUsize) -> usize {
    until(|| repaints.load(Ordering::SeqCst) > 0);
    let mut last = usize::MAX;
    loop {
        std::thread::sleep(Duration::from_millis(60));
        for _ in 0..3 {
            let _ = ctx.run_ui(egui::RawInput::default(), |_| {});
        }
        let now = repaints.load(Ordering::SeqCst);
        if now == last {
            return now;
        }
        last = now;
    }
}

pub(crate) fn settled(controller: &Controller, choice: Choice, status: Status) {
    until(|| controller.report() == Report { choice, status });
}

// ---- the choices -----------------------------------------------------------

#[test]
fn choices_serialize_parse_and_default_when_missing_or_unknown() {
    for choice in Choice::ALL {
        assert_eq!(Choice::parse(choice.encode()), Some(choice));
    }
    assert_eq!(
        Choice::ALL.map(Choice::encode),
        [
            "ctrl+shift+backtick",
            "ctrl+alt+esc",
            "win+shift+esc",
            "off"
        ]
    );
    assert_eq!(
        Choice::parse("  CTRL+Shift+BackTick \n"),
        Some(Choice::CtrlShiftBacktick)
    );
    // The enum default is the shipped default, and it is what absent or
    // unrecognized stored text falls back to (see the preferences tests).
    assert_eq!(Choice::default(), Choice::CtrlShiftBacktick);
    for bad in ["", "on", "ctrl+shift+esc", "ctrl+shift+f12", "Ctrl+Shift+`"] {
        assert_eq!(Choice::parse(bad), None, "{bad:?}");
    }
    assert_eq!(
        Choice::ALL.map(Choice::label),
        ["Ctrl+Shift+`", "Ctrl+Alt+Esc", "Win+Shift+Esc", "Off"]
    );
}

#[test]
fn chords_match_the_requested_combinations_and_never_collide() {
    assert_eq!(Choice::CtrlShiftBacktick.chord(), Some(DEFAULT_CHORD));
    assert_eq!(
        Choice::CtrlAltEsc.chord(),
        Some((MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, 0x1B))
    );
    assert_eq!(
        Choice::WinShiftEsc.chord(),
        Some((MOD_WIN | MOD_SHIFT | MOD_NOREPEAT, 0x1B))
    );
    assert_eq!(Choice::Off.chord(), None);
    let chords: Vec<_> = Choice::ALL.into_iter().filter_map(Choice::chord).collect();
    for (index, chord) in chords.iter().enumerate() {
        assert!(!chords[index + 1..].contains(chord));
    }
}

#[cfg(windows)]
#[test]
fn modifier_bits_equal_the_windows_constants() {
    use windows::Win32::UI::Input::KeyboardAndMouse as k;
    assert_eq!(MOD_ALT, k::MOD_ALT.0);
    assert_eq!(MOD_CONTROL, k::MOD_CONTROL.0);
    assert_eq!(MOD_SHIFT, k::MOD_SHIFT.0);
    assert_eq!(MOD_WIN, k::MOD_WIN.0);
    assert_eq!(MOD_NOREPEAT, k::MOD_NOREPEAT.0);
    assert_eq!(VK_ESCAPE, k::VK_ESCAPE.0 as u32);
    assert_eq!(VK_OEM_3, k::VK_OEM_3.0 as u32);
}

#[test]
fn decision_table_hides_only_a_visible_restored_foreground_window() {
    for visible in [false, true] {
        for minimized in [false, true] {
            for foreground in [false, true] {
                let expected = if visible && !minimized && foreground {
                    Action::Hide
                } else {
                    Action::Show
                };
                assert_eq!(
                    action(visible, minimized, foreground),
                    expected,
                    "visible={visible} minimized={minimized} foreground={foreground}"
                );
            }
        }
    }
    // The named cases from the request.
    assert_eq!(action(false, false, false), Action::Show, "hidden to tray");
    assert_eq!(action(true, true, false), Action::Show, "minimized");
    assert_eq!(
        action(true, false, false),
        Action::Show,
        "behind other windows"
    );
    assert_eq!(
        action(true, false, true),
        Action::Hide,
        "visible and focused"
    );
}

#[test]
fn notices_name_the_combo_only_when_it_is_not_working() {
    let report = |choice, status| Report { choice, status };
    assert_eq!(
        report(Choice::CtrlShiftBacktick, Status::InUse)
            .notice()
            .as_deref(),
        Some("Ctrl+Shift+` is in use by another app")
    );
    assert_eq!(
        report(Choice::WinShiftEsc, Status::InUse)
            .notice()
            .as_deref(),
        Some("Win+Shift+Esc is in use by another app")
    );
    assert_eq!(
        report(Choice::CtrlAltEsc, Status::Failed)
            .notice()
            .as_deref(),
        Some("Windows would not register Ctrl+Alt+Esc")
    );
    for status in [Status::Pending, Status::Off, Status::Active] {
        assert_eq!(report(Choice::CtrlAltEsc, status).notice(), None);
    }
}

// ---- registration, without a thread -----------------------------------------

#[test]
fn applying_a_choice_registers_its_chord_and_off_releases_it() {
    let fake = Fake::default();
    let mut slot = Slot::new(&fake);
    assert_eq!(slot.apply(Choice::CtrlShiftBacktick), Status::Active);
    assert_eq!(fake.held(), [(1, DEFAULT_CHORD.0, DEFAULT_CHORD.1)]);
    assert_eq!(slot.apply(Choice::Off), Status::Off);
    assert!(fake.held().is_empty());
    assert_eq!(
        fake.events(),
        ["register 1 0x4006 0xc0", "unregister 1"],
        "Off must register nothing"
    );
}

#[test]
fn changing_the_choice_releases_the_old_chord_before_registering_the_new_one() {
    let fake = Fake::default();
    let mut slot = Slot::new(&fake);
    slot.apply(Choice::CtrlShiftBacktick);
    assert_eq!(slot.apply(Choice::CtrlAltEsc), Status::Active);
    assert_eq!(
        fake.events(),
        [
            "register 1 0x4006 0xc0",
            "unregister 1",
            "register 1 0x4003 0x1b"
        ]
    );
    assert_eq!(
        fake.held(),
        [(1, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, 0x1B)]
    );
}

#[test]
fn a_taken_chord_reports_in_use_holds_nothing_and_releases_nothing_of_anyone_elses() {
    let fake = Fake::default();
    fake.state().taken.push(DEFAULT_CHORD);
    let mut slot = Slot::new(&fake);
    assert_eq!(slot.apply(Choice::CtrlShiftBacktick), Status::InUse);
    assert!(slot.held.is_none() && fake.held().is_empty());
    assert_eq!(fake.events(), ["register 1 0x4006 0xc0"]);
    drop(slot);
    assert_eq!(fake.events(), ["register 1 0x4006 0xc0"], "no unregister");
    // Another failure kind is reported separately.
    let fake = Fake::default();
    fake.state().refuse = true;
    assert_eq!(Slot::new(&fake).apply(Choice::WinShiftEsc), Status::Failed);
}

#[test]
fn retrying_the_same_choice_recovers_once_the_other_app_lets_go() {
    let fake = Fake::default();
    fake.state().taken.push(DEFAULT_CHORD);
    let mut slot = Slot::new(&fake);
    assert_eq!(slot.apply(Choice::CtrlShiftBacktick), Status::InUse);
    fake.state().taken.clear();
    assert_eq!(slot.apply(Choice::CtrlShiftBacktick), Status::Active);
    assert_eq!(fake.held().len(), 1);
}

#[test]
fn presses_coalesce_other_ids_are_ignored_and_stray_messages_are_drained() {
    let fake = Fake::default();
    let mut slot = Slot::new(&fake);
    slot.apply(Choice::CtrlAltEsc);
    fake.state().messages.extend([2, 1, 1, 99, 1]);
    assert!(slot.pressed());
    assert!(!slot.pressed());
    fake.state().messages.extend([2, 99]);
    assert!(!slot.pressed());
    assert!(fake.state().messages.is_empty());
    // A flood of noise is bounded per call but is still consumed, never left to
    // keep a wait loop awake.
    fake.state()
        .messages
        .extend((0..100).map(|_| 99).chain([1]));
    assert!(!slot.pressed());
    assert_eq!(fake.state().messages.len(), 37);
    assert!(slot.pressed());
}

#[test]
fn nothing_counts_as_a_press_while_no_chord_is_held() {
    let fake = Fake::default();
    let mut slot = Slot::new(&fake);
    slot.apply(Choice::Off);
    fake.press(1);
    assert!(!slot.pressed());
    assert!(fake.state().messages.is_empty());
}

#[test]
fn a_press_queued_for_the_old_chord_does_not_toggle_the_new_one() {
    let fake = Fake::default();
    let mut slot = Slot::new(&fake);
    slot.apply(Choice::CtrlShiftBacktick);
    fake.press(1);
    slot.apply(Choice::CtrlAltEsc);
    assert!(!slot.pressed());
}

#[test]
fn the_registration_is_released_exactly_once_on_drop_and_during_unwinding() {
    let fake = Fake::default();
    {
        let mut slot = Slot::new(&fake);
        slot.apply(Choice::WinShiftEsc);
        assert!(
            fake.events()
                .iter()
                .all(|event| event.starts_with("register"))
        );
    }
    assert_eq!(fake.events().last().unwrap(), "unregister 1");
    assert_eq!(
        fake.events()
            .iter()
            .filter(|e| *e == "unregister 1")
            .count(),
        1
    );
    let fake = Fake::default();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut slot = Slot::new(&fake);
        slot.apply(Choice::CtrlAltEsc);
        panic!("isolated hotkey worker failure");
    }));
    assert!(result.is_err());
    assert!(fake.held().is_empty());
}

// ---- the worker thread, with the fake API -----------------------------------

#[test]
fn the_worker_registers_the_initial_choice_and_reports_active() {
    let (controller, fake, _) = spawn(Choice::CtrlShiftBacktick);
    settled(&controller, Choice::CtrlShiftBacktick, Status::Active);
    assert_eq!(fake.held(), [(1, DEFAULT_CHORD.0, DEFAULT_CHORD.1)]);
}

#[test]
fn a_press_wakes_the_ui_once_and_repeats_coalesce() {
    let (controller, fake, ctx, repaints) = spawn_with_context(Choice::CtrlShiftBacktick);
    settled(&controller, Choice::CtrlShiftBacktick, Status::Active);
    assert!(!controller.take_press());
    let before = quiesce(&ctx, &repaints);
    fake.press(1);
    fake.press(1);
    fake.press(1);
    controller.nudge();
    until(|| controller.take_press());
    assert!(!controller.take_press(), "one toggle per burst");
    assert!(repaints.load(Ordering::SeqCst) > before, "a press repaints");
    // Another id is not ours: no toggle and no repaint.
    let before = quiesce(&ctx, &repaints);
    fake.press(7);
    controller.nudge();
    std::thread::sleep(Duration::from_millis(100));
    assert!(!controller.take_press());
    assert_eq!(repaints.load(Ordering::SeqCst), before);
}

#[test]
fn an_idle_listener_requests_no_repaints_at_all() {
    // The hidden-window CPU landmine: nothing may poke the UI on a timer.
    let (controller, _fake, ctx, repaints) = spawn_with_context(Choice::CtrlShiftBacktick);
    settled(&controller, Choice::CtrlShiftBacktick, Status::Active);
    let baseline = quiesce(&ctx, &repaints);
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(repaints.load(Ordering::SeqCst), baseline);
    assert!(!controller.take_press());
}

#[test]
fn rebinding_reregisters_live_without_a_restart() {
    let (controller, fake, _) = spawn(Choice::CtrlShiftBacktick);
    settled(&controller, Choice::CtrlShiftBacktick, Status::Active);
    controller.set(Choice::CtrlAltEsc);
    settled(&controller, Choice::CtrlAltEsc, Status::Active);
    assert_eq!(
        fake.held(),
        [(1, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, 0x1B)]
    );
    controller.set(Choice::Off);
    settled(&controller, Choice::Off, Status::Off);
    assert!(fake.held().is_empty());
    controller.set(Choice::WinShiftEsc);
    settled(&controller, Choice::WinShiftEsc, Status::Active);
    assert_eq!(fake.held(), [(1, MOD_WIN | MOD_SHIFT | MOD_NOREPEAT, 0x1B)]);
    assert_eq!(
        fake.events(),
        [
            "register 1 0x4006 0xc0",
            "unregister 1",
            "register 1 0x4003 0x1b",
            "unregister 1",
            "register 1 0x400c 0x1b",
        ]
    );
}

#[test]
fn a_taken_chord_shows_in_use_then_a_different_or_retried_choice_works() {
    let (ctx, _) = counting_context();
    let fake = Fake::default();
    fake.state().taken.push(DEFAULT_CHORD);
    let worker_fake = fake.clone();
    let controller =
        Controller::spawn(ctx, Choice::CtrlShiftBacktick, move || worker_fake).unwrap();
    settled(&controller, Choice::CtrlShiftBacktick, Status::InUse);
    assert_eq!(
        controller.report().notice().as_deref(),
        Some("Ctrl+Shift+` is in use by another app")
    );
    assert!(fake.held().is_empty());
    controller.set(Choice::CtrlAltEsc);
    settled(&controller, Choice::CtrlAltEsc, Status::Active);
    assert_eq!(controller.report().notice(), None);
    // The other app lets go: choosing the same combo again retries it.
    fake.state().taken.clear();
    controller.set(Choice::CtrlShiftBacktick);
    settled(&controller, Choice::CtrlShiftBacktick, Status::Active);
    assert_eq!(fake.held().len(), 1);
}

#[test]
fn changing_the_choice_reports_pending_for_the_new_choice_immediately() {
    let (controller, _fake, _) = spawn(Choice::CtrlShiftBacktick);
    settled(&controller, Choice::CtrlShiftBacktick, Status::Active);
    controller.set(Choice::WinShiftEsc);
    assert_eq!(controller.report().choice, Choice::WinShiftEsc);
    settled(&controller, Choice::WinShiftEsc, Status::Active);
}

#[test]
fn dropping_the_controller_unregisters_on_its_thread_and_joins() {
    let (controller, fake, _) = spawn(Choice::CtrlShiftBacktick);
    settled(&controller, Choice::CtrlShiftBacktick, Status::Active);
    let started = Instant::now();
    drop(controller);
    assert!(fake.held().is_empty(), "released before drop returned");
    assert_eq!(fake.events().last().unwrap(), "unregister 1");
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn dropping_while_off_or_in_use_has_nothing_to_release() {
    let (controller, fake, _) = spawn(Choice::Off);
    settled(&controller, Choice::Off, Status::Off);
    drop(controller);
    assert!(fake.events().is_empty());
}

// ---- real Win32, one private chord, no desktop input -------------------------

#[cfg(windows)]
#[test]
#[ignore = "Registers only a private Ctrl+Alt+Shift+Win+F24 chord and posts to its own worker thread; no desktop input"]
fn native_audit_private_chord_is_in_use_wakes_the_worker_and_releases_on_drop() {
    use super::native::NativeHotkeys;
    use std::sync::atomic::AtomicU32;
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_HOTKEY};

    const PRIVATE: (u32, u32) = (
        MOD_ALT | MOD_CONTROL | MOD_SHIFT | MOD_WIN | MOD_NOREPEAT,
        0x87,
    );

    /// The real API, but always registering the private chord and recording the
    /// worker thread so the test can post a message to its own queue.
    struct Private(Arc<AtomicU32>);
    impl HotkeyApi for Private {
        fn register(&self, id: i32, _: u32, _: u32) -> Result<(), RegisterError> {
            self.0
                .store(unsafe { GetCurrentThreadId() }, Ordering::SeqCst);
            NativeHotkeys.register(id, PRIVATE.0, PRIVATE.1)
        }
        fn unregister(&self, id: i32) -> Result<(), String> {
            NativeHotkeys.unregister(id)
        }
        fn receive(&self) -> Option<i32> {
            NativeHotkeys.receive()
        }
    }

    let (ctx, _) = counting_context();
    let thread = Arc::new(AtomicU32::new(0));
    let worker_thread = Arc::clone(&thread);
    let controller = Controller::spawn(ctx, Choice::CtrlShiftBacktick, move || {
        Private(worker_thread)
    })
    .unwrap();
    settled(&controller, Choice::CtrlShiftBacktick, Status::Active);

    // The same chord from another thread is refused as "in use" (error 1409).
    let taken = std::thread::spawn(|| NativeHotkeys.register(1, PRIVATE.0, PRIVATE.1))
        .join()
        .unwrap();
    assert_eq!(taken, Err(RegisterError::InUse));

    // A message in the worker's own queue wakes its wait and raises one press.
    let id = thread.load(Ordering::SeqCst);
    assert_ne!(id, 0);
    unsafe { PostThreadMessageW(id, WM_HOTKEY, WPARAM(1), LPARAM(0)) }.unwrap();
    until(|| controller.take_press());
    // Another id does not.
    unsafe { PostThreadMessageW(id, WM_HOTKEY, WPARAM(2), LPARAM(0)) }.unwrap();
    std::thread::sleep(Duration::from_millis(100));
    assert!(!controller.take_press());

    // Dropping the controller released the chord on its thread: it is free again.
    drop(controller);
    let free = std::thread::spawn(|| {
        let result = NativeHotkeys.register(1, PRIVATE.0, PRIVATE.1);
        let _ = NativeHotkeys.unregister(1);
        result
    })
    .join()
    .unwrap();
    assert_eq!(free, Ok(()));
}
