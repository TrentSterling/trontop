use crate::format;
use crate::model::{SortColumn, SortDirection};
use crate::theme::{self, ThemeSettings, Tokens};
use eframe::egui;
use egui::{Align, Color32, FontId, Layout, RichText, Sense, Stroke, Vec2};
use std::collections::VecDeque;

pub fn tront_mark(ui: &mut egui::Ui, primary: Color32, secondary: Color32, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, size * 0.22, theme::mix(primary, Color32::BLACK, 0.20));
    let inset = rect.shrink(size * 0.17);
    painter.line_segment(
        [inset.left_top(), inset.right_top()],
        Stroke::new((size * 0.11).max(2.0), secondary),
    );
    painter.line_segment(
        [
            egui::pos2(inset.center().x, inset.top()),
            egui::pos2(inset.center().x, inset.bottom()),
        ],
        Stroke::new((size * 0.11).max(2.0), Color32::WHITE),
    );
}

pub fn status_pill(ui: &mut egui::Ui, label: &str, color: Color32) {
    egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(
            color.r(),
            color.g(),
            color.b(),
            38,
        ))
        .corner_radius(20.0)
        .inner_margin(egui::Margin::symmetric(8, 3))
        .show(ui, |ui| {
            ui.label(RichText::new(label).size(10.0).strong().color(color));
        });
}

pub fn nav_button(ui: &mut egui::Ui, selected: bool, icon: &str, label: &str, t: Tokens) -> bool {
    let text = RichText::new(format!("{icon}   {label}"))
        .size(12.0)
        .color(if selected {
            Color32::WHITE
        } else {
            t.text_muted
        });
    ui.add(
        egui::Button::new(text)
            .fill(if selected {
                t.accent_dim
            } else {
                Color32::TRANSPARENT
            })
            .stroke(if selected {
                Stroke::new(1.0, theme::mix(t.accent, t.border, 0.55))
            } else {
                Stroke::NONE
            })
            .min_size(Vec2::new(ui.available_width(), 31.0)),
    )
    .clicked()
}

pub fn mini_meter(ui: &mut egui::Ui, label: &str, value: f32, color: Color32, t: Tokens) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(10.0).color(t.text_muted));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(format::percent(value))
                    .monospace()
                    .size(10.0)
                    .color(t.text),
            );
        });
    });
    ui.add(
        egui::ProgressBar::new((value / 100.0).clamp(0.0, 1.0))
            .fill(color)
            .desired_width(ui.available_width())
            .desired_height(3.0),
    );
    ui.add_space(6.0);
}

pub fn stat_card(
    ui: &mut egui::Ui,
    label: &str,
    value: &str,
    detail: &str,
    color: Color32,
    settings: ThemeSettings,
    t: Tokens,
) {
    egui::Frame::new()
        .fill(theme::raised_color(settings))
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(settings.roundness)
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_min_height(68.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(label).size(10.0).strong().color(t.text_muted));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(6.0), Sense::hover());
                    ui.painter().circle_filled(rect.center(), 3.0, color);
                });
            });
            ui.label(RichText::new(value).size(19.0).monospace().color(t.text));
            ui.add(
                egui::Label::new(RichText::new(detail).size(10.0).color(t.text_muted)).truncate(),
            );
        });
}

pub fn detail_row(ui: &mut egui::Ui, label: &str, value: &str, t: Tokens) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(10.0).color(t.text_muted));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add(
                egui::Label::new(RichText::new(value).size(10.0).monospace().color(t.text))
                    .truncate(),
            );
        });
    });
}

pub fn table_header(
    header: &mut egui_extras::TableRow<'_, '_>,
    label: &str,
    column: SortColumn,
    active: SortColumn,
    direction: SortDirection,
    requested: &mut Option<SortColumn>,
    t: Tokens,
) {
    table_column(header, t, |ui| {
        let arrow = if active == column {
            match direction {
                SortDirection::Ascending => "  ^",
                SortDirection::Descending => "  v",
            }
        } else {
            ""
        };
        if table_label(
            ui,
            RichText::new(format!("{label}{arrow}"))
                .size(10.0)
                .strong()
                .color(if active == column {
                    t.text
                } else {
                    t.text_muted
                }),
        )
        .clicked()
        {
            *requested = Some(column);
        }
    });
}

/// A restrained vertical gradient over the row stripe, with consistent cell insets.
/// Translucency keeps the native row selection and hover background visible.
pub fn table_column(
    row: &mut egui_extras::TableRow<'_, '_>,
    t: Tokens,
    contents: impl FnOnce(&mut egui::Ui),
) {
    let banded = row.col_index() % 2 == 1;
    row.col(|ui| {
        let rect = ui.max_rect();
        if banded {
            let mut mesh = egui::Mesh::default();
            let tint = |color: Color32, alpha| {
                Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
            };
            mesh.colored_vertex(rect.left_top(), tint(t.accent, 16));
            mesh.colored_vertex(rect.right_top(), tint(t.secondary, 10));
            mesh.colored_vertex(rect.right_bottom(), tint(t.secondary, 10));
            mesh.colored_vertex(rect.left_bottom(), tint(t.accent, 16));
            mesh.add_triangle(0, 1, 2);
            mesh.add_triangle(0, 2, 3);
            ui.painter().add(egui::Shape::mesh(mesh));
        }
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(rect.shrink2(Vec2::new(8.0, 3.0)))
                .layout(Layout::left_to_right(Align::Center)),
            contents,
        );
    });
}

pub fn table_cell(ui: &mut egui::Ui, text: RichText) -> bool {
    table_label(ui, text).clicked()
}

fn table_label(ui: &mut egui::Ui, text: RichText) -> egui::Response {
    let label = text.text().to_owned();
    let galley = egui::WidgetText::from(text).into_galley(
        ui,
        Some(egui::TextWrapMode::Truncate),
        ui.available_width(),
        egui::TextStyle::Body,
    );
    let response = ui.allocate_response(ui.available_size(), Sense::click());
    let position = egui::pos2(
        response.rect.left(),
        response.rect.center().y - galley.size().y * 0.5,
    );
    ui.painter()
        .galley(position, galley, ui.visuals().text_color());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Label, ui.is_enabled(), label.as_str())
    });
    response
}

pub fn heat_cell(ui: &mut egui::Ui, value: f32, label: String, color: Color32, t: Tokens) -> bool {
    let response = ui.allocate_response(ui.available_size(), Sense::click());
    if value > 0.05 {
        let alpha = (18.0 + value.clamp(0.0, 100.0) * 0.72) as u8;
        ui.painter().rect_filled(
            response.rect.shrink2(Vec2::new(1.0, 2.0)),
            2.0,
            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha),
        );
    }
    ui.painter().text(
        response.rect.right_center() - Vec2::new(3.0, 0.0),
        egui::Align2::RIGHT_CENTER,
        label,
        FontId::monospace(11.0),
        t.text,
    );
    response.clicked()
}

pub fn performance_heading(
    ui: &mut egui::Ui,
    label: &str,
    detail: &str,
    value: &str,
    color: Color32,
    t: Tokens,
) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new(label).size(24.0).strong().color(t.text));
            ui.add(
                egui::Label::new(RichText::new(detail).size(10.0).color(t.text_muted)).truncate(),
            );
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(value).size(24.0).monospace().color(color));
        });
    });
    ui.add_space(9.0);
}

pub fn history_graph(
    ui: &mut egui::Ui,
    history: &VecDeque<f32>,
    color: Color32,
    height: f32,
    fixed_max: Option<f32>,
    t: Tokens,
) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, t.graph_bg);
    for index in 1..5 {
        let y = egui::lerp(rect.top()..=rect.bottom(), index as f32 / 5.0);
        painter.line_segment(
            [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
            Stroke::new(1.0, t.border),
        );
    }
    for index in 1..8 {
        let x = egui::lerp(rect.left()..=rect.right(), index as f32 / 8.0);
        painter.line_segment(
            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
            Stroke::new(0.5, t.border),
        );
    }
    if history.len() > 1 {
        let observed = history.iter().copied().fold(0.0_f32, f32::max);
        let maximum = fixed_max
            .unwrap_or_else(|| observed.max(1.0) * 1.15)
            .max(0.001);
        let denominator = (history.capacity().max(history.len()).max(2) - 1) as f32;
        let points = history
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let x = rect.right()
                    - ((history.len() - 1 - index) as f32 / denominator) * rect.width();
                let y = rect.bottom() - (value.clamp(0.0, maximum) / maximum) * rect.height();
                egui::pos2(x, y)
            })
            .collect::<Vec<_>>();
        let fill_color = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 30);
        let mut fill = egui::Mesh::default();
        for pair in points.windows(2) {
            let base = fill.vertices.len() as u32;
            fill.colored_vertex(pair[0], fill_color);
            fill.colored_vertex(pair[1], fill_color);
            fill.colored_vertex(egui::pos2(pair[1].x, rect.bottom()), Color32::TRANSPARENT);
            fill.colored_vertex(egui::pos2(pair[0].x, rect.bottom()), Color32::TRANSPARENT);
            fill.add_triangle(base, base + 1, base + 2);
            fill.add_triangle(base, base + 2, base + 3);
        }
        painter.add(egui::Shape::mesh(fill));
        painter.add(egui::Shape::line(points, Stroke::new(2.0, color)));
        painter.text(
            rect.left_top() + Vec2::new(8.0, 7.0),
            egui::Align2::LEFT_TOP,
            format!("MAX {:.1}", maximum),
            FontId::monospace(9.0),
            t.text_muted,
        );
        painter.text(
            rect.left_bottom() + Vec2::new(8.0, -7.0),
            egui::Align2::LEFT_BOTTOM,
            "120 SECONDS",
            FontId::monospace(9.0),
            t.text_muted,
        );
    }
}

pub fn device_button(
    ui: &mut egui::Ui,
    selected: bool,
    label: &str,
    value: &str,
    history: &VecDeque<f32>,
    color: Color32,
    t: Tokens,
) -> bool {
    let response = ui.allocate_response(Vec2::new(ui.available_width(), 52.0), Sense::click());
    let rect = response.rect;
    ui.painter().rect_filled(
        rect,
        4.0,
        if selected {
            t.accent_dim
        } else {
            Color32::TRANSPARENT
        },
    );
    if selected {
        ui.painter().rect_filled(
            egui::Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height())),
            2.0,
            color,
        );
    }
    ui.painter().text(
        rect.left_top() + Vec2::new(10.0, 9.0),
        egui::Align2::LEFT_TOP,
        label,
        FontId::proportional(10.0),
        t.text_muted,
    );
    ui.painter().text(
        rect.left_bottom() + Vec2::new(10.0, -9.0),
        egui::Align2::LEFT_BOTTOM,
        value,
        FontId::monospace(11.0),
        t.text,
    );
    if history.len() > 1 {
        let graph = egui::Rect::from_min_max(
            egui::pos2(rect.right() - 68.0, rect.top() + 8.0),
            egui::pos2(rect.right() - 8.0, rect.bottom() - 8.0),
        );
        let max = history.iter().copied().fold(1.0_f32, f32::max);
        let points = history
            .iter()
            .rev()
            .take(30)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .enumerate()
            .map(|(index, value)| {
                let x = egui::lerp(graph.left()..=graph.right(), index as f32 / 29.0);
                let y = graph.bottom() - (*value / max).clamp(0.0, 1.0) * graph.height();
                egui::pos2(x, y)
            })
            .collect::<Vec<_>>();
        if points.len() > 1 {
            ui.painter()
                .add(egui::Shape::line(points, Stroke::new(1.2, color)));
        }
    }
    response.clicked()
}

pub fn metric(ui: &mut egui::Ui, label: &str, value: &str, t: Tokens) {
    ui.label(RichText::new(label).size(10.0).strong().color(t.text_muted));
    ui.label(RichText::new(value).size(17.0).monospace().color(t.text));
}

pub fn engine_meter(ui: &mut egui::Ui, label: &str, value: f32, color: Color32, t: Tokens) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [72.0, 24.0],
            egui::Label::new(RichText::new(label).size(10.0).color(t.text_muted)).truncate(),
        );
        let (rect, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width().clamp(80.0, 300.0), 24.0),
            Sense::hover(),
        );
        ui.painter().rect_filled(rect, 12.0, t.graph_bg);
        let fill_width = (rect.width() * (value / 100.0).clamp(0.0, 1.0)).max(if value > 0.0 {
            3.0
        } else {
            0.0
        });
        if fill_width > 0.0 {
            ui.painter().rect_filled(
                egui::Rect::from_min_size(rect.min, Vec2::new(fill_width, rect.height())),
                12.0,
                color,
            );
        }
        ui.painter().text(
            rect.right_center() - Vec2::new(9.0, 0.0),
            egui::Align2::RIGHT_CENTER,
            format::percent(value),
            FontId::monospace(10.0),
            t.text,
        );
    });
}

pub fn section_label(ui: &mut egui::Ui, label: &str, t: Tokens) {
    ui.label(RichText::new(label).size(9.0).strong().color(t.text_muted));
    ui.add_space(7.0);
}

pub fn empty_state(ui: &mut egui::Ui, title: &str, detail: &str, t: Tokens) {
    ui.add_space(60.0);
    ui.vertical_centered(|ui| {
        ui.label(RichText::new(title).size(18.0).strong().color(t.text));
        ui.add_space(5.0);
        ui.label(RichText::new(detail).size(11.0).color(t.text_muted));
    });
}

pub fn inventory_table(
    ui: &mut egui::Ui,
    id: &str,
    headers: [&str; 3],
    rows: Vec<[String; 3]>,
    t: Tokens,
) {
    ui.push_id(id, |ui| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        let width = ui.available_width();
        let height = (ui.available_height() - 36.0).max(32.0);
        egui_extras::TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .cell_layout(Layout::left_to_right(Align::Center))
            .column(
                egui_extras::Column::initial(width * 0.38)
                    .at_least(180.0)
                    .clip(true),
            )
            .column(
                egui_extras::Column::initial(width * 0.38)
                    .at_least(180.0)
                    .clip(true),
            )
            .column(egui_extras::Column::remainder().at_least(120.0).clip(true))
            .min_scrolled_height(0.0)
            .max_scroll_height(height)
            .header(34.0, |mut header| {
                for label in headers {
                    table_column(&mut header, t, |ui| {
                        table_cell(
                            ui,
                            RichText::new(label).size(10.0).strong().color(t.text_muted),
                        );
                    });
                }
            })
            .body(|body| {
                body.rows(32.0, rows.len(), |mut row| {
                    for value in &rows[row.index()] {
                        table_column(&mut row, t, |ui| {
                            table_label(ui, RichText::new(value).size(12.0).color(t.text))
                                .on_hover_text(value);
                        });
                    }
                })
            });
    });
}

pub fn push_history(history: &mut VecDeque<f32>, value: f32, limit: usize) {
    if history.len() == limit {
        history.pop_front();
    }
    history.push_back(value);
}

#[cfg(test)]
mod tests {
    use super::*;

    // Real egui layout and input, but no native window, OS input, tray or sampler.
    #[test]
    fn headless_labels_are_left_aligned_centered_vertically_and_clickable_across_cell() {
        for width in [76.0, 240.0, 420.0] {
            let ctx = egui::Context::default();
            let rect = egui::Rect::from_min_size(egui::pos2(20.0, 20.0), Vec2::new(width, 30.0));
            let mut clicked = false;
            for phase in 0..3 {
                let pointer = rect.right_center() - Vec2::new(2.0, 0.0);
                let events = if phase == 0 {
                    vec![]
                } else {
                    vec![
                        egui::Event::PointerMoved(pointer),
                        egui::Event::PointerButton {
                            pos: pointer,
                            button: egui::PointerButton::Primary,
                            pressed: phase == 1,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ]
                };
                let output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            Vec2::new(640.0, 480.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        ui.scope_builder(
                            egui::UiBuilder::new()
                                .max_rect(rect)
                                .layout(Layout::left_to_right(Align::Center)),
                            |ui| {
                                clicked |= table_cell(ui, RichText::new("Unity.exe").size(12.0));
                            },
                        );
                    },
                );
                let text = output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) => Some(text),
                        _ => None,
                    })
                    .expect("label must produce text geometry");
                assert!(
                    (text.pos.x - rect.left()).abs() < 0.1,
                    "text should hug the left inset at width {width}"
                );
                assert!((text.pos.y + text.galley.size().y * 0.5 - rect.center().y).abs() < 0.1);
                assert_eq!(
                    text.galley.rows.len(),
                    1,
                    "labels must never character-wrap"
                );
            }
            assert!(clicked, "empty right-hand cell area must remain clickable");
        }
    }

    #[test]
    fn headless_heat_values_are_right_aligned_and_vertically_centered() {
        let ctx = egui::Context::default();
        let rect = egui::Rect::from_min_size(egui::pos2(20.0, 20.0), Vec2::new(98.0, 30.0));
        let t = theme::tokens(ThemeSettings::default());
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.scope_builder(
                egui::UiBuilder::new()
                    .max_rect(rect)
                    .layout(Layout::left_to_right(Align::Center)),
                |ui| {
                    heat_cell(ui, 25.0, "25.0%".into(), t.accent, t);
                },
            );
        });
        let text = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text),
                _ => None,
            })
            .expect("numeric value must produce text geometry");
        assert!((text.pos.x + text.galley.size().x - (rect.right() - 3.0)).abs() < 0.1);
        assert!((text.pos.y + text.galley.size().y * 0.5 - rect.center().y).abs() < 0.1);
    }
}
