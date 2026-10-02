use super::*;
use crate::startup::control::{
    Action, Approval, Control, Controller, Key, RawValue, Receipt, Registration, Request,
};
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

fn setup(dark: bool) -> (egui::Context, TrontopApp, mpsc::Receiver<Request>) {
    let ctx = egui::Context::default();
    let settings = ThemeSettings {
        dark,
        ..Default::default()
    };
    theme::install(&ctx, settings);
    let mut app = app(settings, true);
    app.page = Page::Startup;
    for source in &mut std::sync::Arc::make_mut(&mut app.snapshot.startup).sources {
        let at = Instant::now() - Duration::from_secs(1);
        source.last_attempt = Some(at);
        source.last_complete = Some(at);
        for entry in &mut source.entries {
            entry.observed_at = at;
            entry.row.control = Some(Control {
                registration: Some(Registration::Run(RawValue {
                    kind: 1,
                    bytes: entry
                        .row
                        .command
                        .encode_utf16()
                        .chain(Some(0))
                        .flat_map(u16::to_le_bytes)
                        .collect(),
                })),
                approval: Approval::Missing,
            });
        }
    }
    let key = Key::of(&app.snapshot.startup.rows().next().unwrap().1.row);
    app.selected_startup = Some(key);
    let (sent, received) = mpsc::channel();
    app.startup_controller = Controller::with_backend(ctx.clone(), move |request| {
        sent.send(request.clone()).unwrap();
        let before = request.target.control.as_ref().unwrap().approval.clone();
        let after = match &request.action {
            Action::Restore(approval) => approval.clone(),
            _ => before
                .changed(matches!(request.action, Action::Enable), 55)
                .unwrap(),
        };
        Ok(Receipt {
            target: request.target.clone(),
            before,
            after,
            observed_at: Instant::now(),
        })
    });
    (ctx, app, received)
}
fn poll(ctx: &egui::Context, app: &mut TrontopApp, size: Vec2) {
    let start = Instant::now();
    while app.startup_controller.busy() {
        frame(ctx, app, size, vec![]);
        assert!(start.elapsed() < Duration::from_secs(3));
        std::thread::sleep(Duration::from_millis(2));
    }
}
#[test]
fn startup_confirmation_keeps_source_identity_and_undo_uses_the_verified_approval() {
    let (ctx, mut app, received) = setup(true);
    let size = Vec2::new(1000.0, 580.0);
    click_local_text(&ctx, &mut app, size, "Disable");
    let original = app.pending_startup.as_ref().unwrap().target.clone();
    assert!(received.try_recv().is_err());
    app.selected_startup = Some(Key {
        source: crate::startup::Source::MachineRun,
        name: original.key.to_lowercase(),
    });
    click_local_text(&ctx, &mut app, size, "Disable startup");
    let request = received.recv_timeout(Duration::from_secs(3)).unwrap();
    assert_eq!(request.target.source, original.source);
    assert_eq!(request.target.command, original.command);
    assert!(app.pending_startup.is_none());
    poll(&ctx, &mut app, size);
    assert!(!app.message.as_ref().unwrap().1);
    app.selected_startup = Some(Key::of(&original));
    for _ in 0..10 {
        frame(&ctx, &mut app, size, vec![]);
    }
    let output = frame(&ctx, &mut app, size, vec![]);
    assert!(
        text_shapes(&output)
            .iter()
            .any(|(text, _)| text.galley.job.text == "Disabled"),
        "visible text: {:?}",
        text_shapes(&output)
            .iter()
            .map(|(text, _)| text.galley.job.text.as_str())
            .collect::<Vec<_>>()
    );
    assert!(
        text_shapes(&output)
            .iter()
            .any(|(text, _)| text.galley.job.text == "Command read")
    );
    click_local_text(&ctx, &mut app, size, "Undo");
    click_local_text(&ctx, &mut app, size, "Undo startup change");
    let undo = received.recv_timeout(Duration::from_secs(3)).unwrap();
    assert_eq!(undo.action, Action::Restore(Approval::Missing));
    assert_eq!(
        undo.target.control.as_ref().unwrap().approval.state(),
        crate::startup::control::State::Disabled
    );
}
#[test]
fn changed_cached_expired_and_unknown_records_cannot_submit_a_startup_change() {
    for case in 0..4 {
        let (ctx, mut app, received) = setup(true);
        let size = Vec2::new(1000.0, 580.0);
        click_local_text(&ctx, &mut app, size, "Disable");
        let mut snapshot = (*app.snapshot.startup).clone();
        let source = &mut snapshot.sources[0];
        match case {
            0 => source.entries[0].row.command = "replacement.exe".into(),
            1 => {
                source.result = Some(crate::startup::ReadState::Failed);
                source.entries[0].observed_in_attempt = false;
            }
            2 => app.pending_startup.as_mut().unwrap().confirmed_at -= Duration::from_secs(31),
            _ => {
                source.entries[0].row.control.as_mut().unwrap().approval =
                    Approval::Value(RawValue {
                        kind: 3,
                        bytes: vec![8; 12],
                    })
            }
        }
        app.snapshot.startup = std::sync::Arc::new(snapshot);
        click_local_text(&ctx, &mut app, size, "Disable startup");
        assert!(received.try_recv().is_err(), "unsafe case {case} submitted");
        assert!(app.pending_startup.is_some());
        click_local_text(&ctx, &mut app, size, "Cancel");
        assert!(app.pending_startup.is_none());
    }
}
#[test]
fn startup_table_selection_uses_source_and_name_and_dialog_fits_compact_themes() {
    for dark in [true, false] {
        let (ctx, mut app, received) = setup(dark);
        let size = Vec2::new(1000.0, 580.0);
        app.inspector_visible = false;
        // A unique display name still carries its full source/name selection key.
        let mut snapshot = (*app.snapshot.startup).clone();
        snapshot.sources[1].entries[0].row.name = "Machine duplicate".into();
        app.snapshot.startup = std::sync::Arc::new(snapshot);
        click_local_text(&ctx, &mut app, size, "Machine duplicate");
        assert_eq!(
            app.selected_startup.as_ref().unwrap().source,
            crate::startup::Source::MachineRun
        );
        click_local_text(&ctx, &mut app, size, "Disable");
        assert_eq!(
            app.pending_startup.as_ref().unwrap().target.source,
            crate::startup::Source::MachineRun
        );
        for _ in 0..10 {
            frame(&ctx, &mut app, size, vec![]);
        }
        let output = frame(&ctx, &mut app, size, vec![]);
        for label in ["Confirm startup change", "Disable startup", "Cancel"] {
            let (text, clip) = text_shapes(&output)
                .into_iter()
                .find(|(text, _)| text.galley.job.text == label)
                .unwrap();
            assert!(
                clip.contains_rect(text.visual_bounding_rect()),
                "clipped {label}"
            );
            assert!(
                egui::Rect::from_min_size(egui::Pos2::ZERO, size)
                    .contains_rect(text.visual_bounding_rect())
            );
        }
        assert!(received.try_recv().is_err());
    }
}
#[test]
fn graphics_recovery_keeps_confirmation_pending_and_an_inflight_result_is_still_polled() {
    let (ctx, mut app, received) = setup(true);
    let size = Vec2::new(1000.0, 580.0);
    click_local_text(&ctx, &mut app, size, "Disable");
    app.graphics_recovering
        .store(true, std::sync::atomic::Ordering::Release);
    frame(&ctx, &mut app, size, vec![]);
    assert!(app.pending_startup.is_some());
    assert!(received.try_recv().is_err());
    app.graphics_recovering
        .store(false, std::sync::atomic::Ordering::Release);
    click_local_text(&ctx, &mut app, size, "Disable startup");
    received.recv_timeout(Duration::from_secs(3)).unwrap();
    app.graphics_recovering
        .store(true, std::sync::atomic::Ordering::Release);
    poll(&ctx, &mut app, size);
    assert!(app.message.is_some());
}
#[test]
#[ignore = "Offscreen Startup review; no windows, desktop input or real startup changes"]
fn render_startup_control_review() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/ui-smoke/startup-controls-alpha44");
    std::fs::create_dir_all(&directory).unwrap();
    let mut renderer = offscreen::Renderer::new();
    for (dark, confirmation, filename) in [
        (true, false, "startup-dark.png"),
        (false, false, "startup-light.png"),
        (true, true, "startup-confirmation.png"),
    ] {
        let (ctx, mut app, _receiver) = setup(dark);
        app.inspector_visible = false;
        let size = Vec2::new(1000.0, 580.0);
        if confirmation {
            // Stage before the first frame so its full font texture delta is
            // included in the offscreen output rather than discarded by clicks.
            let row = app.snapshot.startup.rows().next().unwrap().1.row.clone();
            app.pending_startup = Some(Request::new(row, Action::Disable));
        }
        let mut output = egui::FullOutput::default();
        for _ in 0..10 {
            output.append(frame(&ctx, &mut app, size, vec![]));
        }
        renderer.save(&ctx, output, size, &directory.join(filename));
    }
}
