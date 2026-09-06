//! Synthetic chart data and offscreen review; never touches desktop windows/input.
use super::*;
use std::time::{Duration, Instant};

pub(super) fn populated(settings: ThemeSettings) -> TrontopApp {
    populated_with_networks(settings, None)
}

fn populated_with_networks(settings: ThemeSettings, networks: Option<usize>) -> TrontopApp {
    let mut app = super::app(settings, false);
    let start = Instant::now() - Duration::from_secs(120);
    for index in 0..=120 {
        let at = start + Duration::from_secs(index);
        let mut s = fixture();
        if let Some(count) = networks {
            s.networks = (0..count)
                .map(|i| NetworkRow {
                    name: format!("Synthetic interface {i:03}"),
                    received_bytes_per_sec: 512_000.0 + index as f64 * 1_000.0,
                    transmitted_bytes_per_sec: 128_000.0,
                    ..Default::default()
                })
                .collect();
        }
        s.sequence = index + 1;
        s.cpu_percent = 30.0 + (index as f32 * 0.2).sin() * 13.0;
        s.memory_used_bytes += ((index as f32 * 0.1).sin().abs() * 2_000_000_000.0) as u64;
        s.memory_details.as_mut().unwrap().commit_bytes +=
            ((index as f32 * 0.1).sin().abs() * 3_000_000_000.0) as u64;
        for provider in crate::diagnostics::Provider::ALL {
            s.diagnostics.get_mut(provider).record(
                at,
                Duration::ZERO,
                crate::diagnostics::State::Live,
                None,
                None,
            );
        }
        s.gpu.utilization_percent = 20.0 + (index as f32 * 0.15).sin() * 15.0;
        s.gpu_sensors.sampled_at = Some(at);
        s.gpu_sensors.last_success = Some(at);
        s.gpu_sensors.adapters[0].temperature_c = Some(42 + index as u32 / 20);
        s.gpu_sensors.adapters[0].power_w = Some(80.0 + (index as f32 * 0.2).sin() * 30.0);
        let storage = std::sync::Arc::make_mut(&mut s.storage_sensors);
        storage.drives[0].last_attempt = Some(start + Duration::from_secs(index / 5 * 5));
        storage.drives[0].last_success = storage.drives[0].last_attempt;
        s.physical_disks = std::sync::Arc::new(crate::disk_activity::Snapshot {
            at: Some(at),
            generation: index + 2,
            devices: vec![crate::disk_activity::Device {
                number: 0,
                instance: "0 Fixture NVMe".into(),
                readings: [70.0, 4.0, 0.2, 152_000_000.0, 1_000_000.0].map(|value| {
                    crate::disk_activity::Reading {
                        value: Some(value * (0.7 + 0.2 * (index as f64 * 0.1).sin())),
                        at: Some(at),
                    }
                }),
            }],
            ..Default::default()
        });
        // The production history receives provider timestamps; drive liveness is
        // evaluated at this synthetic sample time, not the time the test executes.
        app.graphs.sample(&s, at);
        if index < 120 {
            // Populate the existing performance plots as well as the graph wall.
            // These are synthetic fixture histories, never a runtime fallback.
            widgets::push_history(&mut app.cpu_history, s.cpu_percent, HISTORY_LENGTH);
            widgets::push_history(&mut app.memory_history, memory_percent(&s), HISTORY_LENGTH);
        }
        if index == 120 {
            app.accept_sample(s);
        }
    }
    app.page = Page::Graphs;
    app
}

#[test]
#[ignore = "CPU-only graph wall timing with synthetic histories; no native window, input or providers"]
fn graph_wall_cpu_timing_probe() {
    use std::hint::black_box;
    for networks in [0, 64, 256] {
        for size in [Vec2::new(1040.0, 640.0), Vec2::new(1920.0, 1080.0)] {
            for (label, all) in [("reference", true), ("visible_rows", false)] {
                let mut app = populated_with_networks(ThemeSettings::default(), Some(networks));
                app.graphs.draw_all_rows = all;
                let ctx = egui::Context::default();
                theme::install(&ctx, app.theme);
                let mut times = Vec::with_capacity(120);
                for index in 0..140 {
                    let start = Instant::now();
                    let output = frame(
                        &ctx,
                        &mut app,
                        size,
                        vec![egui::Event::PointerMoved(egui::pos2(
                            430.0 + (index % 20) as f32 * 5.0,
                            390.0,
                        ))],
                    );
                    black_box(ctx.tessellate(output.shapes, output.pixels_per_point));
                    if index >= 20 {
                        times.push(start.elapsed().as_secs_f64() * 1e6);
                    }
                }
                times.sort_by(f64::total_cmp);
                println!(
                    "GRAPH_WALL_CPU mode={label} networks={networks} size={size:?} cards={} median_us={:.1} p95_us={:.1} max_us={:.1}",
                    app.graphs.laid_out_cards, times[60], times[114], times[119]
                );
            }
        }
    }
}

fn visible_text(output: &egui::FullOutput) -> Vec<(String, egui::Rect)> {
    text_shapes(output)
        .into_iter()
        .filter(|(text, clip)| clip.intersects(text.visual_bounding_rect()))
        .map(|(text, _)| (text.galley.job.text.clone(), text.visual_bounding_rect()))
        .collect()
}

#[test]
fn graph_wall_visible_rows_match_full_layout_through_scroll_and_scale() {
    for dark in [true, false] {
        for (size, scale) in [
            (Vec2::new(1040.0, 640.0), 1.0),
            (Vec2::new(1280.0, 760.0), 1.5),
            (Vec2::new(1920.0, 1080.0), 2.0),
        ] {
            let settings = ThemeSettings {
                dark,
                ..Default::default()
            };
            let mut reference = populated_with_networks(settings, Some(256));
            reference.graphs.draw_all_rows = true;
            let mut actual = populated_with_networks(settings, Some(256));
            // Both fixtures are deliberately stale at the same frozen instant;
            // execution speed must not move just one across a status boundary.
            let now = Instant::now() + Duration::from_secs(10);
            reference.graphs.fixed_now = Some(now);
            actual.graphs.fixed_now = Some(now);
            let stale = Instant::now() - Duration::from_secs(30);
            for app in [&mut reference, &mut actual] {
                app.snapshot
                    .diagnostics
                    .get_mut(crate::diagnostics::Provider::System)
                    .record(
                        stale,
                        Duration::ZERO,
                        crate::diagnostics::State::Live,
                        None,
                        None,
                    );
            }
            let reference_ctx = egui::Context::default();
            let actual_ctx = egui::Context::default();
            for ctx in [&reference_ctx, &actual_ctx] {
                theme::install(ctx, settings);
                ctx.set_pixels_per_point(scale);
            }
            for scroll in [0.0, -2400.0, -200_000.0, 200_000.0] {
                let event = || {
                    if scroll == 0.0 {
                        vec![]
                    } else {
                        vec![
                            egui::Event::PointerMoved(egui::pos2(size.x * 0.75, 500.0)),
                            egui::Event::MouseWheel {
                                phase: egui::TouchPhase::Move,
                                unit: egui::MouseWheelUnit::Point,
                                delta: Vec2::new(0.0, scroll),
                                modifiers: egui::Modifiers::NONE,
                            },
                        ]
                    }
                };
                frame(&reference_ctx, &mut reference, size, event());
                frame(&actual_ctx, &mut actual, size, event());
                // Let egui's local scroll animation settle without native input.
                for _ in 0..24 {
                    frame(&reference_ctx, &mut reference, size, vec![]);
                    frame(&actual_ctx, &mut actual, size, vec![]);
                }
                let expected = frame(&reference_ctx, &mut reference, size, vec![]);
                let output = frame(&actual_ctx, &mut actual, size, vec![]);
                let expected = visible_text(&expected);
                let output = visible_text(&output);
                assert_eq!(
                    output.len(),
                    expected.len(),
                    "visible text count: {size:?}/{scale}/{scroll}"
                );
                for ((text, rect), (expected_text, expected_rect)) in output.iter().zip(&expected) {
                    assert_eq!(
                        text, expected_text,
                        "visible text: {size:?}/{scale}/{scroll}"
                    );
                    let tolerance = 0.1;
                    assert!(
                        (rect.min - expected_rect.min).length() <= tolerance
                            && (rect.max - expected_rect.max).length() <= tolerance,
                        "{text} moved: {rect:?} vs {expected_rect:?} at {size:?}/{scale}/{scroll}"
                    );
                }
                assert_eq!(reference.graphs.laid_out_cards, 512);
                assert!(
                    actual.graphs.laid_out_cards <= 24,
                    "offscreen cards still laid out: {}",
                    actual.graphs.laid_out_cards
                );
            }
        }
    }
}

#[test]
fn graph_wall_controls_respond_to_local_input_without_native_commands() {
    let ctx = egui::Context::default();
    let mut app = populated(ThemeSettings::default());
    theme::install(&ctx, app.theme);
    let size = Vec2::new(1280.0, 900.0);
    for _ in 0..3 {
        frame(&ctx, &mut app, size, vec![]);
    }
    click_local_text(&ctx, &mut app, size, "Bars");
    click_local_text(&ctx, &mut app, size, "Temperatures & power");
    let output = frame(&ctx, &mut app, size, vec![]);
    let texts = text_shapes(&output);
    assert!(
        texts
            .iter()
            .any(|(text, _)| text.galley.job.text == "GPU temperature")
    );
    assert!(
        !texts
            .iter()
            .any(|(text, _)| text.galley.job.text == "CPU usage")
    );
    click_local_text(&ctx, &mut app, size, "Everything");
    let output = frame(&ctx, &mut app, size, vec![]);
    assert!(
        text_shapes(&output)
            .iter()
            .any(|(text, _)| text.galley.job.text == "CPU usage")
    );
}

#[test]
fn graph_wall_filter_returns_to_first_row_after_deep_scroll() {
    let ctx = egui::Context::default();
    let mut app = populated_with_networks(ThemeSettings::default(), Some(256));
    theme::install(&ctx, app.theme);
    let size = Vec2::new(1040.0, 640.0);
    for _ in 0..3 {
        frame(&ctx, &mut app, size, vec![]);
    }
    frame(
        &ctx,
        &mut app,
        size,
        vec![
            egui::Event::PointerMoved(egui::pos2(850.0, 500.0)),
            egui::Event::MouseWheel {
                phase: egui::TouchPhase::Move,
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, -200_000.0),
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    for _ in 0..24 {
        frame(&ctx, &mut app, size, vec![]);
    }
    let output = frame(&ctx, &mut app, size, vec![]);
    assert!(
        visible_text(&output)
            .iter()
            .any(|(text, _)| text.starts_with("Synthetic interface")),
        "scroll must actually reach the network tail"
    );
    click_local_text(&ctx, &mut app, size, "Memory");
    for _ in 0..4 {
        frame(&ctx, &mut app, size, vec![]);
    }
    let output = frame(&ctx, &mut app, size, vec![]);
    assert!(
        visible_text(&output)
            .iter()
            .any(|(text, _)| text == "Commit charge")
    );
    assert!(
        !visible_text(&output)
            .iter()
            .any(|(text, _)| text.starts_with("Synthetic interface"))
    );
    click_local_text(&ctx, &mut app, size, "Everything");
    for _ in 0..4 {
        frame(&ctx, &mut app, size, vec![]);
    }
    let output = frame(&ctx, &mut app, size, vec![]);
    assert!(
        visible_text(&output)
            .iter()
            .any(|(text, _)| text == "CPU usage")
    );
}

#[test]
#[ignore = "offscreen GPU graph-wall QA; only test PNG files, no native window or input"]
fn render_graph_wall_visual_pass() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/ui-smoke");
    std::fs::create_dir_all(&directory).unwrap();
    let mut renderer = offscreen::Renderer::new();
    for (name, light, bars, thermal, empty, size) in [
        (
            "graphs-dark",
            false,
            false,
            false,
            false,
            Vec2::new(1920.0, 1080.0),
        ),
        (
            "graphs-bars",
            false,
            true,
            false,
            false,
            Vec2::new(1920.0, 1080.0),
        ),
        (
            "graphs-thermal",
            false,
            false,
            true,
            false,
            Vec2::new(1280.0, 900.0),
        ),
        (
            "graphs-light",
            true,
            false,
            false,
            false,
            Vec2::new(1280.0, 900.0),
        ),
        (
            "graphs-compact",
            false,
            false,
            false,
            false,
            Vec2::new(1040.0, 640.0),
        ),
        (
            "graphs-empty",
            false,
            false,
            false,
            true,
            Vec2::new(1040.0, 640.0),
        ),
    ] {
        let settings = ThemeSettings {
            dark: !light,
            ..ThemeSettings::tront_stack()
        };
        let ctx = egui::Context::default();
        theme::install(&ctx, settings);
        let mut app = if empty {
            super::app(settings, false)
        } else {
            populated(settings)
        };
        app.page = Page::Graphs;
        let mut output = egui::FullOutput::default();
        for _ in 0..3 {
            output.append(frame(&ctx, &mut app, size, vec![]));
        }
        if bars {
            click_local_text(&ctx, &mut app, size, "Bars");
        }
        if thermal {
            click_local_text(&ctx, &mut app, size, "Temperatures & power");
        }
        for _ in 0..3 {
            output.append(frame(&ctx, &mut app, size, vec![]));
        }
        renderer.save(&ctx, output, size, &directory.join(format!("{name}.png")));
    }
    println!("Graph wall: 6 offscreen PNGs; synthetic data only; no native windows or OS input");
}
