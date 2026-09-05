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
                assert!(clip.contains_rect(bounds), "{label} clipped at {size:?}");
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
