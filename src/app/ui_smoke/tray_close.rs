//! Production app commands and tray worker with an owner-thread fake backend.
//! No HWND, real notification icon, native input or user preference files.
use super::*;
use crate::tray::{Backend, TraySample, TrayState};
use std::sync::{
    Arc,
    atomic::{AtomicU8, AtomicUsize, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

struct FakeTray {
    mode: Arc<AtomicU8>,
    updates: Arc<AtomicUsize>,
}

impl Backend for FakeTray {
    fn update(&mut self, _: TraySample) -> Result<(), ()> {
        self.updates.fetch_add(1, Ordering::Release);
        match self.mode.load(Ordering::Acquire) {
            1 => Err(()),
            2 => {
                // Only this disposable worker's own queue, never a window.
                unsafe {
                    windows::Win32::UI::WindowsAndMessaging::PostQuitMessage(0);
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

fn until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !condition() {
        assert!(Instant::now() < deadline, "owned fake tray timed out");
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn attach(app: &mut TrontopApp, ctx: &egui::Context) -> (Arc<AtomicU8>, Arc<AtomicUsize>) {
    let mode = Arc::new(AtomicU8::new(0));
    let updates = Arc::new(AtomicUsize::new(0));
    let backend = FakeTray {
        mode: Arc::clone(&mode),
        updates: Arc::clone(&updates),
    };
    app.tray = TrayController::with_backend(ctx.clone(), move |_, _| Some(backend));
    until(|| app.tray.as_ref().unwrap().state() == TrayState::Ready);
    (mode, updates)
}

fn render(
    ctx: &egui::Context,
    app: &mut TrontopApp,
    close: bool,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    let mut raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            Vec2::new(1040.0, 640.0),
        )),
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
    // Inspect only. These commands never reach an OS window.
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
fn exits(output: &egui::FullOutput) -> bool {
    commands(output).any(|c| matches!(c, egui::ViewportCommand::Close))
}

fn publish(app: &TrontopApp, cpu: f32) {
    app.tray.as_ref().unwrap().sink().publish(TraySample {
        cpu_percent: cpu,
        memory_percent: 42.0,
        gpu_percent: Some(13.0),
        process_count: 120,
    });
}

fn click_text(ctx: &egui::Context, app: &mut TrontopApp, label: &str) -> egui::FullOutput {
    let mut output = render(ctx, app, false, vec![]);
    for _ in 0..3 {
        output = render(ctx, app, false, vec![]);
    }
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
            false,
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
fn tray_close_os_close_cancels_exit_hides_and_keeps_sampling_without_ui_frames() {
    let ctx = egui::Context::default();
    let mut app = app(ThemeSettings::default(), true);
    let (_, updates) = attach(&mut app, &ctx);
    app.page = Page::Graphs;
    app.query = "retained search".into();
    let output = render(&ctx, &mut app, true, vec![]);
    assert!(hides(&output));
    assert!(commands(&output).any(|c| matches!(c, egui::ViewportCommand::CancelClose)));
    assert!(!exits(&output));
    assert!(app.hidden_to_tray && !app.close_authorized && app.closing_at.is_none());
    for index in 1..=4 {
        publish(&app, index as f32 * 20.0);
        until(|| updates.load(Ordering::Acquire) == index);
    }
    assert_eq!(app.tray.as_ref().unwrap().state(), TrayState::Ready);
    app.tray.as_ref().unwrap().queue_action(TrayAction::Show);
    let output = render(&ctx, &mut app, false, vec![]);
    assert!(shows(&output));
    assert!(commands(&output).any(|c| matches!(c, egui::ViewportCommand::Minimized(false))));
    assert!(commands(&output).any(|c| matches!(c, egui::ViewportCommand::Focus)));
    assert!(!app.hidden_to_tray && app.page == Page::Graphs);
    assert_eq!(app.query, "retained search");
    let output = render(&ctx, &mut app, true, vec![]);
    assert!(hides(&output) && !exits(&output));
}

#[test]
fn tray_close_titlebar_x_uses_the_same_hide_path() {
    let ctx = egui::Context::default();
    let mut app = app(ThemeSettings::default(), true);
    attach(&mut app, &ctx);
    render(&ctx, &mut app, false, vec![]);
    let mut output = render(&ctx, &mut app, false, vec![]);
    // Frameless window button at the titlebar's far right. Context-only input.
    let pos = egui::pos2(1010.0, 22.0);
    for pressed in [true, false] {
        output = render(
            &ctx,
            &mut app,
            false,
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
    assert!(hides(&output) && !exits(&output));
    assert!(app.hidden_to_tray);
}

#[test]
fn tray_close_titlebar_context_menu_quits_even_without_a_tray() {
    let ctx = egui::Context::default();
    let mut app = app(ThemeSettings::default(), true);
    render(&ctx, &mut app, false, vec![]);
    let pos = egui::pos2(1010.0, 22.0);
    for pressed in [true, false] {
        render(
            &ctx,
            &mut app,
            false,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Secondary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    let output = click_text(&ctx, &mut app, "Quit Trontop");
    assert!(exits(&output) && app.close_authorized);
}

#[test]
fn tray_close_about_quit_control_fits_compact_themes_and_scales() {
    for dark in [true, false] {
        for scale in [1.0, 1.5, 2.0] {
            let ctx = egui::Context::default();
            ctx.set_pixels_per_point(scale);
            let settings = ThemeSettings {
                dark,
                ..Default::default()
            };
            theme::install(&ctx, settings);
            let mut app = app(settings, true);
            app.show_diagnostics = true;
            let mut output = render(&ctx, &mut app, false, vec![]);
            for _ in 0..8 {
                output = render(&ctx, &mut app, false, vec![]);
            }
            for label in [
                "Quit Trontop",
                "Copy support report",
                "Copy log location",
                "Third-party licenses",
            ] {
                assert!(
                    text_shapes(&output)
                        .into_iter()
                        .any(|(text, clip)| text.galley.job.text == label
                            && clip.contains_rect(text.visual_bounding_rect())
                            && ctx
                                .content_rect()
                                .contains_rect(text.visual_bounding_rect())),
                    "clipped {label}, dark={dark}, scale={scale}"
                );
            }
        }
    }
}

#[test]
fn tray_close_unavailable_keeps_window_reachable_and_about_quit_exits() {
    for dark in [true, false] {
        let ctx = egui::Context::default();
        let settings = ThemeSettings {
            dark,
            ..Default::default()
        };
        theme::install(&ctx, settings);
        let mut app = app(settings, true);
        let output = render(&ctx, &mut app, true, vec![]);
        assert!(!hides(&output) && !exits(&output));
        assert!(
            app.message
                .as_ref()
                .unwrap()
                .0
                .contains("window stays open")
        );
        assert!(!app.hidden_to_tray && !app.close_authorized);
        app.show_diagnostics = true;
        let output = click_text(&ctx, &mut app, "Quit Trontop");
        assert!(exits(&output) && app.close_authorized);
    }
}

#[test]
#[ignore = "offscreen close-to-tray UI review; no native window or OS input"]
fn render_tray_about_review() {
    let mut renderer = super::offscreen::Renderer::new();
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/ui-smoke/tray-alpha50");
    std::fs::create_dir_all(&directory).unwrap();
    for (name, dark) in [("about-dark", true), ("about-light", false)] {
        let ctx = egui::Context::default();
        let settings = ThemeSettings {
            dark,
            ..Default::default()
        };
        theme::install(&ctx, settings);
        let mut app = app(settings, true);
        app.show_diagnostics = true;
        let mut output = egui::FullOutput::default();
        for _ in 0..8 {
            output.append(render(&ctx, &mut app, false, vec![]));
        }
        renderer.save(
            &ctx,
            output,
            Vec2::new(1040.0, 640.0),
            &directory.join(format!("{name}.png")),
        );
    }
    println!(
        "TRAY_REVIEW: 2 offscreen fixture-only views in {}",
        directory.display()
    );
}

#[test]
fn tray_close_startup_failure_never_hides_or_loses_the_window() {
    let ctx = egui::Context::default();
    let mut app = app(ThemeSettings::default(), true);
    let (release, blocked) = mpsc::channel::<()>();
    app.tray = TrayController::with_backend(ctx.clone(), move |_, _| {
        let _ = blocked.recv_timeout(Duration::from_secs(3));
        None::<FakeTray>
    });
    assert_eq!(app.tray.as_ref().unwrap().state(), TrayState::Starting);
    let output = render(&ctx, &mut app, true, vec![]);
    assert!(!hides(&output) && !exits(&output));
    drop(release);
    until(|| app.tray.as_ref().unwrap().state() == TrayState::Unavailable);
    let output = render(&ctx, &mut app, true, vec![]);
    assert!(!hides(&output) && !exits(&output));
    assert!(!app.hidden_to_tray);
}

#[test]
fn tray_close_icon_failure_or_worker_exit_restores_without_focus_and_can_recover() {
    for mode in [1, 2] {
        let ctx = egui::Context::default();
        let mut app = app(ThemeSettings::default(), true);
        let (status, _) = attach(&mut app, &ctx);
        assert!(hides(&render(&ctx, &mut app, true, vec![])));
        status.store(mode, Ordering::Release);
        publish(&app, 77.0);
        until(|| {
            app.tray.as_ref().unwrap().state()
                == if mode == 1 {
                    TrayState::UpdateFailed
                } else {
                    TrayState::Unavailable
                }
        });
        let output = render(&ctx, &mut app, false, vec![]);
        assert!(shows(&output) && !exits(&output));
        assert!(!commands(&output).any(|c| matches!(c, egui::ViewportCommand::Focus)));
        assert!(!app.hidden_to_tray);
        assert!(!hides(&render(&ctx, &mut app, true, vec![])));
        if mode == 1 {
            status.store(0, Ordering::Release);
            publish(&app, 20.0);
            until(|| app.tray.as_ref().unwrap().state() == TrayState::Ready);
            assert!(hides(&render(&ctx, &mut app, true, vec![])));
        }
    }
}

#[test]
fn tray_close_quit_from_hidden_exits_and_show_cannot_overwrite_pending_quit() {
    let ctx = egui::Context::default();
    let mut app = app(ThemeSettings::default(), true);
    attach(&mut app, &ctx);
    assert!(hides(&render(&ctx, &mut app, true, vec![])));
    let tray = app.tray.as_ref().unwrap();
    tray.queue_action(TrayAction::Quit);
    tray.queue_action(TrayAction::Show);
    let output = render(&ctx, &mut app, false, vec![]);
    assert!(exits(&output) && app.close_authorized);
    assert!(!hides(&output));
    assert!(app.tray.as_ref().unwrap().poll().is_none());
    assert!(!commands(&output).any(|c| matches!(c, egui::ViewportCommand::Focus)));
}

#[test]
fn tray_close_hides_during_slow_save_but_quit_exposes_and_waits_for_save_controls() {
    let ctx = egui::Context::default();
    let mut harness = preferences::install(&ctx, true, "pending");
    let app = harness.app.as_mut().unwrap();
    attach(app, &ctx);
    let output = render(&ctx, app, true, vec![]);
    assert!(hides(&output) && !exits(&output));
    assert!(app.preferences.pending() && app.closing_at.is_none());
    app.tray.as_ref().unwrap().queue_action(TrayAction::Quit);
    let output = render(&ctx, app, false, vec![]);
    assert!(shows(&output) && !exits(&output));
    assert!(!app.hidden_to_tray && app.closing_at.is_some());
    assert!(!commands(&output).any(|c| matches!(c, egui::ViewportCommand::Focus)));
    let output = render(&ctx, app, true, vec![]);
    assert!(!hides(&output) && !exits(&output));
    preferences::release(&mut harness);
    let app = harness.app.as_mut().unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let output = render(&ctx, app, false, vec![]);
        if exits(&output) {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(app.close_authorized && !app.preferences.pending());
}

#[test]
fn tray_close_failed_save_can_keep_open_then_quit_explicitly_without_claiming_saved() {
    let ctx = egui::Context::default();
    let mut harness = preferences::install(&ctx, true, "error");
    let app = harness.app.as_mut().unwrap();
    attach(app, &ctx);
    assert!(hides(&render(&ctx, app, true, vec![])));
    app.tray.as_ref().unwrap().queue_action(TrayAction::Quit);
    assert!(shows(&render(&ctx, app, false, vec![])));
    let output = click_text(&ctx, app, "Keep open");
    assert!(!exits(&output));
    assert!(app.closing_at.is_none() && app.preferences.pending());
    assert!(hides(&render(&ctx, app, true, vec![])));
    app.tray.as_ref().unwrap().queue_action(TrayAction::Quit);
    assert!(shows(&render(&ctx, app, false, vec![])));
    let output = click_text(&ctx, app, "Close anyway");
    assert!(exits(&output) && app.close_authorized);
    assert!(app.preferences.pending());
}
