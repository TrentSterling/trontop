use crate::format;
use crate::icons::Icon;
use crate::model::{ProcessRow, SortColumn, SortDirection};
use crate::theme::{self, ThemeSettings, Tokens};
use eframe::egui;
use egui::{Align, Color32, FontId, Layout, RichText, Sense, Stroke, Vec2};
use std::collections::VecDeque;

#[cfg(test)]
mod contrast_tests;
#[cfg(test)]
mod table_interaction_tests;

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
    let stroke_width = states.inactive.bg_stroke.width;
    for state in [
        &mut states.inactive,
        &mut states.hovered,
        &mut states.active,
    ] {
        state.fg_stroke.color = theme::readable_text(t.text, state.weak_bg_fill);
        state.bg_stroke.color = theme::readable_text(state.bg_stroke.color, state.weak_bg_fill);
        state.bg_stroke.width = stroke_width;
    }
    // A caller's fixed RichText color must not defeat per-state button contrast.
    // PLACEHOLDER is resolved by egui's button painter using that state's ink.
    let text = text.into().color(Color32::PLACEHOLDER);
    let response = ui.add_enabled(enabled, egui::Button::new(text).min_size(size));
    ui.visuals_mut().widgets = before;
    response
}

pub fn tront_mark(ui: &mut egui::Ui, primary: Color32, secondary: Color32, size: f32) {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let painter = ui.painter_at(rect);
    let fill = if response.hovered() {
        theme::mix(primary, secondary, 0.35)
    } else {
        theme::mix(primary, Color32::BLACK, 0.20)
    };
    painter.rect_filled(rect, size * 0.22, fill);
    let inset = rect.shrink(size * 0.17);
    painter.line_segment(
        [inset.left_top(), inset.right_top()],
        Stroke::new(
            (size * 0.11).max(2.0),
            theme::readable_text(secondary, fill),
        ),
    );
    painter.line_segment(
        [
            egui::pos2(inset.center().x, inset.top()),
            egui::pos2(inset.center().x, inset.bottom()),
        ],
        Stroke::new(
            (size * 0.11).max(2.0),
            theme::readable_text(Color32::WHITE, fill),
        ),
    );
}

pub fn status_pill(ui: &mut egui::Ui, label: &str, color: Color32) {
    let dark = ui.visuals().dark_mode;
    let frame = egui::Frame::new()
        .fill(theme::text_surface(
            theme::mix(ui.visuals().window_fill, color, 0.15),
            dark,
        ))
        .corner_radius(20.0)
        .inner_margin(egui::Margin::symmetric(8, 3));
    hover_frame(ui, frame, |ui| {
        ui.label(
            RichText::new(label)
                .size(10.0)
                .strong()
                .color(theme::ink(color, dark)),
        );
    });
}

pub fn nav_button(ui: &mut egui::Ui, selected: bool, icon: Icon, label: &str, t: Tokens) -> bool {
    let height = if ui.ctx().content_rect().height() < 700.0 {
        28.0
    } else {
        32.0
    };
    let response = ui.allocate_response(Vec2::new(ui.available_width(), height), Sense::click());
    paint_interactive_surface(ui, &response, selected, t.panel_raised, t);
    icon.paint(
        ui.painter(),
        egui::Rect::from_center_size(
            response.rect.left_center() + Vec2::new(19.0, 0.0),
            Vec2::splat(18.0),
        ),
        t.text,
    );
    if selected {
        let bar = egui::Rect::from_min_size(
            response.rect.left_top() + Vec2::new(2.0, 6.0),
            Vec2::new(2.0, height - 12.0),
        );
        ui.painter().rect_filled(bar, 1.0, t.accent);
    }
    ui.painter().text(
        response.rect.left_center() + Vec2::new(36.0, 0.0),
        egui::Align2::LEFT_CENTER,
        label,
        FontId::proportional(12.0),
        t.text,
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), selected, label)
    });
    response.clicked()
}

/// An icon-only or icon-and-label button with the same stable hover geometry.
/// Text is also the accessible name. Callers label icon-only controls explicitly.
pub fn icon_button(
    ui: &mut egui::Ui,
    icon: Icon,
    label: &str,
    size: Vec2,
    base: Color32,
    t: Tokens,
) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(
        label.into(),
        FontId::proportional(12.0),
        Color32::PLACEHOLDER,
    );
    let desired = if label.is_empty() {
        Vec2::splat(20.0).max(size)
    } else {
        Vec2::new(galley.size().x + 42.0, 28.0).max(size)
    };
    let response = ui.allocate_response(desired, Sense::click());
    let fill = paint_interactive_surface(ui, &response, false, base, t);
    let color = theme::readable_text(
        if ui.is_enabled() {
            t.text
        } else {
            t.text_muted
        },
        fill,
    );
    let center = if label.is_empty() {
        response.rect.center()
    } else {
        response.rect.left_center() + Vec2::new(18.0, 0.0)
    };
    icon.paint(
        ui.painter(),
        egui::Rect::from_center_size(center, Vec2::splat(17.0)),
        color,
    );
    if !label.is_empty() {
        ui.painter().galley(
            response.rect.left_center() + Vec2::new(33.0, -galley.size().y / 2.0),
            galley,
            color,
        );
    }
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    response
}

fn paint_interactive_surface(
    ui: &egui::Ui,
    response: &egui::Response,
    selected: bool,
    base: Color32,
    t: Tokens,
) -> Color32 {
    let fill = if response.is_pointer_button_down_on() {
        t.surface(theme::mix(t.accent_dim, t.secondary, 0.24))
    } else if response.hovered() || response.has_focus() {
        if selected {
            t.surface(theme::mix(t.accent_dim, t.secondary, 0.22))
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
                theme::readable_text(t.accent, fill)
            } else if response.hovered() {
                theme::readable_text(t.secondary, fill)
            } else {
                t.border
            },
        ),
        egui::StrokeKind::Inside,
    );
    fill
}

/// Frameless 20 px sidebar meter: label left, a 40x10 two-minute sparkline and
/// the caller-formatted value right (e.g. a GPU lower bound shown as `3.1%+`),
/// and a 3 px bar underneath. A missing value reads as a muted `--`, never 0.
/// `history` uses NaN for missing samples, which the sparkline leaves as gaps.
pub fn mini_meter_text(
    ui: &mut egui::Ui,
    label: &str,
    text: &str,
    value: Option<f32>,
    history: &VecDeque<f32>,
    color: Color32,
    t: Tokens,
) -> egui::Response {
    const VALUE_WIDTH: f32 = 46.0;
    const SPARK: Vec2 = Vec2::new(40.0, 10.0);
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 20.0), Sense::hover());
    let painter = ui.painter_at(rect.expand(1.0));
    let text_y = rect.top() + 7.0;
    painter.text(
        egui::pos2(rect.left(), text_y),
        egui::Align2::LEFT_CENTER,
        label,
        FontId::proportional(10.0),
        t.text_muted,
    );
    painter.text(
        egui::pos2(rect.right(), text_y),
        egui::Align2::RIGHT_CENTER,
        text,
        FontId::monospace(10.0),
        if value.is_some() {
            t.text
        } else {
            t.text_muted
        },
    );
    let spark = egui::Rect::from_min_size(
        egui::pos2(
            rect.right() - VALUE_WIDTH - 4.0 - SPARK.x,
            text_y - SPARK.y / 2.0,
        ),
        SPARK,
    );
    sparkline(
        &painter,
        spark,
        history.iter().map(|value| Some(*value)),
        100.0,
        color,
        t,
    );
    let track = egui::Rect::from_min_max(
        egui::pos2(rect.left(), rect.bottom() - 3.0),
        rect.right_bottom(),
    );
    // Hover lifts the track: the meter is frameless, but still answers the
    // pointer before its tooltip appears.
    let track_fill = if response.hovered() {
        theme::mix(t.panel_raised, t.text_muted, 0.45)
    } else {
        theme::mix(t.panel_raised, t.border, 0.5)
    };
    painter.rect_filled(track, 1.5, track_fill);
    if let Some(value) = value {
        let fraction = (value / 100.0).clamp(0.0, 1.0);
        if fraction > 0.0 {
            let mut fill = track;
            fill.set_width((track.width() * fraction).max(2.0));
            painter.rect_filled(fill, 1.5, t.ink(color));
        }
    }
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::ProgressIndicator,
            true,
            format!("{label} {text}"),
        )
    });
    response
}

/// A labeled value card with a colored status dot and a muted detail line. The
/// polish gauntlet moved every page off this in favor of compact cards and KPI
/// tiles (see `theme::space`, `kpi_tile`); kept for the hover/background fuzz
/// coverage below and any future page that wants this exact shape again.
#[allow(dead_code)]
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
        ui.add(egui::Label::new(RichText::new(detail).size(10.0).color(t.text_muted)).truncate())
            .on_hover_text(detail);
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

/// Keep the full target on hover without allowing an OS-supplied name to move
/// confirmation controls. The separate identity line never shares name width.
pub fn identity_card(ui: &mut egui::Ui, name: &str, identity: &str, t: Tokens) {
    hover_frame(ui, surface(ui, t, false), |ui| {
        ui.set_min_width(ui.available_width());
        ui.add(egui::Label::new(RichText::new(name).size(17.0).strong().color(t.text)).truncate())
            .on_hover_text(name);
        ui.add(
            egui::Label::new(
                RichText::new(identity)
                    .monospace()
                    .size(11.0)
                    .color(t.text_muted),
            )
            .truncate(),
        )
        .on_hover_text(identity);
    });
}

/// Shared column geometry for the History page: [`history_header`] and
/// [`history_row`] both lay their fields out from the same left edges, so
/// header and data always line up.
struct HistoryColumns {
    name_left: f32,
    pid_left: f32,
    cpu_left: f32,
    io_left: f32,
}

fn history_columns(rect: egui::Rect) -> HistoryColumns {
    let io_left = rect.right() - 110.0;
    let cpu_left = io_left - 10.0 - 134.0;
    let pid_left = cpu_left - 10.0 - 94.0;
    let name_left = rect.left() + 42.0;
    HistoryColumns {
        name_left,
        pid_left,
        cpu_left,
        io_left,
    }
}

/// A static column header for the History table: RANK / PROCESS / PID / CPU
/// TIME / TOTAL I/O, styled like [`table_header`] but over plain painted
/// columns rather than an `egui_extras` table (History has no sortable
/// columns of its own).
pub fn history_header(ui: &mut egui::Ui, t: Tokens) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 22.0), Sense::hover());
    let cols = history_columns(rect);
    let font = FontId::proportional(10.0);
    for (text, left, right, align) in [
        ("RANK", rect.left(), cols.name_left - 8.0, Align::Min),
        ("PROCESS", cols.name_left, cols.pid_left - 10.0, Align::Min),
        ("PID", cols.pid_left, cols.cpu_left - 10.0, Align::Max),
        ("CPU TIME", cols.cpu_left, cols.io_left - 10.0, Align::Max),
        ("TOTAL I/O", cols.io_left, rect.right(), Align::Max),
    ] {
        paint_text(
            ui,
            egui::Rect::from_min_max(
                egui::pos2(left, rect.top()),
                egui::pos2(right, rect.bottom()),
            ),
            text,
            font.clone(),
            t.text_muted,
            align,
        );
    }
    ui.add_space(theme::space::XS);
}

/// Fixed numeric tracks keep names from consuming the PID and lifetime totals.
/// Only the name track flexes; every field uses one vertically centered line.
pub fn history_row(ui: &mut egui::Ui, rank: usize, process: &ProcessRow, t: Tokens) {
    let ordinal = format!("{:02}", rank + 1);
    let pid = format!("PID {}", process.pid);
    let cpu = format::millis(process.accumulated_cpu_millis);
    let io = format!(
        "{} I/O",
        format::bytes(
            process
                .total_read_bytes
                .saturating_add(process.total_write_bytes)
        )
    );
    hover_frame(
        ui,
        surface(ui, t, rank % 2 == 1).inner_margin(egui::Margin::symmetric(10, 5)),
        |ui| {
            let (rect, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 20.0), Sense::hover());
            let cols = history_columns(rect);
            for (text, left, right, font, color, align) in [
                (
                    ordinal.as_str(),
                    rect.left(),
                    cols.name_left - 8.0,
                    FontId::monospace(11.0),
                    t.ink(t.accent),
                    Align::Min,
                ),
                (
                    process.name.as_str(),
                    cols.name_left,
                    cols.pid_left - 10.0,
                    FontId::proportional(13.0),
                    t.text,
                    Align::Min,
                ),
                (
                    pid.as_str(),
                    cols.pid_left,
                    cols.cpu_left - 10.0,
                    FontId::monospace(10.0),
                    t.text_muted,
                    Align::Max,
                ),
                (
                    cpu.as_str(),
                    cols.cpu_left,
                    cols.io_left - 10.0,
                    FontId::monospace(11.0),
                    t.ink(t.secondary),
                    Align::Max,
                ),
                (
                    io.as_str(),
                    cols.io_left,
                    rect.right(),
                    FontId::monospace(11.0),
                    t.text_muted,
                    Align::Max,
                ),
            ] {
                paint_text(
                    ui,
                    egui::Rect::from_min_max(
                        egui::pos2(left, rect.top()),
                        egui::pos2(right, rect.bottom()),
                    ),
                    text,
                    font,
                    color,
                    align,
                );
            }
        },
    )
    .response
    .on_hover_text(format!(
        "{}\n{pid}\nLifetime CPU: {cpu}\nTotal: {io}",
        process.name
    ));
}

/// One clipped, non-wrapping line aligned inside `rect`.
pub fn paint_text(
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

/// Elides the middle of a long file name so a short extension (".exe", ".dll",
/// ...) survives instead of egui's plain tail ellipsis dropping it. Returns
/// `name` untouched when it already fits or carries no short extension to
/// protect; the caller's own truncating label still clips it as a fallback.
pub fn truncate_keep_extension(ui: &egui::Ui, name: &str, max_width: f32) -> String {
    let font = egui::TextStyle::Body.resolve(ui.style());
    let measure = |text: &str| {
        ui.painter()
            .layout_no_wrap(text.to_owned(), font.clone(), Color32::WHITE)
            .size()
            .x
    };
    if max_width <= 0.0 || measure(name) <= max_width {
        return name.to_string();
    }
    let Some(dot) = name.rfind('.').filter(|&i| i > 0 && name.len() - i <= 5) else {
        return name.to_string();
    };
    let (stem, extension) = name.split_at(dot);
    const ELLIPSIS: char = '\u{2026}';
    let mut end = stem.len();
    loop {
        let candidate = format!("{}{ELLIPSIS}{extension}", &stem[..end]);
        if end == 0 || measure(&candidate) <= max_width {
            return candidate;
        }
        end -= 1;
        while end > 0 && !stem.is_char_boundary(end) {
            end -= 1;
        }
    }
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
        if banded && t.column_strength > 0.0 {
            let mut mesh = egui::Mesh::default();
            let tint = |color: Color32, alpha| {
                Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
            };
            let alpha = (t.column_strength * 255.0) as u8;
            mesh.colored_vertex(rect.left_top(), tint(t.surface(t.accent), alpha));
            mesh.colored_vertex(rect.right_top(), tint(t.surface(t.secondary), alpha));
            mesh.colored_vertex(rect.right_bottom(), tint(t.surface(t.secondary), alpha));
            mesh.colored_vertex(rect.left_bottom(), tint(t.surface(t.accent), alpha));
            mesh.add_triangle(0, 1, 2);
            mesh.add_triangle(0, 2, 3);
            ui.painter().add(egui::Shape::mesh(mesh));
        }
        if response.contains_pointer() && ui.is_enabled() {
            let color = t.surface(t.secondary);
            ui.painter().rect_filled(
                rect.shrink(1.0),
                ui.visuals().widgets.inactive.corner_radius,
                Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 38),
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

pub(crate) fn table_label(ui: &mut egui::Ui, text: RichText) -> egui::Response {
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
    paint_table_focus(ui, &response);
    response
}

/// Table cells paint their own content, so egui cannot draw their focus state.
/// Use the existing outer-cell padding without changing text or allocation.
fn paint_table_focus(ui: &egui::Ui, response: &egui::Response) {
    if response.enabled() && response.has_focus() {
        let rect = response
            .rect
            .expand2(Vec2::new(4.0, 0.0))
            .intersect(ui.clip_rect());
        ui.painter_at(rect).rect_stroke(
            rect.shrink(1.0),
            ui.visuals().widgets.inactive.corner_radius,
            Stroke::new(1.5, ui.visuals().widgets.active.bg_stroke.color),
            egui::StrokeKind::Inside,
        );
    }
}

pub fn heat_cell(ui: &mut egui::Ui, value: f32, label: String, color: Color32, t: Tokens) -> bool {
    heat_cell_response(ui, value, label, color, t.text, t).clicked()
}

/// Unreported GPU rows read "--" with no "%" (there is no measurement to round),
/// and an exact-zero measured row reads a calm "0%" instead of a busy "0.00%".
/// Both are muted; every other state keeps [`crate::gpu_activity::Usage::label`]
/// unchanged, which already gives Partial its "x.xx%+" lower-bound marker.
pub fn gpu_cell(
    ui: &mut egui::Ui,
    usage: crate::gpu_activity::Usage,
    grouped: bool,
    t: Tokens,
) -> bool {
    use crate::gpu_activity::Usage;
    let (label, muted, mut explanation) = match usage {
        Usage::Unreported => (
            "--".to_string(),
            true,
            "This process has no GPU engine instance, so Windows reports nothing for it."
                .to_string(),
        ),
        Usage::Measured(value) if value <= 0.0 => {
            ("0%".to_string(), true, usage.explanation().to_string())
        }
        _ => (usage.label(), false, usage.explanation().to_string()),
    };
    if grouped {
        explanation.push_str(" Grouped rows sum process peaks; this is not whole-GPU utilization.");
    }
    heat_cell_response(
        ui,
        usage.value().unwrap_or_default(),
        label,
        t.secondary,
        if muted { t.text_muted } else { t.text },
        t,
    )
    .on_hover_text(explanation)
    .clicked()
}

fn heat_cell_response(
    ui: &mut egui::Ui,
    value: f32,
    label: String,
    color: Color32,
    text_color: Color32,
    t: Tokens,
) -> egui::Response {
    let response = ui.allocate_response(ui.available_size(), Sense::click());
    if value > 0.05 {
        let amount = (18.0 + value.clamp(0.0, 100.0) * 0.72) / 255.0;
        ui.painter().rect_filled(
            response.rect.shrink2(Vec2::new(1.0, 2.0)),
            2.0,
            t.surface(theme::mix(t.panel_raised, color, amount)),
        );
    }
    ui.painter().text(
        response.rect.right_center() - Vec2::new(3.0, 0.0),
        egui::Align2::RIGHT_CENTER,
        label,
        FontId::monospace(11.0),
        text_color,
    );
    paint_table_focus(ui, &response);
    response
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
            // Measure the value's real width first, so a short reading (most
            // Performance tiles) leaves the device name room to fit before it
            // truncates, instead of always reserving a fixed 37%/200px chunk.
            let value_galley =
                ui.painter()
                    .layout_no_wrap(value.into(), FontId::monospace(24.0), t.text);
            let value_width = (value_galley.size().x + 4.0).min(width * 0.6);
            ui.allocate_ui_with_layout(
                Vec2::new((width - value_width - 8.0).max(0.0), 47.0),
                Layout::top_down(Align::Min),
                |ui| {
                    ui.add(
                        egui::Label::new(RichText::new(label).size(18.0).strong().color(t.text))
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
    history_graph_with_window(
        ui,
        history,
        color,
        height,
        fixed_max,
        t,
        ("120 SECONDS", history.capacity().max(history.len())),
        None,
    );
}

/// Explicit window units for series sampled independently from the UI refresh.
/// `unit_fmt`, when given, formats the top-left maximum label (e.g. a rate with
/// its unit); `None` keeps the plain `MAX x` fallback.
#[allow(clippy::too_many_arguments)]
pub fn history_graph_with_window(
    ui: &mut egui::Ui,
    history: &VecDeque<f32>,
    color: Color32,
    height: f32,
    fixed_max: Option<f32>,
    t: Tokens,
    window: (&str, usize),
    unit_fmt: Option<&dyn Fn(f32) -> String>,
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
        let denominator = (window.1.max(history.len()).max(2) - 1) as f32;
        let points = history
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let x = rect.right()
                    - ((history.len() - 1 - index) as f32 / denominator) * rect.width();
                let y = rect.bottom() - (value.clamp(0.0, maximum) / maximum) * rect.height();
                value.is_finite().then_some(egui::pos2(x, y))
            })
            .collect::<Vec<_>>();
        let fill = t.surface(color);
        let fill_color = Color32::from_rgba_unmultiplied(fill.r(), fill.g(), fill.b(), 30);
        let mut fill = egui::Mesh::default();
        for pair in points.windows(2) {
            let [Some(first), Some(second)] = pair else {
                continue;
            };
            let base = fill.vertices.len() as u32;
            fill.colored_vertex(*first, fill_color);
            fill.colored_vertex(*second, fill_color);
            fill.colored_vertex(egui::pos2(second.x, rect.bottom()), Color32::TRANSPARENT);
            fill.colored_vertex(egui::pos2(first.x, rect.bottom()), Color32::TRANSPARENT);
            fill.add_triangle(base, base + 1, base + 2);
            fill.add_triangle(base, base + 2, base + 3);
        }
        painter.add(egui::Shape::mesh(fill));
        for run in points.split(Option::is_none) {
            let run: Vec<_> = run.iter().flatten().copied().collect();
            if run.len() > 1 {
                painter.add(egui::Shape::line(run, Stroke::new(2.0, t.ink(color))));
            }
        }
        painter.text(
            rect.left_top() + Vec2::new(8.0, 7.0),
            egui::Align2::LEFT_TOP,
            match unit_fmt {
                Some(format) => format(maximum),
                None => format!("MAX {maximum:.1}"),
            },
            FontId::monospace(9.0),
            t.text_muted,
        );
        painter.text(
            rect.left_bottom() + Vec2::new(8.0, -7.0),
            egui::Align2::LEFT_BOTTOM,
            "-120 s",
            FontId::monospace(9.0),
            t.text_muted,
        );
        painter.text(
            rect.right_bottom() - Vec2::new(8.0, 7.0),
            egui::Align2::RIGHT_BOTTOM,
            "now",
            FontId::monospace(9.0),
            t.text_muted,
        );
    }
}

/// A small background trend line: `values` yields one point per sample, in order.
/// Fewer than two points draws nothing, never a flat single-point line. A `None`
/// sample breaks the line instead of bridging across the gap.
pub fn sparkline(
    painter: &egui::Painter,
    rect: egui::Rect,
    values: impl Iterator<Item = Option<f32>>,
    max: f32,
    color: Color32,
    t: Tokens,
) {
    let values: Vec<Option<f32>> = values.collect();
    if values.len() < 2 {
        return;
    }
    let max = max.max(0.001);
    let denominator = (values.len() - 1).max(1) as f32;
    let points: Vec<Option<egui::Pos2>> = values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            value.filter(|v| v.is_finite()).map(|v| {
                let x = egui::lerp(rect.left()..=rect.right(), index as f32 / denominator);
                let y = rect.bottom() - (v.clamp(0.0, max) / max) * rect.height();
                egui::pos2(x, y)
            })
        })
        .collect();

    let ink = t.ink(color);
    let fill = t.surface(color);
    let alpha = (255.0_f32 * 0.18).round() as u8;
    let fill_color = Color32::from_rgba_unmultiplied(fill.r(), fill.g(), fill.b(), alpha);

    let mut mesh = egui::Mesh::default();
    for pair in points.windows(2) {
        let [Some(first), Some(second)] = pair else {
            continue;
        };
        let base = mesh.vertices.len() as u32;
        mesh.colored_vertex(*first, fill_color);
        mesh.colored_vertex(*second, fill_color);
        mesh.colored_vertex(egui::pos2(second.x, rect.bottom()), Color32::TRANSPARENT);
        mesh.colored_vertex(egui::pos2(first.x, rect.bottom()), Color32::TRANSPARENT);
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base, base + 2, base + 3);
    }
    if !mesh.indices.is_empty() {
        painter.add(egui::Shape::mesh(mesh));
    }

    for run in points.split(Option::is_none) {
        let run: Vec<_> = run.iter().flatten().copied().collect();
        if run.len() > 1 {
            painter.add(egui::Shape::line(run, Stroke::new(1.2, ink)));
        }
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
        let windowed: Vec<f32> = history
            .iter()
            .rev()
            .take(30)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .copied()
            .collect();
        sparkline(
            ui.painter(),
            graph,
            windowed
                .iter()
                .map(|value| value.is_finite().then_some(*value)),
            max,
            color,
            t,
        );
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), selected, label)
    });
    response
        .on_hover_text(format!("{label}: {value}"))
        .clicked()
}

/// Content for one [`kpi_tile`]. A later polish-gauntlet package wires this and
/// `kpi_tile` into the Overview page's KPI row.
#[allow(dead_code)]
pub struct Kpi<'a> {
    pub label: &'a str,
    pub value: &'a str,
    pub sub: &'a str,
    pub hover: &'a str,
    pub series: &'a [Option<f32>],
    pub max: Option<f32>,
    pub color: Color32,
    /// `Some` only when the tile is not Live (e.g. "Cached", "Partial", "Stale").
    pub state: Option<&'a str>,
}

/// Fixed height of a [`kpi_tile`], regardless of width.
#[allow(dead_code)]
pub const KPI_TILE_HEIGHT: f32 = 84.0;

/// A raised, fixed-height at-a-glance tile: a label row, a big value, a small
/// caption, and a full-width sparkline band along the bottom.
/// A later polish-gauntlet package wires this into the Overview page.
#[allow(dead_code)]
pub fn kpi_tile(
    ui: &mut egui::Ui,
    kpi: &Kpi<'_>,
    settings: ThemeSettings,
    t: Tokens,
) -> egui::Response {
    let response = ui.allocate_response(
        Vec2::new(ui.available_width(), KPI_TILE_HEIGHT),
        Sense::hover(),
    );
    let rect = response.rect;
    let hovered = response.contains_pointer() && ui.is_enabled();
    let fill = if hovered {
        t.surface(theme::mix(theme::raised_color(settings), t.row_hover, 0.5))
    } else {
        theme::raised_color(settings)
    };
    ui.painter().rect(
        rect,
        settings.roundness,
        fill,
        Stroke::new(1.0, t.border),
        egui::StrokeKind::Inside,
    );

    let pad = theme::CARD_PAD;
    let content = egui::Rect::from_min_max(
        rect.min + Vec2::new(f32::from(pad.left), f32::from(pad.top)),
        rect.max - Vec2::new(f32::from(pad.right), f32::from(pad.bottom)),
    );

    // Sparkline: a full-width band under the caption, so it never runs
    // through the value or the caption at narrow tile widths.
    let spark_rect = egui::Rect::from_min_max(
        egui::pos2(content.left(), rect.top() + 60.0),
        egui::pos2(content.right(), content.bottom()),
    );
    if spark_rect.width() > 2.0 && spark_rect.height() > 2.0 {
        let max = kpi.max.unwrap_or_else(|| {
            format::nice_top(kpi.series.iter().flatten().copied().fold(0.0_f32, f32::max))
        });
        sparkline(
            ui.painter(),
            spark_rect,
            kpi.series.iter().copied(),
            max,
            kpi.color,
            t,
        );
    }

    // Row 1: label, plus a color dot or a state chip on the right.
    let row1 = egui::Rect::from_min_size(content.min, Vec2::new(content.width(), 12.0));
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(row1)
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            ui.add(
                egui::Label::new(
                    RichText::new(kpi.label.to_uppercase())
                        .size(10.0)
                        .strong()
                        .color(t.text_muted),
                )
                .truncate(),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if let Some(state) = kpi.state {
                    status_pill(ui, state, t.text_muted);
                } else {
                    let (dot, _) = ui.allocate_exact_size(Vec2::splat(6.0), Sense::hover());
                    ui.painter().circle_filled(dot.center(), 3.0, kpi.color);
                }
            });
        },
    );

    // Row 2: the big value.
    let row2 = egui::Rect::from_min_size(
        egui::pos2(content.left(), rect.top() + 20.0),
        Vec2::new(content.width(), 24.0),
    );
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(row2)
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            ui.add(
                egui::Label::new(
                    RichText::new(kpi.value)
                        .size(22.0)
                        .monospace()
                        .color(t.text),
                )
                .truncate(),
            );
        },
    );

    // Row 3: the caption.
    let row3 = egui::Rect::from_min_max(
        egui::pos2(content.left(), rect.top() + 44.0),
        egui::pos2(content.right(), rect.top() + 58.0),
    );
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(row3)
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            ui.add(
                egui::Label::new(RichText::new(kpi.sub).size(10.0).color(t.text_muted)).truncate(),
            );
        },
    );

    response.on_hover_text(kpi.hover)
}

pub fn metric(ui: &mut egui::Ui, label: &str, value: &str, t: Tokens) {
    metric_banded(ui, label, value, false, t);
}

pub fn metric_banded(ui: &mut egui::Ui, label: &str, value: &str, banded: bool, t: Tokens) {
    hover_frame(ui, surface(ui, t, banded), |ui| {
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

pub fn engine_meter(ui: &mut egui::Ui, label: &str, value: Option<f32>, color: Color32, t: Tokens) {
    let caption = value.map_or_else(|| "-- %".into(), format::percent);
    let value = value.unwrap_or(0.0);
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
                    t.surface(theme::mix(t.graph_bg, color, 0.32)),
                );
            }
            ui.painter().text(
                rect.right_center() - Vec2::new(9.0, 0.0),
                egui::Align2::RIGHT_CENTER,
                &caption,
                FontId::monospace(10.0),
                t.text,
            );
        });
    })
    .response
    .on_hover_text(format!("{label}: {caption}"));
}

/// A section title: an accent bar, an 11 px sentence-case label, and an optional
/// right-aligned link. Reserves 20 px of height plus `space::S` below it, and
/// returns `true` when the link was clicked.
pub fn section_header(ui: &mut egui::Ui, title: &str, link: Option<&str>, t: Tokens) -> bool {
    let mut clicked = false;
    let response = ui.allocate_response(Vec2::new(ui.available_width(), 20.0), Sense::hover());
    let rect = response.rect;

    let bar_height = 11.0;
    let bar = egui::Rect::from_min_size(
        egui::pos2(rect.left(), rect.center().y - bar_height / 2.0),
        Vec2::new(3.0, bar_height),
    );
    ui.painter().rect_filled(bar, 1.0, t.ink(t.accent));

    let mut title_right = rect.right();
    if let Some(link_text) = link {
        let galley = ui.painter().layout_no_wrap(
            link_text.into(),
            FontId::proportional(11.0),
            t.ink(t.accent),
        );
        let link_rect = egui::Rect::from_min_size(
            egui::pos2(rect.right() - galley.size().x, rect.top()),
            Vec2::new(galley.size().x, rect.height()),
        );
        title_right = link_rect.left() - 10.0;
        let link_response = ui.interact(
            link_rect,
            ui.next_auto_id().with("section_header_link"),
            Sense::click(),
        );
        if link_response.hovered() {
            ui.painter().line_segment(
                [link_rect.left_bottom(), link_rect.right_bottom()],
                Stroke::new(1.0, t.ink(t.accent)),
            );
        }
        clicked = link_response.clicked();
        ui.painter().galley(
            egui::pos2(link_rect.left(), rect.center().y - galley.size().y / 2.0),
            galley,
            t.ink(t.accent),
        );
    }

    paint_text(
        ui,
        egui::Rect::from_min_max(
            egui::pos2(bar.right() + 6.0, rect.top()),
            egui::pos2(title_right, rect.bottom()),
        ),
        title,
        FontId::proportional(11.0),
        t.text_muted,
        Align::Min,
    );

    ui.add_space(theme::space::S);
    clicked
}

pub fn section_label(ui: &mut egui::Ui, label: &str, t: Tokens) {
    section_header(ui, label, None, t);
}

/// The only allowed rendering of unavailable data: one compact 22 px row with a
/// muted label and a muted pill giving a short reason (e.g. "Not reported"). The
/// full reason shows on hover. Never a giant "Unavailable" card or empty plot.
/// This is the ONLY allowed rendering of unavailable data from P3 onward; later
/// polish-gauntlet packages wire it into pages that currently show empty states.
#[allow(dead_code)]
pub fn gap_row(
    ui: &mut egui::Ui,
    label: &str,
    short_reason: &str,
    hover: &str,
    t: Tokens,
) -> egui::Response {
    let response = ui.allocate_response(Vec2::new(ui.available_width(), 22.0), Sense::hover());
    let rect = response.rect;

    let galley = ui.painter().layout_no_wrap(
        short_reason.into(),
        FontId::proportional(10.0),
        t.text_muted,
    );
    let pad = 6.0;
    let pill_size = Vec2::new(galley.size().x + pad * 2.0, 16.0);
    let pill_rect = egui::Rect::from_min_size(
        egui::pos2(
            rect.right() - pill_size.x,
            rect.center().y - pill_size.y / 2.0,
        ),
        pill_size,
    );
    ui.painter().rect_filled(
        pill_rect,
        pill_size.y / 2.0,
        t.surface(theme::mix(t.panel_raised, t.text_muted, 0.28)),
    );
    ui.painter().galley(
        egui::pos2(
            pill_rect.left() + pad,
            pill_rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        t.text_muted,
    );

    paint_text(
        ui,
        egui::Rect::from_min_max(
            rect.left_top(),
            egui::pos2(pill_rect.left() - 8.0, rect.bottom()),
        ),
        label,
        FontId::proportional(11.0),
        t.text_muted,
        Align::Min,
    );

    response.on_hover_text(format!("{label}: {hover}"))
}

/// Fixed-height metadata surface: changing freshness text must not move the table.
pub fn inventory_status(
    ui: &mut egui::Ui,
    title: &str,
    status: &str,
    detail: &str,
    color: Color32,
    t: Tokens,
    banded: bool,
) {
    hover_frame(ui, surface(ui, t, banded).inner_margin(8), |ui| {
        let width = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 30.0), Sense::hover());
        let top = egui::Rect::from_min_size(rect.min, Vec2::new(width, 15.0));
        let status_width = 96.0f32.min(width * 0.4);
        let title_rect = egui::Rect::from_min_max(
            top.min,
            egui::pos2(top.right() - status_width - 8.0, top.bottom()),
        );
        let status_rect =
            egui::Rect::from_min_max(egui::pos2(top.right() - status_width, top.top()), top.max);
        paint_text(
            ui,
            title_rect,
            title,
            FontId::proportional(11.0),
            t.text,
            Align::Min,
        );
        paint_text(
            ui,
            status_rect,
            status,
            FontId::monospace(10.0),
            t.ink(color),
            Align::Max,
        );
        paint_text(
            ui,
            egui::Rect::from_min_max(egui::pos2(rect.left(), top.bottom()), rect.max),
            detail,
            FontId::proportional(10.0),
            t.text_muted,
            Align::Min,
        );
    })
    .response
    .on_hover_text(format!("{title}: {status}\n{detail}"));
}

pub fn inventory_table<const N: usize>(
    ui: &mut egui::Ui,
    id: &str,
    headers: [&str; N],
    row_count: usize,
    selected: Option<usize>,
    mut values: impl FnMut(usize) -> [String; N],
    t: Tokens,
) -> Option<usize> {
    let mut clicked = None;
    ui.push_id(id, |ui| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        let width = ui.available_width();
        let height = (ui.available_height() - 36.0).max(32.0);
        let mut table = egui_extras::TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .cell_layout(Layout::left_to_right(Align::Center));
        for index in 0..N {
            let (fraction, minimum) = if N == 4 {
                [(0.24, 140.0), (0.34, 170.0), (0.27, 175.0), (0.15, 90.0)][index]
            } else {
                (0.38, if index + 1 == N { 120.0 } else { 180.0 })
            };
            table = table.column(if index + 1 == N {
                egui_extras::Column::remainder()
                    .at_least(minimum)
                    .clip(true)
            } else {
                egui_extras::Column::initial(width * fraction)
                    .at_least(minimum)
                    .clip(true)
            });
        }
        table
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
                body.rows(32.0, row_count, |mut row| {
                    let index = row.index();
                    row.set_selected(selected == Some(index));
                    for value in &values(index) {
                        table_column(&mut row, t, |ui| {
                            if table_label(ui, RichText::new(value).size(12.0).color(t.text))
                                .on_hover_text(value)
                                .clicked()
                            {
                                clicked = Some(index);
                            }
                        });
                    }
                })
            });
    });
    clicked
}

pub fn push_history(history: &mut VecDeque<f32>, value: f32, limit: usize) {
    if history.len() == limit {
        history.pop_front();
    }
    history.push_back(value);
}

/// Column count for a responsive tile grid: 1 below 520 px, 2 below 760, 3 below
/// 1100, otherwise 4. Later polish-gauntlet packages wire this into page grids.
#[allow(dead_code)]
pub fn tile_grid_columns(width: f32) -> usize {
    if width < 520.0 {
        1
    } else if width < 760.0 {
        2
    } else if width < 1100.0 {
        3
    } else {
        4
    }
}

/// Lay out `items` in a [`tile_grid_columns`]-wide grid with no orphan cards: full
/// rows use `ui.columns(columns)`, and the trailing partial row uses
/// `ui.columns(chunk.len())` instead, so its cards stretch to fill the row rather
/// than leaving empty gaps. Later polish-gauntlet packages wire this into pages.
#[allow(dead_code)]
pub fn fill_last_row<T>(
    ui: &mut egui::Ui,
    items: &[T],
    columns: usize,
    mut render: impl FnMut(&mut egui::Ui, &T),
) {
    let columns = columns.max(1);
    for chunk in items.chunks(columns) {
        ui.columns(chunk.len(), |cells| {
            for (cell, item) in cells.iter_mut().zip(chunk) {
                render(cell, item);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparkline_draws_nothing_for_fewer_than_two_points_and_breaks_on_gaps() {
        let ctx = egui::Context::default();
        let settings = ThemeSettings::default();
        theme::install(&ctx, settings);
        let t = theme::tokens(settings);
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(100.0, 40.0));

        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            sparkline(
                ui.painter(),
                rect,
                std::iter::once(Some(1.0)),
                10.0,
                t.accent,
                t,
            );
        });
        assert!(
            output.shapes.is_empty(),
            "fewer than two points must draw nothing, not a flat line"
        );

        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let values = [Some(1.0), Some(2.0), None, Some(3.0), Some(4.0)];
            sparkline(ui.painter(), rect, values.into_iter(), 10.0, t.accent, t);
        });
        let paths: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Path(path) if !path.closed => Some(path),
                _ => None,
            })
            .collect();
        assert_eq!(
            paths.len(),
            2,
            "a None sample must split the line, not bridge it"
        );
    }

    #[test]
    fn kpi_tile_height_is_fixed_regardless_of_width() {
        for width in [140.0_f32, 300.0] {
            let ctx = egui::Context::default();
            let settings = ThemeSettings::default();
            theme::install(&ctx, settings);
            let t = theme::tokens(settings);
            let mut height = 0.0;
            let _ = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(640.0, 480.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    ui.scope_builder(
                        egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                            egui::pos2(10.0, 10.0),
                            Vec2::new(width, 200.0),
                        )),
                        |ui| {
                            let response = kpi_tile(
                                ui,
                                &Kpi {
                                    label: "CPU",
                                    value: "37.2%",
                                    sub: "12 cores",
                                    hover: "CPU utilization",
                                    series: &[Some(1.0), Some(2.0), Some(3.0)],
                                    max: None,
                                    color: t.accent,
                                    state: None,
                                },
                                settings,
                                t,
                            );
                            height = response.rect.height();
                        },
                    );
                },
            );
            assert_eq!(
                height, KPI_TILE_HEIGHT,
                "kpi_tile must stay 84 px tall at width {width}"
            );
        }
    }

    #[test]
    fn gap_row_is_a_fixed_height_inline_row() {
        let ctx = egui::Context::default();
        let settings = ThemeSettings::default();
        theme::install(&ctx, settings);
        let t = theme::tokens(settings);
        let mut height = 0.0;
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            height = gap_row(
                ui,
                "WD Black SN850",
                "Needs sensor app",
                "No SMART/NVMe temperature exposed by this drive.",
                t,
            )
            .rect
            .height();
        });
        assert_eq!(height, 22.0);
    }

    #[test]
    fn tile_grid_columns_follows_width_breakpoints() {
        assert_eq!(tile_grid_columns(400.0), 1);
        assert_eq!(tile_grid_columns(600.0), 2);
        assert_eq!(tile_grid_columns(900.0), 3);
        assert_eq!(tile_grid_columns(1200.0), 4);
    }

    #[test]
    fn inventory_formats_visible_rows_instead_of_the_entire_cache() {
        let ctx = egui::Context::default();
        let settings = ThemeSettings::default();
        theme::install(&ctx, settings);
        let mut formatted = Vec::new();
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(1040.0, 640.0),
                )),
                ..Default::default()
            },
            |ui| {
                inventory_table(
                    ui,
                    "virtual_inventory",
                    ["NAME", "COMMAND", "SOURCE", "FRESHNESS"],
                    20_000,
                    None,
                    |index| {
                        formatted.push(index);
                        [
                            format!("Fixture {index}"),
                            "file".into(),
                            "source".into(),
                            "Cached".into(),
                        ]
                    },
                    theme::tokens(settings),
                );
            },
        );
        assert!(!formatted.is_empty());
        assert!(
            formatted.len() < 100,
            "formatted {} rows for one viewport",
            formatted.len()
        );
        assert!(formatted.into_iter().all(|index| index < 100));
        assert!(output.platform_output.commands.is_empty());
    }

    #[test]
    fn disabled_icon_actions_never_click_or_shift() {
        let settings = ThemeSettings::default();
        let t = theme::tokens(settings);
        let mut expected = None;
        for enabled in [true, false] {
            let ctx = egui::Context::default();
            theme::install(&ctx, settings);
            let mut bounds = egui::Rect::NOTHING;
            let mut clicked = false;
            for phase in 0..6 {
                let events = if phase >= 4 {
                    vec![
                        egui::Event::PointerMoved(bounds.center()),
                        egui::Event::PointerButton {
                            pos: bounds.center(),
                            button: egui::PointerButton::Primary,
                            pressed: phase == 4,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ]
                } else {
                    vec![]
                };
                let output = ctx.run_ui(
                    egui::RawInput {
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        let response = ui
                            .add_enabled_ui(enabled, |ui| {
                                icon_button(
                                    ui,
                                    Icon::Stop,
                                    "End task",
                                    Vec2::ZERO,
                                    t.panel_raised,
                                    t,
                                )
                            })
                            .inner;
                        bounds = response.rect;
                        clicked |= response.clicked();
                    },
                );
                assert!(output.platform_output.commands.is_empty());
            }
            assert_eq!(clicked, enabled);
            if let Some(expected) = expected {
                assert_eq!(bounds, expected);
            }
            expected = Some(bounds);
        }
    }

    #[test]
    fn missing_graph_samples_make_gaps_not_zero_or_bridged_lines() {
        let ctx = egui::Context::default();
        let settings = ThemeSettings::default();
        crate::theme::install(&ctx, settings);
        let values = VecDeque::from(vec![10.0, 20.0, f32::NAN, 80.0, 90.0]);
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            history_graph(
                ui,
                &values,
                Color32::RED,
                100.0,
                Some(100.0),
                crate::theme::tokens(settings),
            );
        });
        let paths: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Path(path) if !path.closed => Some(path),
                _ => None,
            })
            .collect();
        assert_eq!(paths.len(), 2);
        assert!(
            paths
                .iter()
                .all(|path| path.points.len() == 2 && path.points.iter().all(|p| p.is_finite()))
        );
        assert!(paths[0].points[1].x < paths[1].points[0].x);
    }

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
            for widget in 0..15 {
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
                                            nav_button(
                                                ui,
                                                false,
                                                Icon::Performance,
                                                "Performance",
                                                t,
                                            );
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
                                        6 => {
                                            mini_meter_text(
                                                ui,
                                                "CPU",
                                                "37.2%",
                                                Some(37.2),
                                                &VecDeque::from([12.0, 37.2]),
                                                t.accent,
                                                t,
                                            );
                                        }
                                        7 => engine_meter(ui, "3D", Some(37.2), t.accent, t),
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
                                            nav_button(
                                                ui,
                                                true,
                                                Icon::Performance,
                                                "Performance",
                                                t,
                                            );
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
                                        13 => {
                                            icon_button(
                                                ui,
                                                Icon::Theme,
                                                "Theme Studio",
                                                Vec2::new(240.0, 32.0),
                                                t.accent_dim,
                                                t,
                                            );
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
                                        14 => inventory_status(
                                            ui,
                                            "Startup source",
                                            "Cached",
                                            "Last complete 30s ago",
                                            t.text,
                                            t,
                                            true,
                                        ),
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
