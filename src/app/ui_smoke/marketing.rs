//! Opt-in public screenshots using the production sampler and UI.
//! Reads real telemetry; no native windows, desktop input, stress workload or process actions.
use super::*;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn consume_live_sample(
    sampler: &Sampler,
    app: &mut TrontopApp,
    trace: &mut Vec<serde_json::Value>,
    started: Instant,
) {
    if let Some(mut snapshot) = sampler.latest_after(app.seen_generation) {
        let network = crate::app::overview::network_throughput(&snapshot);
        trace.push(serde_json::json!({
            "elapsed_seconds": started.elapsed().as_secs_f64(),
            "sequence": snapshot.sequence,
            "cpu_percent": snapshot.cpu_percent,
            "gpu_percent": snapshot.gpu.reading().exact(),
            "memory_used_bytes": snapshot.memory_used_bytes,
            "memory_total_bytes": snapshot.memory_total_bytes,
            "disk_bytes_per_second": crate::app::overview::disk_throughput(&snapshot.physical_disks, Instant::now()).map(|v| v.0),
            "network_bytes_per_second": network.map(|(rx, tx)| rx + tx),
            "process_count": snapshot.process_count,
        }));
        // Keep measured counters, hardware names and process names. Only omit
        // personal account/path/command strings from the public screenshots.
        snapshot.host_name = "Trent's PC".into();
        for process in &mut snapshot.processes {
            process.user = "Local user".into();
            process.command.clear();
            process.executable = None;
            process.cwd = None;
        }
        app.graphs.fixed_now = None;
        app.accept_sample(snapshot);
    }
}

#[test]
#[ignore = "Explicit opt-in: capture 125 seconds of real system telemetry for public screenshots"]
fn render_marketing_gallery() {
    assert_eq!(
        std::env::var("TRONTOP_MARKETING_LIVE").as_deref(),
        Ok("1"),
        "Set TRONTOP_MARKETING_LIVE=1 only when real telemetry capture is intended"
    );
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/marketing");
    std::fs::create_dir_all(&directory).unwrap();
    let sampler = Sampler::spawn(egui::Context::default(), None);
    let mut app = super::app(ThemeSettings::default(), false);
    let mut trace = Vec::new();
    let started = Instant::now();
    let captured_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    println!(
        "Capturing 125 seconds of real telemetry; no windows, native input or generated workload"
    );
    while started.elapsed() < Duration::from_secs(125) {
        consume_live_sample(&sampler, &mut app, &mut trace, started);
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        trace.len() >= 100,
        "Too few real samples to render a representative history"
    );
    assert!(app.cpu_history.len() >= 100);
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
            consume_live_sample(&sampler, &mut app, &mut trace, started);
            let ctx = egui::Context::default();
            theme::install(&ctx, settings);
            ctx.data_mut(|d| d.insert_persisted(egui::Id::new(widgets::CHART_BARS_KEY), bars));
            app.theme = settings;
            app.theme_studio = Default::default();
            app.page = page;
            app.show_theme_editor = studio;
            app.graphs.fixed_now = Some(Instant::now());
            if view == "processes" {
                app.inspector_visible = true;
                app.selected_pid = app
                    .snapshot
                    .processes
                    .iter()
                    .filter(|p| p.name.eq_ignore_ascii_case("Unity.exe"))
                    .max_by_key(|p| p.memory_bytes)
                    .or_else(|| app.snapshot.processes.iter().max_by_key(|p| p.memory_bytes))
                    .map(|p| p.pid);
                app.process_actions =
                    crate::process_actions::Controller::with_backend(ctx.clone(), |_| {
                        Err("Actions disabled in screenshot renderer".into())
                    });
            }
            let size = Vec2::new(1440.0, 900.0);
            let mut output = egui::FullOutput::default();
            for _ in 0..14 {
                output.append(frame(&ctx, &mut app, size, vec![]));
            }
            let file = format!("{slug}-{view}.png");
            renderer.save(&ctx, output, size, &directory.join(&file));
            manifest.push(serde_json::json!({
                "theme":name,"slug":slug,"view":view,"file":file,"theme_file":theme_file,
                "width":1440,"height":900,"demo_data":false,"data_source":"live-telemetry",
                "capture_started_unix":captured_at,"sample_sequence":app.snapshot.sequence,
                "sample_count":trace.len(),"cpu_percent":app.snapshot.cpu_percent,
                "gpu_percent":app.snapshot.gpu.reading().exact(),
                "private_fields_omitted":["account","paths","command lines","host identifier"]
            }));
        }
    }
    std::fs::write(
        directory.join("gallery.json"),
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();
    std::fs::write(
        directory.join("capture-metrics.json"),
        serde_json::to_string_pretty(&trace).unwrap(),
    )
    .unwrap();
    println!(
        "Rendered {} production-UI screenshots from {} real samples",
        manifest.len(),
        trace.len()
    );
}
