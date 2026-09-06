//! Memory-counter layout and missing states, with no native sampling or OS input.
use super::*;
use crate::diagnostics::{Provider, State};
use std::time::{Duration, Instant};

fn state(app: &mut TrontopApp, mode: u8) {
    if mode == 1 {
        let at = Instant::now() - Duration::from_secs(5);
        let h = app.snapshot.diagnostics.get_mut(Provider::MemoryCounters);
        h.record(at, Duration::ZERO, State::Live, None, None);
        h.record(
            Instant::now(),
            Duration::ZERO,
            State::Unavailable,
            None,
            None,
        );
    } else if mode == 2 {
        app.snapshot.memory_details = None;
        *app.snapshot.diagnostics.get_mut(Provider::MemoryCounters) = Default::default();
    }
}

#[test]
fn memory_counters_layout_retains_eight_aligned_fields_in_missing_and_cached_states() {
    for dark in [false, true] {
        for width in [440.0, 820.0] {
            let ctx = egui::Context::default();
            let settings = ThemeSettings {
                dark,
                ..ThemeSettings::default()
            };
            theme::install(&ctx, settings);
            let mut baseline = Vec::new();
            for mode in 0..3 {
                let mut app = app(settings, true);
                state(&mut app, mode);
                let mut output = egui::FullOutput::default();
                for _ in 0..3 {
                    output = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                Vec2::new(width, 1100.0),
                            )),
                            ..Default::default()
                        },
                        |ui| app.memory_performance(ui),
                    );
                }
                let texts = text_shapes(&output);
                let (explanation, clip) = texts
                    .iter()
                    .find(|(t, _)| t.galley.job.text.starts_with("Commit is allocated"))
                    .unwrap();
                assert!(
                    clip.contains_rect(explanation.visual_bounding_rect()),
                    "explanation must wrap inside the panel"
                );
                let labels = [
                    "IN USE",
                    "AVAILABLE",
                    "COMMITTED",
                    "COMMIT LIMIT",
                    "COMMIT PEAK",
                    "SYSTEM CACHE",
                    "PAGED POOL",
                    "NONPAGED POOL",
                ];
                let positions: Vec<_> = labels
                    .into_iter()
                    .map(|label| {
                        let (text, clip) = texts
                            .iter()
                            .find(|(text, _)| text.galley.job.text == label)
                            .unwrap();
                        assert!(
                            clip.contains_rect(text.visual_bounding_rect()),
                            "clipped {label}"
                        );
                        assert_eq!(text.galley.rows.len(), 1, "wrapped {label}");
                        text.pos
                    })
                    .collect();
                if mode == 0 {
                    baseline = positions;
                } else {
                    assert_eq!(positions, baseline, "state moved field labels");
                }
                let expected = [
                    "Memory counters: Live",
                    "Memory counters: Cached",
                    "Memory counters: Unavailable",
                ][mode as usize];
                assert!(
                    texts
                        .iter()
                        .any(|(text, clip)| text.galley.job.text == expected
                            && clip.contains_rect(text.visual_bounding_rect()))
                );
                let commit = format::bytes(57_000_000_000);
                assert_eq!(
                    texts.iter().any(|(t, _)| t.galley.job.text == commit),
                    mode != 2
                );
                assert!(!texts.iter().any(|(t, _)| t.galley.job.text == "SWAP"));
                if mode == 2 {
                    assert_eq!(
                        texts
                            .iter()
                            .filter(|(t, _)| t.galley.job.text == "--")
                            .count(),
                        6
                    );
                }
            }
        }
    }
}

#[test]
#[ignore = "offscreen memory-counter QA; synthetic data and PNG files only, no native window"]
fn render_memory_counters_visual_pass() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/ui-smoke");
    std::fs::create_dir_all(&directory).unwrap();
    let mut renderer = offscreen::Renderer::new();
    for (name, dark, mode, graph, size) in [
        ("memory-live", true, 0, false, Vec2::new(1920.0, 1080.0)),
        ("memory-light", false, 0, false, Vec2::new(1280.0, 900.0)),
        (
            "memory-cached-compact",
            true,
            1,
            false,
            Vec2::new(1040.0, 900.0),
        ),
        (
            "memory-unavailable",
            true,
            2,
            false,
            Vec2::new(1280.0, 900.0),
        ),
        ("memory-graphs", true, 0, true, Vec2::new(1280.0, 900.0)),
        (
            "memory-graphs-cached",
            false,
            1,
            true,
            Vec2::new(1280.0, 900.0),
        ),
    ] {
        let settings = ThemeSettings {
            dark,
            ..ThemeSettings::tront_stack()
        };
        let ctx = egui::Context::default();
        theme::install(&ctx, settings);
        let mut app = graphs::populated(settings);
        state(&mut app, mode);
        if mode != 0 {
            app.graphs.sample(&app.snapshot, Instant::now());
        }
        app.page = if graph {
            Page::Graphs
        } else {
            Page::Performance
        };
        app.performance_device = PerformanceDevice::Memory;
        let mut output = egui::FullOutput::default();
        for _ in 0..3 {
            output.append(frame(&ctx, &mut app, size, vec![]));
        }
        if graph {
            click_local_text(&ctx, &mut app, size, "Memory");
        }
        for _ in 0..3 {
            output.append(frame(&ctx, &mut app, size, vec![]));
        }
        renderer.save(&ctx, output, size, &directory.join(format!("{name}.png")));
    }
    println!("Memory counters: 6 offscreen PNGs; synthetic data only, no native window/input");
}
