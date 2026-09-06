//! Synthetic CPU clock presentation tests only; no native window/input/provider.
use super::*;
use crate::diagnostics::{Provider, State};
use std::time::{Duration, Instant};

pub(super) fn values(count: usize, phase: f64) -> crate::cpu_clock::Values {
    let processors: Vec<_> = (0..count)
        .map(|index| crate::cpu_clock::Processor {
            group: (index / 64) as u16,
            number: (index % 64) as u32,
            nominal_mhz: if index % 3 == 0 { 3700 } else { 3200 },
            mhz: Some(4400.0 + (phase + index as f64 * 0.3).sin() * 600.0),
        })
        .collect();
    let mhz = || processors.iter().filter_map(|p| p.mhz);
    crate::cpu_clock::Values {
        average_mhz: mhz().sum::<f64>() / count.max(1) as f64,
        fastest_mhz: mhz().fold(0.0, f64::max),
        slowest_mhz: mhz().fold(100_000.0, f64::min),
        interval_seconds: 1.0,
        processors,
    }
}

fn set_state(app: &mut TrontopApp, state: State) {
    let at = Instant::now();
    let h = app.snapshot.diagnostics.get_mut(Provider::CpuClock);
    *h = Default::default();
    if state == State::Unavailable || state == State::Starting {
        app.snapshot.cpu.clocks = None;
    } else {
        h.record(at, Duration::ZERO, State::Live, None, None);
    }
    h.record(at, Duration::ZERO, state, None, None);
}

#[test]
fn cpu_clock_fields_are_aligned_and_never_fall_back_to_legacy_speed() {
    for dark in [true, false] {
        for width in [440.0, 820.0] {
            let settings = ThemeSettings {
                dark,
                ..Default::default()
            };
            let ctx = egui::Context::default();
            theme::install(&ctx, settings);
            let mut baseline = Vec::new();
            for state in [
                State::Live,
                State::Stale,
                State::Unavailable,
                State::Starting,
            ] {
                let mut app = super::app(settings, true);
                app.graphs.cpu_total = true;
                app.snapshot.cpu.frequency_mhz = 9999;
                set_state(&mut app, state);
                let mut output = egui::FullOutput::default();
                for _ in 0..3 {
                    output = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                Vec2::new(width, 1200.0),
                            )),
                            ..Default::default()
                        },
                        |ui| app.cpu_performance(ui),
                    );
                }
                let texts = text_shapes(&output);
                assert!(
                    !texts
                        .iter()
                        .any(|(t, _)| t.galley.job.text.contains("10.00 GHz"))
                );
                let positions: Vec<_> = ["AVG CLOCK", "FASTEST PROCESSOR", "SLOWEST REPORTING"]
                    .into_iter()
                    .map(|label| {
                        let (text, clip) = texts
                            .iter()
                            .find(|(t, _)| t.galley.job.text == label)
                            .unwrap();
                        assert_eq!(text.galley.rows.len(), 1, "wrapped {label}");
                        assert!(
                            clip.contains_rect(text.visual_bounding_rect()),
                            "clipped {label}"
                        );
                        text.pos
                    })
                    .collect();
                if state == State::Live {
                    baseline = positions;
                } else {
                    assert_eq!(positions, baseline, "{width}/{state:?} shifted fields");
                }
                let ghz: Vec<_> = texts
                    .iter()
                    .filter(|(t, _)| t.galley.job.text.ends_with(" GHz"))
                    .collect();
                assert_eq!(ghz.len(), 3);
                for (text, clip) in ghz {
                    let value = &text.galley.job.text;
                    assert_eq!(text.galley.rows.len(), 1, "clock stacked: {value}");
                    assert!(clip.contains_rect(text.visual_bounding_rect()));
                    if state == State::Stale {
                        assert!(value.starts_with('~'));
                    }
                    if matches!(state, State::Starting | State::Unavailable) {
                        assert_eq!(value, "-- GHz");
                    }
                }
            }
        }
    }
}

#[test]
#[ignore = "Offscreen alpha34 CPU clock views with synthetic values; no native desktop/input"]
fn render_cpu_clock_visual_pass() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/ui-smoke");
    std::fs::create_dir_all(&directory).unwrap();
    let mut renderer = offscreen::Renderer::new();
    for (name, state, dark, page, size) in [
        (
            "cpu",
            State::Live,
            true,
            Page::Performance,
            Vec2::new(1280.0, 900.0),
        ),
        (
            "cached",
            State::Stale,
            true,
            Page::Performance,
            Vec2::new(1040.0, 760.0),
        ),
        (
            "missing",
            State::Unavailable,
            false,
            Page::Performance,
            Vec2::new(1040.0, 760.0),
        ),
        (
            "overview",
            State::Live,
            true,
            Page::Overview,
            Vec2::new(1920.0, 1080.0),
        ),
    ] {
        let settings = ThemeSettings {
            dark,
            ..ThemeSettings::tront_stack()
        };
        let ctx = egui::Context::default();
        theme::install(&ctx, settings);
        let mut app = graphs::populated(settings);
        set_state(&mut app, state);
        app.page = page;
        app.performance_device = PerformanceDevice::Cpu;
        app.graphs.cpu_total = true;
        let mut output = egui::FullOutput::default();
        for _ in 0..4 {
            output.append(frame(&ctx, &mut app, size, vec![]));
        }
        renderer.save(
            &ctx,
            output,
            size,
            &directory.join(format!("alpha34-clock-{name}.png")),
        );
    }
    println!("CPU clock review: 4 synthetic offscreen PNGs; no native desktop interaction");
}

#[test]
fn cpu_clock_source_expander_keeps_large_processor_inventory_virtualized() {
    let ctx = egui::Context::default();
    let settings = ThemeSettings::default();
    theme::install(&ctx, settings);
    let mut app = super::app(settings, true);
    app.graphs.cpu_total = true;
    app.snapshot.cpu.clocks = Some(values(4096, 0.0));
    let mut draw = |events| {
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(820.0, 1400.0),
                )),
                events,
                ..Default::default()
            },
            |ui| app.cpu_performance(ui),
        )
    };
    let mut output = egui::FullOutput::default();
    for _ in 0..3 {
        output = draw(vec![]);
    }
    let position = text_shapes(&output)
        .iter()
        .find(|(t, _)| t.galley.job.text == "Clock source and per-processor readings")
        .unwrap()
        .0
        .visual_bounding_rect()
        .center();
    for pressed in [true, false] {
        draw(vec![
            egui::Event::PointerMoved(position),
            egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
    }
    for _ in 0..4 {
        output = draw(vec![]);
    }
    let texts = text_shapes(&output);
    let rows = texts
        .iter()
        .filter(|(t, _)| t.galley.job.text.starts_with("Group "))
        .count();
    assert!(
        rows > 0 && rows < 20,
        "laid out {rows} of 4096 processor rows"
    );
    assert!(
        texts
            .iter()
            .any(|(t, _)| t.galley.job.text == crate::cpu_clock::SOURCE)
    );
}
