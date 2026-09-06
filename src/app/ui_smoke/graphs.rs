//! Synthetic chart data and offscreen review; never touches desktop windows/input.
use super::*;
use std::time::{Duration, Instant};

pub(super) fn populated(settings: ThemeSettings) -> TrontopApp {
    let mut app = super::app(settings, false);
    let start = Instant::now() - Duration::from_secs(120);
    for index in 0..=120 {
        let at = start + Duration::from_secs(index);
        let mut s = fixture();
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
