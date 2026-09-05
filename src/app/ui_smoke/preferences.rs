use super::*;
use crate::preferences::{Backend, Loaded, Snapshot};
use std::sync::{atomic::AtomicBool, mpsc};
use std::time::{Duration, Instant};

struct Store {
    theme: ThemeSettings,
    load_wait: Option<mpsc::Receiver<()>>,
    save_wait: Option<mpsc::Receiver<()>>,
    fail: bool,
    entered: mpsc::Sender<()>,
    done: mpsc::Sender<()>,
}
impl Backend for Store {
    fn load(&mut self) -> Result<Loaded, String> {
        if let Some(wait) = self.load_wait.take() {
            let _ = wait.recv();
        }
        Ok(Loaded {
            theme: self.theme,
            ..Default::default()
        })
    }
    fn save(&mut self, _: Snapshot, _: &AtomicBool) -> Result<(), String> {
        let _ = self.entered.send(());
        if let Some(wait) = self.save_wait.take() {
            let _ = wait.recv();
        }
        if std::mem::take(&mut self.fail) {
            Err("Cannot replace settings; existing file preserved.".into())
        } else {
            Ok(())
        }
    }
}
impl Drop for Store {
    fn drop(&mut self) {
        let _ = self.done.send(());
    }
}

pub(super) struct Harness {
    pub app: Option<TrontopApp>,
    release: Option<mpsc::Sender<()>>,
    entered: mpsc::Receiver<()>,
    done: mpsc::Receiver<()>,
}
impl Drop for Harness {
    fn drop(&mut self) {
        self.release.take();
        self.app.take();
        self.done
            .recv_timeout(Duration::from_secs(3))
            .expect("owned preference test worker did not finish");
    }
}

pub(super) fn install(ctx: &egui::Context, dark: bool, state: &str) -> Harness {
    let theme = ThemeSettings {
        dark,
        ..Default::default()
    };
    let mut app = app(theme, true);
    theme::install(ctx, theme);
    let (release, wait) = mpsc::channel();
    let (entered, started) = mpsc::channel();
    let (done, finished) = mpsc::channel();
    let (load_wait, save_wait) = if state == "loading" {
        (Some(wait), None)
    } else if state == "pending" {
        (None, Some(wait))
    } else {
        (None, None)
    };
    let mut stored_theme = theme;
    if state == "loading" {
        stored_theme.accent = [155, 100, 230];
    }
    app.preferences = crate::preferences::Controller::with_backend(
        Store {
            theme: stored_theme,
            load_wait,
            save_wait,
            fail: state == "error",
            entered,
            done,
        },
        ctx.clone(),
    );
    if state != "loading" {
        let deadline = Instant::now() + Duration::from_secs(3);
        while !app.preferences.can_edit() {
            app.poll_preferences(ctx);
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    let mut harness = Harness {
        app: Some(app),
        release: Some(release),
        entered: started,
        done: finished,
    };
    if matches!(state, "pending" | "error") {
        let app = harness.app.as_mut().unwrap();
        app.theme.accent = [34, 165, 220];
        app.capture_preferences(ctx);
        app.preferences.dispatch(true);
        harness
            .entered
            .recv_timeout(Duration::from_secs(3))
            .unwrap();
        if state == "error" {
            let deadline = Instant::now() + Duration::from_secs(3);
            while app.preferences.error().is_none() {
                app.poll_preferences(ctx);
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }
    harness
}

fn render(
    ctx: &egui::Context,
    app: &mut TrontopApp,
    size: Vec2,
    events: Vec<egui::Event>,
    close: bool,
) -> egui::FullOutput {
    let mut raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
        events,
        ..Default::default()
    };
    if close {
        raw.viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .events
            .push(egui::ViewportEvent::Close);
    }
    app.raw_input_hook(ctx, &mut raw);
    let output = ctx.run_ui(raw, |ui| {
        app.logic(ui.ctx(), &mut eframe::Frame::_new_kittest());
        app.ui(ui, &mut eframe::Frame::_new_kittest());
    });
    assert!(output.platform_output.commands.is_empty());
    for command in output.viewport_output.values().flat_map(|vp| &vp.commands) {
        assert!(
            matches!(
                command,
                egui::ViewportCommand::Close | egui::ViewportCommand::CancelClose
            ),
            "unexpected native command: {command:?}"
        );
    }
    // This fixture only inspects commands. It never executes them against a HWND.
    output
}

fn commands(output: &egui::FullOutput) -> impl Iterator<Item = &egui::ViewportCommand> {
    output.viewport_output.values().flat_map(|vp| &vp.commands)
}

fn click(ctx: &egui::Context, app: &mut TrontopApp, label: &str) -> egui::FullOutput {
    let size = Vec2::new(1040.0, 640.0);
    let mut output = render(ctx, app, size, vec![], false);
    for _ in 0..3 {
        output = render(ctx, app, size, vec![], false);
    }
    let pos = text_shapes(&output)
        .into_iter()
        .find(|(t, c)| t.galley.job.text == label && c.contains_rect(t.visual_bounding_rect()))
        .unwrap_or_else(|| panic!("missing visible control {label}"))
        .0
        .visual_bounding_rect()
        .center();
    for pressed in [true, false] {
        output = render(
            ctx,
            app,
            size,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            false,
        );
    }
    output
}

#[test]
fn preferences_loading_keeps_navigation_live_without_overwriting_saved_theme() {
    let ctx = egui::Context::default();
    let mut h = install(&ctx, true, "loading");
    let app = h.app.as_mut().unwrap();
    click(&ctx, app, "Users");
    assert!(matches!(app.page, Page::Users));
    app.show_theme_editor = true;
    let before = app.theme;
    click(&ctx, app, "Reset to TrontStack");
    assert_eq!(app.theme, before);
    assert!(!app.preferences.can_edit());
    assert!(!app.preferences.pending());
    h.release.take();
    let deadline = Instant::now() + Duration::from_secs(3);
    while !app.preferences.can_edit() {
        render(&ctx, app, Vec2::new(1040.0, 640.0), vec![], false);
        assert!(Instant::now() < deadline);
    }
    assert!(matches!(app.page, Page::Users));
    let restored = app.theme;
    assert_eq!(restored.accent, [155, 100, 230]);
    click(&ctx, app, "Reset to TrontStack");
    click(&ctx, app, "Revert session");
    assert_eq!(
        app.theme, restored,
        "loading defaults replaced the saved revert target"
    );
}

#[test]
fn preferences_native_close_defers_while_blocked_then_closes_only_after_latest_save() {
    let ctx = egui::Context::default();
    let mut h = install(&ctx, true, "pending");
    let app = h.app.as_mut().unwrap();
    let size = Vec2::new(1040.0, 640.0);
    let output = render(&ctx, app, size, vec![], true);
    assert!(commands(&output).any(|c| matches!(c, egui::ViewportCommand::CancelClose)));
    assert!(!commands(&output).any(|c| matches!(c, egui::ViewportCommand::Close)));
    app.closing_at = Some(Instant::now() - Duration::from_secs(1));
    for _ in 0..20 {
        let output = render(&ctx, app, size, vec![], false);
        assert!(!commands(&output).any(|c| matches!(c, egui::ViewportCommand::Close)));
    }
    assert!(!app.close_authorized);
    h.release.take();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let output = render(&ctx, app, size, vec![], false);
        if commands(&output).any(|c| matches!(c, egui::ViewportCommand::Close)) {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(app.close_authorized);
    assert!(!app.preferences.pending());
}

#[test]
fn preferences_failed_close_keeps_changes_and_retry_uses_real_ui_controls() {
    let ctx = egui::Context::default();
    let mut h = install(&ctx, false, "error");
    let app = h.app.as_mut().unwrap();
    app.request_close(&ctx);
    let output = click(&ctx, app, "Keep open");
    assert!(!commands(&output).any(|c| matches!(c, egui::ViewportCommand::Close)));
    assert!(app.closing_at.is_none());
    assert!(app.preferences.pending());
    click(&ctx, app, "Users");
    assert!(matches!(app.page, Page::Users));
    app.request_close(&ctx);
    let output = click(&ctx, app, "Retry save");
    let mut closed = commands(&output).any(|c| matches!(c, egui::ViewportCommand::Close));
    let deadline = Instant::now() + Duration::from_secs(3);
    while !closed {
        let output = render(&ctx, app, Vec2::new(1040.0, 640.0), vec![], false);
        closed = commands(&output).any(|c| matches!(c, egui::ViewportCommand::Close));
        assert!(Instant::now() < deadline);
    }
    assert!(!app.preferences.pending());
}

#[test]
fn preferences_close_anyway_requires_explicit_control_and_loading_can_close_without_writes() {
    let ctx = egui::Context::default();
    let mut h = install(&ctx, true, "error");
    let app = h.app.as_mut().unwrap();
    app.request_close(&ctx);
    let output = click(&ctx, app, "Close anyway");
    assert!(commands(&output).any(|c| matches!(c, egui::ViewportCommand::Close)));
    assert!(app.close_authorized);
    assert!(app.preferences.pending()); // No fake saved acknowledgement.
    drop(h);
    let ctx = egui::Context::default();
    let mut h = install(&ctx, true, "loading");
    let app = h.app.as_mut().unwrap();
    let output = render(&ctx, app, Vec2::new(1040.0, 640.0), vec![], true);
    assert!(commands(&output).any(|c| matches!(c, egui::ViewportCommand::Close)));
    assert!(app.close_authorized);
    assert!(h.entered.try_recv().is_err());
}

#[test]
fn preferences_status_and_close_controls_fit_compact_dark_light_scaled_ui() {
    for dark in [true, false] {
        for scale in [1.0, 1.5, 2.0] {
            for state in ["loading", "saved", "pending", "error"] {
                let ctx = egui::Context::default();
                ctx.set_pixels_per_point(scale);
                let mut h = install(&ctx, dark, state);
                let app = h.app.as_mut().unwrap();
                if matches!(state, "pending" | "error") {
                    app.closing_at = Some(Instant::now() - Duration::from_secs(1));
                }
                let size = Vec2::new(1040.0, 640.0);
                let mut output = render(&ctx, app, size, vec![], false);
                for _ in 0..3 {
                    output = render(&ctx, app, size, vec![], false);
                }
                let mut labels = vec![app.preferences.status()];
                if matches!(state, "pending" | "error") {
                    labels.extend(["Keep open", "Retry save", "Close anyway"]);
                }
                if state == "error" {
                    labels.push("Retry save / load");
                }
                for label in labels {
                    assert!(
                        text_shapes(&output)
                            .into_iter()
                            .any(|(text, clip)| text.galley.job.text == label
                                && clip.contains_rect(text.visual_bounding_rect())
                                && egui::Rect::from_min_size(egui::Pos2::ZERO, size)
                                    .contains_rect(text.visual_bounding_rect())),
                        "clipped {label}, dark={dark} scale={scale} state={state}"
                    );
                }
            }
        }
    }
}

#[test]
fn preferences_app_roundtrip_preserves_latest_theme_named_palette_and_ui_memory() {
    let fixture = crate::preferences::tests::Fixture::new();
    let ctx = egui::Context::default();
    let mut app = app(ThemeSettings::default(), true);
    app.preferences = crate::preferences::Controller::native(Some(fixture.0.clone()), ctx.clone());
    let size = Vec2::new(1040.0, 640.0);
    let deadline = Instant::now() + Duration::from_secs(4);
    while !app.preferences.can_edit() {
        render(&ctx, &mut app, size, vec![], false);
        assert!(Instant::now() < deadline);
    }
    app.theme.stops[1].color = [50, 90, 170];
    app.theme.accent = [90, 180, 230];
    let library = serde_json::json!({"version":1,"themes":[{"name":"Saved from UI fixture","theme":app.theme.encode()}]}).to_string();
    assert!(app.theme_studio.load_library(&library));
    ctx.set_zoom_factor(1.25);
    render(&ctx, &mut app, size, vec![], false);
    assert!(app.preferences.pending()); // Actual end-of-UI dirty detection.
    let expected = app.theme;
    app.request_close(&ctx);
    let deadline = Instant::now() + Duration::from_secs(4);
    while !app.close_authorized {
        render(&ctx, &mut app, size, vec![], false);
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    drop(app);
    let ctx = egui::Context::default();
    let mut app = super::app(ThemeSettings::default(), true);
    app.preferences = crate::preferences::Controller::native(Some(fixture.0.clone()), ctx.clone());
    let deadline = Instant::now() + Duration::from_secs(4);
    while !app.preferences.can_edit() {
        render(&ctx, &mut app, size, vec![], false);
        assert!(Instant::now() < deadline);
    }
    assert_eq!(app.theme, expected);
    assert_eq!(app.theme_studio.encode_library(), library);
    assert_eq!(ctx.zoom_factor(), 1.25);
    assert!(!app.preferences.pending());
    drop(app);
}
