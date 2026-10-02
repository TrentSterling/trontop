//! Expire retained telemetry through the real UI without a sampler or OS input.
use super::*;
use crate::diagnostics::{Provider, State};
use std::time::{Duration, Instant};

fn sample_at(at: Instant) -> SystemSnapshot {
    let mut sample = fixture();
    sample.diagnostics = crate::diagnostics::Diagnostics::default();
    for provider in Provider::ALL {
        sample
            .diagnostics
            .get_mut(provider)
            .record(at, Duration::ZERO, State::Live, None, None);
    }
    sample
}

fn settled_frame(ctx: &egui::Context, app: &mut TrontopApp) -> egui::FullOutput {
    let mut output = egui::FullOutput::default();
    for _ in 0..5 {
        output = frame(ctx, app, Vec2::new(1040.0, 640.0), vec![]);
    }
    output
}

#[test]
fn stalled_sampler_schedules_stale_badge_without_input() {
    let ctx = egui::Context::default();
    let mut app = app(ThemeSettings::default(), false);
    theme::install(&ctx, app.theme);
    let at = Instant::now();
    app.accept_sample(sample_at(at));
    app.graphs.fixed_now = Some(at + Duration::from_secs(1));

    let output = settled_frame(&ctx, &mut app);
    assert!(
        !text_shapes(&output)
            .iter()
            .any(|(text, _)| text.galley.text() == "STALE")
    );
    let delay = output.viewport_output[&egui::ViewportId::ROOT].repaint_delay;
    assert!(
        delay > Duration::ZERO && delay <= Duration::from_millis(2001),
        "Live telemetry has no expiry repaint: {delay:?}"
    );

    // Advance only the clock, as a native delayed repaint would do. No sample,
    // pointer movement, keyboard event or window manipulation wakes this view.
    app.graphs.fixed_now = Some(at + Duration::from_millis(3001));
    let output = settled_frame(&ctx, &mut app);
    assert!(
        text_shapes(&output)
            .iter()
            .any(|(text, _)| text.galley.text() == "STALE")
    );
    assert_eq!(app.snapshot.sequence, 1);
    assert_eq!(app.snapshot.cpu_percent, 37.2);
    assert_eq!(app.cpu_history.len(), 1, "Expiry fabricated a CPU sample");

    app.graphs.fixed_now = Some(at + Duration::from_secs(120));
    let output = settled_frame(&ctx, &mut app);
    assert_eq!(
        output.viewport_output[&egui::ViewportId::ROOT].repaint_delay,
        Duration::MAX,
        "Expired telemetry must not keep polling"
    );

    let recovered = at + Duration::from_secs(120);
    let mut sample = sample_at(recovered);
    sample.sequence = 2;
    sample.cpu_percent = 12.0;
    app.accept_sample(sample);
    let output = settled_frame(&ctx, &mut app);
    assert!(
        !text_shapes(&output)
            .iter()
            .any(|(text, _)| text.galley.text() == "STALE")
    );
    assert!(output.viewport_output[&egui::ViewportId::ROOT].repaint_delay < Duration::MAX);
    assert_eq!(app.snapshot.cpu_percent, 12.0);
}

#[test]
fn freshness_timers_skip_minimized_and_occluded_views_but_keep_unfocused_views() {
    for (minimized, occluded, expect_timer) in [
        (true, false, false),
        (false, true, false),
        (false, false, true),
    ] {
        let ctx = egui::Context::default();
        let mut app = app(ThemeSettings::default(), false);
        theme::install(&ctx, app.theme);
        let at = Instant::now();
        app.accept_sample(sample_at(at));
        app.graphs.fixed_now = Some(at);
        let mut output = egui::FullOutput::default();
        for _ in 0..5 {
            let mut input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(1040.0, 640.0),
                )),
                focused: false,
                ..Default::default()
            };
            let viewport = input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap();
            viewport.minimized = Some(minimized);
            viewport.occluded = Some(occluded);
            viewport.focused = Some(false);
            output = ctx.run_ui(input, |ui| app.ui(ui, &mut eframe::Frame::_new_kittest()));
        }
        assert_eq!(
            output.viewport_output[&egui::ViewportId::ROOT].repaint_delay < Duration::MAX,
            expect_timer,
            "minimized={minimized} occluded={occluded}"
        );
        assert!(app.sampler.is_none() && app.tray.is_none());
    }
}
