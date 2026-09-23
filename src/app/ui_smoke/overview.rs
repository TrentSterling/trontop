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
    let labels = ["CPU", "MEMORY", "GPU", "DISK", "NETWORK"];
    let found: Vec<egui::Rect> = labels
        .iter()
        .map(|label| {
            texts
                .iter()
                .filter(|(text, _)| text.galley.job.text == *label && text.pos.x > 196.0)
                .map(|(text, _)| text.visual_bounding_rect())
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
    assert!(tiles.iter().any(|tile| tile.label == "GPU temp"));
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
