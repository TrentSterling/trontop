//! Production plot interaction and drawing, entirely local egui input.
use super::*;
use history::{Chart, Id, Point, Unit};

fn chart(now: Instant, title: &str, points: &[(i64, Option<f32>, bool)]) -> Chart {
    Chart {
        id: Id::Activity(title.into()),
        title: title.into(),
        detail: "Fixture only".into(),
        group: Group::System,
        unit: Unit::Percent,
        points: points
            .iter()
            .map(|&(age, value, partial)| Point {
                at: if age >= 0 {
                    now - Duration::from_secs(age as u64)
                } else {
                    now + Duration::from_secs(age.unsigned_abs())
                },
                value,
                partial,
            })
            .collect(),
        maximum: Some(100.0),
        current: points.last().and_then(|point| point.1),
        state: "Live",
        measured_at: Some(now),
        last_seen: now,
        cadence: Duration::from_secs(1),
        partial: false,
        device: None,
        wall: true,
    }
}

fn draw(card: &wall::Card<'_>, now: Instant, style: Style, age: Option<f32>) -> egui::FullOutput {
    let ctx = egui::Context::default();
    let settings = ThemeSettings::default();
    theme::install(&ctx, settings);
    ctx.global_style_mut(|s| s.interaction.tooltip_delay = 0.0);
    let mut output = egui::FullOutput::default();
    for index in 0..8 {
        let events = age
            .map(|age| {
                vec![egui::Event::PointerMoved(egui::pos2(
                    585.0 - age / 120.0 * 578.0,
                    70.0,
                ))]
            })
            .unwrap_or_default();
        output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(600.0, 320.0),
                )),
                time: Some(index as f64),
                events,
                ..Default::default()
            },
            |ui| {
                ui.set_width(592.0);
                plot(
                    ui,
                    card,
                    now,
                    style,
                    &[theme::tokens(settings).accent],
                    theme::tokens(settings),
                    160.0,
                );
            },
        );
        assert!(output.platform_output.commands.is_empty());
        assert!(
            output
                .viewport_output
                .values()
                .all(|v| v.commands.is_empty())
        );
    }
    output
}

fn text(output: &egui::FullOutput) -> String {
    fn visit(shape: &egui::Shape, strings: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(t) => strings.push(t.galley.job.text.clone()),
            egui::Shape::Vec(shapes) => {
                for s in shapes {
                    visit(s, strings);
                }
            }
            _ => {}
        }
    }
    let mut strings = vec![];
    for s in &output.shapes {
        visit(&s.shape, &mut strings);
    }
    strings.join("\n")
}

#[test]
fn graph_hover_refuses_other_series_values_outside_the_hovered_interval() {
    let now = Instant::now();
    let first = chart(now, "Read", &[(10, Some(45.0), false)]);
    for (age, value) in [(55, 11.0), (-1, 22.0), (121, 33.0)] {
        let second = chart(now, "Write", &[(age, Some(value), false)]);
        let mut card = wall::Card::single(&first);
        card.series[0].label = "Read";
        card.series.push(wall::Series {
            chart: &second,
            label: "Write",
        });
        for style in [Style::Lines, Style::Bars] {
            let rendered = text(&draw(&card, now, style, Some(10.0)));
            assert!(rendered.contains("Read: 45.0%"), "{rendered}");
            assert!(
                rendered.contains("Write: No exact measurement"),
                "{rendered}"
            );
            assert!(
                !rendered.contains(&format!("Write: {value:.1}%")),
                "{rendered}"
            );
        }
    }
}

#[test]
fn graph_hover_preserves_missing_samples_and_lower_bound_markers() {
    let now = Instant::now();
    let chart = chart(
        now,
        "Fixture CPU",
        &[(10, None, false), (9, Some(42.0), true)],
    );
    let card = wall::Card::single(&chart);
    for style in [Style::Lines, Style::Bars] {
        let missing = text(&draw(&card, now, style, Some(10.0)));
        assert!(missing.contains("No exact measurement"), "{missing}");
        let partial = text(&draw(&card, now, style, Some(9.0)));
        assert!(partial.contains("42.0%+ (lower bound)"), "{partial}");
    }
}

#[test]
fn plot_lines_leave_explicit_and_long_gaps_and_draw_partial_points_hollow() {
    let now = Instant::now();
    let chart = chart(
        now,
        "Fixture gaps",
        &[
            (10, Some(20.0), false),
            (9, None, false),
            (3, Some(40.0), false),
            (2, Some(50.0), true),
        ],
    );
    let output = draw(&wall::Card::single(&chart), now, Style::Lines, None);
    let segments: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::LineSegment { points, stroke } if stroke.width == 1.5 => Some(*points),
            _ => None,
        })
        .collect();
    assert_eq!(
        segments.len(),
        1,
        "only the two adjacent measured samples can connect"
    );
    assert!(segments[0][0].x < segments[0][1].x);
    let fill_indices: usize = output
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Mesh(mesh) => Some(mesh.indices.len()),
            _ => None,
        })
        .sum();
    assert_eq!(fill_indices, 6, "fill must stop at both gaps");
    assert!(output.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Circle(circle) if circle.radius == 2.0 && circle.fill == Color32::TRANSPARENT && circle.stroke.width == 1.0)));
    let bars = draw(&wall::Card::single(&chart), now, Style::Bars, None);
    assert!(!bars.shapes.iter().any(
        |s| matches!(&s.shape, egui::Shape::LineSegment { stroke, .. } if stroke.width == 1.5)
    ));
}

#[test]
fn graph_hover_respects_each_series_cadence_and_preserves_its_missing_sample() {
    let now = Instant::now();
    let reference = chart(now, "Read", &[(10, Some(45.0), false)]);
    let mut adjacent = chart(now, "Write", &[(11, Some(7.0), false)]);
    adjacent.cadence = Duration::from_secs(10);
    let mut card = wall::Card::single(&reference);
    card.series.push(wall::Series {
        chart: &adjacent,
        label: "Write",
    });
    let rendered = text(&draw(&card, now, Style::Lines, Some(10.0)));
    assert!(rendered.contains("Write: 7.0%"), "{rendered}");
    adjacent.points.push_back(Point {
        at: now - Duration::from_secs(10),
        value: None,
        partial: false,
    });
    let mut card = wall::Card::single(&reference);
    card.series.push(wall::Series {
        chart: &adjacent,
        label: "Write",
    });
    let rendered = text(&draw(&card, now, Style::Lines, Some(10.0)));
    assert!(
        rendered.contains("Write: No exact measurement"),
        "{rendered}"
    );
    assert!(!rendered.contains("Write: 7.0%"), "{rendered}");
}
