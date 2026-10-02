//! Overview layout: KPI row, Thermals and power band, top processes above the
//! fold at 1000x580, and gaps as one compact row, never tiles or "Unavailable".
use super::*;
use std::time::{Duration, Instant};

const SMALL: Vec2 = Vec2::new(1000.0, 580.0);

fn render(app: &mut TrontopApp, size: Vec2) -> egui::FullOutput {
    let ctx = egui::Context::default();
    theme::install(&ctx, app.theme);
    app.page = Page::Overview;
    let mut output = egui::FullOutput::default();
    for _ in 0..3 {
        output = frame(&ctx, app, size, vec![]);
    }
    output
}

/// A present drive whose storage driver reports no temperature sensor.
fn add_silent_drive(app: &mut TrontopApp) {
    let mut snapshot = app.snapshot.clone();
    let mut storage = (*snapshot.storage_sensors).clone();
    let at = Instant::now() + Duration::from_secs(3600);
    storage.drives.push(crate::storage_sensors::DriveReading {
        device: crate::storage_sensors::Device {
            id: "FIXTURE-SILENT-DRIVE".into(),
            name: "WDC WD60EZAX-00C8VB0".into(),
        },
        temperatures: Default::default(),
        last_attempt: Some(at),
        last_success: None,
        query_millis: Some(1.0),
        error: Some(crate::storage_sensors::Error::Windows(1)),
        present: true,
    });
    snapshot.storage_sensors = std::sync::Arc::new(storage);
    snapshot.sequence += 1;
    app.accept_sample(snapshot);
}

#[test]
fn kpi_row_renders_five_tiles_in_one_row_at_1000px() {
    let mut app = app(ThemeSettings::default(), true);
    let output = render(&mut app, SMALL);
    let texts = text_shapes(&output);
    let labels = ["CPU", "Memory", "GPU", "Disk", "Network"];
    let found: Vec<egui::Rect> = labels
        .iter()
        .map(|label| {
            texts
                .iter()
                .filter(|(text, _)| text.galley.job.text == *label && text.pos.x > 196.0)
                // The galley rect, not the glyph bounds: sentence-case
                // labels have different ascender heights.
                .map(|(text, _)| egui::Rect::from_min_size(text.pos, text.galley.size()))
                .min_by(|a, b| a.top().total_cmp(&b.top()))
                .unwrap_or_else(|| panic!("missing KPI tile {label}"))
        })
        .collect();
    for pair in found.windows(2) {
        assert!(
            (pair[0].top() - pair[1].top()).abs() < 0.5,
            "KPI tiles must share one row: {found:?}"
        );
        assert!(pair[0].left() < pair[1].left(), "KPI order: {found:?}");
    }
    assert_eq!(app.overview_kpis(Instant::now(), app.colors()).len(), 5);
}

#[test]
fn top_cpu_rows_lie_above_the_fold_at_1000x580() {
    let mut app = app(ThemeSettings::default(), true);
    let output = render(&mut app, SMALL);
    let texts = text_shapes(&output);
    let header = texts
        .iter()
        .find(|(text, _)| text.galley.job.text == "Top CPU")
        .expect("Top CPU header");
    assert!(header.0.visual_bounding_rect().bottom() <= SMALL.y);
    // The fixture's five busiest processes are workers 59 to 63.
    for index in 59..=63 {
        let name = format!("Fixture.Worker.With.A.Deliberately.Long.Name.{index}.exe");
        let rows: Vec<_> = texts
            .iter()
            .filter(|(text, _)| text.galley.job.text == name)
            .collect();
        assert!(!rows.is_empty(), "missing top process row {index}");
        for (text, clip) in rows {
            let rect = text.visual_bounding_rect();
            assert!(
                rect.bottom() <= SMALL.y && clip.intersects(rect) && clip.bottom() <= SMALL.y,
                "top process row {index} is below the fold: {rect:?}"
            );
        }
    }
}

#[test]
fn overview_never_shows_unavailable_text() {
    for populated in [true, false] {
        let mut app = app(ThemeSettings::default(), populated);
        if populated {
            add_silent_drive(&mut app);
        }
        for size in [SMALL, Vec2::new(1600.0, 1000.0)] {
            let output = render(&mut app, size);
            for (text, _) in text_shapes(&output) {
                assert!(
                    !text.galley.job.text.contains("Unavailable")
                        && !text.galley.job.text.contains("No measured samples"),
                    "Overview shows {:?} (populated {populated}, {size:?})",
                    text.galley.job.text
                );
            }
        }
    }
}

#[test]
fn drive_without_sensors_is_a_gap_row_not_a_tile() {
    let mut app = app(ThemeSettings::default(), true);
    add_silent_drive(&mut app);
    let (tiles, gaps) = app.overview_thermals(Instant::now(), app.colors());
    assert!(
        tiles.iter().all(|tile| !tile.label.contains("WD60EZAX")),
        "a drive with no sensor value must not become a tile"
    );
    assert!(
        tiles.iter().any(|tile| tile.label == "Fixture NVMe"),
        "a drive with sensors is one tile"
    );
    // Three sensors on one drive make one tile, showing the hottest.
    let drive = tiles
        .iter()
        .find(|tile| tile.label == "Fixture NVMe")
        .unwrap();
    assert_eq!(drive.value, "44 °C");
    assert!(drive.hover.contains("Sensor 2: 42 °C"));
    let gap = gaps
        .iter()
        .find(|gap| gap.label == "WDC WD60EZAX")
        .expect("silent drive gap");
    assert!(gap.reason.contains("does not report temperature"));
    // GPU temperature and power are tiles; there is no summed system power.
    assert!(tiles.iter().any(|tile| tile.label == "GPU temperature"));
    assert!(tiles.iter().any(|tile| tile.label == "GPU power"));
    assert!(tiles.iter().all(|tile| !tile.label.contains("System")));

    let output = render(&mut app, SMALL);
    let texts = text_shapes(&output);
    let gap_lines: Vec<_> = texts
        .iter()
        .filter(|(text, _)| text.galley.job.text.contains("not reported:"))
        .collect();
    assert_eq!(gap_lines.len(), 1, "one gap line");
    let line = &gap_lines[0].0.galley.job.text;
    assert!(
        line.contains("WDC WD60EZAX"),
        "the silent drive is listed on the gap line"
    );
    // Counted the way the Graphs footer counts: the CPU temperature has its
    // own shared wording, not a slot in the "N signals" count.
    assert!(
        line.contains(&format!(
            "{} not reported:",
            super::super::graphs::signal_count(gaps.len())
        )),
        "gap line count must match the listed gaps: {line}"
    );
    assert!(
        gaps.iter().all(|gap| gap.label != "CPU temperature"),
        "CPU temperature is not counted as a gap"
    );
    // One element: no separate "N missing" or reason chip beside it.
    assert!(
        texts.iter().all(|(text, _)| {
            let text = &text.galley.job.text;
            !text.ends_with(" missing") && text != "No sensor" && text != "Not checked"
        }),
        "the gap line carries no chip"
    );
}

#[test]
fn disk_total_marks_partial_coverage_and_leaves_a_gap_without_readings() {
    use crate::disk_activity::{Device, Reading, Snapshot};
    let now = Instant::now();
    let live = |value| Reading {
        value: Some(value),
        at: Some(now),
    };
    let mut first = Device {
        instance: "0 C:".into(),
        number: 0,
        ..Default::default()
    };
    first.readings[3] = live(1_000.0);
    first.readings[4] = live(500.0);
    let second = Device {
        instance: "1 D:".into(),
        number: 1,
        ..Default::default()
    };
    let snapshot = Snapshot {
        at: Some(now),
        generation: 5,
        devices: vec![first.clone()],
        ..Default::default()
    };
    assert_eq!(
        super::super::overview::disk_throughput(&snapshot, now),
        Some((1_500.0, false))
    );
    let partial = Snapshot {
        devices: vec![first, second],
        ..snapshot.clone()
    };
    assert_eq!(
        super::super::overview::disk_throughput(&partial, now),
        Some((1_500.0, true))
    );
    assert_eq!(
        super::super::overview::disk_throughput(&Snapshot::default(), now),
        None
    );

    // No live disk reading pushes a gap, never a zero.
    let app = app(ThemeSettings::default(), true);
    assert!(app.overview_disk_total.iter().all(|v| v.is_nan()));
    assert!(app.overview_net_total.iter().all(|v| v.is_finite()));
}

/// Names of the fixture's Top CPU rows, top to bottom, with their text rects.
fn top_cpu_names(output: &egui::FullOutput) -> Vec<egui::Rect> {
    let mut rows: Vec<egui::Rect> = text_shapes(output)
        .iter()
        .filter(|(text, _)| {
            text.galley
                .job
                .text
                .starts_with("Fixture.Worker.With.A.Deliberately.Long.Name.")
        })
        .map(|(text, _)| text.visual_bounding_rect())
        .collect();
    rows.sort_by(|a, b| a.top().total_cmp(&b.top()));
    rows
}

#[test]
fn top_list_rows_are_24px_apart_at_every_size() {
    for size in [SMALL, Vec2::new(1280.0, 800.0), Vec2::new(1600.0, 1000.0)] {
        let mut app = app(ThemeSettings::default(), true);
        let output = render(&mut app, size);
        // Both lists show fixture workers; keep the left (Top CPU) column.
        let left = top_cpu_names(&output)
            .into_iter()
            .map(|rect| rect.left())
            .fold(f32::INFINITY, f32::min);
        let names: Vec<_> = top_cpu_names(&output)
            .into_iter()
            .filter(|rect| (rect.left() - left).abs() < 1.0)
            .collect();
        assert!(names.len() >= 5, "{size:?}: only {} rows", names.len());
        for pair in names.windows(2) {
            let pitch = pair[1].top() - pair[0].top();
            assert!(
                pitch >= super::super::overview::PROCESS_ROW_HEIGHT - 0.5,
                "{size:?}: Top CPU row pitch {pitch} px is below 24"
            );
        }
    }
}

#[test]
fn overview_plan_fits_small_windows_and_caps_tile_growth() {
    use super::super::overview::{CORE_CELL_MAX, KPI_TILE_MAX, Plan, Shape, THERMAL_TILE_MAX};
    let shape = Shape {
        kpi_rows: 1,
        thermal_rows: 1,
        gaps: true,
        stacked_lists: false,
        core_rows: 1,
        degraded: true,
    };
    // The base layout is used whenever the window has no room to spare.
    let small = Plan::fit(440.0, &shape);
    assert_eq!(small.slots, 5);
    assert_eq!(small.core_cell, 18.0);
    // 1000x580 leaves a 464 px viewport: the base layout, gap line and
    // degraded footer included, fits without a scroll bar.
    assert!(Plan::fit(464.0, &shape).height(&shape) < 464.0);
    // Spare height goes to rows, then cores, then tiles up to their caps;
    // the rest stays empty.
    for viewport in [700.0, 900.0, 2000.0] {
        let plan = Plan::fit(viewport, &shape);
        assert_eq!(plan.slots, 10, "{viewport}");
        assert_eq!(plan.core_cell, CORE_CELL_MAX, "{viewport}");
        assert!(plan.kpi_height <= KPI_TILE_MAX && plan.thermal_height <= THERMAL_TILE_MAX);
        assert!(plan.height(&shape) < viewport, "{viewport}");
    }
    let huge = Plan::fit(2000.0, &shape);
    assert_eq!(huge.kpi_height, KPI_TILE_MAX);
    assert_eq!(huge.thermal_height, THERMAL_TILE_MAX);

    // Rendered: tiles stop at their caps on a large window and the page
    // still ends inside it.
    for size in [SMALL, Vec2::new(1280.0, 800.0), Vec2::new(1600.0, 1000.0)] {
        let mut app = app(ThemeSettings::default(), true);
        add_silent_drive(&mut app);
        let output = render(&mut app, size);
        let texts = text_shapes(&output);
        let all = rect_shapes(&output);
        let tile_height = |label: &str| {
            let pos = texts
                .iter()
                .find(|(text, _)| text.galley.job.text == label && text.pos.x > 205.0)
                .unwrap_or_else(|| panic!("{size:?}: missing tile {label}"))
                .0
                .pos;
            all.iter()
                .filter(|rect| {
                    rect.contains(pos + Vec2::splat(1.0))
                        && rect.width() < size.x * 0.6
                        && rect.height() >= 60.0
                })
                .map(|rect| rect.height())
                .fold(0.0_f32, f32::max)
        };
        let kpi = tile_height("Memory");
        let thermal = tile_height("GPU power");
        assert!(
            (84.0..=KPI_TILE_MAX + 0.5).contains(&kpi),
            "{size:?}: KPI tile {kpi}"
        );
        assert!(
            (70.0..=THERMAL_TILE_MAX + 0.5).contains(&thermal),
            "{size:?}: thermal tile {thermal}"
        );
        // The last line (the degraded footer or the core strip) is on screen.
        let bottom = output
            .shapes
            .iter()
            .filter_map(|clipped| {
                let rect = clipped.shape.visual_bounding_rect();
                (rect.is_positive()
                    && rect.left() > 205.0
                    && rect.height() < 400.0
                    && rect.top() > 90.0)
                    .then_some(rect.bottom())
            })
            .fold(0.0_f32, f32::max);
        assert!(
            bottom <= size.y,
            "{size:?}: Overview content ends at {bottom}"
        );
    }
}

#[test]
fn share_fill_spans_the_row_from_its_left_edge_behind_the_text() {
    use super::super::overview::{PROCESS_ROW_HEIGHT, RowLayout};
    let row = egui::Rect::from_min_size(
        egui::pos2(200.0, 300.0),
        Vec2::new(380.0, PROCESS_ROW_HEIGHT),
    );
    for value_width in [0.0, 40.0, 70.0] {
        let layout = RowLayout::new(row, value_width);
        for fraction in [0.0, 0.01, 0.5, 1.0, 3.0] {
            let fill = layout.fill(fraction);
            assert!(row.contains_rect(fill), "fill {fill:?} leaves row {row:?}");
            assert_eq!(fill.left(), row.left(), "fill starts at the left edge");
            assert!(
                fill.height() >= PROCESS_ROW_HEIGHT - 2.0,
                "fill covers the row height, not an underline: {fill:?}"
            );
            assert!(
                fill.top() <= layout.name.center().y && fill.bottom() >= layout.name.center().y
            );
        }
        assert!((layout.fill(0.5).width() - row.width() * 0.5).abs() < 0.01);
        assert!((layout.fill(1.0).width() - row.width()).abs() < 0.01);
    }
}

/// Three instances of one executable, differing only in case.
fn add_find_instances(app: &mut TrontopApp) {
    let mut snapshot = app.snapshot.clone();
    for (offset, name) in ["find.exe", "FIND.EXE", "Find.exe"].into_iter().enumerate() {
        let mut row = snapshot.processes[1].clone();
        row.pid = 700_000 + offset as u32;
        row.name = name.into();
        row.cpu_percent = 4.2;
        row.memory_bytes = 10_000_000;
        snapshot.processes.push(row);
    }
    snapshot.process_count = snapshot.processes.len();
    snapshot.sequence += 1;
    app.accept_sample(snapshot);
}

fn rect_shapes(output: &egui::FullOutput) -> Vec<egui::Rect> {
    fn visit(shape: &egui::Shape, out: &mut Vec<egui::Rect>) {
        match shape {
            egui::Shape::Rect(rect) => out.push(rect.rect),
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| visit(shape, out)),
            _ => {}
        }
    }
    let mut all = Vec::new();
    for clipped in &output.shapes {
        visit(&clipped.shape, &mut all);
    }
    all
}

#[test]
fn top_lists_group_instances_by_app_with_a_count_chip() {
    use super::super::overview::top_apps;
    let mut app = app(ThemeSettings::default(), true);
    add_find_instances(&mut app);
    let top = top_apps(&app.snapshot.processes, false);
    let find: Vec<_> = top
        .iter()
        .filter(|group| group.lead.name.eq_ignore_ascii_case("find.exe"))
        .collect();
    assert_eq!(find.len(), 1, "three instances make one row");
    assert_eq!(find[0].pids.len(), 3);
    assert!(
        (find[0].value - 12.6).abs() < 1e-4,
        "summed CPU {}",
        find[0].value
    );
    assert!(!find[0].partial);
    assert!(
        std::ptr::eq(top[0].lead, find[0].lead),
        "12.6% outranks every single worker"
    );
    let memory = top_apps(&app.snapshot.processes, true);
    assert!(memory.len() <= 10);
    assert!(
        memory.windows(2).all(|pair| pair[0].value >= pair[1].value),
        "memory list is heaviest first"
    );

    // A non-finite instance is left out of the sum and marks a lower bound.
    let mut rows = app.snapshot.processes.clone();
    rows.last_mut().unwrap().cpu_percent = f32::NAN;
    let partial = top_apps(&rows, false);
    let find = partial
        .iter()
        .find(|group| group.lead.name.eq_ignore_ascii_case("find.exe"))
        .unwrap();
    assert!(find.partial && (find.value - 8.4).abs() < 1e-4 && find.pids.len() == 3);

    let output = render(&mut app, SMALL);
    let texts = text_shapes(&output);
    let chip = texts
        .iter()
        .find(|(text, _)| text.galley.job.text == "x3")
        .expect("x3 instance chip");
    let value = texts
        .iter()
        .find(|(text, _)| text.galley.job.text == "12.6%")
        .expect("summed value 12.6%");
    assert!(
        (chip.0.visual_bounding_rect().center().y - value.0.visual_bounding_rect().center().y)
            .abs()
            < 2.0,
        "the chip and the summed value share one row"
    );
    let names = texts
        .iter()
        .filter(|(text, _)| text.galley.job.text.eq_ignore_ascii_case("find.exe"))
        .count();
    assert_eq!(names, 1, "find.exe is listed once in Top CPU");
}

#[test]
fn top_rows_draw_no_underline_bars() {
    use super::super::overview::PROCESS_ROW_HEIGHT;
    let mut app = app(ThemeSettings::default(), true);
    let output = render(&mut app, SMALL);
    let names = top_cpu_names(&output);
    assert!(names.len() >= 5);
    let all = rect_shapes(&output);
    let mut fills = 0;
    for name in &names {
        for rect in &all {
            let below = rect.top() >= name.bottom() - 1.0 && rect.top() < name.bottom() + 10.0;
            let overlaps = rect.left() < name.right() && rect.right() > name.left();
            assert!(
                !(below && overlaps && rect.height() <= 4.0),
                "underline bar {rect:?} under row {name:?}"
            );
        }
        if all.iter().any(|rect| {
            rect.height() >= PROCESS_ROW_HEIGHT - 2.0
                && rect.height() <= PROCESS_ROW_HEIGHT
                && rect.top() <= name.center().y
                && rect.bottom() >= name.center().y
                && rect.left() < name.left()
        }) {
            fills += 1;
        }
    }
    assert!(
        fills >= 5,
        "each top row has a full-height share fill ({fills})"
    );
}

#[test]
fn disk_and_drive_sparklines_follow_the_theme_preset() {
    let mut colors = Vec::new();
    for settings in [ThemeSettings::default(), ThemeSettings::copper_legacy()] {
        let app = app(settings, true);
        let t = app.colors();
        let kpis = app.overview_kpis(Instant::now(), t);
        let disk = kpis.iter().find(|tile| tile.label == "Disk").unwrap();
        assert_eq!(disk.color, theme::mix(t.accent, t.secondary, 0.25));
        assert_ne!(disk.color, t.good, "Disk must not use the fixed good color");
        let (tiles, _) = app.overview_thermals(Instant::now(), t);
        let drive = tiles
            .iter()
            .find(|tile| tile.label == "Fixture NVMe")
            .unwrap();
        assert_eq!(drive.color, t.secondary);
        assert_ne!(drive.color, t.good);
        colors.push((disk.color, drive.color));
    }
    assert_ne!(colors[0].0, colors[1].0, "presets recolor the Disk KPI");
    assert_ne!(
        colors[0].1, colors[1].1,
        "presets recolor drive temperatures"
    );
}

#[test]
fn thermal_band_range_keeps_a_steady_reading_mid_band() {
    let steady = [Some(50.0), Some(50.0), Some(51.0)];
    let (lo, hi) = widgets::padded_range(&steady, 10.0).unwrap();
    assert!(hi - lo >= 10.0 && lo < 50.0 && hi > 51.0);
    let mid = (lo + hi) / 2.0;
    assert!((mid - 50.5).abs() < 0.01);
    let watts = [Some(0.5), Some(1.0)];
    let (lo, hi) = widgets::padded_range(&watts, 20.0).unwrap();
    assert!(
        lo >= 0.0 && hi - lo >= 20.0,
        "power never ranges below zero"
    );
    assert!(widgets::padded_range(&[None, None], 10.0).is_none());
}

#[test]
fn network_short_caption_shares_one_unit() {
    use super::super::overview::rate_pair;
    assert_eq!(rate_pair(2_058.0, 6_420.0), "down 2.01 \u{b7} up 6.27 KB/s");
    assert_eq!(rate_pair(532.0, 1_229.0), "down 0.52 \u{b7} up 1.20 KB/s");
    assert_eq!(rate_pair(180.0, 146.0), "down 180 \u{b7} up 146 B/s");
    assert_eq!(rate_pair(0.0, 0.0), "down 0 \u{b7} up 0 B/s");
}

/// A KPI tile with no current value draws no sparkline, on all five tiles:
/// "--" beside a live-looking trend would contradict itself.
#[test]
fn kpi_tiles_without_a_value_draw_no_trend() {
    let mut app = app(ThemeSettings::default(), true);
    let t = app.colors();
    for _ in 0..30 {
        for history in [
            &mut app.cpu_history,
            &mut app.memory_history,
            &mut app.gpu_history,
            &mut app.overview_disk_total,
            &mut app.overview_net_total,
        ] {
            history.push_back(12.0);
        }
    }
    let now = Instant::now();
    for tile in app.overview_kpis(now, t) {
        if tile.value == "--" {
            assert!(
                tile.series.is_empty(),
                "{} draws a trend beside --",
                tile.label
            );
        }
    }

    // Every current value missing: no sample yet, no memory total, GPU
    // counters unavailable, disk counters long stale, no network adapter.
    app.seen_generation = 0;
    app.snapshot.memory_total_bytes = 0;
    app.snapshot.gpu = Default::default();
    app.snapshot.gpu.error = Some("fixture".into());
    app.snapshot.networks.clear();
    let later = now + Duration::from_secs(3600);
    let kpis = app.overview_kpis(later, t);
    assert_eq!(kpis.len(), 5);
    for tile in &kpis {
        assert_eq!(tile.value, "--", "{} has no current value", tile.label);
        assert!(
            tile.series.is_empty(),
            "{} keeps a live-looking trend beside --",
            tile.label
        );
    }
}

fn bridge_fixture(app: &mut TrontopApp) {
    use crate::specs::{BridgeReading, BridgeReadings, LiveKey, LiveUnit, Value};
    app.specs_view.bridge = std::sync::Arc::new(BridgeReadings {
        readings: vec![
            BridgeReading {
                key: LiveKey::CpuPackageTemperature,
                label: "Fixture package".into(),
                value: 63.0,
                unit: LiveUnit::Celsius,
                source: "Fixture bridge".into(),
            },
            BridgeReading {
                key: LiveKey::Sensor {
                    id: crate::specs::cpu::PACKAGE_POWER.into(),
                },
                label: "Fixture package power".into(),
                value: 37.0,
                unit: LiveUnit::Watts,
                source: "Fixture bridge".into(),
            },
        ],
        status: Value::known("Fixture bridge"),
        collected_at: Some(Instant::now() + Duration::from_secs(3600)),
        retry_after: None,
    });
}

#[test]
fn overview_and_system_refuse_retained_values_from_an_unavailable_bridge() {
    let mut app = app(ThemeSettings::default(), true);
    bridge_fixture(&mut app);
    let now = Instant::now();
    let (fresh, _) = app.overview_thermals(now, app.colors());
    assert!(
        fresh
            .iter()
            .any(|tile| tile.label == "CPU temperature" && tile.value == "63 °C")
    );
    assert!(
        fresh
            .iter()
            .any(|tile| tile.label == "CPU power" && tile.value == "37.0 W")
    );
    std::sync::Arc::make_mut(&mut app.specs_view.bridge).status =
        crate::specs::Value::unavailable("Fixture bridge disconnected");
    assert!(
        app.cpu_temperature_gap(now).is_some(),
        "retained package readings cannot suppress the unavailable-provider gap"
    );
    let (unavailable, _) = app.overview_thermals(now, app.colors());
    assert!(
        unavailable
            .iter()
            .all(|tile| tile.label != "CPU temperature" && tile.label != "CPU power")
    );
    let live = crate::specs::resolve(
        &crate::specs::LiveKey::CpuPackageTemperature,
        &app.snapshot,
        &app.specs_view.bridge,
        now,
    );
    assert_eq!(
        live.value,
        crate::specs::Value::unavailable("Fixture bridge disconnected")
    );
    assert!(live.celsius.is_none() && live.source.is_none());
    std::sync::Arc::make_mut(&mut app.specs_view.bridge).status =
        crate::specs::Value::known("Fixture bridge");
    assert!(
        app.overview_thermals(now, app.colors())
            .0
            .iter()
            .any(|tile| tile.label == "CPU temperature")
    );
    assert!(app.specs.is_none());
}

#[test]
fn overview_bridge_temperature_rejects_display_overflow_and_expired_polls() {
    let mut app = app(ThemeSettings::default(), true);
    bridge_fixture(&mut app);
    let now = Instant::now();
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, f64::MAX] {
        std::sync::Arc::make_mut(&mut app.specs_view.bridge).readings[0].value = value;
        let (tiles, _) = app.overview_thermals(now, app.colors());
        assert!(tiles.iter().all(|tile| tile.label != "CPU temperature"));
        assert!(tiles.iter().any(|tile| tile.label == "CPU power"));
    }
    let bridge = std::sync::Arc::make_mut(&mut app.specs_view.bridge);
    bridge.readings[0].value = 58.0;
    bridge.collected_at = Some(now - crate::specs::BRIDGE_STALE_AFTER - Duration::from_secs(1));
    assert!(
        app.overview_thermals(now, app.colors())
            .0
            .iter()
            .all(|tile| tile.label != "CPU temperature" && tile.label != "CPU power")
    );
    std::sync::Arc::make_mut(&mut app.specs_view.bridge).collected_at = Some(now);
    let (recovered, _) = app.overview_thermals(now, app.colors());
    assert!(
        recovered
            .iter()
            .any(|tile| tile.label == "CPU temperature" && tile.value == "58 °C")
    );
}

#[test]
fn overview_links_open_sensors_processes_cores_and_degraded_source_details() {
    for (link, expected) in [
        ("Sensor details", Page::Sensors),
        ("All processes", Page::Processes),
        ("All cores", Page::Graphs),
        ("Details", Page::Overview),
    ] {
        let ctx = egui::Context::default();
        let mut app = app(ThemeSettings::default(), true);
        app.page = Page::Overview;
        app.snapshot
            .diagnostics
            .get_mut(crate::diagnostics::Provider::GpuActivity)
            .record(
                Instant::now(),
                Duration::ZERO,
                crate::diagnostics::State::Partial,
                Some((1, 2)),
                None,
            );
        theme::install(&ctx, app.theme);
        let output = click_local_text_output(&ctx, &mut app, Vec2::new(1100.0, 900.0), link);
        assert_eq!(app.page, expected, "{link}");
        if link == "All cores" {
            let output = frame(&ctx, &mut app, Vec2::new(1100.0, 900.0), vec![]);
            assert!(
                text_shapes(&output)
                    .iter()
                    .any(|(text, _)| text.galley.job.text == "CPU 0")
            );
        }
        if link == "Details" {
            assert!(app.show_diagnostics);
        }
        assert!(output.platform_output.commands.is_empty());
    }
}

#[test]
fn overview_group_click_selects_the_heaviest_instance_and_opens_processes() {
    let ctx = egui::Context::default();
    let mut app = app(ThemeSettings::default(), true);
    let original = app.snapshot.processes[0].clone();
    app.snapshot.processes = [(900_010, 35.0), (900_011, 65.0), (900_012, 80.0)]
        .into_iter()
        .map(|(pid, cpu_percent)| {
            let mut row = original.clone();
            row.pid = pid;
            row.parent_pid = None;
            row.name = if pid == 900_012 {
                "Fixture other.exe"
            } else {
                "Fixture browser.exe"
            }
            .into();
            row.executable = Some(std::path::PathBuf::from(if pid == 900_012 {
                r"C:\Fixture\other.exe"
            } else {
                r"C:\Fixture\browser.exe"
            }));
            row.cpu_percent = cpu_percent;
            row.memory_bytes = u64::from(pid - 900_000) * 1_048_576;
            row
        })
        .collect();
    app.page = Page::Overview;
    theme::install(&ctx, app.theme);
    click_local_text(
        &ctx,
        &mut app,
        Vec2::new(1100.0, 900.0),
        "Fixture browser.exe",
    );
    assert_eq!(app.page, Page::Processes);
    assert_eq!(app.selected_pid, Some(900_011));
    assert_eq!(app.selected_process().unwrap().name, "Fixture browser.exe");
    assert!(!app.process_actions.busy());
}

#[test]
fn overview_partial_totals_and_retained_vram_keep_their_measurement_labels() {
    use crate::disk_activity::{Device, Metric, Reading, Snapshot};
    use crate::gpu_adapters::{Adapter, Description};
    let mut app = app(ThemeSettings::default(), true);
    gpu_activity_fixture(&mut app);
    let now = Instant::now();
    let mut adapter = Adapter {
        description: Some(Description {
            name: "Fixture hardware GPU".into(),
            vendor_id: 0,
            device_id: 0,
            dedicated_video: 16 << 30,
            dedicated_system: 0,
            shared_limit: 32 << 30,
            software: false,
        }),
        activity: crate::gpu_activity::Usage::Measured(20.0),
        ..Default::default()
    };
    adapter.memory[0].record(Some(4 << 30), now);
    let mut software = adapter.clone();
    software.description.as_mut().unwrap().software = true;
    software.activity = crate::gpu_activity::Usage::Measured(100.0);
    software.memory[0].record(Some(10 << 30), now);
    app.snapshot.gpu.adapters = vec![software, adapter];
    let mut disk = Device {
        instance: "0 C:".into(),
        ..Default::default()
    };
    disk.readings[Metric::Read as usize] = Reading {
        value: Some(2048.0),
        at: Some(now),
    };
    disk.readings[Metric::Active as usize] = Reading {
        value: Some(43.0),
        at: Some(now),
    };
    app.snapshot.physical_disks = std::sync::Arc::new(Snapshot {
        at: Some(now),
        generation: 2,
        devices: vec![disk],
        ..Default::default()
    });
    let tiles = app.overview_kpis(now, app.colors());
    let gpu = tiles.iter().find(|tile| tile.label == "GPU").unwrap();
    assert!(gpu.value.ends_with('+'));
    assert_eq!(gpu.state, Some("Partial"));
    assert_eq!(gpu.sub, "VRAM 4.00 GiB of 16.0 GiB");
    let disk = tiles.iter().find(|tile| tile.label == "Disk").unwrap();
    assert_eq!(disk.value, "2.00 KB/s+");
    assert_eq!(disk.state, Some("Partial"));
    assert_eq!(disk.sub, "busiest disk 43% active");
    assert!(disk.hover.contains("lower bound"));
    app.snapshot.gpu.adapters[1].memory[0].record(None, now);
    let tiles = app.overview_kpis(now, app.colors());
    let gpu = tiles.iter().find(|tile| tile.label == "GPU").unwrap();
    assert_eq!(gpu.sub, "VRAM ~4.00 GiB of 16.0 GiB");
    assert!(gpu.hover.contains("VRAM is retained"));
    app.snapshot.gpu.adapters[1].memory[0].record(Some(5 << 30), now);
    let gpu = app
        .overview_kpis(now, app.colors())
        .into_iter()
        .find(|tile| tile.label == "GPU")
        .unwrap();
    assert_eq!(gpu.sub, "VRAM 5.00 GiB of 16.0 GiB");
    app.snapshot.gpu.adapters.clear();
    app.snapshot.gpu_sensors.adapters[0].memory = Some((3 << 30, 8 << 30));
    app.snapshot.gpu_sensors.last_success = Some(now);
    app.snapshot.gpu_sensors.using_cached = false;
    let gpu = app
        .overview_kpis(now, app.colors())
        .into_iter()
        .find(|tile| tile.label == "GPU")
        .unwrap();
    assert_eq!(gpu.sub, "VRAM 3.00 GiB of 8.00 GiB");
    app.snapshot.gpu_sensors.using_cached = true;
    let gpu = app
        .overview_kpis(now, app.colors())
        .into_iter()
        .find(|tile| tile.label == "GPU")
        .unwrap();
    assert!(gpu.sub.is_empty());
    app.snapshot.gpu_sensors.using_cached = false;
    app.snapshot.gpu_sensors.last_success = Some(now - Duration::from_secs(4));
    let gpu = app
        .overview_kpis(now, app.colors())
        .into_iter()
        .find(|tile| tile.label == "GPU")
        .unwrap();
    assert!(gpu.sub.is_empty());
}

#[test]
fn overview_multi_gpu_thermal_tiles_keep_device_names_cache_and_provider_gaps() {
    let mut app = app(ThemeSettings::default(), false);
    let now = Instant::now();
    let mut snapshot = fixture();
    snapshot.gpu_sensors.sampled_at = Some(now);
    snapshot.gpu_sensors.last_success = Some(now);
    let mut second = snapshot.gpu_sensors.adapters[0].clone();
    second.uuid = Some("FIXTURE-GPU-1".into());
    second.name = "NVIDIA GeForce Fixture Second".into();
    second.temperature_c = Some(61);
    second.power_w = Some(103.0);
    let mut unidentified = second.clone();
    unidentified.uuid = None;
    unidentified.name = "Fixture GPU without UUID".into();
    snapshot.gpu_sensors.adapters.extend([second, unidentified]);
    app.accept_sample(snapshot.clone());
    let (live, _) = app.overview_thermals(now, app.colors());
    assert!(
        live.iter()
            .any(|tile| tile.label == "GPU temperature Fixture Second"
                && tile.value == "61 °C"
                && tile.state.is_none())
    );
    assert!(
        live.iter()
            .any(|tile| tile.label == "GPU power Fixture Second" && tile.value == "103 W")
    );
    assert!(live.iter().all(|tile| !tile.label.contains("without UUID")));
    snapshot.sequence += 1;
    snapshot.gpu_sensors.sampled_at = Some(now + Duration::from_secs(1));
    snapshot.gpu_sensors.using_cached = true;
    snapshot.gpu_sensors.error = Some("Fixture NVML unavailable".into());
    app.accept_sample(snapshot.clone());
    // Advance the production history with the fixture clock; accept_sample
    // uses wall time and correctly refuses the fixture's future timestamp.
    app.graphs.sample(&snapshot, now + Duration::from_secs(1));
    app.graphs.fixed_now = Some(now + Duration::from_secs(1));
    let (cached, _) = app.overview_thermals(now + Duration::from_secs(1), app.colors());
    let second = cached
        .iter()
        .find(|tile| tile.label == "GPU temperature Fixture Second")
        .unwrap();
    assert_eq!(second.value, "61 °C");
    assert_eq!(second.state, Some("Cached"));
    assert!(second.series.iter().any(Option::is_none));
    snapshot.sequence += 1;
    snapshot.gpu_sensors.adapters.clear();
    app.accept_sample(snapshot);
    let (tiles, gaps) = app.overview_thermals(now + Duration::from_secs(2), app.colors());
    assert!(tiles.iter().all(|tile| !tile.label.starts_with("GPU")));
    assert!(
        gaps.iter()
            .any(|gap| gap.label == "GPU temperature" && gap.reason == "Fixture NVML unavailable")
    );
}

#[test]
fn overview_core_hover_reads_current_load_and_keeps_missing_values_explicit() {
    let ctx = egui::Context::default();
    let mut app = app(ThemeSettings::default(), true);
    app.page = Page::Overview;
    app.snapshot.cpu.logical_usage[0] = Some(37.5);
    theme::install(&ctx, app.theme);
    ctx.global_style_mut(|style| style.interaction.tooltip_delay = 0.0);
    let size = Vec2::new(1100.0, 900.0);
    let mut output = frame(&ctx, &mut app, size, vec![]);
    for _ in 0..8 {
        output = frame(&ctx, &mut app, size, vec![]);
    }
    let cores_top = text_shapes(&output)
        .iter()
        .find(|(text, _)| text.galley.job.text == "Cores")
        .unwrap()
        .0
        .visual_bounding_rect()
        .bottom();
    let mut cells: Vec<_> = rect_shapes(&output)
        .into_iter()
        .filter(|rect| {
            rect.left() > 200.0
                && rect.top() > cores_top
                && (18.0..=36.0).contains(&rect.height())
                && rect.width() < 40.0
        })
        .collect();
    cells.sort_by(|a, b| a.left().total_cmp(&b.left()));
    let position = cells.first().expect("visible core cell").center();
    for value in [Some(37.5), None, Some(f32::NAN), Some(0.0)] {
        app.snapshot.cpu.logical_usage[0] = value;
        for _ in 0..8 {
            output = frame(
                &ctx,
                &mut app,
                size,
                vec![egui::Event::PointerMoved(position)],
            );
        }
        let expected = match value {
            Some(value) if value.is_finite() => format!("CPU 0: {value:.1}%"),
            _ => "CPU 0: not reported".into(),
        };
        assert!(
            text_shapes(&output)
                .iter()
                .any(|(text, _)| text.galley.job.text == expected),
            "missing {expected}"
        );
        assert!(output.platform_output.commands.is_empty());
    }
}

#[test]
#[ignore = "Offscreen Overview telemetry review, fixture data only; no native window or input"]
fn render_overview_truthfulness_review() {
    use crate::gpu_adapters::{Adapter, Description};
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/ui-smoke/overview-state-alpha47");
    std::fs::create_dir_all(&directory).unwrap();
    let mut renderer = offscreen::Renderer::new();
    for (name, dark, unavailable) in [
        ("fresh-dark", true, false),
        ("retained-dark", true, true),
        ("retained-light", false, true),
    ] {
        let ctx = egui::Context::default();
        let settings = ThemeSettings {
            dark,
            ..Default::default()
        };
        let mut app = app(settings, false);
        app.accept_sample(fixture());
        app.page = Page::Overview;
        bridge_fixture(&mut app);
        let now = Instant::now();
        let mut adapter = Adapter {
            description: Some(Description {
                name: "Fixture hardware GPU".into(),
                vendor_id: 0,
                device_id: 0,
                dedicated_video: 16 << 30,
                dedicated_system: 0,
                shared_limit: 32 << 30,
                software: false,
            }),
            activity: crate::gpu_activity::Usage::Measured(20.0),
            ..Default::default()
        };
        adapter.memory[0].record(Some(4 << 30), now);
        if unavailable {
            std::sync::Arc::make_mut(&mut app.specs_view.bridge).status =
                crate::specs::Value::unavailable("Fixture bridge disconnected");
            adapter.memory[0].record(None, now);
            app.snapshot.cpu.logical_usage[0] = None;
        }
        app.snapshot.gpu.adapters = vec![adapter];
        theme::install(&ctx, settings);
        let size = Vec2::new(1200.0, 900.0);
        let mut output = egui::FullOutput::default();
        for _ in 0..20 {
            output.append(frame(&ctx, &mut app, size, vec![]));
        }
        let texts = text_shapes(&output);
        assert!(
            texts
                .iter()
                .any(|(text, _)| text.galley.job.text.contains(if unavailable {
                    "VRAM ~4.00 GiB"
                } else {
                    "VRAM 4.00 GiB"
                }))
        );
        assert_eq!(
            texts
                .iter()
                .any(|(text, _)| text.galley.job.text == "63 °C"),
            !unavailable
        );
        assert_eq!(
            texts.iter().any(|(text, _)| text
                .galley
                .job
                .text
                .contains("CPU temp: not checked yet")),
            unavailable,
            "the worker-free fixture must retain its explicit CPU gap"
        );
        renderer.save(&ctx, output, size, &directory.join(format!("{name}.png")));
    }
    println!(
        "OVERVIEW_STATE: 3 fixture-only offscreen views in {}",
        directory.display()
    );
}
