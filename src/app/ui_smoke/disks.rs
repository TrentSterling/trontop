use super::*;
use crate::disk_activity::{Device, Metric, Reading, Snapshot};
use std::time::{Duration, Instant};

pub(super) fn install(app: &mut TrontopApp, missing: bool) {
    let now = Instant::now();
    app.page = Page::Performance;
    app.performance_device = PerformanceDevice::PhysicalDisks;
    let mut snapshot = Snapshot {
        generation: 2,
        at: Some(now),
        query_millis: 1.25,
        devices: vec![Device {
            instance: "0 C: D: (test fixture)".into(),
            number: 0,
            readings: [73.8, 4.75, 3.0, 165_500_000.0, 725_000.0].map(|v| Reading {
                value: Some(v),
                at: Some(now),
            }),
        }],
        ..Default::default()
    };
    if missing {
        snapshot.devices[0].readings[1].at = Some(now - Duration::from_secs(10));
        snapshot.devices[0].readings[2] = Reading::default();
        snapshot.error = Some("Fixture partial read: response cached, queue unavailable.");
    }
    app.snapshot.physical_disks = std::sync::Arc::new(snapshot);
    let history = app
        .physical_disk_history
        .values
        .entry("0 C: D: (test fixture)".into())
        .or_default();
    for (metric, values) in history.iter_mut().enumerate() {
        *values = (0..120)
            .map(|n| {
                if missing && n > 90 {
                    f32::NAN
                } else {
                    let wave = (n as f32 * 0.19).sin().abs();
                    match metric {
                        0 => 20.0 + wave * 65.0,
                        1 => 0.5 + wave * 5.0,
                        2 => (wave * 4.0).round(),
                        _ => wave * 150_000_000.0,
                    }
                }
            })
            .collect();
    }
}

#[test]
fn physical_disk_metrics_remain_aligned_across_missing_states_and_themes() {
    for dark in [true, false] {
        for size in [Vec2::new(1040.0, 640.0), Vec2::new(1280.0, 760.0)] {
            let settings = ThemeSettings {
                dark,
                ..Default::default()
            };
            let ctx = egui::Context::default();
            theme::install(&ctx, settings);
            let mut app = app(settings, true);
            let mut previous = vec![];
            for missing in [false, true] {
                install(&mut app, missing);
                // Let egui's initial scrollbar-gutter animation finish before
                // comparing geometry between live and partial snapshots.
                for _ in 0..20 {
                    frame(&ctx, &mut app, size, vec![]);
                }
                let output = frame(&ctx, &mut app, size, vec![]);
                let shapes = text_shapes(&output);
                if size.y >= 760.0 {
                    let legends: Vec<_> = shapes
                        .iter()
                        .filter(|(s, _)| s.galley.job.text == "120 SAMPLES")
                        .collect();
                    assert_eq!(legends.len(), 3);
                    assert!(
                        legends
                            .iter()
                            .all(|(s, clip)| clip.contains_rect(s.visual_bounding_rect())),
                        "All three graphs should fit at {size:?}"
                    );
                }
                let mut bounds = vec![];
                let mut baselines = vec![];
                for metric in Metric::ALL {
                    let (text, clip) = shapes
                        .iter()
                        .find(|(s, _)| s.galley.job.text == metric.label())
                        .expect("metric missing");
                    let rect = text.visual_bounding_rect();
                    assert!(
                        clip.contains_rect(rect),
                        "{} clipped at {size:?}",
                        metric.label()
                    );
                    assert_eq!(text.galley.rows.len(), 1);
                    bounds.push(rect);
                    baselines.push(text.pos.y);
                }
                // Compare text origins, not glyph ink bounds (Q has overshoot).
                assert!((baselines[0] - baselines[1]).abs() < 0.1);
                assert!((baselines[1] - baselines[2]).abs() < 0.1);
                assert!((baselines[3] - baselines[4]).abs() < 0.1);
                if !previous.is_empty() {
                    assert_eq!(previous, bounds);
                }
                previous = bounds;
                if missing {
                    assert!(shapes.iter().any(|(s, _)| s.galley.job.text == "Cached"));
                    assert!(
                        shapes
                            .iter()
                            .any(|(s, _)| s.galley.job.text == "Unavailable")
                    );
                }
            }
            app.snapshot.physical_disks = Default::default();
            let output = frame(&ctx, &mut app, size, vec![]);
            assert!(
                text_shapes(&output)
                    .iter()
                    .any(|(s, _)| s.galley.job.text == "Active time")
            );
        }
    }
}

#[test]
fn physical_disk_history_gaps_duplicate_cached_and_removed_samples() {
    let at = Instant::now();
    let mut s = Snapshot {
        generation: 1,
        at: Some(at),
        devices: vec![Device {
            instance: "7".into(),
            readings: [Reading {
                value: Some(0.0),
                at: Some(at),
            }; 5],
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut h = super::super::disks::Histories::default();
    h.push(&s, at);
    assert_eq!(h.values["7"][0][0], 0.0);
    h.push(&s, at);
    assert!(h.values["7"][0][1].is_nan());
    s.generation += 1;
    s.at = Some(at + Duration::from_secs(1));
    h.push(&s, at + Duration::from_secs(1));
    assert!(h.values["7"][0][2].is_nan());
    s.devices.clear();
    h.push(&s, at);
    assert!(h.values.is_empty());
}

#[test]
fn physical_disk_selection_follows_instance_not_inventory_position() {
    let settings = ThemeSettings::default();
    let ctx = egui::Context::default();
    theme::install(&ctx, settings);
    let mut app = app(settings, true);
    install(&mut app, false);
    let mut s = (*app.snapshot.physical_disks).clone();
    let mut second = s.devices[0].clone();
    second.instance = "3 E:".into();
    second.number = 3;
    second.readings[0].value = Some(17.9);
    s.devices.insert(0, second);
    app.snapshot.physical_disks = std::sync::Arc::new(s);
    app.selected_physical_disk = Some("0 C: D: (test fixture)".into());
    let output = frame(&ctx, &mut app, Vec2::new(1280.0, 900.0), vec![]);
    assert!(
        text_shapes(&output)
            .iter()
            .any(|(s, _)| s.galley.job.text == "73.8%")
    );
    let mut s = (*app.snapshot.physical_disks).clone();
    s.devices.pop();
    app.snapshot.physical_disks = std::sync::Arc::new(s);
    let output = frame(&ctx, &mut app, Vec2::new(1280.0, 900.0), vec![]);
    assert!(
        !text_shapes(&output)
            .iter()
            .any(|(s, _)| s.galley.job.text == "17.9%")
    );
    assert_eq!(
        app.selected_physical_disk.as_deref(),
        Some("0 C: D: (test fixture)")
    );
}
