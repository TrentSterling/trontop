use super::*;
use crate::export::{Exporter, Format, Outcome};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The smallest painted, *stroked* rectangle that contains `point`: for a
/// point inside a dialog and nothing else, that is the dialog's own frame.
/// Its drop shadow is the same size and painted first, but has no stroke, so
/// filtering on `stroke.width > 0` tells the real frame from its shadow.
fn smallest_containing_rect(output: &egui::FullOutput, point: egui::Pos2) -> egui::Rect {
    fn visit(shape: &egui::Shape, point: egui::Pos2, best: &mut Option<egui::Rect>) {
        match shape {
            egui::Shape::Rect(r) => {
                if r.stroke.width > 0.0
                    && r.rect.contains(point)
                    && best.is_none_or(|b: egui::Rect| r.rect.area() < b.area())
                {
                    *best = Some(r.rect);
                }
            }
            egui::Shape::Vec(shapes) => {
                for s in shapes {
                    visit(s, point, best);
                }
            }
            _ => {}
        }
    }
    let mut best = None;
    for clipped in &output.shapes {
        visit(&clipped.shape, point, &mut best);
    }
    best.expect("no stroked background rectangle contains the point")
}

#[test]
fn export_heading_sits_inside_the_dialog_frame_at_1000x580() {
    let ctx = egui::Context::default();
    let settings = ThemeSettings::default();
    theme::install(&ctx, settings);
    let mut app = app(settings, true);
    app.show_export = true;
    let size = Vec2::new(1000.0, 580.0);
    let mut output = frame(&ctx, &mut app, size, vec![]);
    for _ in 0..3 {
        output = frame(&ctx, &mut app, size, vec![]);
    }
    let heading = text_shapes(&output)
        .into_iter()
        .find(|(text, _)| text.galley.job.text == crate::app::export::EXPORT_INTRO)
        .expect("export intro")
        .0
        .visual_bounding_rect();
    let dialog = smallest_containing_rect(&output, heading.center());
    let inset = heading.left() - dialog.left();
    assert!(
        inset >= 8.0,
        "export heading sits only {inset} px inside the dialog frame ({heading:?} in {dialog:?})"
    );
}

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
