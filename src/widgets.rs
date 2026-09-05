use crate::format;
use crate::model::{SortColumn, SortDirection};
use crate::theme::{self, ThemeSettings, Tokens};
use eframe::egui;
use egui::{Align, Color32, FontId, Layout, RichText, Sense, Stroke, Vec2};
use std::collections::VecDeque;

/// Passive labels respond visually without becoming selectable or clickable.
pub fn hover_label(ui: &mut egui::Ui, text: impl Into<egui::WidgetText>) -> egui::Response {
    let background = ui.painter().add(egui::Shape::Noop);
    let response = ui.label(text);
    if response.contains_pointer() && ui.is_enabled() {
        ui.painter().set(
            background,
            egui::Shape::rect_filled(
                response.rect.expand2(Vec2::new(2.0, 1.0)),
                ui.visuals().widgets.inactive.corner_radius,
                ui.visuals().widgets.hovered.weak_bg_fill,
            ),
        );
    }
    response
}

/// Paint behind the contents, respect clipping and foreground windows, and never
/// steal input from child controls. No animation timer or persistent hover state.
pub fn hover_frame<R>(
    ui: &mut egui::Ui,
    frame: egui::Frame,
    contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let mut prepared = frame.begin(ui);
    let inner = contents(&mut prepared.content_ui);
    let response = prepared.allocate_space(ui);
    if response.contains_pointer() && ui.is_enabled() {
        prepared.frame.fill = ui.visuals().widgets.hovered.weak_bg_fill;
        prepared.frame.stroke.color = ui.visuals().widgets.hovered.bg_stroke.color;
    }
    prepared.paint(ui);
    egui::InnerResponse { inner, response }
}

pub fn surface(ui: &egui::Ui, t: Tokens, banded: bool) -> egui::Frame {
    egui::Frame::new()
        .fill(if banded {
            ui.visuals().faint_bg_color
        } else {
            t.panel_raised
        })
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(ui.visuals().widgets.inactive.corner_radius)
        .inner_margin(egui::Margin::symmetric(10, 7))
}

pub fn control_row(
    ui: &mut egui::Ui,
    label: &str,
    banded: bool,
    t: Tokens,
    contents: impl FnOnce(&mut egui::Ui),
) {
    hover_frame(ui, surface(ui, t, banded), |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::new(94.0, 24.0), Sense::hover());
            paint_text(
                ui,
                rect,
                label,
                FontId::proportional(12.0),
                t.text,
                Align::Min,
            );
            contents(ui);
        });
    });
}

/// A branded fill must define all interaction states, not override Button::fill.
pub fn action_button(
    ui: &mut egui::Ui,
    text: impl Into<egui::WidgetText>,
    size: Vec2,
    base: Color32,
    t: Tokens,
) -> egui::Response {
    action_button_enabled(ui, text, size, base, t, true)
}

pub fn action_button_enabled(
    ui: &mut egui::Ui,
    text: impl Into<egui::WidgetText>,
    size: Vec2,
    base: Color32,
    t: Tokens,
    enabled: bool,
) -> egui::Response {
    // Scope the visuals, not the layout: nested Ui scopes in a horizontal row
    // centered the action in a taller allocation and shifted its baseline.
    let before = ui.visuals().widgets.clone();
    let states = &mut ui.visuals_mut().widgets;
    states.inactive.weak_bg_fill = base;
    states.hovered.weak_bg_fill = theme::mix(base, t.accent, 0.22);
    states.active.weak_bg_fill = theme::mix(base, t.secondary, 0.28);
    let response = ui.add_enabled(enabled, egui::Button::new(text).min_size(size));
    ui.visuals_mut().widgets = before;
    response
}

pub fn tront_mark(ui: &mut egui::Ui, primary: Color32, secondary: Color32, size: f32) {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(
        rect,
        size * 0.22,
        if response.hovered() {
            theme::mix(primary, secondary, 0.35)
        } else {
            theme::mix(primary, Color32::BLACK, 0.20)
        },
    );
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
    let frame = egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(
            color.r(),
            color.g(),
            color.b(),
            38,
        ))
        .corner_radius(20.0)
        .inner_margin(egui::Margin::symmetric(8, 3));
    hover_frame(ui, frame, |ui| {
        ui.label(RichText::new(label).size(10.0).strong().color(color));
    });
}

pub fn nav_button(ui: &mut egui::Ui, selected: bool, icon: &str, label: &str, t: Tokens) -> bool {
    let response = ui.allocate_response(Vec2::new(ui.available_width(), 32.0), Sense::click());
    paint_interactive_surface(ui, &response, selected, t.panel_raised, t);
    ui.painter().text(
        response.rect.left_center() + Vec2::new(10.0, 0.0),
        egui::Align2::LEFT_CENTER,
        format!("{icon}   {label}"),
        FontId::proportional(12.0),
        t.text,
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), selected, label)
    });
    response.clicked()
}

fn paint_interactive_surface(
    ui: &egui::Ui,
    response: &egui::Response,
    selected: bool,
    base: Color32,
    t: Tokens,
) {
    let fill = if response.is_pointer_button_down_on() {
        theme::mix(t.accent_dim, t.secondary, 0.24)
    } else if response.hovered() || response.has_focus() {
        if selected {
            theme::mix(t.accent_dim, t.secondary, 0.22)
        } else {
            ui.visuals().widgets.hovered.weak_bg_fill
        }
    } else if selected {
        t.accent_dim
    } else {
        base
    };
    ui.painter().rect(
        response.rect,
        ui.visuals().widgets.inactive.corner_radius,
        fill,
        Stroke::new(
            1.0,
            if selected || response.has_focus() {
                t.accent
            } else if response.hovered() {
                t.secondary
            } else {
                t.border
            },
        ),
        egui::StrokeKind::Inside,
    );
}

pub fn mini_meter(ui: &mut egui::Ui, label: &str, value: f32, color: Color32, t: Tokens) {
    hover_frame(ui, surface(ui, t, false), |ui| {
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
    });
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
    let frame = egui::Frame::new()
        .fill(theme::raised_color(settings))
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(settings.roundness)
        .inner_margin(egui::Margin::symmetric(12, 10));
    hover_frame(ui, frame, |ui| {
        ui.set_min_width(ui.available_width());
        ui.set_min_height(68.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(label).size(10.0).strong().color(t.text_muted));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(6.0), Sense::hover());
                ui.painter().circle_filled(rect.center(), 3.0, color);
            });
        });
        ui.add(
            egui::Label::new(RichText::new(value).size(19.0).monospace().color(t.text)).truncate(),
        )
        .on_hover_text(value);
        ui.add(egui::Label::new(RichText::new(detail).size(10.0).color(t.text_muted)).truncate());
    });
}

pub fn detail_row(ui: &mut egui::Ui, label: &str, value: &str, t: Tokens) {
    hover_frame(ui, surface(ui, t, false), |ui| {
        let width = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 18.0), Sense::hover());
        let label_width = (width * 0.44).min(160.0);
        for (text, region, align, color) in [
            (
                label,
                egui::Rect::from_min_max(
                    rect.min,
                    egui::pos2(rect.left() + label_width, rect.bottom()),
                ),
                Align::Min,
                t.text_muted,
            ),
            (
                value,
                egui::Rect::from_min_max(
                    egui::pos2(rect.left() + label_width + 8.0, rect.top()),
                    rect.max,
                ),
                Align::Max,
                t.text,
            ),
        ] {
            let font = if align == Align::Max {
                FontId::monospace(10.0)
            } else {
                FontId::proportional(10.0)
            };
            paint_text(ui, region, text, font, color, align);
        }
    })
    .response
    .on_hover_text(format!("{label}: {value}"));
}

fn paint_text(
    ui: &egui::Ui,
    rect: egui::Rect,
    text: &str,
    font: FontId,
    color: Color32,
    align: Align,
) {
    let mut job = egui::text::LayoutJob::simple_singleline(text.into(), font, color);
    job.wrap.max_width = rect.width().max(0.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
    let x = if align == Align::Max {
        rect.right() - galley.size().x
    } else {
        rect.left()
    };
    ui.painter_at(rect).galley(
        egui::pos2(x, rect.center().y - galley.size().y * 0.5),
        galley,
        color,
    );
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
        let response = ui.interact(rect, ui.next_auto_id().with("cell_hover"), Sense::hover());
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
        if response.contains_pointer() && ui.is_enabled() {
            ui.painter().rect_filled(
                rect.shrink(1.0),
                ui.visuals().widgets.inactive.corner_radius,
                Color32::from_rgba_unmultiplied(
                    t.secondary.r(),
                    t.secondary.g(),
                    t.secondary.b(),
                    38,
                ),
            );
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
    hover_frame(ui, surface(ui, t, false), |ui| {
        ui.horizontal(|ui| {
            let width = ui.available_width();
            let value_width = (width * 0.37).min(200.0);
            ui.allocate_ui_with_layout(
                Vec2::new((width - value_width - 8.0).max(0.0), 47.0),
                Layout::top_down(Align::Min),
                |ui| {
                    ui.add(
                        egui::Label::new(RichText::new(label).size(24.0).strong().color(t.text))
                            .truncate(),
                    );
                    ui.add(
                        egui::Label::new(RichText::new(detail).size(10.0).color(t.text_muted))
                            .truncate(),
                    );
                },
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add(
                    egui::Label::new(RichText::new(value).size(24.0).monospace().color(t.text))
                        .truncate(),
                )
                .on_hover_text(value);
            });
        });
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 2.0), Sense::hover());
        ui.painter().rect_filled(rect, 1.0, color);
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
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(
        rect,
        6.0,
        if response.hovered() {
            theme::mix(t.graph_bg, t.row_hover, 0.65)
        } else {
            t.graph_bg
        },
    );
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
    paint_interactive_surface(ui, &response, selected, t.panel_raised, t);
    if selected {
        ui.painter().rect_filled(
            egui::Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height())),
            2.0,
            color,
        );
    }
    let text_right = rect.right() - 78.0;
    paint_text(
        ui,
        egui::Rect::from_min_max(
            rect.min + Vec2::new(10.0, 6.0),
            egui::pos2(text_right, rect.top() + 24.0),
        ),
        label,
        FontId::proportional(10.0),
        t.text_muted,
        Align::Min,
    );
    paint_text(
        ui,
        egui::Rect::from_min_max(
            rect.min + Vec2::new(10.0, 26.0),
            egui::pos2(text_right, rect.bottom() - 6.0),
        ),
        value,
        FontId::monospace(11.0),
        t.text,
        Align::Min,
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
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), selected, label)
    });
    response
        .on_hover_text(format!("{label}: {value}"))
        .clicked()
}

pub fn metric(ui: &mut egui::Ui, label: &str, value: &str, t: Tokens) {
    hover_frame(ui, surface(ui, t, false), |ui| {
        ui.set_min_width(ui.available_width());
        ui.add(
            egui::Label::new(RichText::new(label).size(10.0).strong().color(t.text_muted))
                .truncate(),
        );
        ui.add(
            egui::Label::new(RichText::new(value).size(17.0).monospace().color(t.text)).truncate(),
        );
    })
    .response
    .on_hover_text(format!("{label}: {value}"));
}

pub fn engine_meter(ui: &mut egui::Ui, label: &str, value: f32, color: Color32, t: Tokens) {
    hover_frame(ui, surface(ui, t, false), |ui| {
        ui.horizontal(|ui| {
            let (label_rect, _) = ui.allocate_exact_size(Vec2::new(96.0, 24.0), Sense::hover());
            paint_text(
                ui,
                label_rect,
                label,
                FontId::proportional(10.0),
                t.text_muted,
                Align::Min,
            );
            let (rect, _) = ui.allocate_exact_size(
                Vec2::new(ui.available_width().max(0.0), 24.0),
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
                    theme::mix(t.graph_bg, color, 0.32),
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
    })
    .response
    .on_hover_text(format!("{label}: {}", format::percent(value)));
}

pub fn section_label(ui: &mut egui::Ui, label: &str, t: Tokens) {
    hover_label(
        ui,
        RichText::new(label).size(9.0).strong().color(t.text_muted),
    );
    ui.add_space(7.0);
}

pub fn empty_state(ui: &mut egui::Ui, title: &str, detail: &str, t: Tokens) {
    ui.add_space(24.0);
    hover_frame(ui, surface(ui, t, false), |ui| {
        ui.vertical_centered(|ui| {
            ui.label(RichText::new(title).size(18.0).strong().color(t.text));
            ui.add_space(5.0);
            ui.label(RichText::new(detail).size(11.0).color(t.text_muted));
        });
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

    fn backgrounds(shape: &egui::Shape, out: &mut Vec<(egui::Rect, Color32)>) {
        match shape {
            egui::Shape::Rect(rect) if rect.fill != Color32::TRANSPARENT => {
                out.push((rect.rect, rect.fill))
            }
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    backgrounds(shape, out);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn shared_surfaces_change_background_on_hover_and_restore_without_layout_shift() {
        for dark in [true, false] {
            for widget in 0..13 {
                let ctx = egui::Context::default();
                let settings = ThemeSettings {
                    dark,
                    ..Default::default()
                };
                theme::install(&ctx, settings);
                let t = theme::tokens(settings);
                let mut normal = Vec::new();
                let mut hover = Vec::new();
                let mut restored = Vec::new();
                let mut bounds = egui::Rect::NOTHING;
                for phase in 0..9 {
                    let inside = (3..6).contains(&phase);
                    let pointer = if inside {
                        bounds.center()
                    } else {
                        egui::pos2(600.0, 440.0)
                    };
                    let output = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                Vec2::new(640.0, 480.0),
                            )),
                            events: vec![egui::Event::PointerMoved(pointer)],
                            ..Default::default()
                        },
                        |ui| {
                            bounds = ui
                                .scope_builder(
                                    egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                                        egui::pos2(20.0, 20.0),
                                        Vec2::new(240.0, 160.0),
                                    )),
                                    |ui| match widget {
                                        0 => {
                                            nav_button(ui, false, "02", "Performance", t);
                                        }
                                        1 => {
                                            device_button(
                                                ui,
                                                false,
                                                "CPU",
                                                "37.2%",
                                                &VecDeque::new(),
                                                t.accent,
                                                t,
                                            );
                                        }
                                        2 => metric(ui, "UPTIME", "5d 03h 28m", t),
                                        3 => detail_row(ui, "Logical processors", "32", t),
                                        4 => stat_card(
                                            ui,
                                            "CPU",
                                            "37.2%",
                                            "Fixture processor",
                                            t.accent,
                                            settings,
                                            t,
                                        ),
                                        5 => status_pill(ui, "LIVE", t.good),
                                        6 => mini_meter(ui, "CPU", 37.2, t.accent, t),
                                        7 => engine_meter(ui, "3D", 37.2, t.accent, t),
                                        8 => {
                                            action_button(
                                                ui,
                                                "Theme Studio",
                                                Vec2::new(240.0, 32.0),
                                                t.accent_dim,
                                                t,
                                            );
                                        }
                                        9 => {
                                            nav_button(ui, true, "02", "Performance", t);
                                        }
                                        10 => {
                                            device_button(
                                                ui,
                                                true,
                                                "CPU",
                                                "37.2%",
                                                &VecDeque::new(),
                                                t.accent,
                                                t,
                                            );
                                        }
                                        11 => {
                                            control_row(ui, "Gradient", true, t, |ui| {
                                                ui.checkbox(&mut true, "Enabled");
                                            });
                                        }
                                        12 => {
                                            history_graph(
                                                ui,
                                                &VecDeque::new(),
                                                t.accent,
                                                96.0,
                                                Some(100.0),
                                                t,
                                            );
                                        }
                                        _ => unreachable!(),
                                    },
                                )
                                .response
                                .rect;
                        },
                    );
                    let target = match phase {
                        2 => &mut normal,
                        5 => &mut hover,
                        8 => &mut restored,
                        _ => continue,
                    };
                    for shape in &output.shapes {
                        backgrounds(&shape.shape, target);
                    }
                }
                assert!(!normal.is_empty());
                assert_ne!(
                    normal, hover,
                    "widget {widget} dark={dark} has no hover background"
                );
                assert_eq!(normal, restored, "widget {widget} hover did not reset");
                assert_eq!(
                    normal.iter().map(|(rect, _)| rect).collect::<Vec<_>>(),
                    hover.iter().map(|(rect, _)| rect).collect::<Vec<_>>(),
                    "hover shifted widget {widget}"
                );
            }
        }
    }

    // Real egui layout and input, but no native window, OS input, tray or sampler.
    #[test]
    fn action_buttons_align_with_plain_buttons_and_restore_visuals() {
        let ctx = egui::Context::default();
        let t = theme::tokens(ThemeSettings::default());
        for enabled in [true, false] {
            let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
                ui.horizontal(|ui| {
                    let before = ui.visuals().widgets.inactive.weak_bg_fill;
                    let cancel = ui.button("Cancel");
                    let action =
                        action_button_enabled(ui, "End process", Vec2::ZERO, t.danger, t, enabled);
                    assert!((cancel.rect.center().y - action.rect.center().y).abs() < 0.5);
                    assert!((cancel.rect.height() - action.rect.height()).abs() < 0.5);
                    assert_eq!(action.enabled(), enabled);
                    assert_eq!(ui.visuals().widgets.inactive.weak_bg_fill, before);
                });
            });
        }
    }

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
