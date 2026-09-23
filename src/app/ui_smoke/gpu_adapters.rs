//! Adapter UI fixtures. No desktop/input/OS calls.
use super::*;
use crate::gpu_activity::Usage;
use crate::gpu_adapters::{Adapter, Description, Engine, Key};
use std::time::{Duration, Instant};

fn populated(dark: bool, missing: bool) -> TrontopApp {
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
                    activity: Usage::Measured(30.0),
                    engines: ["3D", "Copy", "Video decode", "Compute"]
                        .iter()
                        .enumerate()
                        .map(|(engine, kind)| Engine {
                            number: engine as u32,
                            kind: (*kind).into(),
                            usage: Usage::Measured(
                                10.0 + ((index as f32 * 0.14 + engine as f32).sin() + 1.0) * 25.0,
                            ),
                        })
                        .collect(),
                    ..Default::default()
                };
                for (metric, value) in adapter.memory.iter_mut().enumerate() {
                    value.record(
                        (!missing).then_some(
                            [8_355_000_000, 347_000_000, 9_751_000_000][metric] + index * 1_000_000,
                        ),
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
                let positions: Vec<_> = ["DEDICATED USED", "SHARED USED", "COMMITTED"]
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
                assert!(
                    text.iter()
                        .any(|(t, _)| t.galley.job.text == "3D / engine 0")
                );
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
