use super::*;
use crate::export::{Exporter, Format, Outcome};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[test]
fn export_requires_explicit_save_and_captures_options_and_unfiltered_snapshot() {
    let ctx = egui::Context::default();
    let mut app = app(ThemeSettings::default(), true);
    let captures = Arc::new(Mutex::new(Vec::new()));
    let worker_captures = Arc::clone(&captures);
    app.exporter = Exporter::with_backend(move |capture, _| {
        worker_captures.lock().unwrap().push((
            capture.options.format,
            capture.options.private_details,
            capture.snapshot.processes.len(),
            capture.snapshot.sequence,
        ));
        Outcome::Cancelled
    });
    let size = Vec2::new(1280.0, 900.0);
    app.query = "no process matches".into();
    click_local_text(&ctx, &mut app, size, "Export");
    assert!(app.show_export);
    assert!(!app.export_options.private_details);
    assert!(captures.lock().unwrap().is_empty());
    click_local_text(&ctx, &mut app, size, "CSV processes");
    click_local_text(&ctx, &mut app, size, "Include private details");
    assert_eq!(app.export_options.format, Format::Csv);
    assert!(app.export_options.private_details);
    let expected = (app.snapshot.processes.len(), app.snapshot.sequence);
    click_local_text(&ctx, &mut app, size, "Save as...");
    let deadline = Instant::now() + Duration::from_secs(3);
    while app.exporter.busy() {
        assert!(Instant::now() < deadline);
        frame(&ctx, &mut app, size, vec![]);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(
        *captures.lock().unwrap(),
        vec![(Format::Csv, true, expected.0, expected.1)]
    );
    assert!(matches!(app.export_result, Some(Outcome::Cancelled)));
    click_local_text(&ctx, &mut app, size, "Close panel");
    assert!(!app.show_export);
    click_local_text(&ctx, &mut app, size, "Export");
    assert!(!app.export_options.private_details); // Sensitive option never silently persists.
    assert!(app.export_result.is_none()); // An old private save cannot label new limited options Saved.
}

#[test]
fn export_status_geometry_and_disabled_save_survive_missing_data_and_failures() {
    for dark in [true, false] {
        let ctx = egui::Context::default();
        let settings = ThemeSettings {
            dark,
            ..Default::default()
        };
        theme::install(&ctx, settings);
        let mut app = app(settings, true);
        app.show_export = true;
        let size = Vec2::new(1040.0, 640.0);
        for _ in 0..3 {
            frame(&ctx, &mut app, size, vec![]);
        }
        let mut anchor = None;
        for result in [
            None,
            Some(Outcome::Cancelled),
            Some(Outcome::Failed("Fixture: cannot write destination.".into())),
            Some(Outcome::Saved {
                path: "C:/Fixture/test.json".into(),
                bytes: 12000,
                sequence: 41,
            }),
        ] {
            app.export_result = result;
            let output = frame(&ctx, &mut app, size, vec![]);
            assert!(output.platform_output.commands.is_empty());
            let text = text_shapes(&output);
            let mut position = egui::Rect::NOTHING;
            for label in ["Save as...", "Close panel", "Export status"] {
                let (shape, clip) = text
                    .iter()
                    .find(|(text, _)| text.galley.job.text == label)
                    .unwrap();
                let bounds = shape.visual_bounding_rect();
                assert!(
                    clip.contains_rect(bounds),
                    "{label} must not be clipped: {bounds:?} in {clip:?}"
                );
                assert!(
                    egui::Rect::from_min_size(egui::Pos2::ZERO, size).contains_rect(bounds),
                    "{label} must remain on screen"
                );
                if label == "Save as..." {
                    position = bounds;
                }
            }
            if let Some(before) = anchor {
                assert_eq!(position, before);
            } else {
                anchor = Some(position);
            }
            click_local_text(&ctx, &mut app, size, "Save as...");
            assert!(!app.exporter.busy()); // Default test app has no native export backend.
        }
        app.accept_sample(SystemSnapshot::default());
        app.exporter = Exporter::with_backend(|_, _| panic!("Missing sample must not export"));
        click_local_text(&ctx, &mut app, size, "Save as...");
        assert!(!app.exporter.busy());
    }
}
