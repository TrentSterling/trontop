//! Exercises the production UI without a sampler, tray, native window, or OS input.
//! All telemetry below is deliberately synthetic TEST DATA, never a runtime fallback.
use super::*;
use crate::model::{
    CpuInfo, DiskRow, GpuSnapshot, NetworkRow, ServiceRow, StartupRow, UserSummary,
};
use eframe::App;

mod offscreen;

fn fixture() -> SystemSnapshot {
    let processes: Vec<_> = (0..64)
        .map(|index| ProcessRow {
            pid: 900_000 + index,
            parent_pid: (index > 0).then_some(900_000),
            name: if index == 0 {
                "Fixture.Editor.exe".into()
            } else {
                format!("Fixture.Worker.With.A.Deliberately.Long.Name.{index}.exe")
            },
            user: "FIXTURE\\LongAccountName".into(),
            status: "Running".into(),
            cpu_percent: index as f32 * 0.13,
            gpu_percent: index as f32 * 0.05,
            memory_bytes: 256_000_000 + index as u64 * 17_000_000,
            accumulated_cpu_millis: index as u64 * 87_600,
            started_at_unix: 1_700_000_000,
            command: "fixture.exe --headless-test-only --deliberately-long-command-line".into(),
            ..Default::default()
        })
        .collect();
    SystemSnapshot {
        sequence: 1,
        cpu_percent: 37.2,
        memory_used_bytes: 41_000_000_000,
        memory_total_bytes: 64_000_000_000,
        memory_available_bytes: 23_000_000_000,
        swap_used_bytes: 2_000_000_000,
        swap_total_bytes: 16_000_000_000,
        process_count: processes.len(),
        processes,
        uptime_seconds: 587_625,
        sample_seconds: 1.0,
        host_name: "HEADLESS-FIXTURE".into(),
        os_name: "Windows fixture (not live telemetry)".into(),
        cpu: CpuInfo {
            brand: "Fixture processor with a long descriptive model name".into(),
            frequency_mhz: 4900,
            physical_cores: 24,
            logical_cores: 32,
        },
        disks: (0..8)
            .map(|index| DiskRow {
                name: format!("Fixture storage {index}"),
                mount: format!("{}:\\", (b'C' + index) as char),
                kind: "SSD".into(),
                file_system: "NTFS".into(),
                total_bytes: 2_000_000_000_000,
                available_bytes: 350_000_000_000,
                read_bytes_per_sec: 152_000_000.0,
                write_bytes_per_sec: 1_400_000.0,
                removable: false,
            })
            .collect(),
        networks: vec![NetworkRow {
            name: "Fixture Ethernet adapter".into(),
            received_bytes_per_sec: 1_832_000.0,
            transmitted_bytes_per_sec: 742_000.0,
            total_received_bytes: 6_000_000_000,
            total_transmitted_bytes: 940_000_000,
        }],
        gpu: GpuSnapshot {
            available: true,
            utilization_percent: 23.8,
            engine_utilization: vec![
                ("3D".into(), 23.8),
                ("Copy".into(), 1.2),
                ("Video Decode".into(), 5.7),
            ],
            error: None,
        },
        gpu_sensors: crate::gpu_sensors::SensorSnapshot {
            sampled_at: Some(std::time::Instant::now()),
            adapters: vec![crate::gpu_sensors::AdapterSensors {
                name: "Fixture NVIDIA GPU (test data)".into(),
                uuid: Some("FIXTURE-GPU-0".into()),
                temperature_c: Some(49),
                power_w: Some(86.74),
                graphics_clock_mhz: Some(2857),
                memory_clock_mhz: Some(14001),
                fan_percent: Some(0),
                memory: Some((7_924 * 1_048_576, 16_303 * 1_048_576)),
                memory_includes_reserved: false,
                error: None,
            }],
            query_millis: 0.42,
            error: None,
        },
        users: vec![UserSummary {
            name: "FIXTURE\\LongAccountName".into(),
            process_count: 64,
            cpu_percent: 37.2,
            gpu_percent: 23.8,
            memory_bytes: 41_000_000_000,
            disk_bytes_per_sec: 153_400_000.0,
        }],
        startup: (0..40)
            .map(|i| StartupRow {
                name: format!("Fixture startup {i}"),
                command: "C:\\Fixture\\Long Directory Name\\fixture.exe --test-only".into(),
                source: "HKCU Run (fixture)".into(),
            })
            .collect::<Vec<_>>()
            .into(),
        services: (0..40)
            .map(|i| ServiceRow {
                name: format!("FixtureService{i}"),
                display_name: format!("Fixture long service display name {i}"),
                status: "Running".into(),
                pid: 900_000 + i,
            })
            .collect::<Vec<_>>()
            .into(),
    }
}

fn app(settings: ThemeSettings, populated: bool) -> TrontopApp {
    let mut app = TrontopApp::with_services(settings, None, None);
    if populated {
        let mut snapshot = fixture();
        let sensor_start = std::time::Instant::now();
        for sequence in 1..=HISTORY_LENGTH {
            snapshot.sequence = sequence as u64;
            snapshot.cpu_percent = 31.0 + (sequence as f32 * 0.23).sin() * 16.0;
            snapshot.gpu_sensors.sampled_at =
                Some(sensor_start + std::time::Duration::from_secs(sequence as u64));
            snapshot.gpu_sensors.adapters[0].temperature_c = Some(49 + (sequence % 8) as u32);
            snapshot.gpu_sensors.adapters[0].power_w =
                Some(60.0 + (sequence as f32 * 0.2).sin() * 26.0);
            app.accept_sample(snapshot.clone());
        }
    }
    assert!(app.sampler.is_none() && app.tray.is_none());
    app
}

fn frame(
    ctx: &egui::Context,
    app: &mut TrontopApp,
    size: Vec2,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    let output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            events,
            ..Default::default()
        },
        |ui| app.ui(ui, &mut eframe::Frame::_new_kittest()),
    );
    // Commands are never executed by this harness. Also reject any unexpected
    // attempt to open external programs or manipulate a native viewport.
    assert!(output.platform_output.commands.is_empty());
    for viewport in output.viewport_output.values() {
        assert!(
            viewport.commands.is_empty(),
            "UI emitted native viewport commands"
        );
    }
    output
}

fn text_shapes(output: &egui::FullOutput) -> Vec<(&egui::epaint::TextShape, egui::Rect)> {
    fn visit<'a>(
        shape: &'a egui::Shape,
        clip: egui::Rect,
        out: &mut Vec<(&'a egui::epaint::TextShape, egui::Rect)>,
    ) {
        match shape {
            egui::Shape::Text(text) => out.push((text, clip)),
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    visit(shape, clip, out);
                }
            }
            _ => {}
        }
    }
    let mut texts = Vec::new();
    for shape in &output.shapes {
        visit(&shape.shape, shape.clip_rect, &mut texts);
    }
    texts
}

#[test]
fn all_pages_render_headlessly_across_sizes_themes_and_empty_data() {
    let mut cases = 0;
    for mut settings in [
        ThemeSettings::tront_stack(),
        ThemeSettings::demigod(),
        ThemeSettings::monke_portal(),
        ThemeSettings::copper_legacy(),
    ] {
        for dark in [true, false] {
            settings.dark = dark;
            for size in [
                Vec2::new(1040.0, 640.0),
                Vec2::new(1280.0, 760.0),
                Vec2::new(1920.0, 1080.0),
            ] {
                for populated in [true, false] {
                    let ctx = egui::Context::default();
                    theme::install(&ctx, settings);
                    let mut app = app(settings, populated);
                    for (page, _, name) in Page::ALL {
                        app.page = page;
                        let mut output = egui::FullOutput::default();
                        for _ in 0..3 {
                            output = frame(&ctx, &mut app, size, vec![]);
                        }
                        let texts = text_shapes(&output);
                        let title = if page == Page::History {
                            "Resource history"
                        } else {
                            name
                        };
                        assert!(
                            texts
                                .iter()
                                .any(|(text, clip)| text.galley.job.text == title
                                    && clip.contains_rect(text.visual_bounding_rect())),
                            "missing visible heading {title} at {size:?}"
                        );
                        for (text, _) in texts {
                            assert!(
                                text.pos.is_finite() && text.galley.size().is_finite(),
                                "non-finite text bounds on {name}"
                            );
                            if text.galley.job.text.contains("Fixture.Worker")
                                || text.galley.job.text == "FIXTURE\\LongAccountName"
                            {
                                assert_eq!(
                                    text.galley.rows.len(),
                                    1,
                                    "table label wrapped on {name}"
                                );
                            }
                        }
                        cases += 1;
                    }
                }
            }
        }
    }
    assert_eq!(cases, 336);
    println!("UI smoke: {cases} page/size/theme/data cases passed; no native windows or OS input");
}

#[test]
fn search_selection_device_pages_and_dialogs_render_without_native_services() {
    let ctx = egui::Context::default();
    let mut app = app(ThemeSettings::default(), true);
    theme::install(&ctx, app.theme);
    let size = Vec2::new(1040.0, 640.0);
    app.query = "Worker.With".into();
    app.rebuild_visible_processes();
    assert_eq!(app.visible_processes.len(), 63);
    assert_eq!(
        app.visible_process_tree.len(),
        64,
        "search preserves parent context"
    );
    app.selected_pid = Some(900_001);
    for _ in 0..3 {
        frame(&ctx, &mut app, size, vec![]);
    }
    app.page = Page::Performance;
    for device in [
        PerformanceDevice::Cpu,
        PerformanceDevice::Memory,
        PerformanceDevice::Disk(0),
        PerformanceDevice::Network(0),
        PerformanceDevice::Gpu,
        PerformanceDevice::GpuSensors,
        PerformanceDevice::Disk(99),
        PerformanceDevice::Network(99),
    ] {
        app.performance_device = device;
        for _ in 0..3 {
            frame(&ctx, &mut app, size, vec![]);
        }
    }
    for dialog in 0..5 {
        app.show_theme_editor = dialog == 0;
        app.show_run_task = dialog == 1;
        app.show_priority_editor = dialog == 2;
        app.show_affinity_editor = dialog == 3;
        app.pending_end_pid = (dialog == 4).then_some(900_001);
        for _ in 0..3 {
            frame(&ctx, &mut app, size, vec![]);
        }
    }
}

#[test]
fn navigation_and_selection_accept_local_pointer_input() {
    let ctx = egui::Context::default();
    let mut app = app(ThemeSettings::default(), true);
    theme::install(&ctx, app.theme);
    let size = Vec2::new(1280.0, 760.0);
    for (page, number, name) in Page::ALL {
        let output = frame(&ctx, &mut app, size, vec![]);
        let label = format!("{number}   {name}");
        let position = text_shapes(&output)
            .into_iter()
            .find(|(text, _)| text.galley.job.text == label)
            .unwrap()
            .0
            .visual_bounding_rect()
            .center();
        for pressed in [true, false] {
            frame(
                &ctx,
                &mut app,
                size,
                vec![
                    egui::Event::PointerMoved(position),
                    egui::Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        assert!(app.page == page, "navigation failed for {name}");
    }
    app.page = Page::Processes;
    let mut output = frame(&ctx, &mut app, size, vec![]);
    for _ in 0..2 {
        output = frame(&ctx, &mut app, size, vec![]);
    }
    let position = text_shapes(&output)
        .into_iter()
        .find(|(text, _)| text.galley.job.text.starts_with("Fixture.Editor.exe"))
        .unwrap()
        .0
        .visual_bounding_rect()
        .center();
    for pressed in [true, false] {
        frame(
            &ctx,
            &mut app,
            size,
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    assert_eq!(app.selected_pid, Some(900_000));
}

#[test]
fn compact_sidebar_preserves_footer_and_performance_details_are_scrollable() {
    let ctx = egui::Context::default();
    let mut app = app(ThemeSettings::default(), true);
    theme::install(&ctx, app.theme);
    app.page = Page::Performance;
    let size = Vec2::new(1040.0, 640.0);
    let mut output = frame(&ctx, &mut app, size, vec![]);
    for _ in 0..2 {
        output = frame(&ctx, &mut app, size, vec![]);
    }
    let texts = text_shapes(&output);
    let footer = texts
        .iter()
        .find(|(text, _)| text.galley.job.text.starts_with("Native telemetry"))
        .unwrap()
        .0
        .visual_bounding_rect();
    let gpu = texts
        .iter()
        .find(|(text, _)| text.galley.job.text == "GPU" && text.pos.x < 196.0)
        .unwrap();
    assert!(gpu.1.contains_rect(gpu.0.visual_bounding_rect()));
    assert!(gpu.0.visual_bounding_rect().bottom() < footer.top());
    let system_visible = |output: &egui::FullOutput| {
        text_shapes(output).into_iter().any(|(text, clip)| {
            text.galley.job.text == "Operating system"
                && clip.contains_rect(text.visual_bounding_rect())
        })
    };
    assert!(
        !system_visible(&output),
        "fixture must initially require scrolling"
    );
    frame(
        &ctx,
        &mut app,
        size,
        vec![
            egui::Event::PointerMoved(egui::pos2(700.0, 440.0)),
            egui::Event::MouseWheel {
                phase: egui::TouchPhase::Move,
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, -600.0),
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    for _ in 0..30 {
        output = frame(&ctx, &mut app, size, vec![]);
    }
    assert!(
        system_visible(&output),
        "bottom details must be reachable by local scroll input"
    );
}

#[test]
fn sensor_states_render_without_wrapping_or_fabricated_readings() {
    for dark in [true, false] {
        for size in [Vec2::new(1040.0, 640.0), Vec2::new(1280.0, 760.0)] {
            for state in 0..4 {
                let settings = ThemeSettings {
                    dark,
                    ..Default::default()
                };
                let ctx = egui::Context::default();
                theme::install(&ctx, settings);
                let mut app = app(settings, true);
                app.page = Page::Performance;
                app.performance_device = PerformanceDevice::GpuSensors;
                match state {
                    1 => {
                        let adapter = &mut app.snapshot.gpu_sensors.adapters[0];
                        adapter.power_w = None;
                        adapter.fan_percent = None;
                        adapter.memory = None;
                    }
                    2 => {
                        app.snapshot.gpu_sensors.adapters.clear();
                        app.snapshot.gpu_sensors.error =
                            Some("Fixture: driver not installed".into());
                    }
                    3 => {
                        app.snapshot
                            .gpu_sensors
                            .adapters
                            .push(crate::gpu_sensors::AdapterSensors {
                            name:
                                "Fixture second adapter with a deliberately very long hardware name"
                                    .into(),
                            uuid: Some("FIXTURE-GPU-1".into()),
                            ..Default::default()
                        })
                    }
                    _ => {}
                }
                let mut output = frame(&ctx, &mut app, size, vec![]);
                for _ in 0..3 {
                    output = frame(&ctx, &mut app, size, vec![]);
                }
                let texts = text_shapes(&output);
                let visible = |label: &str| {
                    texts.iter().any(|(text, clip)| {
                        text.galley.job.text == label
                            && clip.contains_rect(text.visual_bounding_rect())
                    })
                };
                assert!(visible("GPU sensors"));
                if state == 2 {
                    assert!(visible("Hardware sensors unavailable"));
                    assert!(!visible("GPU temperature"));
                } else {
                    assert!(visible("GPU temperature"));
                    assert!(visible("Board power"));
                    if state == 1 {
                        assert!(visible("Unavailable"));
                    }
                }
                for (text, _) in texts {
                    if text.galley.job.text.ends_with("MHz")
                        || text.galley.job.text == "Unavailable"
                        || text.galley.job.text.ends_with("GiB")
                    {
                        assert_eq!(text.galley.rows.len(), 1, "sensor value wrapped");
                    }
                }
            }
        }
    }
}

#[test]
fn sensor_histories_follow_identity_not_enumeration_order_and_drop_stale_values() {
    let mut app = TrontopApp::with_services(ThemeSettings::default(), None, None);
    let mut snapshot = fixture();
    let start = snapshot.gpu_sensors.sampled_at.unwrap();
    let mut second = snapshot.gpu_sensors.adapters[0].clone();
    second.uuid = Some("FIXTURE-GPU-1".into());
    second.temperature_c = Some(70);
    snapshot.gpu_sensors.adapters.push(second);
    app.accept_sample(snapshot.clone());
    snapshot.gpu_sensors.adapters.reverse();
    snapshot.gpu_sensors.sampled_at = Some(start + std::time::Duration::from_secs(1));
    app.accept_sample(snapshot.clone());
    assert_eq!(app.sensor_history["FIXTURE-GPU-0"].peak(false), Some(49.0));
    assert_eq!(app.sensor_history["FIXTURE-GPU-1"].peak(false), Some(70.0));
    snapshot.gpu_sensors.adapters.clear();
    snapshot.gpu_sensors.sampled_at = Some(start + std::time::Duration::from_secs(2));
    app.accept_sample(snapshot.clone());
    assert!(
        app.sensor_history["FIXTURE-GPU-0"]
            .points
            .back()
            .unwrap()
            .temperature_c
            .is_none()
    );
    snapshot.gpu_sensors.sampled_at = Some(start + std::time::Duration::from_secs(130));
    app.accept_sample(snapshot);
    assert!(app.sensor_history.is_empty());
}

#[test]
#[ignore = "offscreen GPU visual QA; writes test-only PNGs under target/ui-smoke, never opens a window"]
fn render_offscreen_visual_pass() {
    let mut renderer = offscreen::Renderer::new();
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/ui-smoke");
    std::fs::create_dir_all(&directory).unwrap();
    for (index, (page, _, name)) in Page::ALL.into_iter().enumerate() {
        let settings = [
            ThemeSettings::tront_stack(),
            ThemeSettings::demigod(),
            ThemeSettings::monke_portal(),
            ThemeSettings::copper_legacy(),
        ][index % 4];
        let ctx = egui::Context::default();
        theme::install(&ctx, settings);
        let mut app = app(settings, true);
        app.page = page;
        let size = Vec2::new(1040.0, 640.0);
        let mut output = egui::FullOutput::default();
        for _ in 0..3 {
            output.append(frame(&ctx, &mut app, size, vec![]));
        }
        renderer.save(
            &ctx,
            output,
            size,
            &directory.join(format!("{}.png", name.to_lowercase())),
        );
    }
    for variant in [
        "performance-light",
        "performance-hover",
        "theme-studio",
        "inspector",
        "gpu-sensors",
        "gpu-sensors-light",
        "gpu-sensors-compact",
        "gpu-sensors-unavailable",
    ] {
        let ctx = egui::Context::default();
        let settings = ThemeSettings {
            dark: variant != "performance-light" && variant != "gpu-sensors-light",
            ..Default::default()
        };
        theme::install(&ctx, settings);
        let mut app = app(settings, true);
        app.page = if variant == "inspector" {
            Page::Processes
        } else {
            Page::Performance
        };
        app.selected_pid = (variant == "inspector").then_some(900_001);
        app.show_theme_editor = variant == "theme-studio";
        if variant.starts_with("gpu-sensors") {
            app.performance_device = PerformanceDevice::GpuSensors;
        }
        if variant == "gpu-sensors-unavailable" {
            app.snapshot.gpu_sensors.adapters.clear();
            app.snapshot.gpu_sensors.error =
                Some("Fixture: NVIDIA driver unavailable. Other telemetry still works.".into());
        }
        let size = if variant == "gpu-sensors-compact" {
            Vec2::new(1040.0, 640.0)
        } else {
            Vec2::new(1280.0, 760.0)
        };
        let mut output = egui::FullOutput::default();
        for _ in 0..20 {
            let events = if variant == "performance-hover" {
                vec![egui::Event::PointerMoved(egui::pos2(310.0, 258.0))]
            } else {
                vec![]
            };
            output.append(frame(&ctx, &mut app, size, events));
        }
        renderer.save(
            &ctx,
            output,
            size,
            &directory.join(format!("{variant}.png")),
        );
    }
    println!(
        "Offscreen visual pass: 15 PNGs in {}; no native window or OS input",
        directory.display()
    );
}
