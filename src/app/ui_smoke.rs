//! Exercises the production UI without a sampler, tray, native window, or OS input.
//! All telemetry below is deliberately synthetic TEST DATA, never a runtime fallback.
use super::*;
use crate::model::{
    CpuInfo, DiskRow, GpuSnapshot, NetworkRow, ServiceRow, StartupRow, UserSummary,
};
use eframe::App;

mod actions;
mod compact_layout;
mod contrast;
mod cpu_clock;
mod disks;
mod export;
mod failure;
mod gauntlet;
mod gpu_adapters;
mod graphs;
mod memory;
mod offscreen;
mod overview;
mod preferences;
mod process_perf;
mod process_sort;
mod renderer_recovery;
mod service_retention;
mod system;
mod table_layout;

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
            gpu_percent: crate::gpu_activity::Usage::Measured(index as f32 * 0.05),
            memory_bytes: 256_000_000 + index as u64 * 17_000_000,
            accumulated_cpu_millis: index as u64 * 87_600,
            started_at_unix: 1_700_000_000,
            control: crate::model::ProcessControlInfo {
                created_at_100ns: Some(133_444_736_000_000_000 + index as u64),
                priority: PriorityClass::Normal,
                affinity_mask: 0xff,
                system_affinity_mask: 0xff,
                accessible: true,
            },
            command: "fixture.exe --headless-test-only --deliberately-long-command-line".into(),
            executable: Some(std::path::PathBuf::from(format!(
                r"C:\Fixture\app-{}.exe",
                index % 4
            ))),
            ..Default::default()
        })
        .collect();
    let mut diagnostics = crate::diagnostics::Diagnostics::default();
    // Fixtures do not age into stale state while a slow test suite renders.
    let fixture_at = std::time::Instant::now() + std::time::Duration::from_secs(3600);
    for provider in crate::diagnostics::Provider::ALL {
        diagnostics.get_mut(provider).record(
            fixture_at,
            std::time::Duration::from_micros(420),
            crate::diagnostics::State::Live,
            None,
            None,
        );
    }
    SystemSnapshot {
        diagnostics,
        sequence: 1,
        cpu_percent: 37.2,
        memory_used_bytes: 41_000_000_000,
        memory_total_bytes: 64_000_000_000,
        memory_available_bytes: 23_000_000_000,
        memory_details: Some(crate::memory_metrics::Values {
            commit_bytes: 57_000_000_000,
            commit_limit_bytes: 96_000_000_000,
            commit_peak_bytes: 62_000_000_000,
            physical_total_bytes: 64_000_000_000,
            system_cache_bytes: 8_000_000_000,
            kernel_paged_bytes: 1_700_000_000,
            kernel_nonpaged_bytes: 1_300_000_000,
        }),
        process_count: processes.len(),
        processes,
        uptime_seconds: 587_625,
        sample_seconds: 1.0,
        host_name: "HEADLESS-FIXTURE".into(),
        os_name: "Windows fixture (not live telemetry)".into(),
        cpu: CpuInfo {
            brand: "Fixture processor with a long descriptive model name".into(),
            frequency_mhz: 4900,
            clocks: Some(cpu_clock::values(32, 0.0)),
            physical_cores: 24,
            logical_cores: 32,
            logical_usage: (0..32).map(|i| Some((i * 7 % 101) as f32)).collect(),
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
        physical_disks: Default::default(),
        gpu: GpuSnapshot {
            adapters: Vec::new(),
            available: true,
            valid_counters: 32,
            total_counters: 32,
            utilization_percent: 23.8,
            engine_utilization: vec![
                ("3D".into(), crate::gpu_activity::Usage::Measured(23.8)),
                ("Copy".into(), crate::gpu_activity::Usage::Measured(1.2)),
                (
                    "Video Decode".into(),
                    crate::gpu_activity::Usage::Measured(5.7),
                ),
            ],
            error: None,
        },
        gpu_sensors: crate::gpu_sensors::SensorSnapshot {
            last_success: Some(std::time::Instant::now()),
            using_cached: false,
            sampled_at: Some(std::time::Instant::now()),
            attempted_at: Some(std::time::Instant::now()),
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
        storage_sensors: std::sync::Arc::new(crate::storage_sensors::Snapshot {
            inventory_at: Some(fixture_at),
            inventory_error: None,
            drives: vec![crate::storage_sensors::DriveReading {
                device: crate::storage_sensors::Device {
                    id: "PRIVATE-FIXTURE-STORAGE-INTERFACE".into(),
                    name: "Fixture NVMe drive (test data)".into(),
                },
                temperatures: crate::storage_sensors::Temperatures {
                    warning: Some(90),
                    critical: Some(95),
                    sensors: (0..3)
                        .map(|index| crate::storage_sensors::Temperature {
                            index,
                            celsius: Some(44 - index as i16),
                            over_threshold: Some(90),
                            under_threshold: None,
                            event: false,
                        })
                        .collect(),
                },
                last_attempt: Some(fixture_at),
                last_success: Some(fixture_at),
                query_millis: Some(3.72),
                error: None,
                present: true,
            }],
        }),
        users: vec![UserSummary {
            name: "FIXTURE\\LongAccountName".into(),
            process_count: 64,
            cpu_percent: 37.2,
            gpu_percent: crate::gpu_activity::Usage::Measured(23.8),
            memory_bytes: 41_000_000_000,
            disk_bytes_per_sec: 153_400_000.0,
        }],
        startup: {
            let mut startup = crate::startup::Snapshot::default();
            startup.apply(
                crate::startup::Source::ALL
                    .into_iter()
                    .map(|source| crate::startup::Read {
                        source,
                        state: crate::startup::ReadState::Readable,
                        rows: (0..8)
                            .map(|i| StartupRow {
                                key: format!("Fixture startup {i}"),
                                name: format!("Fixture startup {i}"),
                                command:
                                    "C:\\Fixture\\Long Directory Name\\fixture.exe --test-only"
                                        .into(),
                                source,
                            })
                            .collect(),
                    })
                    .collect(),
                fixture_at,
            );
            std::sync::Arc::new(startup)
        },
        services: (0..40)
            .map(|i| ServiceRow {
                name: format!("FixtureService{i}"),
                display_name: format!("Fixture long service display name {i}"),
                status: crate::service_control::Status {
                    state: crate::service_control::State::Running,
                    pid: 900_000 + i,
                    accepts_stop: true,
                    win32_service: true,
                    ..Default::default()
                },
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

/// Deliberately mixed fixture coverage, including a partly measured parent group.
fn gpu_activity_fixture(app: &mut TrontopApp) {
    use crate::gpu_activity::Usage;
    let mut snapshot = fixture();
    let readings = [
        Usage::Measured(0.0),
        Usage::Measured(12.34),
        Usage::Partial(4.56),
        Usage::Unavailable,
        Usage::Warming,
        Usage::Unreported,
    ];
    snapshot.processes.truncate(readings.len());
    snapshot.process_count = readings.len();
    snapshot.users.clear();
    for (index, (process, reading)) in snapshot.processes.iter_mut().zip(readings).enumerate() {
        process.name = format!("Fixture.Gpu.{}.exe", index + 1);
        process.gpu_percent = reading;
        snapshot.users.push(UserSummary {
            name: format!("Fixture account {}", index + 1),
            process_count: 1,
            gpu_percent: reading,
            ..Default::default()
        });
    }
    snapshot.gpu.valid_counters = 24;
    snapshot.gpu.total_counters = 32;
    snapshot.gpu.engine_utilization[0].1 = Usage::Partial(23.8);
    app.sort_column = SortColumn::Pid;
    app.sort_direction = SortDirection::Ascending;
    app.accept_sample(snapshot);
}

/// Synthetic artwork only, never invokes native extraction or supplies runtime icons.
fn install_fixture_icons(app: &mut TrontopApp, ctx: &egui::Context) {
    for index in 0..3 {
        let mut pixels = vec![0; 32 * 32 * 4];
        for y in 3usize..29 {
            for x in 3usize..29 {
                let edge = x.min(31 - x).min(y.min(31 - y));
                if edge < 5 && !(5..=26).contains(&x) && !(5..=26).contains(&y) {
                    continue;
                }
                let color = match index {
                    0 => [32, 186, 177, 255],
                    1 => [126, 76, 203, 255],
                    _ => [239, 151, 55, 255],
                };
                let color =
                    if (8..=23).contains(&x) && ((8..=11).contains(&y) || (20..=23).contains(&y)) {
                        [235, 242, 248, 255]
                    } else {
                        color
                    };
                pixels[(y * 32 + x) * 4..(y * 32 + x + 1) * 4].copy_from_slice(&color);
            }
        }
        app.process_icons.fixture_icon(
            ctx,
            &std::path::PathBuf::from(format!(r"C:\Fixture\app-{index}.exe")),
            pixels,
        );
    }
}

fn inventory_state_fixture(app: &mut TrontopApp, page: Page, state: u8) {
    use crate::diagnostics::{Health, Issue, Provider, State};
    use crate::startup::{Read, ReadState, Snapshot, Source};
    use std::time::{Duration, Instant};
    let at = Instant::now() + Duration::from_secs(3600);
    let mut health = Health::default();
    let mut startup = (*app.snapshot.startup).clone();
    let live = || {
        startup
            .sources
            .iter()
            .map(|s| Read {
                source: s.source,
                state: ReadState::Readable,
                rows: s.entries.iter().map(|e| e.row.clone()).collect(),
            })
            .collect::<Vec<_>>()
    };
    match state {
        0 | 3 => {
            startup.apply(live(), at);
            health.record(at, Duration::ZERO, State::Live, None, None);
        }
        1 => {
            let mut reads = live();
            reads[0].state = ReadState::Failed;
            reads[0].rows.truncate(1);
            reads[1].state = ReadState::Failed;
            reads[1].rows.clear();
            reads[2].state = ReadState::Missing;
            reads[2].rows.clear();
            reads[3].rows.clear();
            startup.apply(reads, at);
            health.record(
                at,
                Duration::ZERO,
                State::Partial,
                Some((3, 5)),
                Some(Issue::StartupSources),
            );
        }
        2 | 6 => {
            health.record(
                at - Duration::from_secs(30),
                Duration::ZERO,
                State::Live,
                None,
                None,
            );
            startup.apply(vec![], at);
            health.record(
                at,
                Duration::ZERO,
                State::Unavailable,
                None,
                Some(Issue::ServiceQuery),
            );
            if state == 6 {
                for source in &mut startup.sources {
                    source.last_attempt = Some(Instant::now() - Duration::from_secs(120));
                }
            }
        }
        4 | 5 => {
            startup = Snapshot::default();
            app.snapshot.services = std::sync::Arc::new(Vec::new());
            if state == 4 {
                startup.apply(vec![], at);
                health.record(at, Duration::ZERO, State::Unavailable, None, None);
            }
        }
        7 => {
            startup.apply(
                Source::ALL
                    .into_iter()
                    .map(|source| Read {
                        source,
                        state: ReadState::Readable,
                        rows: Vec::new(),
                    })
                    .collect(),
                at,
            );
            app.snapshot.services = std::sync::Arc::new(Vec::new());
            health.record(at, Duration::ZERO, State::Live, None, None);
        }
        _ => unreachable!(),
    }
    app.snapshot.startup = std::sync::Arc::new(startup);
    *app.snapshot.diagnostics.get_mut(if page == Page::Startup {
        Provider::Startup
    } else {
        Provider::Services
    }) = health;
    app.page = page;
}

fn frame(
    ctx: &egui::Context,
    app: &mut TrontopApp,
    size: Vec2,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    checked_frame(ctx, app, size, events, false)
}

fn checked_frame(
    ctx: &egui::Context,
    app: &mut TrontopApp,
    size: Vec2,
    events: Vec<egui::Event>,
    allow_copy: bool,
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
    for command in &output.platform_output.commands {
        assert!(
            allow_copy && matches!(command, egui::OutputCommand::CopyText(_)),
            "unexpected platform command"
        );
    }
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
                        // The page title lives in the command bar.
                        let title = name;
                        assert!(
                            texts
                                .iter()
                                .any(|(text, clip)| text.galley.job.text == title
                                    && text.pos.x >= 196.0
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
    assert_eq!(cases, 48 * Page::ALL.len());
    println!("UI smoke: {cases} page/size/theme/data cases passed; no native windows or OS input");
}

#[test]
fn about_report_is_only_emitted_on_explicit_copy_and_excludes_private_fixture_fields() {
    let ctx = egui::Context::default();
    let mut app = app(ThemeSettings::default(), true);
    theme::install(&ctx, app.theme);
    app.show_diagnostics = true;
    let size = Vec2::new(1280.0, 900.0);
    let mut output = frame(&ctx, &mut app, size, vec![]);
    for _ in 0..3 {
        output = frame(&ctx, &mut app, size, vec![]);
    }
    let position = text_shapes(&output)
        .into_iter()
        .find(|(text, _)| text.galley.job.text == "Copy support report")
        .unwrap()
        .0
        .visual_bounding_rect()
        .center();
    let event = |pressed| {
        vec![
            egui::Event::PointerMoved(position),
            egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    };
    frame(&ctx, &mut app, size, event(true));
    // Only inspect the copy command. Never forward it to the real OS clipboard.
    let output = checked_frame(&ctx, &mut app, size, event(false), true);
    let [egui::OutputCommand::CopyText(report)] = output.platform_output.commands.as_slice() else {
        panic!("expected exactly one copy command");
    };
    assert!(report.contains("Build:") && report.contains("System telemetry: Live"));
    for secret in [
        &app.snapshot.host_name,
        &app.snapshot.processes[0].name,
        &app.snapshot.processes[0].user,
        &app.snapshot.processes[0].command,
        &app.snapshot.startup.sources[0].entries[0].row.command,
        app.snapshot.gpu_sensors.adapters[0].uuid.as_ref().unwrap(),
    ] {
        assert!(
            !report.contains(secret),
            "private fixture field entered report"
        );
    }
}

#[test]
fn sensor_fields_keep_geometry_when_data_is_missing_or_cached_and_cache_does_not_extend_history() {
    let ctx = egui::Context::default();
    let mut app = app(ThemeSettings::default(), true);
    theme::install(&ctx, app.theme);
    app.page = Page::Performance;
    app.performance_device = PerformanceDevice::GpuSensors;
    let size = Vec2::new(1280.0, 900.0);
    let mut bounds = Vec::new();
    for state in 0..3 {
        if state == 1 {
            let mut snapshot = app.snapshot.clone();
            snapshot.sequence += 1;
            snapshot.gpu_sensors.sampled_at = snapshot
                .gpu_sensors
                .sampled_at
                .map(|at| at + std::time::Duration::from_secs(1));
            snapshot.gpu_sensors.using_cached = true;
            app.accept_sample(snapshot);
            assert!(
                app.sensor_history["FIXTURE-GPU-0"]
                    .points
                    .back()
                    .unwrap()
                    .temperature_c
                    .is_none()
            );
        } else if state == 2 {
            app.snapshot.gpu_sensors.adapters.clear();
            app.snapshot.gpu_sensors.using_cached = false;
        }
        let mut output = frame(&ctx, &mut app, size, vec![]);
        for _ in 0..3 {
            output = frame(&ctx, &mut app, size, vec![]);
        }
        let texts = text_shapes(&output);
        let current: Vec<_> = [
            "GPU temperature",
            "Board power",
            "Graphics clock",
            "Memory clock",
            "Fan target",
            "VRAM used",
        ]
        .into_iter()
        .map(|label| {
            texts
                .iter()
                .find(|(text, _)| text.galley.job.text == label)
                .unwrap()
                .0
                .visual_bounding_rect()
        })
        .collect();
        if state == 0 {
            bounds = current;
        } else {
            assert_eq!(
                current, bounds,
                "sensor field layout shifted at state {state}"
            );
        }
        if state == 1 {
            assert!(
                texts
                    .iter()
                    .any(|(text, _)| text.galley.job.text == "Cached")
            );
        }
    }
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
        app.pending_end_task = (dialog == 4).then(|| PendingEndTask {
            identity: app.snapshot.processes[1].identity().unwrap(),
            name: app.snapshot.processes[1].name.clone(),
        });
        for _ in 0..3 {
            frame(&ctx, &mut app, size, vec![]);
        }
    }
}

#[test]
fn performance_rail_lists_devices_in_the_polish_gauntlet_order() {
    let ctx = egui::Context::default();
    let mut app = app(ThemeSettings::default(), true);
    theme::install(&ctx, app.theme);
    let now = std::time::Instant::now();
    app.snapshot.physical_disks = std::sync::Arc::new(crate::disk_activity::Snapshot {
        generation: 2,
        at: Some(now),
        devices: vec![
            crate::disk_activity::Device {
                instance: "0 C: (fixture)".into(),
                number: 0,
                readings: [51.0, 3.0, 1.0, 10_000_000.0, 2_000_000.0].map(|v| {
                    crate::disk_activity::Reading {
                        value: Some(v),
                        at: Some(now),
                    }
                }),
            },
            crate::disk_activity::Device {
                instance: "1 D: (fixture)".into(),
                number: 1,
                readings: [12.0, 1.0, 0.0, 500_000.0, 100_000.0].map(|v| {
                    crate::disk_activity::Reading {
                        value: Some(v),
                        at: Some(now),
                    }
                }),
            },
        ],
        ..Default::default()
    });
    app.page = Page::Performance;
    let output = frame(&ctx, &mut app, Vec2::new(1280.0, 900.0), vec![]);
    let shapes = text_shapes(&output);
    // CPU, Memory, GPU, GPU thermals, each physical disk, then mounted
    // volumes, then network adapters, top to bottom.
    let expected = [
        "CPU",
        "Memory",
        "GPU",
        "GPU thermals",
        "Disk 0",
        "Disk 1",
        "Volume C:\\",
        "Volume D:\\",
        "Fixture Ethernet adapter",
    ];
    let mut previous_y = f32::MIN;
    for label in expected {
        // The rail sits in its own column, right of the nav sidebar and left
        // of the content pane; "CPU"/"GPU" also appear as sidebar meter and
        // content-pane heading text outside that column.
        let y = shapes
            .iter()
            .find(|(s, _)| s.galley.job.text == label && s.pos.x > 210.0 && s.pos.x < 450.0)
            .unwrap_or_else(|| panic!("rail label {label:?} missing"))
            .0
            .pos
            .y;
        assert!(
            y > previous_y,
            "{label:?} at y={y} is out of order (previous y={previous_y})"
        );
        previous_y = y;
    }
}

#[test]
fn cpu_detail_has_no_badge_row_and_defaults_to_total_cpu() {
    let ctx = egui::Context::default();
    let mut app = app(ThemeSettings::default(), true);
    theme::install(&ctx, app.theme);
    app.page = Page::Performance;
    app.performance_device = PerformanceDevice::Cpu;
    assert!(!app.graphs.cpu_all_cores, "Total CPU is the default view");
    for all_cores in [false, true] {
        app.graphs.cpu_all_cores = all_cores;
        let mut output = frame(&ctx, &mut app, Vec2::new(1280.0, 800.0), vec![]);
        for _ in 0..2 {
            output = frame(&ctx, &mut app, Vec2::new(1280.0, 800.0), vec![]);
        }
        for (text, _) in text_shapes(&output) {
            let text = &text.galley.job.text;
            for banned in [
                "UTILIZATION",
                "UPTIME",
                "AVG CLOCK",
                "CPU clocks: Live",
                "Operating system",
                "logical processors / 120 seconds",
            ] {
                assert!(
                    !text.contains(banned),
                    "CPU detail still shows {banned:?} in {text:?}"
                );
            }
        }
    }
}

#[test]
fn all_cores_grid_leaves_no_orphan_row() {
    let ctx = egui::Context::default();
    let mut app = app(ThemeSettings::default(), true);
    theme::install(&ctx, app.theme);
    app.page = Page::Performance;
    app.performance_device = PerformanceDevice::Cpu;
    app.graphs.cpu_all_cores = true;
    app.graphs.draw_all_rows = true;
    for (cores, size) in [
        // Width picks the column count; a tall window keeps every row on
        // screen so each label is counted.
        (24, Vec2::new(1280.0, 2400.0)),
        (24, Vec2::new(1000.0, 2400.0)),
        (24, Vec2::new(1600.0, 2400.0)),
        (16, Vec2::new(1280.0, 2400.0)),
    ] {
        app.snapshot.cpu.logical_cores = cores;
        let mut output = frame(&ctx, &mut app, size, vec![]);
        for _ in 0..3 {
            output = frame(&ctx, &mut app, size, vec![]);
        }
        let mut rows = std::collections::BTreeMap::<i32, usize>::new();
        for (text, _) in text_shapes(&output) {
            let label = &text.galley.job.text;
            if let Some(index) = label
                .strip_prefix("CPU ")
                .and_then(|n| n.parse::<usize>().ok())
                && index < cores
                && text.pos.x > 400.0
            {
                *rows.entry(text.pos.y.round() as i32).or_default() += 1;
            }
        }
        let counts: Vec<_> = rows.values().copied().collect();
        assert_eq!(
            counts.iter().sum::<usize>(),
            cores,
            "{cores} cores at {size:?}"
        );
        assert!(
            counts.windows(2).all(|pair| pair[0] == pair[1]),
            "{cores} cores at {size:?} left an orphan row: {counts:?}"
        );
    }
}

#[test]
fn physical_disks_graphs_label_their_axes_with_units_not_max() {
    let ctx = egui::Context::default();
    let mut app = app(ThemeSettings::default(), true);
    theme::install(&ctx, app.theme);
    disks::install(&mut app, false);
    let size = Vec2::new(1280.0, 900.0);
    let mut output = frame(&ctx, &mut app, size, vec![]);
    for _ in 0..3 {
        output = frame(&ctx, &mut app, size, vec![]);
    }
    let texts = text_shapes(&output);
    assert!(
        !texts
            .iter()
            .any(|(s, _)| s.galley.job.text.starts_with("MAX ")),
        "a unitless MAX label is back on Physical disks"
    );
    for unit in ["100%", " ms", " req"] {
        assert!(
            texts.iter().any(|(s, _)| s.galley.job.text.ends_with(unit)),
            "no axis label ends with {unit:?}"
        );
    }
    // No routine "Live" caption under the metric tiles, and no stray raw PDH
    // instance line; the hero reads "Disk 0 · C: D:".
    assert!(!texts.iter().any(|(s, _)| s.galley.job.text == "Live"));
    assert!(
        !texts
            .iter()
            .any(|(s, _)| s.galley.job.text.starts_with("0 C: D:"))
    );
    assert!(
        texts
            .iter()
            .any(|(s, _)| s.galley.job.text == "Disk 0 \u{b7} C: D:")
    );
}

#[test]
fn network_performance_graph_label_carries_a_rate_unit() {
    let ctx = egui::Context::default();
    let mut app = app(ThemeSettings::default(), true);
    theme::install(&ctx, app.theme);
    app.page = Page::Performance;
    app.performance_device = PerformanceDevice::Network(0);
    let output = frame(&ctx, &mut app, Vec2::new(1280.0, 900.0), vec![]);
    let content = text_shapes(&output)
        .into_iter()
        .filter(|(s, _)| s.pos.x > 400.0)
        .collect::<Vec<_>>();
    assert!(
        content
            .iter()
            .any(|(s, _)| s.galley.job.text.ends_with("B/s")),
        "the graph's max label should carry a rate unit, matching the header rate"
    );
    assert!(
        !content
            .iter()
            .any(|(s, _)| s.galley.job.text.starts_with("MAX ")
                && !s.galley.job.text.ends_with("B/s")),
        "a unitless MAX label means the graph and header rates mismatch again"
    );
}

#[test]
fn volume_graph_draws_read_and_write_with_a_legend() {
    let ctx = egui::Context::default();
    let mut app = app(ThemeSettings::default(), true);
    theme::install(&ctx, app.theme);
    app.page = Page::Performance;
    app.performance_device = PerformanceDevice::Disk(0);
    let mut output = frame(&ctx, &mut app, Vec2::new(1280.0, 900.0), vec![]);
    for _ in 0..2 {
        output = frame(&ctx, &mut app, Vec2::new(1280.0, 900.0), vec![]);
    }
    let content: Vec<_> = text_shapes(&output)
        .into_iter()
        .filter(|(s, _)| s.pos.x > 450.0)
        .collect();
    // The legend sits in the plot's top strip, above the Read / Write tiles.
    let tile = |label: &str| {
        content
            .iter()
            .find(|(s, _)| s.galley.job.text == label)
            .unwrap_or_else(|| panic!("{label} tile missing"))
            .0
            .pos
            .y
    };
    for label in ["Read", "Write"] {
        let tile_y = tile(label);
        assert!(
            content
                .iter()
                .any(|(s, _)| s.galley.job.text.starts_with(label)
                    && s.pos.y < tile_y - 40.0
                    && s.galley.job.text.len() <= label.len() + 14),
            "the volume graph legend has no {label} entry"
        );
    }
    assert!(
        content
            .iter()
            .any(|(s, _)| s.galley.job.text.ends_with("B/s")
                && s.galley
                    .job
                    .text
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_digit())),
        "the volume axis carries a rate unit"
    );
}

#[test]
fn performance_rail_scrolls_a_selected_device_below_the_fold_into_view() {
    let ctx = egui::Context::default();
    let mut app = app(ThemeSettings::default(), true);
    theme::install(&ctx, app.theme);
    let size = Vec2::new(1000.0, 580.0);
    app.page = Page::Performance;
    let last = app.snapshot.networks.len() - 1;
    let name = app.snapshot.networks[last].name.clone();
    let rail_item = |output: &egui::FullOutput| {
        text_shapes(output)
            .into_iter()
            .find(|(s, _)| s.galley.job.text == name && s.pos.x > 200.0 && s.pos.x < 420.0)
            .map(|(s, clip)| (s.visual_bounding_rect(), clip))
    };
    let mut output = frame(&ctx, &mut app, size, vec![]);
    for _ in 0..2 {
        output = frame(&ctx, &mut app, size, vec![]);
    }
    let (rect, clip) = rail_item(&output).expect("rail lists the last adapter");
    assert!(
        !clip.contains_rect(rect),
        "fixture precondition: the last rail device starts below the fold"
    );
    app.performance_device = PerformanceDevice::Network(last);
    for _ in 0..3 {
        output = frame(&ctx, &mut app, size, vec![]);
    }
    let (rect, clip) = rail_item(&output).expect("rail lists the last adapter");
    assert!(
        clip.contains_rect(rect),
        "selected rail item {rect:?} is outside the rail clip {clip:?}"
    );
    // Following happens once per selection: scrolling the rail back up by
    // hand is not undone on the next frames.
    let followed = app.rail_followed.clone();
    for _ in 0..2 {
        frame(&ctx, &mut app, size, vec![]);
    }
    assert!(app.rail_followed == followed);
}

#[test]
fn navigation_and_selection_accept_local_pointer_input() {
    let ctx = egui::Context::default();
    let mut app = app(ThemeSettings::default(), true);
    theme::install(&ctx, app.theme);
    let size = Vec2::new(1280.0, 760.0);
    for (page, _, name) in Page::ALL {
        let output = frame(&ctx, &mut app, size, vec![]);
        let position = nav_entry(&output, name).expect("nav entry");
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
    // The tall All cores view is what must scroll to reach the clock details.
    app.graphs.cpu_all_cores = true;
    let size = Vec2::new(1040.0, 640.0);
    let mut output = frame(&ctx, &mut app, size, vec![]);
    for _ in 0..2 {
        output = frame(&ctx, &mut app, size, vec![]);
    }
    let texts = text_shapes(&output);
    let footer = texts
        .iter()
        // Host name and sample interval share one footer line.
        .find(|(text, _)| text.galley.job.text.contains('\u{b7}') && text.pos.x < 196.0)
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
            text.galley.job.text == "Per-processor clocks"
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
                // All-core mode is taller than the old aggregate-only page.
                // Test reaching the bottom, not an obsolete content height.
                delta: Vec2::new(0.0, -200_000.0),
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
fn end_confirmation_keeps_original_identity_and_selection_expires_on_pid_reuse() {
    let ctx = egui::Context::default();
    let mut app = app(ThemeSettings::default(), true);
    theme::install(&ctx, app.theme);
    app.selected_pid = Some(900_001);
    app.request_end_selected(); // stages a confirmation only; no native calls
    let original = app.pending_end_task.clone().unwrap();
    let mut snapshot = app.snapshot.clone();
    let replacement = &mut snapshot.processes[1];
    replacement.name = "Replacement.Must.Never.Be.Targeted.exe".into();
    replacement.control.created_at_100ns = Some(original.identity.created_at_100ns + 1);
    app.accept_sample(snapshot);
    assert!(app.selected_pid.is_none());
    assert_eq!(
        app.pending_end_task.as_ref().unwrap().identity,
        original.identity
    );
    let size = Vec2::new(1040.0, 640.0);
    let mut output = frame(&ctx, &mut app, size, vec![]);
    for _ in 0..20 {
        output = frame(&ctx, &mut app, size, vec![]);
    }
    let texts = text_shapes(&output);
    assert!(
        texts
            .iter()
            .any(|(text, _)| text.galley.job.text == original.name)
    );
    assert!(texts.iter().any(|(text, _)| text.galley.job.text
        == "The original process exited or changed. Cancel and select again."));
    // Only click Cancel. The production destructive action is never invoked by
    // the UI harness; native action tests own isolated disposable children.
    let cancel = texts
        .iter()
        .find(|(text, _)| text.galley.job.text == "Cancel")
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
                egui::Event::PointerMoved(cancel),
                egui::Event::PointerButton {
                    pos: cancel,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    assert!(app.pending_end_task.is_none());
    app.selected_pid = Some(900_001);
    app.snapshot.processes[1].control.created_at_100ns = None;
    app.request_end_selected();
    assert!(app.pending_end_task.is_none());
    assert!(
        app.message
            .as_ref()
            .unwrap()
            .0
            .contains("identity is unavailable")
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
                    assert!(visible("GPU temperature"));
                    assert!(visible("Board power"));
                } else {
                    assert!(visible("GPU temperature"));
                    assert!(visible("Board power"));
                    if state == 1 {
                        // Unreported readings are a muted "--", never a word.
                        assert!(visible("--"));
                        assert!(!visible("Unavailable"));
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
fn drive_sensor_fields_stay_aligned_for_live_cached_unavailable_and_disconnected_data() {
    // Chip vocabulary is restricted to Live (no chip), Cached and Stale; raw
    // "Unavailable"/"Disconnected" driver text never becomes a visible chip.
    let expected_chip: [Option<&str>; 4] = [None, Some("Cached"), Some("Stale"), Some("Stale")];
    for dark in [true, false] {
        let mut expected = None;
        for (state, chip) in expected_chip.into_iter().enumerate() {
            let settings = ThemeSettings {
                dark,
                ..Default::default()
            };
            let ctx = egui::Context::default();
            theme::install(&ctx, settings);
            let mut app = app(settings, true);
            let storage = std::sync::Arc::make_mut(&mut app.snapshot.storage_sensors);
            let drive = &mut storage.drives[0];
            if state == 1 {
                drive.error = Some(crate::storage_sensors::Error::Timeout);
            }
            if state == 2 {
                drive.last_success = None;
                drive.error = Some(crate::storage_sensors::Error::Windows(5));
                for sensor in &mut drive.temperatures.sensors {
                    sensor.celsius = None;
                }
            }
            if state == 3 {
                drive.present = false;
            }
            let mut output = egui::FullOutput::default();
            for _ in 0..3 {
                output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            Vec2::new(640.0, 480.0),
                        )),
                        ..Default::default()
                    },
                    |ui| app.storage_sensor_cards(ui),
                );
            }
            assert!(output.platform_output.commands.is_empty());
            let texts = text_shapes(&output);
            let bounds: Vec<_> = (0..3)
                .map(|index| {
                    let (text, clip) = texts
                        .iter()
                        .find(|(text, _)| text.galley.job.text == format!("S{index}"))
                        .unwrap();
                    assert!(clip.contains_rect(text.visual_bounding_rect()));
                    text.visual_bounding_rect()
                })
                .collect();
            if let Some(expected) = &expected {
                assert_eq!(&bounds, expected, "sensor row shifted at state {state}");
            }
            expected = Some(bounds);
            match chip {
                Some(chip) => assert!(
                    texts.iter().any(|(text, _)| text.galley.job.text == chip),
                    "state {state} missing chip {chip}"
                ),
                None => {
                    assert!(
                        !texts.iter().any(|(text, _)| ["Cached", "Stale"]
                            .contains(&text.galley.job.text.as_str())),
                        "a Live drive must show no state chip"
                    )
                }
            }
            assert!(
                !texts
                    .iter()
                    .any(|(text, _)| text.galley.job.text.contains("Unavailable")),
                "raw driver wording must never reach the sensors page (state {state})"
            );
            assert!(!texts.iter().any(|(text, _)| {
                text.galley
                    .job
                    .text
                    .contains("PRIVATE-FIXTURE-STORAGE-INTERFACE")
            }));
            let values: Vec<_> = texts
                .iter()
                .filter(|(text, _)| {
                    ["44 °C", "43 °C", "42 °C", "-- °C"].contains(&text.galley.job.text.as_str())
                })
                .collect();
            assert_eq!(values.len(), 3);
            for (text, clip) in values {
                assert_eq!(text.galley.rows.len(), 1);
                assert!(clip.contains_rect(text.visual_bounding_rect()));
                if state == 2 {
                    assert_eq!(text.galley.job.text, "-- °C");
                }
            }
        }
    }
}

/// No sensor bridge provider has ever polled (the fixture default): the CPU
/// and motherboard section collapses to one gap row, never four separate
/// "Unavailable" rows and a paragraph of provider jargon.
#[test]
fn no_sensor_bridge_provider_is_one_gap_row_not_four_unavailable_rows() {
    let ctx = egui::Context::default();
    let mut app = app(ThemeSettings::default(), true);
    theme::install(&ctx, app.theme);
    app.page = Page::Sensors;
    let size = Vec2::new(1280.0, 900.0);
    let mut output = egui::FullOutput::default();
    for _ in 0..3 {
        output = frame(&ctx, &mut app, size, vec![]);
    }
    let texts = text_shapes(&output);
    assert!(
        !texts
            .iter()
            .any(|(text, _)| text.galley.job.text.contains("Unavailable")),
        "no provider must never surface raw Unavailable text on the Sensors page"
    );
    let gap_rows = texts
        .iter()
        .filter(|(text, _)| text.galley.job.text == "CPU and motherboard sensors")
        .count();
    assert_eq!(
        gap_rows, 1,
        "exactly one gap row explains the missing CPU sensors"
    );
    assert!(
        texts
            .iter()
            .any(|(text, _)| text.galley.job.text == "Needs sensor app"),
        "the gap row carries its short reason chip"
    );
    for label in [
        "CPU package temperature",
        "Motherboard temperature",
        "CPU package power",
        "CPU core voltage",
    ] {
        assert!(
            !texts.iter().any(|(text, _)| text.galley.job.text == label),
            "individual bridge rows must not render without a provider ({label})"
        );
    }
}

/// A drive whose storage driver reports no temperature sensor is a compact
/// gap row on the Sensors page too, never a card with an empty plot.
#[test]
fn sensors_page_drive_without_sensors_is_a_gap_row_not_a_card() {
    let ctx = egui::Context::default();
    let mut app = app(ThemeSettings::default(), true);
    theme::install(&ctx, app.theme);
    app.page = Page::Sensors;
    let storage = std::sync::Arc::make_mut(&mut app.snapshot.storage_sensors);
    storage.drives.push(crate::storage_sensors::DriveReading {
        device: crate::storage_sensors::Device {
            id: "FIXTURE-SILENT-DRIVE".into(),
            name: "WDC WD60EZAX-00C8VB0".into(),
        },
        temperatures: crate::storage_sensors::Temperatures::default(),
        last_attempt: Some(std::time::Instant::now()),
        last_success: None,
        query_millis: Some(1.0),
        error: Some(crate::storage_sensors::Error::Windows(1)),
        present: true,
    });
    let size = Vec2::new(1280.0, 900.0);
    let mut output = egui::FullOutput::default();
    for _ in 0..3 {
        output = frame(&ctx, &mut app, size, vec![]);
    }
    let texts = text_shapes(&output);
    assert!(
        texts
            .iter()
            .any(|(text, _)| text.galley.job.text == "WDC WD60EZAX-00C8VB0"),
        "the silent drive's gap row shows its name"
    );
    assert!(
        texts
            .iter()
            .any(|(text, _)| text.galley.job.text == "Not reported"),
        "the silent drive's gap row carries a short reason chip"
    );
    // The drive with real sensors still gets its compact card, not a gap.
    assert!(texts.iter().any(|(text, _)| text.galley.job.text == "S0"));
    assert!(
        !texts
            .iter()
            .any(|(text, _)| text.galley.job.text.contains("Unavailable")),
        "a silent drive must never render a giant Unavailable card"
    );
}

/// At Trent's 1000x580 window the Sensors page keeps a tight vertical
/// rhythm: the GPU block (cards plus the metric row) stays within 250 px and
/// the Drives section starts above the fold, in both themes.
#[test]
fn sensors_page_drives_start_above_the_fold_at_1000x580() {
    for dark in [true, false] {
        let settings = ThemeSettings {
            dark,
            ..Default::default()
        };
        let ctx = egui::Context::default();
        theme::install(&ctx, settings);
        let mut app = app(settings, true);
        app.page = Page::Sensors;
        let size = Vec2::new(1000.0, 580.0);
        let mut output = egui::FullOutput::default();
        for _ in 0..3 {
            output = frame(&ctx, &mut app, size, vec![]);
        }
        let texts = text_shapes(&output);
        let top = |label: &str| {
            texts
                .iter()
                .find(|(text, _)| text.galley.job.text == label)
                .unwrap_or_else(|| panic!("{label} not rendered"))
                .0
                .visual_bounding_rect()
        };
        let drives = top("Drives");
        assert!(
            drives.top() < 560.0,
            "Drives header starts at {} px, below the fold",
            drives.top()
        );
        let block = top("GPU temperature").top();
        let metric_row = top("VRAM used").top();
        assert!(
            drives.top() - block <= 250.0,
            "GPU block is {} px tall at 1000x580",
            drives.top() - block
        );
        assert!(metric_row < drives.top());
        // The drive card and its sensor values are fully on screen too.
        for label in ["S0", "S1", "S2"] {
            assert!(top(label).bottom() < 580.0, "{label} below the fold");
        }
    }
}

/// Card anatomy on the Sensors page: the peak is a small chip in the title
/// row, never a "Peak" footer line under the plot.
#[test]
fn sensors_page_has_no_peak_footer_text() {
    let ctx = egui::Context::default();
    let mut app = app(ThemeSettings::default(), true);
    theme::install(&ctx, app.theme);
    app.page = Page::Sensors;
    for size in [
        Vec2::new(1000.0, 580.0),
        Vec2::new(1280.0, 800.0),
        Vec2::new(1600.0, 1000.0),
    ] {
        let mut output = egui::FullOutput::default();
        for _ in 0..3 {
            output = frame(&ctx, &mut app, size, vec![]);
        }
        let texts = text_shapes(&output);
        assert!(
            !texts
                .iter()
                .any(|(text, _)| text.galley.job.text.starts_with("Peak")),
            "no Peak footer on the Sensors page at {size:?}"
        );
        assert!(
            texts
                .iter()
                .any(|(text, _)| text.galley.job.text.starts_with("peak ")),
            "the peak lives in the title chip at {size:?}"
        );
    }
}

#[test]
fn gpu_cells_keep_right_alignment_and_distinguish_unknown_zero_and_partial() {
    use crate::gpu_activity::Usage;
    for dark in [true, false] {
        let settings = ThemeSettings {
            dark,
            ..Default::default()
        };
        let mut right_edge: Option<f32> = None;
        for usage in [
            Usage::Measured(0.0),
            Usage::Measured(99.9),
            Usage::Partial(12.34),
            Usage::Unavailable,
            Usage::Warming,
            Usage::Unreported,
        ] {
            let ctx = egui::Context::default();
            theme::install(&ctx, settings);
            let mut output = egui::FullOutput::default();
            for _ in 0..3 {
                output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            Vec2::new(100.0, 36.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        widgets::gpu_cell(ui, usage, true, theme::tokens(settings));
                    },
                );
            }
            assert!(output.platform_output.commands.is_empty());
            let texts = text_shapes(&output);
            // gpu_cell renders its own compact label for two states instead of
            // Usage::label()'s generic text: an exact-zero measured reading drops
            // the "0.00%" clutter for a calm "0%", and Unreported drops the "%"
            // entirely since there is nothing to round.
            let expected_label = match usage {
                Usage::Measured(value) if value <= 0.0 => "0%".to_string(),
                Usage::Unreported => "--".to_string(),
                _ => usage.label(),
            };
            let (text, clip) = texts
                .iter()
                .find(|(text, _)| text.galley.job.text == expected_label)
                .unwrap();
            let bounds = text.visual_bounding_rect();
            assert!(clip.contains_rect(bounds));
            assert_eq!(text.galley.rows.len(), 1);
            if let Some(expected) = right_edge {
                assert!((bounds.right() - expected).abs() < 1.1);
            }
            right_edge = Some(bounds.right());
            if usage.value().is_none() {
                assert!(
                    !texts
                        .iter()
                        .any(|(text, _)| text.galley.job.text == "0.00%")
                );
            }
        }
    }
}

#[test]
fn gpu_activity_states_reach_process_user_and_inspector_surfaces() {
    for dark in [true, false] {
        for page in [Page::Processes, Page::Details, Page::Users] {
            let settings = ThemeSettings {
                dark,
                ..Default::default()
            };
            let ctx = egui::Context::default();
            theme::install(&ctx, settings);
            let mut app = app(settings, true);
            gpu_activity_fixture(&mut app);
            app.page = page;
            app.tree_mode = false;
            let size = Vec2::new(1280.0, 760.0);
            let mut output = egui::FullOutput::default();
            for _ in 0..3 {
                output = frame(&ctx, &mut app, size, vec![]);
            }
            for expected in ["0%", "12.3%", "4.6%+", "-- %"] {
                assert!(
                    text_shapes(&output).iter().any(|(text, clip)| {
                        text.galley.job.text == expected
                            && text.galley.rows.len() == 1
                            && clip.contains_rect(text.visual_bounding_rect())
                    }),
                    "missing or clipped {expected} on page {}, dark={dark}",
                    page as u8
                );
            }
            if page == Page::Processes {
                app.tree_mode = true;
                let output = frame(&ctx, &mut app, size, vec![]);
                assert!(
                    text_shapes(&output)
                        .iter()
                        .any(|(text, _)| { text.galley.job.text == "16.9%+" })
                );
                app.selected_pid = Some(900_005);
                let output = frame(&ctx, &mut app, size, vec![]);
                assert!(
                    text_shapes(&output)
                        .iter()
                        .any(|(text, _)| { text.galley.job.text == "No counter reported" })
                );
            }
        }
    }
}

#[test]
fn details_user_column_shows_protected_for_unknown_account_without_touching_the_model() {
    // Details shares the Users page's display mapping: an unreadable owner
    // reads the short, untruncated "Protected" (reason on hover), never the
    // raw "Unknown account" sentinel value, and the underlying model field
    // is left untouched.
    for dark in [true, false] {
        let settings = ThemeSettings {
            dark,
            ..Default::default()
        };
        let ctx = egui::Context::default();
        theme::install(&ctx, settings);
        let mut app = app(settings, true);
        app.snapshot.processes[0].user = "Unknown account".into();
        app.page = Page::Details;
        app.tree_mode = false;
        // Sort by PID ascending so the edited process (lowest PID) is on the
        // first visible row, not scrolled out of view by the default CPU sort.
        app.sort_column = SortColumn::Pid;
        app.sort_direction = SortDirection::Ascending;
        app.rebuild_visible_processes();
        let size = Vec2::new(1280.0, 760.0);
        let mut output = egui::FullOutput::default();
        for _ in 0..3 {
            output = frame(&ctx, &mut app, size, vec![]);
        }
        assert!(
            text_shapes(&output)
                .iter()
                .any(|(text, _)| text.galley.job.text == "Protected" && !text.galley.elided),
            "Details USER column should read 'Protected' in full for an unknown account"
        );
        assert!(
            !text_shapes(&output)
                .iter()
                .any(|(text, _)| text.galley.job.text == "Unknown account"),
            "the raw 'Unknown account' sentinel must never be shown verbatim"
        );
        assert_eq!(app.snapshot.processes[0].user, "Unknown account");
    }
}

#[test]
fn executable_icons_and_fallbacks_keep_names_aligned_and_accept_row_selection() {
    for dark in [true, false] {
        let settings = ThemeSettings {
            dark,
            ..Default::default()
        };
        let ctx = egui::Context::default();
        theme::install(&ctx, settings);
        let mut app = app(settings, true);
        gpu_activity_fixture(&mut app);
        app.tree_mode = false;
        app.page = Page::Processes;
        let size = Vec2::new(1280.0, 760.0);
        let mut previous = None;
        for loaded in [false, true] {
            if loaded {
                install_fixture_icons(&mut app, &ctx);
            }
            let mut output = egui::FullOutput::default();
            for _ in 0..3 {
                output = frame(&ctx, &mut app, size, vec![]);
            }
            let texts = text_shapes(&output);
            let (text, clip) = texts
                .iter()
                .find(|(t, _)| t.galley.job.text == "Fixture.Gpu.1.exe")
                .unwrap();
            let bounds = text.visual_bounding_rect();
            assert!(clip.contains_rect(bounds));
            if let Some(expected) = previous {
                assert_eq!(bounds, expected);
            }
            previous = Some(bounds);
        }
        // The icon is immediately before the text, with a six-point gap.
        let bounds = previous.unwrap();
        let position = egui::pos2(bounds.left() - 15.0, bounds.center().y);
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
}

#[test]
fn inspector_gpu_status_cannot_shift_neighboring_fields() {
    use crate::gpu_activity::Usage;
    for dark in [true, false] {
        let settings = ThemeSettings {
            dark,
            ..Default::default()
        };
        let ctx = egui::Context::default();
        theme::install(&ctx, settings);
        let mut app = app(settings, true);
        gpu_activity_fixture(&mut app);
        app.page = Page::Processes;
        app.selected_pid = Some(900_005);
        let size = Vec2::new(1040.0, 640.0);
        let mut anchor: Option<egui::Rect> = None;
        for usage in [
            Usage::Measured(0.0),
            Usage::Partial(12.34),
            Usage::Warming,
            Usage::Unreported,
            Usage::Unavailable,
        ] {
            app.snapshot.processes[5].gpu_percent = usage;
            let mut output = egui::FullOutput::default();
            for _ in 0..3 {
                output = frame(&ctx, &mut app, size, vec![]);
            }
            let texts = text_shapes(&output);
            let (status, clip) = texts
                .iter()
                .find(|(t, _)| t.galley.job.text == usage.status())
                .unwrap();
            assert_eq!(status.galley.rows.len(), 1);
            assert!(clip.contains_rect(status.visual_bounding_rect()));
            let (working_set, clip) = texts
                .iter()
                .find(|(t, _)| t.galley.job.text == "Working set")
                .unwrap();
            let rect = working_set.visual_bounding_rect();
            assert!(clip.contains_rect(rect));
            if let Some(previous) = anchor {
                assert_eq!(rect, previous);
            }
            anchor = Some(rect);
        }
    }
}

#[test]
fn inventory_states_keep_sources_and_table_headers_in_place() {
    for page in [Page::Startup, Page::Services] {
        for dark in [true, false] {
            let settings = ThemeSettings {
                dark,
                ..Default::default()
            };
            let ctx = egui::Context::default();
            theme::install(&ctx, settings);
            let mut app = app(settings, true);
            let mut expected = None;
            for state in [0, 1, 2, 3, 4, 5, 6, 7] {
                app.snapshot = fixture();
                inventory_state_fixture(&mut app, page, state);
                let mut output = egui::FullOutput::default();
                for _ in 0..3 {
                    output = frame(&ctx, &mut app, Vec2::new(1040.0, 640.0), vec![]);
                }
                let texts = text_shapes(&output);
                let title = if page == Page::Startup {
                    "NAME"
                } else {
                    "DISPLAY NAME"
                };
                let (text, clip) = texts
                    .iter()
                    .find(|(text, _)| text.galley.job.text == title)
                    .expect("inventory table header disappeared");
                let bounds = text.visual_bounding_rect();
                assert!(clip.contains_rect(bounds));
                if let Some(expected) = expected {
                    assert_eq!(
                        bounds, expected,
                        "table moved in state {state}, dark={dark}"
                    );
                }
                expected = Some(bounds);
                if page == Page::Startup {
                    for source in crate::startup::Source::ALL {
                        // The chip row's own (possibly shortened) label is
                        // always present; the table's full source name only
                        // shows up when that source has at least one row.
                        let label = super::inventory::chip_label(source);
                        let (text, clip) = texts
                            .iter()
                            .find(|(text, _)| text.galley.job.text == label)
                            .expect("source chip disappeared");
                        assert!(
                            clip.contains_rect(text.visual_bounding_rect()),
                            "state={state} dark={dark} source={source:?}"
                        );
                        assert_eq!(text.galley.rows.len(), 1);
                    }
                }
                if state == 2 {
                    assert!(
                        texts
                            .iter()
                            .any(|(text, _)| text.galley.job.text.starts_with("Cached"))
                    );
                    assert!(
                        texts
                            .iter()
                            .any(|(text, _)| text.galley.job.text.starts_with(
                                if page == Page::Startup {
                                    "Fixture startup"
                                } else {
                                    "Fixture long service"
                                }
                            ))
                    );
                }
            }
        }
    }
}

#[test]
fn startup_and_services_tables_start_above_y_200_at_1000x580() {
    for page in [Page::Startup, Page::Services] {
        let ctx = egui::Context::default();
        let settings = ThemeSettings::default();
        theme::install(&ctx, settings);
        let mut app = app(settings, true);
        app.page = page;
        let size = Vec2::new(1000.0, 580.0);
        let output = frame(&ctx, &mut app, size, vec![]);
        let title = if page == Page::Startup {
            "NAME"
        } else {
            "DISPLAY NAME"
        };
        let (text, _) = text_shapes(&output)
            .into_iter()
            .find(|(text, _)| text.galley.job.text == title)
            .expect("inventory table header");
        let top = text.visual_bounding_rect().top();
        assert!(top < 200.0, "{page:?} table header starts at y={top}");
    }
}

#[test]
fn startup_and_services_hide_freshness_header_when_every_row_is_live() {
    // The default fixture is Live end to end (every Startup source readable,
    // every service inventory read Live): the common case, and the one where
    // the FRESHNESS column must give its width back to the other columns.
    for page in [Page::Startup, Page::Services] {
        let ctx = egui::Context::default();
        let settings = ThemeSettings::default();
        theme::install(&ctx, settings);
        let mut app = app(settings, true);
        app.page = page;
        let size = Vec2::new(1000.0, 580.0);
        let output = frame(&ctx, &mut app, size, vec![]);
        let title = if page == Page::Startup {
            "NAME"
        } else {
            "DISPLAY NAME"
        };
        assert!(
            text_shapes(&output)
                .iter()
                .any(|(text, _)| text.galley.job.text == title),
            "{page:?} lost its table header"
        );
        assert!(
            !text_shapes(&output)
                .iter()
                .any(|(text, _)| text.galley.job.text == "FRESHNESS"),
            "{page:?} still shows a FRESHNESS header while every row is Live"
        );
    }
}

#[test]
fn startup_and_services_footers_never_say_retained() {
    for page in [Page::Startup, Page::Services] {
        for state in [0, 1, 2, 3, 4, 5, 6, 7] {
            let ctx = egui::Context::default();
            let settings = ThemeSettings::default();
            theme::install(&ctx, settings);
            let mut app = app(settings, true);
            app.snapshot = fixture();
            inventory_state_fixture(&mut app, page, state);
            let size = Vec2::new(1000.0, 580.0);
            let output = frame(&ctx, &mut app, size, vec![]);
            assert!(
                !text_shapes(&output).iter().any(|(text, _)| text
                    .galley
                    .job
                    .text
                    .contains("retained")),
                "{page:?} state {state} still says \"retained\""
            );
        }
    }
}

fn fixture_service_controls(app: &mut TrontopApp, ctx: &egui::Context) {
    // These fixtures exercise chronological action results. Unlike static page
    // screenshots, the inventory must not be dated an hour after the command.
    app.snapshot
        .diagnostics
        .get_mut(crate::diagnostics::Provider::Services)
        .record(
            std::time::Instant::now() - std::time::Duration::from_secs(1),
            std::time::Duration::ZERO,
            crate::diagnostics::State::Live,
            None,
            None,
        );
    app.page = Page::Services;
    app.selected_service = Some(app.snapshot.services[0].name.clone());
    app.service_controller =
        crate::service_control::fixture_controller(ctx.clone(), app.snapshot.services[0].status);
}

#[test]
fn failed_inventory_cannot_replace_newer_command_observation_but_complete_read_can() {
    use crate::diagnostics::{Provider, State as ProviderState};
    use crate::service_control::{Action, Event, State, Status};
    use std::time::{Duration, Instant};
    let ctx = egui::Context::default();
    let mut app = app(ThemeSettings::default(), true);
    fixture_service_controls(&mut app, &ctx);
    let at = Instant::now();
    app.service_event = Some(Event {
        name: "FixtureService0".into(),
        action: Action::Stop,
        phase: "Completed",
        command_at: Some(at),
        observed: Some((
            at,
            Status {
                state: State::Stopped,
                pid: 0,
                ..app.snapshot.services[0].status
            },
        )),
        done: true,
        error: None,
    });
    app.service_observations
        .record(app.service_event.as_ref().unwrap());
    app.snapshot.diagnostics.get_mut(Provider::Services).record(
        at + Duration::from_millis(1),
        Duration::ZERO,
        ProviderState::Unavailable,
        None,
        None,
    );
    assert_eq!(
        app.service_status(&app.snapshot.services[0]).0.state,
        State::Stopped
    );
    app.snapshot.diagnostics.get_mut(Provider::Services).record(
        at + Duration::from_millis(2),
        Duration::ZERO,
        ProviderState::Live,
        None,
        None,
    );
    assert_eq!(
        app.service_status(&app.snapshot.services[0]).0.state,
        State::Running
    );
    assert!(!app.service_status(&app.snapshot.services[0]).1);
}

/// Where the command bar drew the button labelled `label` in the last frame.
/// Center of a visible sidebar page entry. Entry labels start past the icon
/// (x > 30); section headings with the same words ("Processes", "System")
/// sit at the left edge and are not buttons.
fn nav_entry(output: &egui::FullOutput, label: &str) -> Option<egui::Pos2> {
    text_shapes(output)
        .into_iter()
        .find(|(text, clip)| {
            text.galley.job.text == label
                && text.pos.x > 30.0
                && text.pos.x < 196.0
                && clip.contains_rect(text.visual_bounding_rect())
        })
        .map(|(text, _)| text.visual_bounding_rect().center())
}

fn command_rect(ctx: &egui::Context, label: &str) -> Option<egui::Rect> {
    ctx.data(|data| data.get_temp::<Vec<(String, egui::Rect)>>(command_rects_id()))
        .unwrap_or_default()
        .into_iter()
        .find(|(name, _)| name == label)
        .map(|(_, rect)| rect)
}

fn click_local_text(ctx: &egui::Context, app: &mut TrontopApp, size: Vec2, label: &str) {
    click_local_text_output(ctx, app, size, label);
}

// Visual passes must retain texture deltas from every simulated event frame,
// including frames whose shapes will be replaced before the final screenshot.
fn click_local_text_output(
    ctx: &egui::Context,
    app: &mut TrontopApp,
    size: Vec2,
    label: &str,
) -> egui::FullOutput {
    let mut output = frame(ctx, app, size, vec![]);
    for _ in 0..20 {
        output.append(frame(ctx, app, size, vec![]));
    }
    // Compact command bars draw icon-only buttons with no text shape; fall
    // back to the button the command bar recorded under that label.
    // Page and dialog controls first: sidebar meter labels ("Memory") and
    // section headings ("Processes") repeat words used by page controls.
    let visible: Vec<_> = text_shapes(&output)
        .into_iter()
        .filter(|(text, clip)| {
            text.galley.job.text == label && clip.contains_rect(text.visual_bounding_rect())
        })
        .collect();
    let position = visible
        .iter()
        .find(|(text, _)| text.pos.x >= 196.0)
        .or_else(|| visible.first())
        .map(|(text, _)| text.visual_bounding_rect().center())
        .or_else(|| command_rect(ctx, label).map(|rect| rect.center()))
        .unwrap_or_else(|| panic!("missing visible local control {label}"));
    for pressed in [true, false] {
        output.append(frame(
            ctx,
            app,
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
        ));
    }
    output
}

#[test]
fn service_selection_confirmation_cancel_and_submit_use_only_fake_backend() {
    for dark in [true, false] {
        let settings = ThemeSettings {
            dark,
            ..Default::default()
        };
        let ctx = egui::Context::default();
        theme::install(&ctx, settings);
        let mut app = app(settings, true);
        fixture_service_controls(&mut app, &ctx);
        let size = Vec2::new(1040.0, 640.0);
        app.selected_service = None;
        // Select through the table, then verify merely staging and cancelling has
        // not submitted anything to even the simulated backend.
        click_local_text(&ctx, &mut app, size, "FixtureService0");
        assert_eq!(app.selected_service.as_deref(), Some("FixtureService0"));
        click_local_text(&ctx, &mut app, size, "Restart");
        assert!(app.pending_service.is_some());
        assert!(!app.service_controller.busy());
        assert!(app.service_event.is_none());
        click_local_text(&ctx, &mut app, size, "Cancel");
        assert!(app.pending_service.is_none());
        assert!(app.service_event.is_none());
        click_local_text(&ctx, &mut app, size, "Stop");
        click_local_text(&ctx, &mut app, size, "Confirm command");
        let started = std::time::Instant::now();
        while app.service_event.as_ref().is_none_or(|event| !event.done) {
            frame(&ctx, &mut app, size, vec![]);
            assert!(started.elapsed() < std::time::Duration::from_secs(5));
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(app.service_event.as_ref().unwrap().error.is_none());
        assert_eq!(
            app.service_status(&app.snapshot.services[0]).0.state,
            crate::service_control::State::Stopped
        );
    }
}

#[test]
fn service_confirmations_expire_and_do_not_retarget_changed_service() {
    use crate::service_control::{Action, Request};
    for expired in [true, false] {
        let ctx = egui::Context::default();
        let mut app = app(ThemeSettings::default(), true);
        theme::install(&ctx, app.theme);
        fixture_service_controls(&mut app, &ctx);
        let original = app.snapshot.services[0].clone();
        app.pending_service = Some(Request {
            name: original.name,
            display_name: original.display_name,
            action: Action::Restart,
            expected: original.status,
            staged_at: std::time::Instant::now()
                - std::time::Duration::from_secs(if expired { 31 } else { 0 }),
        });
        if !expired {
            std::sync::Arc::make_mut(&mut app.snapshot.services)[0]
                .status
                .pid += 1;
        }
        click_local_text(&ctx, &mut app, Vec2::new(1040.0, 640.0), "Confirm command");
        assert!(app.pending_service.is_some());
        assert!(!app.service_controller.busy());
        assert!(app.service_event.is_none());
    }
}

#[test]
fn service_action_states_keep_table_headers_fixed_and_unknown_outcome_disables_retry() {
    use crate::service_control::{Action, Event, State, Status};
    let size = Vec2::new(1040.0, 640.0);
    for dark in [true, false] {
        let settings = ThemeSettings {
            dark,
            ..Default::default()
        };
        let ctx = egui::Context::default();
        theme::install(&ctx, settings);
        let mut app = app(settings, true);
        fixture_service_controls(&mut app, &ctx);
        let mut expected = None;
        for state in 0..5 {
            let now = std::time::Instant::now();
            app.service_event = (state != 0).then(|| Event { name: "FixtureService0".into(), action: Action::Restart,
                phase: if state == 1 { "Stopping" } else if state == 2 { "Completed" } else { "Not completed" },
                command_at: Some(now), observed: (state != 3).then_some((now, Status {
                    state: if state == 1 { State::Stopping } else { State::Running },
                    ..app.snapshot.services[0].status
                })), done: state != 1, error: (state >= 3).then(|| "Fixture Windows access/query failure. Outcome unknown; refresh before retrying.".into()) });
            if let Some(event) = &app.service_event {
                app.service_observations.record(event);
            }
            let mut output = frame(&ctx, &mut app, size, vec![]);
            for _ in 0..2 {
                output = frame(&ctx, &mut app, size, vec![]);
            }
            let texts = text_shapes(&output);
            let (header, clip) = texts
                .iter()
                .find(|(text, _)| text.galley.job.text == "DISPLAY NAME")
                .unwrap();
            let bounds = header.visual_bounding_rect();
            assert!(clip.contains_rect(bounds));
            if let Some(expected) = expected {
                assert_eq!(bounds, expected);
            }
            expected = Some(bounds);
            if state == 3 {
                assert!(
                    texts
                        .iter()
                        .any(|(text, _)| text.galley.job.text == "Pre-command")
                );
                click_local_text(&ctx, &mut app, size, "Restart");
                assert!(app.pending_service.is_none());
            }
            assert!(output.platform_output.commands.is_empty());
        }
    }
}

#[test]
fn sidebar_gpu_meter_shows_measured_and_partial_readings_not_dashes() {
    let ctx = egui::Context::default();
    theme::install(&ctx, ThemeSettings::default());
    let mut app = app(ThemeSettings::default(), true);
    let size = Vec2::new(1000.0, 580.0);
    let meter_text = |output: &egui::FullOutput, label: &str| {
        text_shapes(output)
            .iter()
            // The navigation rail is 196 px wide; only its footer meter counts.
            .any(|(text, _)| text.galley.job.text == label && text.pos.x < 196.0)
    };
    let mut output = egui::FullOutput::default();
    for _ in 0..3 {
        output = frame(&ctx, &mut app, size, vec![]);
    }
    let measured = app.snapshot.gpu.reading();
    assert!(measured.exact().is_some());
    assert!(meter_text(&output, &measured.label()), "measured GPU meter");
    // A partial sample (one counter warming) used to blank the meter to "-- %".
    gpu_activity_fixture(&mut app);
    for _ in 0..3 {
        output = frame(&ctx, &mut app, size, vec![]);
    }
    let partial = app.snapshot.gpu.reading();
    assert!(partial.exact().is_none() && partial.value().is_some());
    assert!(partial.label().ends_with('+'));
    assert!(meter_text(&output, &partial.label()), "partial GPU meter");
}

#[test]
fn sidebar_labels_are_readable_sentence_case_and_nothing_scrolls_at_1000x580() {
    let ctx = egui::Context::default();
    let settings = ThemeSettings::default();
    theme::install(&ctx, settings);
    let mut app = app(settings, true);
    let size = Vec2::new(1000.0, 580.0);
    let mut output = frame(&ctx, &mut app, size, vec![]);
    for _ in 0..3 {
        output = frame(&ctx, &mut app, size, vec![]);
    }
    let texts = text_shapes(&output);
    let sidebar = |label: &str, left_edge: bool| {
        texts
            .iter()
            .find(|(text, _)| {
                text.galley.job.text == label
                    && text.pos.x < 196.0
                    && (text.pos.x < 30.0) == left_edge
            })
            .unwrap_or_else(|| panic!("missing sidebar text {label}"))
    };
    for heading in ["Monitor", "Processes", "System"] {
        let (text, _) = sidebar(heading, true);
        assert!(
            text.galley.job.sections[0].format.font_id.size >= 11.0,
            "{heading}"
        );
    }
    for meter in ["CPU", "Memory", "GPU"] {
        let (text, _) = sidebar(meter, true);
        assert!(
            text.galley.job.sections[0].format.font_id.size >= 11.0,
            "{meter}"
        );
    }
    for gone in ["MONITOR", "PROCESSES", "SYSTEM", "MEMORY"] {
        assert!(
            !texts
                .iter()
                .any(|(text, _)| text.galley.job.text == gone && text.pos.x < 196.0),
            "{gone} still uppercase"
        );
    }
    let (host, _) = texts
        .iter()
        .find(|(text, _)| text.galley.job.text.starts_with("HEADLESS-FIXTURE"))
        .expect("host line");
    assert!(host.galley.job.sections[0].format.font_id.size >= 10.0);
    // Every page entry is fully visible above the meters, with a gap.
    let footer_top = sidebar("CPU", true).0.visual_bounding_rect().top();
    for (_, _, entry) in Page::ALL {
        let (text, clip) = sidebar(entry, false);
        let rect = text.visual_bounding_rect();
        assert!(clip.contains_rect(rect), "{entry} clipped");
        assert!(
            rect.bottom() + 12.0 < footer_top,
            "{entry} crowds the meters"
        );
    }
}

#[test]
fn app_shell_fits_1000x580_with_grouped_nav_titles_and_short_chrome() {
    let mut grouped: Vec<Page> = Vec::new();
    for (_, pages) in Page::NAV_GROUPS {
        grouped.extend_from_slice(pages);
    }
    assert_eq!(grouped.len(), Page::ALL.len());
    for (page, _, _) in Page::ALL {
        assert_eq!(grouped.iter().filter(|p| **p == page).count(), 1);
    }
    let removed = [
        "SYSTEM CONTROL DECK",
        "MACHINE OVERVIEW",
        "LIVE GRAPH WALL",
        "HARDWARE SENSORS",
        "PROCESS MATRIX",
        "PERFORMANCE ARRAY",
        "RESOURCE HISTORY",
        "BOOT SEQUENCE",
        "USER SESSIONS",
        "PROCESS DETAILS",
        "SERVICE CONTROL",
        "SYSTEM SPECIFICATIONS",
        "CONTROL",
    ];
    for settings in [
        ThemeSettings::default(),
        ThemeSettings {
            dark: false,
            ..ThemeSettings::copper_legacy()
        },
    ] {
        let ctx = egui::Context::default();
        theme::install(&ctx, settings);
        let mut app = app(settings, true);
        let size = Vec2::new(1000.0, 580.0);
        for (page, _, name) in Page::ALL {
            app.page = page;
            let mut output = frame(&ctx, &mut app, size, vec![]);
            for _ in 0..3 {
                output = frame(&ctx, &mut app, size, vec![]);
            }
            let texts = text_shapes(&output);
            let sidebar = |label: &str| {
                texts
                    .iter()
                    .find(|(text, _)| text.galley.job.text == label && text.pos.x < 196.0)
                    .unwrap_or_else(|| panic!("missing sidebar text {label} on {name}"))
            };
            // Every nav entry sits fully inside the nav clip, above the footer:
            // nothing is scrolled away and no scrollbar is needed.
            let footer_top = sidebar("CPU").0.visual_bounding_rect().top();
            for (_, _, entry) in Page::ALL {
                let (text, clip) = sidebar(entry);
                let rect = text.visual_bounding_rect();
                assert!(
                    clip.contains_rect(rect) && rect.bottom() < footer_top,
                    "nav entry {entry} clipped or under the footer on {name}: {rect:?}"
                );
            }
            let (_, clip) = sidebar("Theme Studio");
            assert!(clip.contains_rect(sidebar("Theme Studio").0.visual_bounding_rect()));
            assert!(sidebar("Theme Studio").0.visual_bounding_rect().bottom() <= size.y);
            // The page title is in the 44 px command bar under the 42 px title bar.
            let (title, clip) = texts
                .iter()
                .find(|(text, _)| {
                    text.galley.job.text == name && text.pos.x >= 196.0 && text.pos.y < 86.0
                })
                .unwrap_or_else(|| panic!("missing command bar title {name}"));
            assert!(clip.contains_rect(title.visual_bounding_rect()));
            assert!(!title.galley.elided);
            for gone in removed {
                assert!(
                    !texts.iter().any(|(text, _)| text.galley.job.text == gone),
                    "{gone} still drawn on {name}"
                );
            }
            assert!(command_rect(&ctx, "Theme").is_none());
            assert!(command_rect(&ctx, "Run task").is_some());
            // Chrome budget: title bar, command bar and intro end by 110 px.
            let intro = page.intro();
            if !intro.is_empty() {
                let (text, clip) = texts
                    .iter()
                    .find(|(text, _)| text.galley.job.text == intro)
                    .unwrap_or_else(|| panic!("missing intro on {name}"));
                let rect = text.visual_bounding_rect();
                assert_eq!(text.galley.rows.len(), 1, "intro wrapped on {name}");
                assert!(clip.contains_rect(rect));
                assert!(
                    rect.bottom() <= 110.0,
                    "chrome too tall on {name}: {rect:?}"
                );
            }
        }
    }
}

#[test]
fn partial_gpu_coverage_leaves_history_gaps_and_keeps_engine_names() {
    let mut app = app(ThemeSettings::default(), true);
    assert_eq!(app.gpu_history.back().copied(), Some(23.8));
    gpu_activity_fixture(&mut app);
    assert!(app.gpu_history.back().unwrap().is_nan());
    let engine_names = app.gpu_engine_names.clone();
    let mut snapshot = app.snapshot.clone();
    snapshot.gpu = GpuSnapshot {
        error: Some("Fixture provider failed".into()),
        ..Default::default()
    };
    app.accept_sample(snapshot.clone());
    assert!(app.gpu_history.back().unwrap().is_nan());
    assert_eq!(app.gpu_engine_names, engine_names);
    snapshot.gpu = fixture().gpu;
    snapshot.gpu.utilization_percent = 0.0;
    app.accept_sample(snapshot);
    assert_eq!(app.gpu_history.back().copied(), Some(0.0));
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
        "physical-disks",
        "physical-disks-light",
        "physical-disks-partial-compact",
        "theme-studio",
        "theme-studio-light",
        "theme-studio-compact",
        "theme-appearance",
        "theme-appearance-light",
        "theme-presets",
        "theme-presets-light",
        "theme-library",
        "inspector",
        "inspector-compact",
        "inspector-hidden-compact",
        "processes-wide-light",
        "action-pending-compact",
        "action-slow-light",
        "action-error-compact",
        "gpu-sensors",
        "gpu-sensors-light",
        "gpu-sensors-compact",
        "gpu-sensors-unavailable",
        "confirm-end-task",
        "confirm-stale-task",
        "overview-light",
        "overview-empty",
        "about",
        "about-light",
        "about-compact",
        "gpu-sensors-cached",
        "storage-sensors-light",
        "storage-sensors-cached",
        "storage-sensors-unavailable",
        "gpu-activity-processes-compact",
        "gpu-activity-tree-light",
        "gpu-activity-users",
        "gpu-activity-inspector-light",
        "gpu-activity-partial",
        "process-icons",
        "process-icons-light",
        "process-icons-inspector",
        "startup-partial",
        "startup-cached-light",
        "startup-unavailable-compact",
        "startup-starting-compact",
        "services-cached",
        "services-unavailable-light",
        "service-controls",
        "service-controls-light",
        "service-controls-compact",
        "service-confirmation",
        "service-control-error",
        "service-outcomes",
        "startup-timeout",
        "services-timeout-light",
        "export",
        "export-private-light",
        "export-failed-compact",
        "process-tree-deep",
        "process-tree-deep-light",
        "process-sort-totals",
        "process-sort-totals-light",
        "preferences-saved",
        "preferences-saved-light",
        "preferences-loading",
        "preferences-loading-light",
        "preferences-pending",
        "preferences-pending-light",
        "preferences-error",
        "preferences-error-light",
    ] {
        let ctx = egui::Context::default();
        let settings = ThemeSettings {
            dark: !variant.ends_with("-light"),
            ..Default::default()
        };
        theme::install(&ctx, settings);
        let mut preferences_fixture = variant.starts_with("preferences-").then(|| {
            preferences::install(
                &ctx,
                settings.dark,
                variant
                    .trim_start_matches("preferences-")
                    .trim_end_matches("-light"),
            )
        });
        let mut app = preferences_fixture
            .as_mut()
            .and_then(|fixture| fixture.app.take())
            .unwrap_or_else(|| app(settings, variant != "overview-empty"));
        if variant.contains("preferences-pending") || variant.contains("preferences-error") {
            app.closing_at = Some(std::time::Instant::now() - std::time::Duration::from_secs(1));
        }
        app.page = if variant == "inspector" {
            Page::Processes
        } else {
            Page::Performance
        };
        app.selected_pid = (variant == "inspector").then_some(900_001);
        let _action_release = if variant.starts_with("action-") {
            app.page = Page::Processes;
            if variant == "action-error-compact" {
                app.message = Some(("Set priority: Fixture.exe (900001): Access denied. The process could not be opened with scheduling rights; no fallback or elevation was attempted.".into(), true));
                None
            } else {
                Some(actions::install_pending(
                    &mut app,
                    &ctx,
                    variant.contains("slow"),
                ))
            }
        } else {
            None
        };
        if variant.starts_with("inspector-") {
            app.page = Page::Processes;
            app.selected_pid = Some(900_001);
            app.inspector_visible = variant != "inspector-hidden-compact";
        }
        if variant == "processes-wide-light" {
            app.page = Page::Processes;
            app.selected_pid = None;
        }
        if variant.starts_with("confirm-") {
            app.process_actions =
                crate::process_actions::Controller::with_backend(ctx.clone(), |_| Ok(()));
            app.page = Page::Processes;
            app.selected_pid = Some(900_001);
            app.request_end_selected();
            if variant == "confirm-stale-task" {
                app.snapshot.processes[1].control.created_at_100ns = None;
            }
        }
        app.show_theme_editor = variant.starts_with("theme-");
        if variant.contains("preferences-loading") {
            app.show_theme_editor = true;
        }
        app.theme_studio.tab = if variant.starts_with("theme-appearance") {
            1
        } else if variant.starts_with("theme-presets") {
            2
        } else if variant.starts_with("theme-library") {
            3
        } else {
            0
        };
        if variant.starts_with("physical-disks") {
            disks::install(&mut app, variant.contains("partial"));
        }
        if variant.starts_with("process-tree-deep") {
            process_perf::install_chain(&mut app);
        }
        if variant.starts_with("process-sort-totals") {
            process_sort::install_groups(&mut app);
        }
        if variant.starts_with("service-") {
            fixture_service_controls(&mut app, &ctx);
            if variant == "service-confirmation" {
                let row = &app.snapshot.services[0];
                app.pending_service = Some(crate::service_control::Request {
                    name: row.name.clone(),
                    display_name: row.display_name.clone(),
                    action: crate::service_control::Action::Restart,
                    expected: row.status,
                    staged_at: std::time::Instant::now(),
                });
            } else if variant == "service-control-error" {
                app.service_event = Some(crate::service_control::Event {
                    name: "FixtureService0".into(), action: crate::service_control::Action::Restart,
                    phase: "Not completed", command_at: Some(std::time::Instant::now()),
                    observed: None, done: true,
                    error: Some("Fixture access denied by Windows. No automatic elevation or recursive dependent-service stop.".into()),
                });
                app.service_observations
                    .record(app.service_event.as_ref().unwrap());
            }
        }
        if (variant.starts_with("startup-") || variant.starts_with("services-"))
            && !variant.contains("timeout")
        {
            let state = if variant.contains("partial") {
                1
            } else if variant.contains("cached") {
                2
            } else if variant.contains("starting") {
                5
            } else {
                4
            };
            let page = if variant.starts_with("startup-") {
                Page::Startup
            } else {
                Page::Services
            };
            inventory_state_fixture(&mut app, page, state);
        }
        if variant == "service-outcomes" {
            fixture_service_controls(&mut app, &ctx);
            service_retention::retained_fixture(&mut app);
        }
        if variant.contains("timeout") {
            let provider = if variant.starts_with("startup-") {
                crate::diagnostics::Provider::Startup
            } else {
                crate::diagnostics::Provider::Services
            };
            let page = if provider == crate::diagnostics::Provider::Startup {
                Page::Startup
            } else {
                Page::Services
            };
            inventory_state_fixture(&mut app, page, 2);
            assert!(app.snapshot.startup.rows().count() > 0);
            assert!(!app.snapshot.services.is_empty());
            let health = app.snapshot.diagnostics.get_mut(provider);
            health.issue = Some(crate::diagnostics::Issue::InventoryTimeout);
            health.query_millis = None;
        }
        if variant.starts_with("process-icons") {
            gpu_activity_fixture(&mut app);
            install_fixture_icons(&mut app, &ctx);
            app.page = Page::Processes;
            app.selected_pid = (variant == "process-icons-inspector").then_some(900_001);
        }
        if variant.starts_with("gpu-activity") {
            gpu_activity_fixture(&mut app);
            app.page = Page::Processes;
            app.tree_mode = variant == "gpu-activity-tree-light";
            if variant == "gpu-activity-users" {
                app.page = Page::Users;
            } else if variant == "gpu-activity-inspector-light" {
                app.selected_pid = Some(900_005);
            } else if variant == "gpu-activity-partial" {
                app.page = Page::Performance;
                app.performance_device = PerformanceDevice::Gpu;
            }
        }
        app.show_diagnostics = variant.starts_with("about");
        if variant.starts_with("export") {
            app.page = Page::Processes;
            app.show_export = true;
            app.exporter =
                crate::export::Exporter::with_backend(|_, _| crate::export::Outcome::Cancelled);
            app.export_options.private_details = variant == "export-private-light";
            if variant == "export-failed-compact" {
                app.export_result = Some(crate::export::Outcome::Failed(
                    "Fixture: access denied. The destination was not replaced.".into(),
                ));
            }
        }
        if variant.starts_with("overview") {
            app.page = Page::Overview;
        }
        if variant == "gpu-sensors-cached" {
            app.snapshot.gpu_sensors.using_cached = true;
            app.snapshot.gpu_sensors.error = Some("Fixture provider unavailable".into());
        }
        if variant.starts_with("storage-sensors") {
            app.page = Page::Sensors;
            let storage = std::sync::Arc::make_mut(&mut app.snapshot.storage_sensors);
            if variant.ends_with("-cached") {
                storage.drives[0].error = Some(crate::storage_sensors::Error::Timeout);
            } else if variant.ends_with("-unavailable") {
                storage.drives[0]
                    .temperatures
                    .sensors
                    .iter_mut()
                    .for_each(|s| s.celsius = None);
                storage.drives[0].last_success = None;
                storage.drives[0].error = Some(crate::storage_sensors::Error::Windows(5));
            }
        }
        if variant.starts_with("gpu-sensors") {
            app.performance_device = PerformanceDevice::GpuSensors;
        }
        if variant == "gpu-sensors-unavailable" {
            app.snapshot.gpu_sensors.adapters.clear();
            app.snapshot.gpu_sensors.error =
                Some("Fixture: NVIDIA driver unavailable. Other telemetry still works.".into());
        }
        let size = if variant.ends_with("-compact") || variant.starts_with("preferences-") {
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
    contrast::render_cases(&mut renderer, &directory);
    compact_layout::render_cases(&mut renderer, &directory);
    println!(
        "Offscreen visual pass: 101 PNGs in {}; no native window or OS input",
        directory.display()
    );
}
