//! Real UI event paths with fixture-only workers. No desktop input or OS actions.
use super::*;
use crate::process_actions::{Action, Controller, Request};
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

fn setup() -> (egui::Context, TrontopApp, mpsc::Receiver<Action>) {
    let ctx = egui::Context::default();
    let settings = ThemeSettings::default();
    theme::install(&ctx, settings);
    let mut app = app(settings, true);
    app.selected_pid = Some(900_001);
    app.sort_column = SortColumn::Pid;
    app.sort_direction = SortDirection::Ascending;
    app.rebuild_visible_processes();
    let (sent, received) = mpsc::channel();
    app.process_actions = Controller::with_backend(ctx.clone(), move |action| {
        sent.send(action.clone()).unwrap();
        Ok(())
    });
    (ctx, app, received)
}
fn settle_action(ctx: &egui::Context, app: &mut TrontopApp, size: Vec2) {
    let start = Instant::now();
    while app.process_actions.busy() {
        frame(ctx, app, size, vec![]);
        assert!(start.elapsed() < Duration::from_secs(3));
        std::thread::sleep(Duration::from_millis(2));
    }
}
#[test]
fn verified_own_suspension_updates_inspector_and_details_without_attaching_to_reused_pid() {
    let (ctx, mut app, received) = setup();
    let size = Vec2::new(1600.0, 1000.0);
    let identity = app.selected_process().unwrap().identity().unwrap();
    app.process_actions
        .submit(Request::new(Action::Suspend(identity), "fixture".into()))
        .unwrap();
    assert_eq!(
        received.recv_timeout(Duration::from_secs(3)).unwrap(),
        Action::Suspend(identity)
    );
    settle_action(&ctx, &mut app, size);
    assert!(app.process_actions.holds_suspension(identity));
    for page in [Page::Processes, Page::Details] {
        app.page = page;
        app.inspector_visible = page == Page::Processes;
        let output = frame(&ctx, &mut app, size, vec![]);
        assert!(
            text_shapes(&output)
                .iter()
                .any(|(text, _)| text.galley.job.text == "Suspended"),
            "{page:?} must report the verified own hold"
        );
    }
    let process = app
        .snapshot
        .processes
        .iter_mut()
        .find(|process| process.pid == identity.pid)
        .unwrap();
    process.control.created_at_100ns = Some(identity.created_at_100ns + 1);
    let output = frame(&ctx, &mut app, size, vec![]);
    assert!(
        !text_shapes(&output)
            .iter()
            .any(|(text, _)| text.galley.job.text == "Suspended"),
        "reused PID must not inherit another process's suspension"
    );
    app.snapshot.processes[1].control.created_at_100ns = Some(identity.created_at_100ns);
    app.process_actions
        .submit(Request::new(Action::Resume(identity), "fixture".into()))
        .unwrap();
    assert_eq!(
        received.recv_timeout(Duration::from_secs(3)).unwrap(),
        Action::Resume(identity)
    );
    settle_action(&ctx, &mut app, size);
    assert!(!app.process_actions.holds_suspension(identity));
    let output = frame(&ctx, &mut app, size, vec![]);
    assert!(
        !text_shapes(&output)
            .iter()
            .any(|(text, _)| text.galley.job.text == "Suspended")
    );
}

#[test]
fn failed_suspend_does_not_change_state_and_failed_resume_retains_verified_hold() {
    for fail_suspend in [true, false] {
        let (ctx, mut app, _) = setup();
        app.process_actions = Controller::with_backend(ctx.clone(), move |action| {
            if matches!(action, Action::Suspend(_)) == fail_suspend {
                Err("Fixture refusal".into())
            } else {
                Ok(())
            }
        });
        let size = Vec2::new(1600.0, 1000.0);
        let identity = app.selected_process().unwrap().identity().unwrap();
        app.process_actions
            .submit(Request::new(Action::Suspend(identity), "fixture".into()))
            .unwrap();
        settle_action(&ctx, &mut app, size);
        assert_eq!(
            app.process_actions.holds_suspension(identity),
            !fail_suspend
        );
        if !fail_suspend {
            app.process_actions
                .submit(Request::new(Action::Resume(identity), "fixture".into()))
                .unwrap();
            settle_action(&ctx, &mut app, size);
        }
        let output = frame(&ctx, &mut app, size, vec![]);
        assert_eq!(
            text_shapes(&output)
                .iter()
                .any(|(text, _)| text.galley.job.text == "Suspended"),
            !fail_suspend
        );
        let (message, failed) = app.message.as_ref().unwrap();
        assert!(*failed);
        assert!(message.contains("Fixture refusal"));
    }
}

#[test]
fn details_state_sort_follows_verified_suspend_and_resume_without_waiting_for_sample() {
    let (ctx, mut app, received) = setup();
    let size = Vec2::new(1600.0, 1000.0);
    app.page = Page::Details;
    app.inspector_visible = false;
    app.sort_by(SortColumn::Status);
    // A second click selects descending order: Suspended before Running.
    app.sort_by(SortColumn::Status);
    let identity = app.selected_process().unwrap().identity().unwrap();
    for action in [Action::Suspend(identity), Action::Resume(identity)] {
        app.process_actions
            .submit(Request::new(action.clone(), "fixture".into()))
            .unwrap();
        assert_eq!(
            received.recv_timeout(Duration::from_secs(3)).unwrap(),
            action
        );
        settle_action(&ctx, &mut app, size);
        let first_pid = app.snapshot.processes[app.visible_processes[0]].pid;
        assert_eq!(
            first_pid,
            if matches!(action, Action::Suspend(_)) {
                identity.pid
            } else {
                900_063
            }
        );
    }
}

#[test]
fn compact_details_reads_suspended_state_in_full_in_dark_and_light() {
    for dark in [true, false] {
        let (ctx, mut app, received) = setup();
        app.theme.dark = dark;
        theme::install(&ctx, app.theme);
        app.page = Page::Details;
        app.inspector_visible = false;
        let size = Vec2::new(1000.0, 580.0);
        let identity = app.selected_process().unwrap().identity().unwrap();
        app.process_actions
            .submit(Request::new(Action::Suspend(identity), "fixture".into()))
            .unwrap();
        assert_eq!(
            received.recv_timeout(Duration::from_secs(3)).unwrap(),
            Action::Suspend(identity)
        );
        settle_action(&ctx, &mut app, size);
        let output = frame(&ctx, &mut app, size, vec![]);
        let (text, clip) = text_shapes(&output)
            .into_iter()
            .find(|(text, _)| text.galley.job.text == "Suspended")
            .unwrap();
        assert!(
            !text.galley.elided,
            "Suspended must remain readable at the minimum window width"
        );
        assert!(clip.contains_rect(text.visual_bounding_rect()));
    }
}

#[test]
fn priority_editor_reviews_and_confirms_original_process_without_realtime_action() {
    for priority in [PriorityClass::BelowNormal, PriorityClass::High] {
        let (ctx, mut app, received) = setup();
        let size = Vec2::new(1600.0, 1200.0);
        let original = app.selected_process().unwrap().identity().unwrap();
        click_local_text(&ctx, &mut app, size, "Set priority");
        assert!(app.show_priority_editor);
        let output = frame(&ctx, &mut app, size, vec![]);
        assert!(
            !text_shapes(&output)
                .iter()
                .any(|(text, _)| text.galley.job.text == "Realtime")
        );
        click_local_text(&ctx, &mut app, size, "Normal  |  CURRENT");
        assert!(app.pending_control_action.is_none());
        assert!(received.try_recv().is_err());
        click_local_text(&ctx, &mut app, size, priority.label());
        assert!(!app.show_priority_editor);
        assert!(app.pending_control_action.is_some());
        assert!(received.try_recv().is_err());
        app.selected_pid = Some(900_002);
        click_local_text(&ctx, &mut app, size, "Apply priority");
        assert_eq!(
            received.recv_timeout(Duration::from_secs(3)).unwrap(),
            Action::Priority(original, priority)
        );
        settle_action(&ctx, &mut app, size);
        assert!(app.pending_control_action.is_none());
        assert!(received.try_recv().is_err());
    }
}

#[test]
fn affinity_editor_reviews_exact_mask_and_preserves_original_process() {
    let (ctx, mut app, received) = setup();
    let size = Vec2::new(1600.0, 1200.0);
    let original = app.selected_process().unwrap().identity().unwrap();
    click_local_text(&ctx, &mut app, size, "CPU affinity");
    assert!(app.show_affinity_editor);
    click_local_text(&ctx, &mut app, size, "CPU 00");
    assert_eq!(app.affinity_draft, 0xfe);
    click_local_text(&ctx, &mut app, size, "Current mask");
    assert_eq!(app.affinity_draft, 0xff);
    click_local_text(&ctx, &mut app, size, "CPU 01");
    click_local_text(&ctx, &mut app, size, "Select all");
    assert_eq!(app.affinity_draft, 0xff);
    click_local_text(&ctx, &mut app, size, "CPU 00");
    click_local_text(&ctx, &mut app, size, "Review change");
    assert!(!app.show_affinity_editor);
    assert!(app.pending_control_action.is_some());
    assert!(received.try_recv().is_err());
    app.selected_pid = Some(900_002);
    click_local_text(&ctx, &mut app, size, "Apply affinity");
    assert_eq!(
        received.recv_timeout(Duration::from_secs(3)).unwrap(),
        Action::Affinity(original, 0xfe)
    );
    settle_action(&ctx, &mut app, size);
    assert!(app.pending_control_action.is_none());
    assert!(received.try_recv().is_err());
}

#[test]
fn affinity_editor_refuses_empty_out_of_group_and_unchanged_masks() {
    for mask in [0, 0x100, 0xff] {
        let (ctx, mut app, received) = setup();
        let size = Vec2::new(1600.0, 1200.0);
        click_local_text(&ctx, &mut app, size, "CPU affinity");
        app.affinity_draft = mask;
        click_local_text(&ctx, &mut app, size, "Review change");
        assert!(app.show_affinity_editor);
        assert!(app.pending_control_action.is_none());
        assert!(received.try_recv().is_err());
        click_local_text(&ctx, &mut app, size, "Cancel");
        assert!(!app.show_affinity_editor);
    }
}

#[test]
fn scheduling_editors_refuse_identity_lost_after_opening() {
    for priority in [true, false] {
        let (ctx, mut app, received) = setup();
        let size = Vec2::new(1600.0, 1200.0);
        click_local_text(
            &ctx,
            &mut app,
            size,
            if priority {
                "Set priority"
            } else {
                "CPU affinity"
            },
        );
        app.snapshot.processes[1].control.created_at_100ns = None;
        if priority {
            click_local_text(&ctx, &mut app, size, "Below normal");
        } else {
            click_local_text(&ctx, &mut app, size, "CPU 00");
            click_local_text(&ctx, &mut app, size, "Review change");
        }
        assert!(!app.show_priority_editor && !app.show_affinity_editor);
        assert!(app.pending_control_action.is_none());
        assert!(received.try_recv().is_err());
        assert!(app.message.as_ref().unwrap().1);
    }
}

fn key_event(key: egui::Key, modifiers: egui::Modifiers, pressed: bool) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers,
    }
}

#[test]
fn run_task_accepts_typed_command_once_via_button_or_enter_without_native_launch() {
    for enter in [true, false] {
        let (ctx, mut app, received) = setup();
        let size = Vec2::new(1040.0, 640.0);
        frame(
            &ctx,
            &mut app,
            size,
            vec![key_event(egui::Key::R, egui::Modifiers::CTRL, true)],
        );
        frame(
            &ctx,
            &mut app,
            size,
            vec![key_event(egui::Key::R, egui::Modifiers::CTRL, false)],
        );
        assert!(app.show_run_task);
        frame(
            &ctx,
            &mut app,
            size,
            vec![egui::Event::Text("  fixture-only.exe --alpha  ".into())],
        );
        assert_eq!(app.run_command, "  fixture-only.exe --alpha  ");
        assert!(received.try_recv().is_err());
        if enter {
            frame(
                &ctx,
                &mut app,
                size,
                vec![key_event(egui::Key::Enter, egui::Modifiers::NONE, true)],
            );
            frame(
                &ctx,
                &mut app,
                size,
                vec![key_event(egui::Key::Enter, egui::Modifiers::NONE, false)],
            );
        } else {
            click_local_text(&ctx, &mut app, size, "Run");
        }
        assert_eq!(
            received.recv_timeout(Duration::from_secs(3)).unwrap(),
            Action::Launch("fixture-only.exe --alpha".into())
        );
        settle_action(&ctx, &mut app, size);
        assert!(!app.show_run_task);
        assert!(app.run_command.is_empty());
        assert!(received.try_recv().is_err());
    }
}

#[test]
fn reveal_queues_selected_executable_and_reports_worker_refusal() {
    let (ctx, mut app, _) = setup();
    let (sent, received) = mpsc::channel();
    app.process_actions = Controller::with_backend(ctx.clone(), move |action| {
        sent.send(action.clone()).unwrap();
        Err("Fixture executable disappeared".into())
    });
    let expected = app.selected_process().unwrap().executable.clone().unwrap();
    let size = Vec2::new(1600.0, 1200.0);
    click_local_text(&ctx, &mut app, size, "Reveal in Explorer");
    assert_eq!(
        received.recv_timeout(Duration::from_secs(3)).unwrap(),
        Action::Reveal(expected)
    );
    settle_action(&ctx, &mut app, size);
    let (message, failed) = app.message.as_ref().unwrap();
    assert!(*failed);
    assert!(message.contains("Fixture executable disappeared"));
    assert!(received.try_recv().is_err());
}

#[test]
#[ignore = "Offscreen status review; fixture worker only, no native window or process action"]
fn render_verified_suspension_status() {
    let mut renderer = offscreen::Renderer::new();
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/ui-smoke/process-state-alpha45");
    std::fs::create_dir_all(&directory).unwrap();
    for (name, dark, page) in [
        ("inspector-dark", true, Page::Processes),
        ("inspector-light", false, Page::Processes),
        ("details-dark", true, Page::Details),
    ] {
        let (ctx, mut app, received) = setup();
        app.theme.dark = dark;
        theme::install(&ctx, app.theme);
        app.page = page;
        app.inspector_visible = page == Page::Processes;
        let identity = app.selected_process().unwrap().identity().unwrap();
        app.process_actions
            .submit(Request::new(Action::Suspend(identity), "fixture".into()))
            .unwrap();
        assert_eq!(
            received.recv_timeout(Duration::from_secs(3)).unwrap(),
            Action::Suspend(identity)
        );
        let size = Vec2::new(1000.0, 580.0);
        let mut output = egui::FullOutput::default();
        for _ in 0..20 {
            output.append(frame(&ctx, &mut app, size, vec![]));
        }
        assert!(
            text_shapes(&output)
                .iter()
                .any(|(text, _)| text.galley.job.text == "Suspended")
        );
        renderer.save(&ctx, output, size, &directory.join(format!("{name}.png")));
    }
    println!(
        "PROCESS_STATE: 3 fixture-only offscreen images in {}",
        directory.display()
    );
}
