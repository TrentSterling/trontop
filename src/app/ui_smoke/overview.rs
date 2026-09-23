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
    assert!(
        texts
            .iter()
            .any(|(text, _)| text.galley.job.text.starts_with("No reading:")
                && text.galley.job.text.contains("WDC WD60EZAX")),
        "the silent drive is listed on the gap row"
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
fn share_bar_lies_inside_its_own_row_and_clears_the_next_icon() {
    use super::super::overview::{PROCESS_ROW_HEIGHT, RowLayout};
    let row = egui::Rect::from_min_size(
        egui::pos2(200.0, 300.0),
        Vec2::new(380.0, PROCESS_ROW_HEIGHT),
    );
    for value_width in [0.0, 40.0, 70.0] {
        let layout = RowLayout::new(row, value_width);
        let next = RowLayout::new(
            row.translate(Vec2::new(0.0, PROCESS_ROW_HEIGHT)),
            value_width,
        );
        for fraction in [0.0, 0.01, 0.5, 1.0, 3.0] {
            let bar = layout.bar(fraction);
            assert!(row.contains_rect(bar), "bar {bar:?} leaves row {row:?}");
            assert!(bar.height() == 2.0 && bar.bottom() < row.bottom());
            assert!(!bar.intersects(next.icon), "bar under the next icon");
            assert!(!bar.intersects(layout.icon) && bar.left() >= layout.name.left());
            assert!(bar.right() <= layout.value_left + value_width + 0.01);
            assert!(
                bar.top() >= layout.line.bottom(),
                "bar crosses the text line"
            );
        }
    }
}

#[test]
fn overview_plan_matches_the_rendered_page_and_fills_tall_windows() {
    use super::super::overview::{Plan, Shape};
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
    // Tall windows spend spare height on rows, then cores, then bands.
    // 1000x580 leaves a 464 px viewport: the base layout, gaps and degraded
    // footer included, fits without a scroll bar.
    assert!(Plan::fit(464.0, &shape).height(&shape) < 464.0);
    for (viewport, margin) in [(700.0, 8.0), (900.0, 8.0)] {
        let plan = Plan::fit(viewport, &shape);
        assert_eq!(plan.slots, 10, "{viewport}");
        assert_eq!(plan.core_cell, 28.0, "{viewport}");
        assert!(plan.kpi_height > small.kpi_height && plan.thermal_height > small.thermal_height);
        let height = plan.height(&shape);
        assert!(
            height <= viewport && viewport - height < margin,
            "{viewport}: plan height {height}"
        );
    }

    // Rendered: the page ends within 80 px of the window bottom when tall,
    // and nothing scrolls.
    for size in [Vec2::new(1280.0, 800.0), Vec2::new(1600.0, 1000.0)] {
        let mut app = app(ThemeSettings::default(), true);
        let output = render(&mut app, size);
        let bottom = output
            .shapes
            .iter()
            .filter(|clipped| clipped.clip_rect.left() > 196.0 || clipped.clip_rect.left() == 0.0)
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
            bottom <= size.y && size.y - bottom <= 80.0,
            "{size:?}: Overview content ends at {bottom}"
        );
    }
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
