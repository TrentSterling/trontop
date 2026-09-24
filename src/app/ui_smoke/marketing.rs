//! Reproducible public screenshots of the production UI with synthetic demo data.
//! Never starts a sampler, reads the desktop, or sends native input/actions.
use super::*;
use std::time::{Duration, Instant};

fn demo(settings: ThemeSettings) -> TrontopApp {
    let mut app = super::app(settings, false);
    let start = Instant::now() - Duration::from_secs(120);
    for index in 0..=120 {
        let at = start + Duration::from_secs(index);
        let mut s = fixture();
        s.sequence = index + 1;
        for provider in crate::diagnostics::Provider::ALL {
            s.diagnostics.get_mut(provider).record(
                at,
                Duration::ZERO,
                crate::diagnostics::State::Live,
                None,
                None,
            );
        }
        s.host_name = "DEMO DATA".into();
        s.os_name = "Windows 11 (demo data)".into();
        s.cpu.brand = "Example 24-thread processor".into();
        s.cpu.physical_cores = 12;
        s.cpu.logical_cores = 24;
        s.cpu.clocks = Some(super::cpu_clock::values(24, index as f64 * 0.12));
        s.cpu_percent = 36.0 + (index as f32 * 0.2).sin() * 13.0;
        s.cpu.logical_usage = (0..24)
            .map(|core| Some((30.0 + (index as f32 * 0.08 + core as f32).sin() * 25.0).max(0.0)))
            .collect();
        s.memory_used_bytes =
            32_000_000_000 + ((index as f64 * 0.07).sin().abs() * 1_000_000_000.0) as u64;
        s.processes.truncate(24);
        for (i, p) in s.processes.iter_mut().enumerate() {
            p.name = [
                "Editor.exe",
                "Compiler.exe",
                "Browser.exe",
                "Terminal.exe",
                "Renderer.exe",
                "AudioEngine.exe",
                "FileManager.exe",
                "Trontop.exe",
                "ImageViewer.exe",
                "Music.exe",
            ][i % 10]
                .into();
            p.parent_pid = if i > 9 {
                Some(900_000 + (i % 10) as u32)
            } else {
                None
            };
            p.user = "Demo".into();
            p.executable = None;
            p.command = p.name.clone();
            p.started_at_unix = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                - 10800;
            p.cpu_percent = (24 - i) as f32 * 0.16;
            p.memory_bytes = (24 - i) as u64 * 72_000_000;
        }
        s.process_count = s.processes.len();
        s.users.clear();
        s.disks.truncate(1);
        s.disks[0].name = "Example NVMe".into();
        s.networks[0].name = "Ethernet (demo)".into();
        s.networks[0].received_bytes_per_sec =
            300_000.0 + (index as f64 * 0.3).sin().abs() * 1_500_000.0;
        s.gpu.utilization_percent = 24.0 + (index as f32 * 0.15).sin() * 15.0;
        s.gpu_sensors.sampled_at = Some(at);
        s.gpu_sensors.last_success = Some(at);
        s.gpu_sensors.adapters[0].name = "Example GPU (demo)".into();
        s.gpu_sensors.adapters[0].uuid = Some("DEMO-GPU-0".into());
        s.gpu_sensors.adapters[0].temperature_c = Some(43 + index as u32 / 20);
        s.gpu_sensors.adapters[0].power_w = Some(80.0 + (index as f32 * 0.2).sin() * 30.0);
        let storage = std::sync::Arc::make_mut(&mut s.storage_sensors);
        storage.drives[0].device.name = "Example NVMe (demo)".into();
        storage.drives[0].device.id = "DEMO-NVME".into();
        storage.drives[0].last_attempt = Some(at);
        storage.drives[0].last_success = Some(at);
        s.physical_disks = std::sync::Arc::new(crate::disk_activity::Snapshot {
            at: Some(at),
            generation: index + 2,
            devices: vec![crate::disk_activity::Device {
                number: 0,
                instance: "0 Example NVMe".into(),
                readings: [24.0, 2.0, 0.2, 42_000_000.0, 1_000_000.0].map(|value| {
                    crate::disk_activity::Reading {
                        value: Some(value * (0.7 + 0.2 * (index as f64 * 0.1).sin())),
                        at: Some(at),
                    }
                }),
            }],
            ..Default::default()
        });
        app.graphs.sample(&s, at);
        if index < 120 {
            widgets::push_history(&mut app.cpu_history, s.cpu_percent, HISTORY_LENGTH);
            widgets::push_history(&mut app.memory_history, memory_percent(&s), HISTORY_LENGTH);
            widgets::push_history(
                &mut app.overview_disk_total,
                crate::app::overview::disk_throughput(&s.physical_disks, at)
                    .unwrap()
                    .0 as f32,
                HISTORY_LENGTH,
            );
            let (receive, send) = crate::app::overview::network_throughput(&s).unwrap();
            widgets::push_history(
                &mut app.overview_net_total,
                (receive + send) as f32,
                HISTORY_LENGTH,
            );
        } else {
            app.accept_sample(s);
        }
    }
    assert!(app.sampler.is_none() && app.tray.is_none());
    app
}

#[test]
#[ignore = "Generate public media using production UI and explicitly synthetic demo data"]
fn render_marketing_gallery() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/marketing");
    std::fs::create_dir_all(&directory).unwrap();
    let mut renderer = offscreen::Renderer::new();
    let mut manifest = Vec::new();
    let mut themes: Vec<_> = ThemeSettings::presets().into_iter().collect();
    let mut vivid = ThemeSettings::monke_portal();
    vivid.gradient_strength = 1.0;
    vivid.frost = 0.12;
    vivid.surface_tint = 0.18;
    vivid.text_strength = 1.0;
    themes.push(("Electric", vivid));
    let mut sunset = ThemeSettings::demigod();
    sunset.gradient_strength = 1.0;
    sunset.frost = 0.08;
    sunset.text_strength = 1.0;
    themes.push(("Ember", sunset));
    let mut candy = vivid;
    candy.stops =
        crate::theme::Stop::palette([[184, 20, 198], [175, 25, 76], [210, 87, 39], [11, 163, 139]]);
    themes.push(("Spectrum", candy));
    let mut ice = vivid;
    ice.dark = false;
    ice.frost_light = 0.35;
    themes.push(("Daylight", ice));
    for (name, settings) in themes {
        let slug = name.to_lowercase().replace(' ', "-");
        let theme_file = format!("{slug}.json");
        std::fs::write(directory.join(&theme_file), settings.encode()).unwrap();
        for (view, page, studio, bars) in [
            ("overview", Page::Overview, false, false),
            ("bars", Page::Overview, false, true),
            ("studio", Page::Overview, true, false),
            ("processes", Page::Processes, false, false),
            ("graphs", Page::Graphs, false, false),
        ] {
            if view != "overview" && name != "Electric" {
                continue;
            }
            let ctx = egui::Context::default();
            theme::install(&ctx, settings);
            ctx.data_mut(|d| d.insert_persisted(egui::Id::new(widgets::CHART_BARS_KEY), bars));
            let mut app = demo(settings);
            app.page = page;
            app.show_theme_editor = studio;
            if view == "processes" {
                app.inspector_visible = true;
                app.selected_pid = Some(900_000);
                app.process_actions =
                    crate::process_actions::Controller::with_backend(ctx.clone(), |_| Ok(()));
            }
            let size = Vec2::new(1440.0, 900.0);
            let mut output = egui::FullOutput::default();
            for _ in 0..14 {
                output.append(frame(&ctx, &mut app, size, vec![]));
            }
            let file = format!("{slug}-{view}.png");
            renderer.save(&ctx, output, size, &directory.join(&file));
            manifest.push(serde_json::json!({"theme":name,"slug":slug,"view":view,"file":file,"theme_file":theme_file,"width":1440,"height":900,"demo_data":true}));
        }
    }
    std::fs::write(
        directory.join("gallery.json"),
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();
    println!(
        "Rendered {} production-UI screenshots with demo data",
        manifest.len()
    );
}
