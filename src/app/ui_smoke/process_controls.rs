use super::*;
use crate::process_actions::{
    Action, Controller,
    tree::{Plan, Scope},
};
use std::sync::mpsc;
use std::time::Duration;

fn setup(settings: ThemeSettings) -> (egui::Context, TrontopApp, mpsc::Receiver<Action>) {
    let ctx = egui::Context::default();
    theme::install(&ctx, settings);
    let mut app = app(settings, true);
    app.page = Page::Processes;
    app.selected_pid = Some(900_001);
    app.snapshot.processes[2].parent_pid = Some(900_001);
    let (sender, receiver) = mpsc::channel();
    app.process_actions = Controller::with_backend(ctx.clone(), move |action| {
        sender.send(action.clone()).unwrap();
        Ok(())
    });
    (ctx, app, receiver)
}

#[test]
fn tree_and_all_instances_confirm_frozen_targets_after_selection_changes() {
    for all in [false, true] {
        let (ctx, mut app, received) = setup(ThemeSettings::default());
        let size = Vec2::new(1040.0, 640.0);
        let root = app.selected_process().unwrap().identity().unwrap();
        click_local_text(&ctx, &mut app, size, "End process tree");
        assert!(app.pending_end_tree.is_some());
        assert!(received.try_recv().is_err(), "opening must never terminate");
        if all {
            click_local_text(&ctx, &mut app, size, "End all instances of this executable");
        }
        let expected = Plan::build(
            &app.snapshot.processes,
            root,
            if all {
                Scope::AllInstances
            } else {
                Scope::SelectedTree
            },
        )
        .unwrap();
        assert_eq!(expected.targets().len(), if all { 17 } else { 2 });
        app.selected_pid = Some(900_003);
        click_local_text(&ctx, &mut app, size, "End listed processes");
        assert_eq!(
            received.recv_timeout(Duration::from_secs(3)).unwrap(),
            Action::EndTree(expected)
        );
        assert!(app.pending_end_tree.is_none());
        assert!(received.try_recv().is_err());
    }
}

#[test]
fn changed_tree_requires_review_refresh_and_cannot_follow_reused_root() {
    let (ctx, mut app, received) = setup(ThemeSettings::default());
    let size = Vec2::new(1040.0, 640.0);
    app.request_end_tree_selected();
    app.snapshot.processes[3].parent_pid = Some(900_001);
    click_local_text(&ctx, &mut app, size, "End listed processes");
    assert!(received.try_recv().is_err());
    click_local_text(&ctx, &mut app, size, "Refresh targets");
    assert!(received.try_recv().is_err());
    click_local_text(&ctx, &mut app, size, "End listed processes");
    let Action::EndTree(plan) = received.recv_timeout(Duration::from_secs(3)).unwrap() else {
        panic!("wrong action");
    };
    assert_eq!(plan.targets().len(), 3);

    let (ctx, mut app, received) = setup(ThemeSettings::default());
    app.request_end_tree_selected();
    app.snapshot.processes[1].control.created_at_100ns = Some(2);
    click_local_text(&ctx, &mut app, size, "Refresh targets");
    // Invalid plan presents its error and omits termination controls.
    let output = frame(&ctx, &mut app, size, vec![]);
    assert!(
        !text_shapes(&output)
            .iter()
            .any(|(text, _)| text.galley.job.text == "3 processes in the reviewed list")
    );
    assert!(received.try_recv().is_err());
    click_local_text(&ctx, &mut app, size, "Cancel");
    assert!(app.pending_end_tree.is_none());
}

#[test]
fn suspend_and_resume_require_confirmation_and_use_original_identity() {
    for suspend in [true, false] {
        let (ctx, mut app, received) = setup(ThemeSettings::default());
        let size = Vec2::new(1040.0, 640.0);
        let original = app.selected_process().unwrap().identity().unwrap();
        click_local_text(
            &ctx,
            &mut app,
            size,
            if suspend { "Suspend" } else { "Resume" },
        );
        assert!(app.pending_control_action.is_some());
        assert!(received.try_recv().is_err());
        app.selected_pid = Some(900_002);
        click_local_text(
            &ctx,
            &mut app,
            size,
            if suspend {
                "Suspend process"
            } else {
                "Resume process"
            },
        );
        assert_eq!(
            received.recv_timeout(Duration::from_secs(3)).unwrap(),
            if suspend {
                Action::Suspend(original)
            } else {
                Action::Resume(original)
            }
        );
    }
}

#[test]
fn tree_dialog_fits_compact_dark_and_light_and_cancel_sends_no_action() {
    for (dark, all) in [(true, false), (true, true), (false, false), (false, true)] {
        for size in [Vec2::new(1000.0, 580.0), Vec2::new(1440.0, 900.0)] {
            let settings = ThemeSettings {
                dark,
                ..Default::default()
            };
            let (ctx, mut app, received) = setup(settings);
            app.selected_pid = Some(900_000);
            app.request_end_tree_selected();
            if all {
                click_local_text(&ctx, &mut app, size, "End all instances of this executable");
            }
            for _ in 0..10 {
                frame(&ctx, &mut app, size, vec![]);
            }
            let output = frame(&ctx, &mut app, size, vec![]);
            for label in [
                "Confirm end process tree",
                "64 processes in the reviewed list",
                "Refresh targets",
                "End listed processes",
                "Cancel",
            ] {
                let (text, clip) = text_shapes(&output)
                    .into_iter()
                    .find(|(text, _)| text.galley.job.text == label)
                    .unwrap_or_else(|| panic!("missing {label}"));
                assert!(
                    clip.contains_rect(text.visual_bounding_rect()),
                    "clipped {label} at {size}"
                );
                assert!(
                    egui::Rect::from_min_size(egui::Pos2::ZERO, size)
                        .contains_rect(text.visual_bounding_rect()),
                    "outside viewport {label}"
                );
            }
            click_local_text(&ctx, &mut app, size, "Cancel");
            assert!(app.pending_end_tree.is_none());
            assert!(received.try_recv().is_err());
        }
    }
}

#[test]
fn tree_confirmation_stays_pending_during_busy_worker_and_graphics_recovery() {
    let (ctx, mut app, received) = setup(ThemeSettings::default());
    let size = Vec2::new(1000.0, 580.0);
    app.request_end_tree_selected();
    app.graphics_recovering
        .store(true, std::sync::atomic::Ordering::Release);
    frame(&ctx, &mut app, size, vec![]);
    assert!(received.try_recv().is_err());
    assert!(app.pending_end_tree.is_some());
    app.graphics_recovering
        .store(false, std::sync::atomic::Ordering::Release);
    let release = actions::install_pending(&mut app, &ctx, false);
    click_local_text(&ctx, &mut app, size, "End listed processes");
    assert!(app.pending_end_tree.is_some());
    assert!(app.process_actions.busy());
    assert!(matches!(
        app.process_actions.active().unwrap().action,
        Action::Launch(_)
    ));
    click_local_text(&ctx, &mut app, size, "Cancel");
    assert!(app.pending_end_tree.is_none());
    release.send(()).unwrap();
}

#[test]
#[ignore = "Offscreen process-control review; no windows, desktop input or native mutations"]
fn render_process_control_review() {
    let mut renderer = offscreen::Renderer::new();
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/ui-smoke/process-controls-alpha43");
    std::fs::create_dir_all(&directory).unwrap();
    for (name, dark, all, dialog) in [
        ("tree-compact-dark", true, false, true),
        ("all-compact-light", false, true, true),
        ("inspector-compact-dark", true, false, false),
    ] {
        let (ctx, mut app, _received) = setup(ThemeSettings {
            dark,
            ..Default::default()
        });
        let size = Vec2::new(1000.0, 580.0);
        let mut output = egui::FullOutput::default();
        if dialog {
            app.selected_pid = Some(900_000);
            app.request_end_tree_selected();
        }
        for _ in 0..15 {
            output.append(frame(&ctx, &mut app, size, vec![]));
        }
        if all {
            output.append(click_local_text_output(
                &ctx,
                &mut app,
                size,
                "End all instances of this executable",
            ));
        }
        for _ in 0..5 {
            output.append(frame(&ctx, &mut app, size, vec![]));
        }
        renderer.save(&ctx, output, size, &directory.join(format!("{name}.png")));
    }
    println!(
        "PROCESS_CONTROLS: 3 offscreen review images in {}",
        directory.display()
    );
}
