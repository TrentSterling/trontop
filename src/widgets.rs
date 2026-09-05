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
            ui.label(RichText::new(label).size(9.0).strong().color(color));
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
        ui.label(RichText::new(label).size(9.0).color(t.text_muted));
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
                ui.label(RichText::new(label).size(9.0).strong().color(t.text_muted));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(6.0), Sense::hover());
                    ui.painter().circle_filled(rect.center(), 3.0, color);
                });
            });
            ui.label(RichText::new(value).size(19.0).monospace().color(t.text));
            ui.add(
                egui::Label::new(RichText::new(detail).size(9.0).color(t.text_muted)).truncate(),
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
    header.col(|ui| {
        let arrow = if active == column {
            match direction {
                SortDirection::Ascending => "  ^",
                SortDirection::Descending => "  v",
            }
        } else {
            ""
        };
        if ui
            .add(
                egui::Label::new(
                    RichText::new(format!("{label}{arrow}"))
                        .size(9.0)
                        .strong()
                        .color(t.text_muted),
                )
                .sense(Sense::click()),
            )
            .clicked()
        {
            *requested = Some(column);
        }
    });
}

pub fn table_cell(ui: &mut egui::Ui, text: RichText) -> bool {
    ui.add(egui::Label::new(text).sense(Sense::click()).truncate())
        .clicked()
}

pub fn heat_cell(ui: &mut egui::Ui, value: f32, label: String, color: Color32, t: Tokens) -> bool {
    let response = ui.allocate_response(Vec2::new(ui.available_width(), 25.0), Sense::click());
    if value > 0.05 {
        let alpha = (18.0 + value.clamp(0.0, 100.0) * 0.72) as u8;
        ui.painter().rect_filled(
            response.rect.shrink2(Vec2::new(1.0, 2.0)),
            2.0,
            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha),
        );
    }
    ui.painter().text(
        response.rect.left_center() + Vec2::new(3.0, 0.0),
        egui::Align2::LEFT_CENTER,
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
    ui.label(RichText::new(label).size(9.0).strong().color(t.text_muted));
    ui.label(RichText::new(value).size(17.0).monospace().color(t.text));
}

pub fn engine_meter(ui: &mut egui::Ui, label: &str, value: f32, color: Color32, t: Tokens) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [72.0, 24.0],
            egui::Label::new(RichText::new(label).size(10.0).color(t.text_muted)).truncate(),
        );
        let (rect, _) = ui.allocate_exact_size(Vec2::new(300.0, 24.0), Sense::hover());
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

pub fn inline_metric(ui: &mut egui::Ui, label: &str, value: &str, t: Tokens) {
    ui.vertical(|ui| {
        ui.label(RichText::new(label).size(8.0).strong().color(t.text_muted));
        ui.label(RichText::new(value).size(11.0).monospace().color(t.text));
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
    egui::ScrollArea::vertical()
        .id_salt(id)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Grid::new(id)
                .num_columns(3)
                .striped(true)
                .min_col_width(160.0)
                .spacing([18.0, 9.0])
                .show(ui, |ui| {
                    for header in headers {
                        ui.label(RichText::new(header).size(9.0).strong().color(t.text_muted));
                    }
                    ui.end_row();
                    for row in rows {
                        for value in row {
                            ui.add(
                                egui::Label::new(RichText::new(value).size(11.0).color(t.text))
                                    .truncate(),
                            );
                        }
                        ui.end_row();
                    }
                });
        });
}

pub fn push_history(history: &mut VecDeque<f32>, value: f32, limit: usize) {
    if history.len() == limit {
        history.pop_front();
    }
    history.push_back(value);
}
