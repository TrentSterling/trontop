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
fn overview_end_actions_confirm_and_complete_without_navigation() {
    for dark in [true, false] {
        for scope in 0..3 {
            let (ctx, mut app, received) = setup(ThemeSettings {
                dark,
                ..Default::default()
            });
            let size = Vec2::new(1100.0, 900.0);
            app.page = Page::Overview;
            app.inspector_visible = true;
            let root = app.selected_process().unwrap().identity().unwrap();
            let expected = match scope {
                0 => Action::End(root),
                1 => Action::EndTree(
                    Plan::build(&app.snapshot.processes, root, Scope::SelectedTree).unwrap(),
                ),
                _ => Action::EndTree(
                    Plan::build(&app.snapshot.processes, root, Scope::AllInstances).unwrap(),
                ),
            };
            if scope == 2 {
                click_local_text(&ctx, &mut app, size, "End all instances");
            } else {
                click_local_text(&ctx, &mut app, size, "More process actions");
                click_local_text(
                    &ctx,
                    &mut app,
                    size,
                    if scope == 0 {
                        "End this process"
                    } else {
                        "End this branch"
                    },
                );
            }
            assert_eq!(app.page, Page::Overview);
            assert!(received.try_recv().is_err());
            click_local_text(
                &ctx,
                &mut app,
                size,
                match scope {
                    0 => "End process",
                    1 => "End 2 processes",
                    _ => "End 17 processes",
                },
            );
            assert_eq!(
                received.recv_timeout(Duration::from_secs(3)).unwrap(),
                expected
            );
            frame(&ctx, &mut app, size, vec![]);
            assert_eq!(app.page, Page::Overview);
            assert!(app.pending_end_task.is_none() && app.pending_end_tree.is_none());
        }
    }
}

#[test]
fn overview_cancel_stays_put_and_details_navigation_is_explicit() {
    let (ctx, mut app, received) = setup(ThemeSettings::default());
    let size = Vec2::new(1100.0, 900.0);
    app.page = Page::Overview;
    app.inspector_visible = true;
    click_local_text(&ctx, &mut app, size, "End all instances");
    click_local_text(&ctx, &mut app, size, "Cancel");
    assert_eq!(app.page, Page::Overview);
    assert!(received.try_recv().is_err());
    click_local_text(&ctx, &mut app, size, "Open in Details");
    assert_eq!(app.page, Page::Details);
}

#[test]
fn tree_and_all_instances_confirm_frozen_targets_after_selection_changes() {
    for all in [false, true] {
        let (ctx, mut app, received) = setup(ThemeSettings::default());
        let size = Vec2::new(1040.0, 640.0);
        let root = app.selected_process().unwrap().identity().unwrap();
        if all {
            click_local_text(&ctx, &mut app, size, "End all instances");
        } else {
            click_local_text(&ctx, &mut app, size, "More process actions");
            click_local_text(&ctx, &mut app, size, "End this branch");
        }
        assert!(app.pending_end_tree.is_some());
        assert!(received.try_recv().is_err(), "opening must never terminate");
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
        click_local_text(
            &ctx,
            &mut app,
            size,
            &format!("End {} processes", expected.targets().len()),
        );
        assert_eq!(
            received.recv_timeout(Duration::from_secs(3)).unwrap(),
            Action::EndTree(expected)
        );
        assert!(app.pending_end_tree.is_none());
        assert!(received.try_recv().is_err());
    }
}

#[test]
fn end_all_is_direct_with_inspector_hidden_on_both_process_pages() {
    for page in [Page::Processes, Page::Details] {
        let (ctx, mut app, received) = setup(ThemeSettings::default());
        let size = Vec2::new(1040.0, 640.0);
        app.page = page;
        app.inspector_visible = false;
        let root = app.selected_process().unwrap().identity().unwrap();
        let expected = Plan::build(&app.snapshot.processes, root, Scope::AllInstances).unwrap();
        let output = frame(&ctx, &mut app, size, vec![]);
        assert!(
            text_shapes(&output).iter().any(|(text, clip)| {
                text.galley.job.text == "End all instances"
                    && clip.contains_rect(text.visual_bounding_rect())
            }),
            "primary action must keep its label at compact sizes"
        );
        click_local_text(&ctx, &mut app, size, "End all instances");
        assert!(received.try_recv().is_err());
        assert!(app.pending_end_task.is_none());
        click_local_text(&ctx, &mut app, size, "End 17 processes");
        assert_eq!(
            received.recv_timeout(Duration::from_secs(3)).unwrap(),
            Action::EndTree(expected)
        );
        assert!(received.try_recv().is_err());
    }
}

#[test]
fn granular_menu_keeps_single_process_confirmation_available() {
    let (ctx, mut app, received) = setup(ThemeSettings::default());
    let size = Vec2::new(1040.0, 640.0);
    let root = app.selected_process().unwrap().identity().unwrap();
    click_local_text(&ctx, &mut app, size, "More process actions");
    click_local_text(&ctx, &mut app, size, "End this process");
    assert!(app.pending_end_task.is_some());
    assert!(app.pending_end_tree.is_none());
    assert!(received.try_recv().is_err());
    click_local_text(&ctx, &mut app, size, "End process");
    assert_eq!(
        received.recv_timeout(Duration::from_secs(3)).unwrap(),
        Action::End(root)
    );
    assert!(received.try_recv().is_err());
}

#[test]
fn end_all_with_unknown_path_explains_failure_without_falling_back_to_one_pid() {
    let (ctx, mut app, received) = setup(ThemeSettings::default());
    let size = Vec2::new(1040.0, 640.0);
    app.snapshot.processes[1].executable = None;
    click_local_text(&ctx, &mut app, size, "End all instances");
    let output = frame(&ctx, &mut app, size, vec![]);
    assert!(text_shapes(&output).iter().any(|(text, _)| {
        text.galley
            .job
            .text
            .contains("All instances cannot be identified safely")
    }));
    click_local_text(&ctx, &mut app, size, "End processes");
    assert!(received.try_recv().is_err());
    assert!(app.pending_end_task.is_none());
    click_local_text(&ctx, &mut app, size, "Cancel");
    assert!(app.pending_end_tree.is_none());
}

#[test]
fn changed_tree_requires_review_refresh_and_cannot_follow_reused_root() {
    let (ctx, mut app, received) = setup(ThemeSettings::default());
    let size = Vec2::new(1040.0, 640.0);
    app.request_end_tree_selected();
    app.snapshot.processes[3].parent_pid = Some(900_001);
    click_local_text(&ctx, &mut app, size, "End 2 processes");
    assert!(received.try_recv().is_err());
    click_local_text(&ctx, &mut app, size, "Refresh targets");
    assert!(received.try_recv().is_err());
    click_local_text(&ctx, &mut app, size, "End 3 processes");
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
            .any(|(text, _)| text.galley.job.text == "End 3 processes?")
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
            if all {
                app.request_end_all_selected();
            } else {
                app.request_end_tree_selected();
            }
            click_local_text(&ctx, &mut app, size, "Review processes");
            for _ in 0..10 {
                frame(&ctx, &mut app, size, vec![]);
            }
            let output = frame(&ctx, &mut app, size, vec![]);
            for label in [
                if all {
                    "Confirm end all instances"
                } else {
                    "Confirm end this branch"
                },
                "End 64 processes?",
                "Refresh targets",
                "End 64 processes",
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
    click_local_text(&ctx, &mut app, size, "End 2 processes");
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
        .join("target/ui-smoke/process-controls-alpha53");
    std::fs::create_dir_all(&directory).unwrap();
    for (name, dark, all, dialog) in [
        ("all-compact-dark", true, true, true),
        ("all-compact-light", false, true, true),
        ("branch-compact-dark", true, false, true),
        ("inspector-compact-dark", true, false, false),
        ("overview-dark", true, false, false),
        ("overview-light-confirm", false, true, true),
    ] {
        let (ctx, mut app, _received) = setup(ThemeSettings {
            dark,
            ..Default::default()
        });
        let size = Vec2::new(1000.0, 580.0);
        if name.starts_with("overview") {
            app.page = Page::Overview;
            app.inspector_visible = true;
        }
        let mut output = egui::FullOutput::default();
        if dialog {
            app.selected_pid = Some(900_000);
            if all {
                app.request_end_all_selected();
            } else {
                app.request_end_tree_selected();
            }
        }
        for _ in 0..15 {
            output.append(frame(&ctx, &mut app, size, vec![]));
        }
        for _ in 0..5 {
            output.append(frame(&ctx, &mut app, size, vec![]));
        }
        renderer.save(&ctx, output, size, &directory.join(format!("{name}.png")));
    }
    println!(
        "PROCESS_CONTROLS: 6 offscreen review images in {}",
        directory.display()
    );
}
