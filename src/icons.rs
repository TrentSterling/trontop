//! Original, theme-colored vector symbols. No font glyphs, shell queries or assets.
use eframe::egui::{self, Color32, Painter, Rect, Shape, Stroke};

#[derive(Clone, Copy, Debug)]
pub enum Icon {
    Overview,
    Graphs,
    Processes,
    Performance,
    History,
    Startup,
    Users,
    Details,
    Services,
    Sensors,
    Theme,
    Info,
    Export,
    Stop,
    Restart,
    Expand,
    Collapse,
    Minimize,
    Maximize,
    Restore,
    Close,
}

impl Icon {
    pub fn paint(self, painter: &Painter, rect: Rect, color: Color32) {
        // A 24-unit grid with inset artwork leaves room for antialiased strokes.
        let scale = rect.width().min(rect.height()) / 24.0;
        let origin = rect.center() - egui::vec2(12.0, 12.0) * scale;
        let point = |x: f32, y: f32| origin + egui::vec2(x, y) * scale;
        let stroke = Stroke::new(1.7 * scale, color);
        let line = |points: &[[f32; 2]]| {
            painter.add(Shape::line(
                points.iter().map(|p| point(p[0], p[1])).collect(),
                stroke,
            ));
        };
        let box_at = |x, y, w, h, radius| {
            painter.rect_stroke(
                Rect::from_min_size(point(x, y), egui::vec2(w, h) * scale),
                radius * scale,
                stroke,
                egui::StrokeKind::Inside,
            );
        };
        let circle = |x, y, radius| {
            painter.circle_stroke(point(x, y), radius * scale, stroke);
        };
        match self {
            Self::Graphs => {
                line(&[[3.0, 3.0], [3.0, 21.0], [22.0, 21.0]]);
                box_at(7.0, 12.0, 3.0, 6.0, 0.5);
                box_at(12.0, 5.0, 3.0, 13.0, 0.5);
                box_at(17.0, 8.0, 3.0, 10.0, 0.5);
            }
            Self::Export => {
                line(&[[12.0, 3.0], [12.0, 15.0]]);
                line(&[[7.0, 10.0], [12.0, 15.0], [17.0, 10.0]]);
                line(&[[4.0, 16.0], [4.0, 21.0], [20.0, 21.0], [20.0, 16.0]]);
            }
            Self::Overview => {
                box_at(3.0, 3.0, 7.0, 8.0, 1.0);
                box_at(14.0, 3.0, 7.0, 5.0, 1.0);
                box_at(3.0, 15.0, 7.0, 6.0, 1.0);
                box_at(14.0, 12.0, 7.0, 9.0, 1.0);
            }
            Self::Processes => {
                line(&[
                    [5.0, 15.0],
                    [3.0, 15.0],
                    [3.0, 3.0],
                    [17.0, 3.0],
                    [17.0, 5.0],
                ]);
                box_at(7.0, 7.0, 14.0, 14.0, 2.0);
                line(&[[8.0, 11.0], [20.0, 11.0]]);
            }
            Self::Performance => line(&[
                [2.0, 13.0],
                [7.0, 13.0],
                [10.0, 4.0],
                [14.0, 21.0],
                [17.0, 11.0],
                [22.0, 11.0],
            ]),
            Self::History => {
                circle(12.0, 12.0, 9.0);
                line(&[[12.0, 6.0], [12.0, 12.0], [16.0, 15.0]]);
            }
            Self::Startup => {
                painter.add(Shape::closed_line(
                    vec![point(7.0, 3.0), point(21.0, 12.0), point(7.0, 21.0)],
                    stroke,
                ));
            }
            Self::Users => {
                circle(12.0, 7.0, 4.0);
                line(&[
                    [3.0, 21.0],
                    [4.0, 17.0],
                    [7.0, 14.0],
                    [17.0, 14.0],
                    [20.0, 17.0],
                    [21.0, 21.0],
                    [3.0, 21.0],
                ]);
            }
            Self::Details => {
                for y in [5.0, 12.0, 19.0] {
                    painter.circle_filled(point(4.0, y), scale, color);
                    line(&[[9.0, y], [21.0, y]]);
                }
            }
            Self::Services => {
                let points = (0..32)
                    .map(|i| {
                        let angle = i as f32 * std::f32::consts::TAU / 32.0;
                        let radius = if i % 4 < 2 { 10.0 } else { 8.0 };
                        point(12.0 + angle.cos() * radius, 12.0 + angle.sin() * radius)
                    })
                    .collect();
                painter.add(Shape::closed_line(points, stroke));
                circle(12.0, 12.0, 3.3);
            }
            Self::Sensors => {
                line(&[
                    [9.0, 14.0],
                    [9.0, 5.0],
                    [10.0, 3.0],
                    [14.0, 3.0],
                    [15.0, 5.0],
                    [15.0, 14.0],
                    [17.0, 17.0],
                    [17.0, 19.0],
                    [15.0, 22.0],
                    [9.0, 22.0],
                    [7.0, 19.0],
                    [7.0, 17.0],
                    [9.0, 14.0],
                ]);
                line(&[[12.0, 9.0], [12.0, 17.0]]);
                painter.circle_filled(point(12.0, 18.0), 1.7 * scale, color);
                line(&[[18.0, 6.0], [21.0, 6.0]]);
                line(&[[18.0, 10.0], [20.0, 10.0]]);
            }
            Self::Theme => {
                line(&[
                    [4.0, 12.0],
                    [14.0, 2.0],
                    [21.0, 9.0],
                    [11.0, 19.0],
                    [4.0, 12.0],
                ]);
                line(&[
                    [7.0, 15.0],
                    [3.0, 19.0],
                    [3.0, 22.0],
                    [6.0, 22.0],
                    [10.0, 18.0],
                ]);
                line(&[[12.0, 4.0], [19.0, 11.0]]);
            }
            Self::Info => {
                circle(12.0, 12.0, 9.0);
                painter.circle_filled(point(12.0, 7.0), scale, color);
                line(&[[12.0, 11.0], [12.0, 17.0]]);
            }
            Self::Stop => box_at(5.0, 5.0, 14.0, 14.0, 2.0),
            Self::Restart => {
                line(&[
                    [20.0, 9.0],
                    [18.0, 5.0],
                    [13.0, 3.0],
                    [7.0, 5.0],
                    [3.0, 10.0],
                    [4.0, 17.0],
                    [9.0, 21.0],
                    [16.0, 20.0],
                    [20.0, 16.0],
                ]);
                line(&[[20.0, 3.0], [20.0, 9.0], [14.0, 9.0]]);
            }
            Self::Expand => line(&[[9.0, 5.0], [16.0, 12.0], [9.0, 19.0]]),
            Self::Collapse => line(&[[5.0, 9.0], [12.0, 16.0], [19.0, 9.0]]),
            Self::Minimize => line(&[[5.0, 16.0], [19.0, 16.0]]),
            Self::Maximize => box_at(5.0, 5.0, 14.0, 14.0, 1.0),
            Self::Restore => {
                line(&[[8.0, 4.0], [20.0, 4.0], [20.0, 16.0], [18.0, 16.0]]);
                box_at(4.0, 8.0, 12.0, 12.0, 1.0);
            }
            Self::Close => {
                line(&[[6.0, 6.0], [18.0, 18.0]]);
                line(&[[18.0, 6.0], [6.0, 18.0]]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbols_are_font_free_and_stay_inside_their_allocation() {
        for size in [16.0, 18.0, 20.0, 32.0] {
            let ctx = egui::Context::default();
            let bounds = Rect::from_min_size(egui::pos2(30.0, 30.0), egui::Vec2::splat(size));
            for icon in [
                Icon::Overview,
                Icon::Processes,
                Icon::Performance,
                Icon::History,
                Icon::Startup,
                Icon::Users,
                Icon::Details,
                Icon::Services,
                Icon::Restart,
                Icon::Sensors,
                Icon::Theme,
                Icon::Info,
                Icon::Export,
                Icon::Stop,
                Icon::Expand,
                Icon::Collapse,
                Icon::Minimize,
                Icon::Maximize,
                Icon::Restore,
                Icon::Close,
            ] {
                let output = ctx.run_ui(egui::RawInput::default(), |ui| {
                    icon.paint(ui.painter(), bounds, Color32::WHITE);
                });
                assert!(!output.shapes.is_empty());
                for shape in &output.shapes {
                    assert!(!matches!(shape.shape, Shape::Text(_)));
                    let visual = shape.shape.visual_bounding_rect();
                    if visual.is_positive() {
                        assert!(
                            bounds.contains_rect(visual),
                            "{icon:?}: {visual:?}, {bounds:?}"
                        );
                    }
                }
                assert!(output.platform_output.commands.is_empty());
            }
        }
    }
}
