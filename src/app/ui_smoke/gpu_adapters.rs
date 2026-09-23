//! Adapter UI fixtures. No desktop/input/OS calls.
use super::*;
use crate::gpu_activity::Usage;
use crate::gpu_adapters::{Adapter, Description, Engine, Key};
use std::time::{Duration, Instant};

fn populated(dark: bool, missing: bool) -> TrontopApp {
    fixture(dark, missing, false, 4)
}

/// Two adapters with `engines` busy engines on the first. With
/// `second_idle`, the second adapter reports only zeros and a few MiB, like
/// an iGPU the desktop is not using.
fn fixture(dark: bool, missing: bool, second_idle: bool, engines: usize) -> TrontopApp {
    let mut app = graphs::populated(ThemeSettings {
        dark,
        ..Default::default()
    });
    let now = Instant::now();
    for index in 0..=120 {
        let at = now - Duration::from_secs(120 - index);
        let mut snapshot = app.snapshot.clone();
        snapshot.gpu.adapters = (0..2)
            .map(|number| {
                let key = Key {
                    high: 0,
                    low: 0x100 + number,
                    physical: 0,
                };
                let idle = second_idle && number == 1;
                let mut adapter = Adapter {
                    key,
                    description: Some(Description {
                        name: if number == 0 {
                            "Fixture RTX 5070 Ti"
                        } else {
                            "Fixture integrated GPU"
                        }
                        .into(),
                        vendor_id: 0x10de,
                        device_id: 0x1234,
                        dedicated_video: 16 * 1024 * 1024 * 1024,
                        dedicated_system: 0,
                        shared_limit: 32 * 1024 * 1024 * 1024,
                        software: false,
                    }),
                    description_current: true,
                    sampled_at: Some(at),
                    last_seen: Some(at),
                    activity: Usage::Measured(if idle { 0.0 } else { 30.0 }),
                    engines: [
                        "3D",
                        "Copy",
                        "Video decode",
                        "Compute",
                        "Video encode",
                        "Copy",
                    ]
                    .iter()
                    .take(if number == 0 { engines } else { 4 })
                    .enumerate()
                    .map(|(engine, kind)| Engine {
                        number: engine as u32,
                        kind: (*kind).into(),
                        usage: Usage::Measured(if idle {
                            0.0
                        } else {
                            10.0 + ((index as f32 * 0.14 + engine as f32).sin() + 1.0) * 25.0
                        }),
                    })
                    .collect(),
                    ..Default::default()
                };
                for (metric, value) in adapter.memory.iter_mut().enumerate() {
                    value.record(
                        (!missing).then_some(if idle {
                            1_000_000
                        } else {
                            [8_355_000_000, 347_000_000, 9_751_000_000][metric] + index * 1_000_000
                        }),
                        at,
                    );
                }
                adapter
            })
            .collect();
        app.graphs.sample(&snapshot, at);
        if index == 120 {
            app.snapshot = snapshot;
        }
    }
    app.graphs.fixed_now = Some(now);
    app.page = Page::Performance;
    app.performance_device = PerformanceDevice::Gpu;
    app
}

#[test]
fn adapter_selection_tracks_identity_and_memory_layout_survives_missing_values() {
    for dark in [true, false] {
        for width in [440.0, 820.0] {
            let mut baseline = None;
            for missing in [false, true] {
                let mut app = populated(dark, missing);
                let key = app.snapshot.gpu.adapters[1].key;
                app.graphs.gpu_selected = Some(key);
                app.snapshot.gpu.adapters.reverse();
                let ctx = egui::Context::default();
                theme::install(&ctx, app.theme);
                let mut output = egui::FullOutput::default();
                for _ in 0..3 {
                    output = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                Vec2::new(width, 1800.0),
                            )),
                            ..Default::default()
                        },
                        |ui| app.gpu_performance(ui),
                    );
                }
                assert_eq!(app.graphs.gpu_selected, Some(key));
                let text = text_shapes(&output);
                let positions: Vec<_> = ["Dedicated used", "Shared used", "Committed"]
                    .iter()
                    .map(|label| {
                        let (t, clip) = text
                            .iter()
                            .find(|(t, _)| t.galley.job.text == *label)
                            .unwrap();
                        assert_eq!(t.galley.rows.len(), 1);
                        assert!(
                            clip.contains_rect(t.visual_bounding_rect()),
                            "clipped {label}"
                        );
                        t.pos
                    })
                    .collect();
                if let Some(before) = &baseline {
                    assert_eq!(before, &positions);
                } else {
                    baseline = Some(positions);
                }
                // "3D / engine 0" renders as a "3D" card with an "engine 0" chip.
                assert!(text.iter().any(|(t, _)| t.galley.job.text == "3D"));
                assert!(text.iter().any(|(t, _)| t.galley.job.text == "engine 0"));
                // Never-measured memory counters fold into one compact gap row
                // instead of empty "No data" charts.
                let expected = if missing {
                    "3 counters with no value in the last 2 minutes"
                } else {
                    "Dedicated VRAM"
                };
                assert!(text.iter().any(|(t, _)| t.galley.job.text == expected));
                assert_eq!(
                    missing,
                    !text
                        .iter()
                        .any(|(t, _)| t.galley.job.text == "Dedicated VRAM")
                );
            }
        }
    }
}

#[test]
fn idle_engines_fold_into_one_muted_line_leaving_active_engines_as_cards() {
    let mut app = graphs::populated(ThemeSettings::default());
    let now = Instant::now();
    let key = Key {
        high: 0,
        low: 0x300,
        physical: 0,
    };
    for index in 0..=120 {
        let at = now - Duration::from_secs(120 - index);
        let mut snapshot = app.snapshot.clone();
        snapshot.gpu.adapters = vec![Adapter {
            key,
            description: Some(Description {
                name: "Fixture idle-engine GPU".into(),
                vendor_id: 0x10de,
                device_id: 0x1234,
                dedicated_video: 8 * 1024 * 1024 * 1024,
                dedicated_system: 0,
                shared_limit: 16 * 1024 * 1024 * 1024,
                software: false,
            }),
            description_current: true,
            sampled_at: Some(at),
            last_seen: Some(at),
            activity: Usage::Measured(30.0),
            engines: vec![
                Engine {
                    number: 0,
                    kind: "3D".into(),
                    usage: Usage::Measured(10.0 + ((index as f32 * 0.14).sin() + 1.0) * 25.0),
                },
                Engine {
                    number: 1,
                    kind: "Copy".into(),
                    // Reported every sample, but never busy: this engine must
                    // fold into the idle footer, not draw a flat 0% card.
                    usage: Usage::Measured(0.0),
                },
            ],
            ..Default::default()
        }];
        app.graphs.sample(&snapshot, at);
        if index == 120 {
            app.snapshot = snapshot;
        }
    }
    app.graphs.fixed_now = Some(now);
    app.page = Page::Performance;
    app.performance_device = PerformanceDevice::Gpu;
    app.graphs.gpu_selected = Some(key);
    let ctx = egui::Context::default();
    theme::install(&ctx, app.theme);
    let mut output = egui::FullOutput::default();
    for _ in 0..3 {
        output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(1280.0, 1200.0),
                )),
                ..Default::default()
            },
            |ui| app.gpu_performance(ui),
        );
    }
    let text = text_shapes(&output);
    assert!(
        text.iter().any(|(t, _)| t.galley.job.text == "3D")
            && text.iter().any(|(t, _)| t.galley.job.text == "engine 0"),
        "the active engine keeps its own card"
    );
    assert!(
        !text
            .iter()
            .any(|(t, _)| t.galley.job.text == "Copy" || t.galley.job.text == "engine 1"),
        "a silent engine must not draw a flat 0% card"
    );
    assert!(
        text.iter()
            .any(|(t, _)| t.galley.job.text == "1 idle engine"),
        "the silent engine folds into one muted line"
    );
}

#[test]
#[ignore = "Offscreen GPU adapter review with synthetic readings; no desktop input"]
fn render_gpu_adapter_visual_pass() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/ui-smoke");
    std::fs::create_dir_all(&directory).unwrap();
    let mut renderer = offscreen::Renderer::new();
    for (name, size, dark, missing, cached) in [
        ("wide", Vec2::new(1280., 1000.), true, false, false),
        ("compact", Vec2::new(1040., 760.), true, false, false),
        ("missing-light", Vec2::new(1040., 760.), false, true, false),
        ("cached", Vec2::new(1280., 1000.), true, false, true),
    ] {
        let mut app = populated(dark, missing);
        if cached {
            let at = app.graphs.now() + Duration::from_secs(1);
            for adapter in &mut app.snapshot.gpu.adapters {
                for value in &mut adapter.memory {
                    value.record(None, at);
                }
                adapter.activity = Usage::Partial(13.2);
                adapter.sampled_at = Some(at);
                for engine in &mut adapter.engines {
                    engine.usage = Usage::Unavailable;
                }
            }
            app.graphs.sample(&app.snapshot, at);
            app.graphs.fixed_now = Some(at);
        }
        let ctx = egui::Context::default();
        theme::install(&ctx, app.theme);
        let mut output = egui::FullOutput::default();
        for _ in 0..5 {
            output.append(frame(&ctx, &mut app, size, vec![]));
        }
        renderer.save(
            &ctx,
            output,
            size,
            &directory.join(format!("alpha35-gpu-{name}.png")),
        );
    }
    println!("GPU adapter review: 4 synthetic offscreen PNGs, no desktop interaction");
}

fn gpu_frame(app: &mut TrontopApp, width: f32) -> egui::FullOutput {
    let ctx = egui::Context::default();
    theme::install(&ctx, app.theme);
    let mut output = egui::FullOutput::default();
    for _ in 0..3 {
        output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(width, 1800.0),
                )),
                ..Default::default()
            },
            |ui| app.gpu_performance(ui),
        );
    }
    output
}

#[test]
fn adapter_picker_hides_when_only_one_adapter_is_active() {
    // An idle iGPU next to the busy card does not make a choice: no picker,
    // and the busy adapter is shown even if the idle one was selected before.
    let mut app = fixture(true, false, true, 4);
    let busy = app.snapshot.gpu.adapters[0].key;
    app.graphs.gpu_selected = Some(app.snapshot.gpu.adapters[1].key);
    let output = gpu_frame(&mut app, 820.0);
    let text = text_shapes(&output);
    assert!(
        !text.iter().any(|(t, _)| t.galley.job.text == "Adapter"),
        "one active adapter must not draw an Adapter picker row"
    );
    assert_eq!(app.graphs.gpu_selected, Some(busy));
    assert!(
        text.iter()
            .any(|(t, _)| t.galley.job.text == "Fixture RTX 5070 Ti")
    );

    // Two busy adapters keep the picker.
    let mut app = fixture(true, false, false, 4);
    let output = gpu_frame(&mut app, 820.0);
    assert!(
        text_shapes(&output)
            .iter()
            .any(|(t, _)| t.galley.job.text == "Adapter"),
        "two active adapters keep the picker"
    );
}

#[test]
fn engine_cards_follow_grid_breakpoints_and_never_stretch_past_one_and_a_half() {
    for width in [440.0, 640.0, 900.0, 1300.0] {
        for engines in 1..=6 {
            let mut app = fixture(true, false, true, engines);
            let output = gpu_frame(&mut app, width);
            let text = text_shapes(&output);
            // A card spans from its title's left edge to its right-aligned
            // "engine N" chip.
            let widths: Vec<f32> = (0..engines)
                .map(|engine| {
                    let chip = text
                        .iter()
                        .find(|(t, _)| t.galley.job.text == format!("engine {engine}"))
                        .unwrap_or_else(|| panic!("engine {engine} card missing at {width}"))
                        .0
                        .visual_bounding_rect();
                    // This card's title: the nearest text left of the chip
                    // on its row that is not another card's chip.
                    let title = text
                        .iter()
                        .filter(|(t, _)| {
                            let r = t.visual_bounding_rect();
                            r.left() < chip.left()
                                && (r.center().y - chip.center().y).abs() < 6.0
                                && !t.galley.job.text.starts_with("engine ")
                        })
                        .map(|(t, _)| t.visual_bounding_rect().left())
                        .fold(f32::MIN, f32::max);
                    chip.right() - title
                })
                .collect();
            let narrow = widths.iter().copied().fold(f32::MAX, f32::min);
            let wide = widths.iter().copied().fold(0.0_f32, f32::max);
            // 1.5x nominal slots: a stretched card also absorbs part of
            // the 8 px grid gap, and the measure excludes card padding.
            assert!(
                wide <= narrow * 1.5 + 24.0,
                "{engines} engines at {width}: card widths {widths:?}"
            );
            // A lone engine never spans the whole pane when the grid has
            // more than one column.
            if engines == 1 && widgets::tile_grid_columns(width) > 1 {
                assert!(
                    wide < width * 0.75,
                    "a lone engine card spans {wide} of {width}"
                );
            }
        }
    }
}
