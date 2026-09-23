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
        .filter(|(text, _)| text.galley.job.text.starts_with("Not reported:"))
        .collect();
    assert_eq!(gap_lines.len(), 1, "one gap line");
    assert!(
        gap_lines[0].0.galley.job.text.contains("WDC WD60EZAX"),
        "the silent drive is listed on the gap line"
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
    assert_eq!(rate_pair(2_058.0, 6_420.0), "in 2.01 \u{b7} out 6.27 KB/s");
    assert_eq!(rate_pair(532.0, 1_229.0), "in 0.52 \u{b7} out 1.20 KB/s");
    assert_eq!(rate_pair(180.0, 146.0), "in 180 \u{b7} out 146 B/s");
    assert_eq!(rate_pair(0.0, 0.0), "in 0 \u{b7} out 0 B/s");
}
