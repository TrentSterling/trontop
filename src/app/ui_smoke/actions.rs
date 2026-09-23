use super::*;
use crate::process_actions::{Action, Controller, Request};
use std::sync::mpsc;
use std::time::{Duration, Instant};

pub(super) fn install_pending(
    app: &mut TrontopApp,
    ctx: &egui::Context,
    slow: bool,
) -> mpsc::Sender<()> {
    let (release, blocked) = mpsc::channel();
    let (entered, started) = mpsc::channel();
    app.process_actions = Controller::with_backend(ctx.clone(), move |_| {
        let _ = entered.send(());
        let _ = blocked.recv();
        Ok(())
    });
    let mut request = Request::new(
        Action::Launch("fixture only".into()),
        "Fixture render worker".into(),
    );
    if slow {
        request.confirmed_at = Instant::now() - Duration::from_secs(6);
    }
    app.process_actions.submit(request).unwrap();
    started.recv_timeout(Duration::from_secs(3)).unwrap();
    release
}

#[test]
fn process_action_confirmation_is_nonblocking_and_keeps_its_original_target() {
    let ctx = egui::Context::default();
    let settings = ThemeSettings::default();
    theme::install(&ctx, settings);
    let mut app = app(settings, true);
    let size = Vec2::new(1040.0, 640.0);
    let (sent, actions) = mpsc::channel();
    let (release, blocked) = mpsc::channel();
    app.process_actions = Controller::with_backend(ctx.clone(), move |action| {
        sent.send(action.clone()).unwrap();
        let _ = blocked.recv();
        Ok(())
    });
    app.selected_pid = Some(900_001);
    let original = app.selected_process().unwrap().clone();
    app.request_end_selected();
    click_local_text(&ctx, &mut app, size, "End process");
    assert_eq!(
        actions.recv_timeout(Duration::from_secs(3)).unwrap(),
        Action::End(original.identity().unwrap())
    );
    assert!(app.pending_end_task.is_none());
    assert!(app.process_actions.busy());
    assert!(
        app.message.is_none(),
        "must not claim success before worker result"
    );
    app.selected_pid = Some(900_002);
    app.request_end_selected();
    assert!(
        app.pending_end_task.is_none(),
        "busy worker cannot accept a duplicate confirmation"
    );
    for (page, _, label) in Page::ALL {
        // Ten navigation entries plus the pending-action footer legitimately
        // need scrolling at minimum size. Exercise that real local scroll path.
        let output = frame(&ctx, &mut app, size, vec![]);
        if nav_entry(&output, label).is_none() {
            frame(
                &ctx,
                &mut app,
                size,
                vec![
                    egui::Event::PointerMoved(egui::pos2(90.0, 240.0)),
                    egui::Event::MouseWheel {
                        phase: egui::TouchPhase::Move,
                        unit: egui::MouseWheelUnit::Point,
                        delta: Vec2::new(0.0, -200.0),
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
            for _ in 0..30 {
                frame(&ctx, &mut app, size, vec![]);
            }
        }
        let output = frame(&ctx, &mut app, size, vec![]);
        let position = nav_entry(&output, label).expect("visible nav entry");
        for pressed in [true, false] {
            frame(
                &ctx,
                &mut app,
                size,
                vec![
                    egui::Event::PointerMoved(position),
                    egui::Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        assert!(app.page == page, "navigation blocked on {label}");
        assert!(app.process_actions.busy());
        frame(&ctx, &mut app, size, vec![]);
    }
    assert!(actions.try_recv().is_err());
    release.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while app.process_actions.busy() {
        frame(&ctx, &mut app, size, vec![]);
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    let (message, error) = app.message.as_ref().unwrap();
    assert!(!error);
    assert!(message.contains(&format!("{} ({})", original.name, original.pid)));
    assert!(message.contains("Request accepted"));
    assert_eq!(app.selected_pid, Some(900_002));
}

#[test]
fn process_action_stale_confirmations_cannot_reach_injected_backend() {
    for control in [
        None,
        Some(PendingControlAction::Priority {
            identity: ProcessIdentity {
                pid: 900_001,
                created_at_100ns: 1,
            },
            priority: PriorityClass::BelowNormal,
        }),
        Some(PendingControlAction::Affinity {
            identity: ProcessIdentity {
                pid: 900_001,
                created_at_100ns: 1,
            },
            affinity_mask: 1,
        }),
    ] {
        let ctx = egui::Context::default();
        let settings = ThemeSettings::default();
        theme::install(&ctx, settings);
        let mut app = app(settings, true);
        app.process_actions =
            Controller::with_backend(ctx.clone(), |_| panic!("stale action executed"));
        app.selected_pid = Some(900_001);
        let label = match control {
            None => {
                app.request_end_selected();
                "End process"
            }
            Some(PendingControlAction::Priority { .. }) => {
                app.pending_control_action = control;
                "Apply priority"
            }
            Some(PendingControlAction::Affinity { .. }) => {
                app.pending_control_action = control;
                "Apply affinity"
            }
        };
        app.snapshot.processes[1].control.created_at_100ns = None;
        click_local_text(&ctx, &mut app, Vec2::new(1040.0, 640.0), label);
        assert!(!app.process_actions.busy());
        assert!(app.process_actions.ready());
    }
}

#[test]
fn process_action_enter_obeys_busy_empty_and_recovery_guards() {
    let ctx = egui::Context::default();
    let settings = ThemeSettings::default();
    theme::install(&ctx, settings);
    let mut app = app(settings, true);
    let size = Vec2::new(1040.0, 640.0);
    let release = install_pending(&mut app, &ctx, false);
    app.show_run_task = true;
    app.run_command = "duplicate fixture".into();
    click_local_text(&ctx, &mut app, size, "Run");
    frame(
        &ctx,
        &mut app,
        size,
        vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert_eq!(app.run_command, "duplicate fixture");
    assert!(app.show_run_task);
    assert_eq!(
        app.process_actions.active().unwrap().target,
        "Fixture render worker"
    );
    drop(release);
    app.process_actions =
        Controller::with_backend(ctx.clone(), |_| panic!("guarded action executed"));
    app.run_command.clear();
    frame(
        &ctx,
        &mut app,
        size,
        vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert!(!app.process_actions.busy());
    app.graphics_recovering
        .store(true, std::sync::atomic::Ordering::Release);
    assert!(!app.submit_process_action(&ctx, Action::Launch("guarded".into()), "guarded".into()));
    assert!(!app.process_actions.busy());
}

#[test]
fn process_action_pending_slow_and_long_error_bars_keep_stable_geometry() {
    for dark in [false, true] {
        let settings = ThemeSettings {
            dark,
            ..Default::default()
        };
        let ctx = egui::Context::default();
        theme::install(&ctx, settings);
        let size = Vec2::new(1040.0, 640.0);
        let mut prior = None;
        for state in 0..3 {
            let mut app = app(settings, true);
            let _release = if state < 2 {
                Some(install_pending(&mut app, &ctx, state == 1))
            } else {
                app.message = Some((
                    "Long Windows error for FixtureExecutable.exe ".repeat(50),
                    true,
                ));
                None
            };
            for _ in 0..4 {
                frame(&ctx, &mut app, size, vec![]);
            }
            let output = frame(&ctx, &mut app, size, vec![]);
            let (text, clip) = text_shapes(&output)
                .into_iter()
                .find(|(text, _)| {
                    let value = &text.galley.job.text;
                    value.starts_with("Working:")
                        || value.starts_with("Still waiting")
                        || value.starts_with("Long Windows error")
                })
                .unwrap();
            let rect = text.visual_bounding_rect();
            assert_eq!(text.galley.rows.len(), 1);
            assert!(clip.expand(0.5).contains_rect(rect));
            assert!(rect.bottom() < size.y);
            if let Some(y) = prior {
                assert_eq!(rect.center().y, y);
            }
            prior = Some(rect.center().y);
            if state == 2 {
                let dismiss = text_shapes(&output)
                    .into_iter()
                    .find(|(text, _)| text.galley.job.text == "Dismiss")
                    .unwrap()
                    .0
                    .visual_bounding_rect();
                assert!(rect.right() <= dismiss.left());
            }
        }
    }
}
