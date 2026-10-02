//! The global hotkey through the production app: press handling, the About
//! control and persistence. Fake hotkey API, fake tray, no HWND, no native input,
//! no foreground change; viewport commands are inspected, never executed.
use super::*;
use crate::hotkey::tests::{Fake, settled, until};
use crate::hotkey::{Choice, Controller, Status};
use crate::preferences::{Backend, Loaded, Snapshot};
use crate::tray::{Backend as TrayBackend, TraySample, TrayState};
use std::sync::{Arc, Mutex, atomic::AtomicBool};
use std::time::{Duration, Instant};

struct QuietTray;
impl TrayBackend for QuietTray {
    fn update(&mut self, _: TraySample) -> Result<(), ()> {
        Ok(())
    }
}

/// An app whose tray is Ready and whose hotkey listener runs on the fake API.
fn rig(ctx: &egui::Context, initial: Choice) -> (TrontopApp, Fake) {
    let mut app = app(ThemeSettings::default(), true);
    app.tray = TrayController::with_backend(ctx.clone(), |_, _| Some(QuietTray));
    until(|| app.tray.as_ref().unwrap().state() == TrayState::Ready);
    let fake = Fake::default();
    attach_hotkey(&mut app, ctx, initial, &fake);
    (app, fake)
}

fn attach_hotkey(app: &mut TrontopApp, ctx: &egui::Context, initial: Choice, fake: &Fake) {
    let worker = fake.clone();
    app.hotkey_choice = initial;
    app.hotkey = Controller::spawn(ctx.clone(), initial, move || worker);
}

/// One frame with the window state the OS would report, as egui sees it.
fn render(
    ctx: &egui::Context,
    app: &mut TrontopApp,
    window: (bool, bool),
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    let (minimized, focused) = window;
    let mut raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            Vec2::new(1040.0, 640.0),
        )),
        events,
        ..Default::default()
    };
    let info = raw.viewports.entry(egui::ViewportId::ROOT).or_default();
    info.minimized = Some(minimized);
    info.focused = Some(focused);
    app.raw_input_hook(ctx, &mut raw);
    let output = ctx.run_ui(raw, |ui| {
        app.logic(ui.ctx(), &mut eframe::Frame::_new_kittest());
        app.ui(ui, &mut eframe::Frame::_new_kittest());
    });
    for command in commands(&output) {
        assert!(
            matches!(
                command,
                egui::ViewportCommand::Visible(_)
                    | egui::ViewportCommand::Minimized(false)
                    | egui::ViewportCommand::Focus
                    | egui::ViewportCommand::CancelClose
                    | egui::ViewportCommand::Close
            ),
            "unexpected command {command:?}"
        );
    }
    output
}

fn commands(output: &egui::FullOutput) -> impl Iterator<Item = &egui::ViewportCommand> {
    output.viewport_output.values().flat_map(|vp| &vp.commands)
}
fn hides(output: &egui::FullOutput) -> bool {
    commands(output).any(|c| matches!(c, egui::ViewportCommand::Visible(false)))
}
fn shows(output: &egui::FullOutput) -> bool {
    commands(output).any(|c| matches!(c, egui::ViewportCommand::Visible(true)))
}
fn focuses(output: &egui::FullOutput) -> bool {
    commands(output).any(|c| matches!(c, egui::ViewportCommand::Focus))
}
fn restores(output: &egui::FullOutput) -> bool {
    commands(output).any(|c| matches!(c, egui::ViewportCommand::Minimized(false)))
}

/// A hotkey message reaches the worker and raises the press flag.
fn press(app: &TrontopApp, fake: &Fake) {
    fake.press(1);
    let hotkey = app.hotkey.as_ref().unwrap();
    hotkey.nudge();
    until(|| hotkey.press_waiting());
}

const FOCUSED: (bool, bool) = (false, true);
const BEHIND: (bool, bool) = (false, false);
const MINIMIZED: (bool, bool) = (true, false);

#[test]
fn hotkey_press_toggles_between_the_tray_show_and_the_close_to_tray_paths() {
    let ctx = egui::Context::default();
    let (mut app, fake) = rig(&ctx, Choice::CtrlShiftBacktick);
    settled(
        app.hotkey.as_ref().unwrap(),
        Choice::CtrlShiftBacktick,
        Status::Active,
    );
    app.page = Page::Graphs;
    app.query = "retained search".into();

    // Visible and in front: hide, exactly like the title-bar X.
    press(&app, &fake);
    let output = render(&ctx, &mut app, FOCUSED, vec![]);
    assert!(hides(&output) && !shows(&output) && !focuses(&output));
    assert!(app.hidden_to_tray && !app.close_authorized && app.closing_at.is_none());

    // Hidden in the tray: show, restore and focus, exactly like tray Show.
    press(&app, &fake);
    let output = render(&ctx, &mut app, BEHIND, vec![]);
    assert!(shows(&output) && restores(&output) && focuses(&output) && !hides(&output));
    assert!(!app.hidden_to_tray);
    assert_eq!(
        (app.page, app.query.as_str()),
        (Page::Graphs, "retained search")
    );

    // Visible but behind other windows: bring it forward, never hide it.
    press(&app, &fake);
    let output = render(&ctx, &mut app, BEHIND, vec![]);
    assert!(shows(&output) && focuses(&output) && !hides(&output));
    assert!(!app.hidden_to_tray);

    // Minimized: restore and focus.
    press(&app, &fake);
    let output = render(&ctx, &mut app, MINIMIZED, vec![]);
    assert!(shows(&output) && restores(&output) && focuses(&output) && !hides(&output));

    // No press, no change: a frame without one sends nothing.
    let output = render(&ctx, &mut app, FOCUSED, vec![]);
    assert!(!hides(&output) && !shows(&output) && !focuses(&output));
}

#[test]
fn hotkey_hide_keeps_the_close_to_tray_refusal_when_the_tray_is_not_ready() {
    let ctx = egui::Context::default();
    let mut app = app(ThemeSettings::default(), true);
    let fake = Fake::default();
    attach_hotkey(&mut app, &ctx, Choice::CtrlAltEsc, &fake);
    settled(
        app.hotkey.as_ref().unwrap(),
        Choice::CtrlAltEsc,
        Status::Active,
    );
    press(&app, &fake);
    let output = render(&ctx, &mut app, FOCUSED, vec![]);
    assert!(!hides(&output) && !app.hidden_to_tray);
    assert!(
        app.message
            .as_ref()
            .unwrap()
            .0
            .contains("window stays open")
    );
}

#[test]
fn a_press_repaints_a_hidden_window_once_and_an_idle_one_never() {
    let ctx = egui::Context::default();
    let repaints = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = Arc::clone(&repaints);
    ctx.set_request_repaint_callback(move |_| {
        counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    });
    let (mut app, fake) = rig(&ctx, Choice::CtrlShiftBacktick);
    settled(
        app.hotkey.as_ref().unwrap(),
        Choice::CtrlShiftBacktick,
        Status::Active,
    );
    press(&app, &fake);
    assert!(hides(&render(&ctx, &mut app, FOCUSED, vec![])));
    // Let every pending request settle (the count stops moving across passes),
    // then watch a hidden, idle window.
    let mut idle = usize::MAX;
    for _ in 0..50 {
        std::thread::sleep(Duration::from_millis(60));
        for _ in 0..3 {
            render(&ctx, &mut app, BEHIND, vec![]);
        }
        let now = repaints.load(std::sync::atomic::Ordering::SeqCst);
        if now == idle {
            break;
        }
        idle = now;
    }
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(repaints.load(std::sync::atomic::Ordering::SeqCst), idle);
    // A real press wakes it once.
    press(&app, &fake);
    assert!(repaints.load(std::sync::atomic::Ordering::SeqCst) > idle);
}

// ---- the About control -------------------------------------------------------

fn about_frames(ctx: &egui::Context, app: &mut TrontopApp) -> egui::FullOutput {
    app.show_diagnostics = true;
    let mut output = render(ctx, app, FOCUSED, vec![]);
    for _ in 0..8 {
        output = render(ctx, app, FOCUSED, vec![]);
    }
    output
}

fn visible_text(ctx: &egui::Context, output: &egui::FullOutput, label: &str) -> bool {
    text_shapes(output).into_iter().any(|(text, clip)| {
        text.galley.job.text == label
            && clip.contains_rect(text.visual_bounding_rect())
            && ctx
                .content_rect()
                .contains_rect(text.visual_bounding_rect())
    })
}

fn click_label(ctx: &egui::Context, app: &mut TrontopApp, label: &str) -> egui::FullOutput {
    let mut output = about_frames(ctx, app);
    let pos = text_shapes(&output)
        .into_iter()
        .find(|(text, clip)| {
            text.galley.job.text == label && clip.contains_rect(text.visual_bounding_rect())
        })
        .unwrap_or_else(|| panic!("missing visible {label}"))
        .0
        .visual_bounding_rect()
        .center();
    for pressed in [true, false] {
        output = render(
            ctx,
            app,
            FOCUSED,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    output
}

#[test]
fn about_shows_the_in_use_notice_only_when_the_chosen_combo_is_taken() {
    for dark in [true, false] {
        for scale in [1.0, 1.5, 2.0] {
            let ctx = egui::Context::default();
            ctx.set_pixels_per_point(scale);
            let settings = ThemeSettings {
                dark,
                ..Default::default()
            };
            theme::install(&ctx, settings);
            let (mut app, fake) = rig(&ctx, Choice::CtrlShiftBacktick);
            app.theme = settings;
            let notice = "Ctrl+Shift+` is in use by another app";
            // Working: the row and the choice are there, the notice is not.
            settled(
                app.hotkey.as_ref().unwrap(),
                Choice::CtrlShiftBacktick,
                Status::Active,
            );
            let output = about_frames(&ctx, &mut app);
            let case = format!("dark={dark} scale={scale}");
            assert!(visible_text(&ctx, &output, "Show / hide hotkey"), "{case}");
            assert!(visible_text(&ctx, &output, "Ctrl+Shift+`"), "{case}");
            assert!(!visible_text(&ctx, &output, notice), "{case}");
            // Taken by another app: the notice shows, fully inside the window.
            fake.state().taken.push((
                crate::hotkey::MOD_CONTROL | crate::hotkey::MOD_ALT | crate::hotkey::MOD_NOREPEAT,
                0x1B,
            ));
            app.apply_hotkey(Choice::CtrlAltEsc);
            settled(
                app.hotkey.as_ref().unwrap(),
                Choice::CtrlAltEsc,
                Status::InUse,
            );
            let output = about_frames(&ctx, &mut app);
            assert!(
                visible_text(&ctx, &output, "Ctrl+Alt+Esc is in use by another app"),
                "{case}"
            );
            assert_eq!(
                app.hotkey_notice().as_deref(),
                Some("Ctrl+Alt+Esc is in use by another app")
            );
            // Back to a free combo: the notice goes away without a restart.
            app.apply_hotkey(Choice::CtrlShiftBacktick);
            settled(
                app.hotkey.as_ref().unwrap(),
                Choice::CtrlShiftBacktick,
                Status::Active,
            );
            let output = about_frames(&ctx, &mut app);
            assert!(!visible_text(
                &ctx,
                &output,
                "Ctrl+Alt+Esc is in use by another app"
            ));
            assert!(app.hotkey_notice().is_none());
        }
    }
}

#[test]
fn about_in_use_notice_for_the_default_names_ctrl_shift_backtick() {
    let ctx = egui::Context::default();
    let mut app = app(ThemeSettings::default(), true);
    let fake = Fake::default();
    fake.state().taken.push((
        crate::hotkey::MOD_CONTROL | crate::hotkey::MOD_SHIFT | crate::hotkey::MOD_NOREPEAT,
        0xC0,
    ));
    attach_hotkey(&mut app, &ctx, Choice::CtrlShiftBacktick, &fake);
    settled(
        app.hotkey.as_ref().unwrap(),
        Choice::CtrlShiftBacktick,
        Status::InUse,
    );
    let output = about_frames(&ctx, &mut app);
    assert!(visible_text(
        &ctx,
        &output,
        "Ctrl+Shift+` is in use by another app"
    ));
    // The app keeps running: no hotkey held, window untouched.
    assert!(fake.held().is_empty() && !app.hidden_to_tray);
}

// ---- choosing and saving -----------------------------------------------------

struct Capture {
    stored: Choice,
    saved: Arc<Mutex<Vec<String>>>,
}
impl Backend for Capture {
    fn load(&mut self) -> Result<Loaded, String> {
        Ok(Loaded {
            hotkey: self.stored,
            ..Default::default()
        })
    }
    fn save(&mut self, snapshot: &Snapshot, _: &AtomicBool) -> Result<(), String> {
        self.saved.lock().unwrap().push(snapshot.hotkey.clone());
        Ok(())
    }
}

#[test]
fn the_saved_choice_loads_live_and_a_new_choice_from_about_re_registers_and_saves() {
    let ctx = egui::Context::default();
    let mut app = app(ThemeSettings::default(), true);
    let fake = Fake::default();
    // Starts on the default; the saved settings say Ctrl+Alt+Esc.
    attach_hotkey(&mut app, &ctx, Choice::default(), &fake);
    settled(
        app.hotkey.as_ref().unwrap(),
        Choice::CtrlShiftBacktick,
        Status::Active,
    );
    let saved = Arc::new(Mutex::new(Vec::new()));
    app.preferences = crate::preferences::Controller::with_backend(
        Capture {
            stored: Choice::CtrlAltEsc,
            saved: Arc::clone(&saved),
        },
        ctx.clone(),
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    while !app.preferences.can_edit() {
        app.poll_preferences(&ctx);
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(app.hotkey_choice, Choice::CtrlAltEsc);
    settled(
        app.hotkey.as_ref().unwrap(),
        Choice::CtrlAltEsc,
        Status::Active,
    );
    assert_eq!(
        fake.held(),
        [(
            1,
            crate::hotkey::MOD_CONTROL | crate::hotkey::MOD_ALT | crate::hotkey::MOD_NOREPEAT,
            0x1B
        )]
    );

    // Open About, open the list, pick Off: released live and queued for saving.
    click_label(&ctx, &mut app, "Ctrl+Alt+Esc");
    click_label(&ctx, &mut app, "Off");
    assert_eq!(app.hotkey_choice, Choice::Off);
    settled(app.hotkey.as_ref().unwrap(), Choice::Off, Status::Off);
    assert!(fake.held().is_empty());
    app.preferences.dispatch(true);
    let deadline = Instant::now() + Duration::from_secs(3);
    while saved.lock().unwrap().is_empty() {
        app.poll_preferences(&ctx);
        assert!(Instant::now() < deadline, "the choice was never saved");
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(saved.lock().unwrap().last().unwrap(), "off");
}

#[test]
fn choosing_the_current_combo_again_retries_it_after_the_other_app_lets_go() {
    let ctx = egui::Context::default();
    let mut app = app(ThemeSettings::default(), true);
    let fake = Fake::default();
    fake.state().taken.push((
        crate::hotkey::MOD_CONTROL | crate::hotkey::MOD_SHIFT | crate::hotkey::MOD_NOREPEAT,
        0xC0,
    ));
    attach_hotkey(&mut app, &ctx, Choice::CtrlShiftBacktick, &fake);
    settled(
        app.hotkey.as_ref().unwrap(),
        Choice::CtrlShiftBacktick,
        Status::InUse,
    );
    fake.state().taken.clear();
    click_label(&ctx, &mut app, "Ctrl+Shift+`");
    // The open list repeats the combo's own text. Its entry is the copy that sits
    // beside the other choices, not the one on the closed button.
    let output = about_frames(&ctx, &mut app);
    let center = |label: &str| -> Vec<egui::Pos2> {
        text_shapes(&output)
            .into_iter()
            .filter(|(text, _)| text.galley.job.text == label)
            .map(|(text, _)| text.visual_bounding_rect().center())
            .collect()
    };
    let neighbour = center("Ctrl+Alt+Esc")[0];
    let pos = *center("Ctrl+Shift+`")
        .iter()
        .min_by(|a, b| {
            (a.y - neighbour.y)
                .abs()
                .total_cmp(&(b.y - neighbour.y).abs())
        })
        .expect("an open list");
    for pressed in [true, false] {
        render(
            &ctx,
            &mut app,
            FOCUSED,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    settled(
        app.hotkey.as_ref().unwrap(),
        Choice::CtrlShiftBacktick,
        Status::Active,
    );
    assert_eq!(fake.held().len(), 1);
    assert!(app.hotkey_notice().is_none());
}

#[test]
fn the_choice_is_locked_until_settings_have_loaded() {
    let ctx = egui::Context::default();
    let mut harness = preferences::install(&ctx, true, "loading");
    let app = harness.app.as_mut().unwrap();
    let fake = Fake::default();
    attach_hotkey(app, &ctx, Choice::CtrlShiftBacktick, &fake);
    assert!(!app.preferences.can_edit());
    click_label(&ctx, app, "Ctrl+Shift+`");
    let output = about_frames(&ctx, app);
    assert!(
        !visible_text(&ctx, &output, "Win+Shift+Esc"),
        "the list must not open while loading"
    );
    preferences::release(&mut harness);
}

#[test]
#[ignore = "offscreen About hotkey review; no native window or OS input"]
fn render_hotkey_about_review() {
    let mut renderer = super::offscreen::Renderer::new();
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/ui-smoke/hotkey-alpha51");
    std::fs::create_dir_all(&directory).unwrap();
    for (name, dark, taken) in [
        ("about-dark", true, false),
        ("about-light-in-use", false, true),
    ] {
        let ctx = egui::Context::default();
        let settings = ThemeSettings {
            dark,
            ..Default::default()
        };
        theme::install(&ctx, settings);
        let mut app = app(settings, true);
        let fake = Fake::default();
        if taken {
            fake.state().taken.push((
                crate::hotkey::MOD_CONTROL | crate::hotkey::MOD_SHIFT | crate::hotkey::MOD_NOREPEAT,
                0xC0,
            ));
        }
        attach_hotkey(&mut app, &ctx, Choice::CtrlShiftBacktick, &fake);
        settled(
            app.hotkey.as_ref().unwrap(),
            Choice::CtrlShiftBacktick,
            if taken { Status::InUse } else { Status::Active },
        );
        app.show_diagnostics = true;
        let mut output = egui::FullOutput::default();
        for _ in 0..8 {
            output.append(render(&ctx, &mut app, FOCUSED, vec![]));
        }
        renderer.save(
            &ctx,
            output,
            Vec2::new(1040.0, 640.0),
            &directory.join(format!("{name}.png")),
        );
    }
}
