use super::*;

#[test]
fn about_failure_log_controls_fit_and_copy_only_after_explicit_local_click() {
    for dark in [true, false] {
        for size in [Vec2::new(1040.0, 640.0), Vec2::new(1280.0, 900.0)] {
            let ctx = egui::Context::default();
            let settings = ThemeSettings {
                dark,
                ..Default::default()
            };
            theme::install(&ctx, settings);
            let mut app = app(settings, true);
            app.show_diagnostics = true;
            for _ in 0..4 {
                frame(&ctx, &mut app, size, vec![]);
            }
            let output = frame(&ctx, &mut app, size, vec![]);
            assert!(output.platform_output.commands.is_empty());
            let text = text_shapes(&output);
            let mut click = egui::Pos2::ZERO;
            for label in [
                "System tray",
                "Unavailable",
                "Local failure log",
                "Copy log location",
                crate::failure::LOCATION_HINT,
            ] {
                let (shape, clip) = text
                    .iter()
                    .find(|(text, _)| text.galley.job.text == label)
                    .unwrap_or_else(|| {
                        panic!(
                            "Missing {label} at {size:?}, dark={dark}: {:?}",
                            text.iter()
                                .map(|(shape, _)| &shape.galley.job.text)
                                .collect::<Vec<_>>()
                        )
                    });
                let bounds = shape.visual_bounding_rect();
                assert!(
                    clip.contains_rect(bounds),
                    "{label} clipped at {size:?}: {bounds:?} {clip:?}"
                );
                assert!(egui::Rect::from_min_size(egui::Pos2::ZERO, size).contains_rect(bounds));
                assert_eq!(
                    shape.galley.rows.len(),
                    1,
                    "Path and action must remain single-line"
                );
                if label == "Copy log location" {
                    click = bounds.center();
                }
            }
            let events = |pressed| {
                vec![
                    egui::Event::PointerMoved(click),
                    egui::Event::PointerButton {
                        pos: click,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ]
            };
            frame(&ctx, &mut app, size, events(true));
            let copied = checked_frame(&ctx, &mut app, size, events(false), true);
            let [egui::OutputCommand::CopyText(path)] = copied.platform_output.commands.as_slice()
            else {
                panic!("Expected only a location copy command, not a native file open");
            };
            assert_eq!(path, crate::failure::LOCATION_HINT);
            assert!(
                app.message
                    .as_ref()
                    .unwrap()
                    .0
                    .contains("only after a recorded failure")
            );
            frame(&ctx, &mut app, size, vec![]);
        }
    }
}

#[test]
fn about_controls_end_well_inside_the_dialog_frame_at_1000x580() {
    let ctx = egui::Context::default();
    let settings = ThemeSettings::default();
    theme::install(&ctx, settings);
    let mut app = app(settings, true);
    app.show_diagnostics = true;
    let size = Vec2::new(1000.0, 580.0);
    let mut output = frame(&ctx, &mut app, size, vec![]);
    for _ in 0..6 {
        output = frame(&ctx, &mut app, size, vec![]);
    }
    let dialog = ctx
        .memory(|memory| memory.area_rect(egui::Id::new(crate::app::diagnostics::ABOUT_WINDOW)))
        .expect("About window area");
    assert!(egui::Rect::from_min_size(egui::Pos2::ZERO, size).contains_rect(dialog));
    let texts = text_shapes(&output);
    let mut last = f32::MIN;
    for label in [
        "Copy support report",
        "Copy log location",
        "Third-party licenses",
    ] {
        let (text, clip) = texts
            .iter()
            .find(|(text, _)| text.galley.job.text == label)
            .unwrap_or_else(|| panic!("missing {label}"));
        let bounds = text.visual_bounding_rect();
        assert!(
            clip.contains_rect(bounds),
            "{label} clipped: {bounds:?} {clip:?}"
        );
        // The button frame extends its padding (6 px) below the text.
        last = last.max(bounds.bottom() + 6.0);
    }
    assert!(
        last <= dialog.bottom() - 8.0,
        "last control ends at {last}, frame bottom {}",
        dialog.bottom()
    );
    // Version table rows are 28 px apart.
    let top = |label: &str| {
        texts
            .iter()
            .find(|(text, _)| text.galley.job.text == label)
            .unwrap_or_else(|| panic!("missing {label}"))
            .0
            .visual_bounding_rect()
            .center()
            .y
    };
    let pitch = top("Build") - top("Version");
    assert!((pitch - 28.0).abs() < 0.6, "version row pitch {pitch}");
    // The dialog title is 16 px and left-aligned in its header.
    let (title, _) = texts
        .iter()
        .find(|(text, _)| text.galley.job.text == "About Trontop")
        .expect("dialog title");
    assert_eq!(title.galley.job.sections[0].format.font_id.size, 16.0);
    assert!(title.visual_bounding_rect().left() - dialog.left() < 24.0);
}
