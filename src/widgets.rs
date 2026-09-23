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

/// A selectable tab pill in a fixed slot sized for its framed (selected or
/// hovered) state: a pill that gains a frame when selected never nudges the
/// tabs after it sideways. Returns the button response.
pub fn stable_tab(ui: &mut egui::Ui, selected: bool, label: &str) -> egui::Response {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let text = ui
        .painter()
        .layout_no_wrap(label.into(), font, Color32::PLACEHOLDER)
        .size();
    let pad = ui.spacing().button_padding;
    let visuals = ui.visuals();
    let stroke = visuals
        .widgets
        .active
        .bg_stroke
        .width
        .max(visuals.widgets.hovered.bg_stroke.width)
        .max(visuals.selection.stroke.width)
        .max(visuals.widgets.inactive.bg_stroke.width);
    let size = Vec2::new(
        text.x + pad.x * 2.0 + stroke * 2.0 + 2.0,
        (text.y + pad.y * 2.0 + stroke * 2.0).max(ui.spacing().interact_size.y),
    )
    .ceil();
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    // A child Ui that never allocates back into the parent: even a framed
    // button a fraction wider than the slot cannot push later tabs.
    let mut slot = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::centered_and_justified(egui::Direction::LeftToRight)),
    );
    slot.add(egui::Button::selectable(selected, label).wrap_mode(egui::TextWrapMode::Extend))
}

/// Returns the pill's label response, so a caller can attach hover text.
pub fn status_pill(ui: &mut egui::Ui, label: &str, color: Color32) -> egui::Response {
    let dark = ui.visuals().dark_mode;
    let frame = egui::Frame::new()
        .fill(theme::text_surface(
            theme::mix(ui.visuals().window_fill, color, 0.15),
            dark,
        ))
        .corner_radius(20.0)
        .inner_margin(egui::Margin::symmetric(8, 3));
    let mut response = None;
    hover_frame(ui, frame, |ui| {
        response = Some(
            ui.label(
                RichText::new(label)
                    .size(10.0)
                    .strong()
                    .color(theme::ink(color, dark)),
            ),
        );
    });
    response.expect("hover_frame runs its contents once")
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
        FontId::proportional(11.0),
        t.text_muted,
    );
    painter.text(
        egui::pos2(rect.right(), text_y),
        egui::Align2::RIGHT_CENTER,
        text,
        FontId::monospace(11.0),
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
    io_right: f32,
}

/// PID, CPU TIME and TOTAL I/O sit directly after PROCESS, not pinned to the
/// table's far right edge: PROCESS is the flexible column, capped at
/// [`HISTORY_PROCESS_MAX`] so a wide window leaves the numeric columns close
/// to the name instead of stretching a huge gap between them.
const HISTORY_PID_WIDTH: f32 = 94.0;
const HISTORY_CPU_WIDTH: f32 = 134.0;
const HISTORY_IO_WIDTH: f32 = 110.0;
const HISTORY_COLUMN_GAP: f32 = 10.0;
const HISTORY_PROCESS_MIN: f32 = 140.0;
const HISTORY_PROCESS_MAX: f32 = 480.0;

fn history_columns(rect: egui::Rect) -> HistoryColumns {
    let name_left = rect.left() + 42.0;
    let trailing =
        HISTORY_COLUMN_GAP * 3.0 + HISTORY_PID_WIDTH + HISTORY_CPU_WIDTH + HISTORY_IO_WIDTH;
    let name_width =
        (rect.right() - name_left - trailing).clamp(HISTORY_PROCESS_MIN, HISTORY_PROCESS_MAX);
    let pid_left = name_left + name_width + HISTORY_COLUMN_GAP;
    let cpu_left = pid_left + HISTORY_PID_WIDTH + HISTORY_COLUMN_GAP;
    let io_left = cpu_left + HISTORY_CPU_WIDTH + HISTORY_COLUMN_GAP;
    let io_right = io_left + HISTORY_IO_WIDTH;
    HistoryColumns {
        name_left,
        pid_left,
        cpu_left,
        io_left,
        io_right,
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
        ("TOTAL I/O", cols.io_left, cols.io_right, Align::Max),
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
/// Rows are flat with zebra striping (no per-row bordered box), a uniform
/// 28 px tall, and share [`history_columns`] geometry with [`history_header`]
/// so the numeric columns line up under their headers exactly.
pub fn history_row(ui: &mut egui::Ui, rank: usize, process: &ProcessRow, t: Tokens) {
    let ordinal = format!("{:02}", rank + 1);
    let pid = process.pid.to_string();
    let cpu = format::millis(process.accumulated_cpu_millis);
    let io = format::bytes(
        process
            .total_read_bytes
            .saturating_add(process.total_write_bytes),
    );
    let (full, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::hover());
    let cols = history_columns(full);
    // The stripe and hover band end with the last column (plus the same
    // 8 px the rank column is inset by), never running on past TOTAL I/O.
    let rect = egui::Rect::from_min_max(
        full.min,
        egui::pos2((cols.io_right + 8.0).min(full.right()), full.bottom()),
    );
    let response = ui.interact(rect, ui.id().with(("history_row", rank)), Sense::hover());
    if rank % 2 == 1 {
        ui.painter()
            .rect_filled(rect, 0.0, ui.visuals().faint_bg_color);
    }
    if response.contains_pointer() && ui.is_enabled() {
        ui.painter()
            .rect_filled(rect, 0.0, ui.visuals().widgets.hovered.weak_bg_fill);
    }
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
            cols.io_right,
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
    response.on_hover_text(format!(
        "{}\nPID {pid}\nLifetime CPU: {cpu}\nTotal I/O: {io}",
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
    // One pixel of slack: a right-aligned glyph's ink may overhang its
    // advance width by a fraction of a pixel and must not be shaved off.
    ui.painter_at(rect.expand2(Vec2::new(1.0, 0.0))).galley(
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

/// A sortable column header: the plain label, plus a small painted triangle
/// (never an ASCII "v"/"^" glued onto the text) marking the active column
/// and its direction.
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
        let is_active = active == column;
        let color = if is_active { t.text } else { t.text_muted };
        let font = FontId::proportional(10.0);
        let galley = ui.painter().layout_no_wrap(label.to_owned(), font, color);
        let response = ui.allocate_response(ui.available_size(), Sense::click());
        let text_pos = egui::pos2(
            response.rect.left(),
            response.rect.center().y - galley.size().y * 0.5,
        );
        ui.painter().galley(text_pos, galley.clone(), color);
        if is_active {
            let tri_left = text_pos.x + galley.size().x + 5.0;
            let half_w = 3.5;
            let half_h = 2.5;
            let cy = response.rect.center().y;
            let points = match direction {
                SortDirection::Ascending => vec![
                    egui::pos2(tri_left, cy + half_h),
                    egui::pos2(tri_left + half_w * 2.0, cy + half_h),
                    egui::pos2(tri_left + half_w, cy - half_h),
                ],
                SortDirection::Descending => vec![
                    egui::pos2(tri_left, cy - half_h),
                    egui::pos2(tri_left + half_w * 2.0, cy - half_h),
                    egui::pos2(tri_left + half_w, cy + half_h),
                ],
            };
            ui.painter()
                .add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
        }
        let accessible_label = if is_active {
            format!(
                "{label}, sorted {}",
                match direction {
                    SortDirection::Ascending => "ascending",
                    SortDirection::Descending => "descending",
                }
            )
        } else {
            label.to_string()
        };
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                ui.is_enabled(),
                accessible_label.clone(),
            )
        });
        paint_table_focus(ui, &response);
        if response.clicked() {
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

/// A CPU-percent heat cell: an exact-zero reading collapses to a calm muted
/// "0%" instead of format::percent's "0.0%", the same convention gpu_cell
/// already uses. Without it a long idle process tree reads as a wall of
/// "0.0%" rows.
pub fn cpu_cell(ui: &mut egui::Ui, value: f32, t: Tokens) -> bool {
    let zero = value <= 0.0;
    let label = if zero {
        "0%".to_string()
    } else {
        format::percent(value)
    };
    heat_cell_response(
        ui,
        value,
        label,
        t.accent,
        if zero { t.text_muted } else { t.text },
        t,
    )
    .clicked()
}

/// Pure label/mute/explanation for [`gpu_cell`], split out so the zero-rounding
/// rule is unit-testable without an egui context. Unreported rows read "--"
/// with no "%" (there is no measurement to round), an exact-zero measured row
/// reads a calm "0%" instead of a busy "0.00%", and a partial reading that
/// rounds to zero reads the same calm "0%+" instead of the noisy "0.0%+" a
/// plain lower-bound marker would produce; a partial reading that still rounds
/// to a nonzero digit keeps its precise "0.2%+". Every other state keeps
/// [`crate::gpu_activity::Usage::label`] unchanged.
fn gpu_cell_parts(usage: crate::gpu_activity::Usage, grouped: bool) -> (String, bool, String) {
    use crate::gpu_activity::Usage;
    const PARTIAL_HOVER: &str = "Partial: some engines not readable.";
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
        Usage::Partial(value) => {
            let percent = format::percent(value);
            if percent == "0.0%" {
                ("0%+".to_string(), true, PARTIAL_HOVER.to_string())
            } else {
                (format!("{percent}+"), false, PARTIAL_HOVER.to_string())
            }
        }
        _ => (usage.label(), false, usage.explanation().to_string()),
    };
    if grouped {
        explanation.push_str(" Grouped rows sum process peaks; this is not whole-GPU utilization.");
    }
    (label, muted, explanation)
}

pub fn gpu_cell(
    ui: &mut egui::Ui,
    usage: crate::gpu_activity::Usage,
    grouped: bool,
    t: Tokens,
) -> bool {
    let (label, muted, explanation) = gpu_cell_parts(usage, grouped);
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

/// The Performance hero: device name, an optional one-line subline (the model
/// or adapter name; an empty `detail` draws no line), the headline value and an
/// accent rule. Returns the hero's response so callers can put provenance on
/// its hover instead of on visible text lines.
pub fn performance_heading(
    ui: &mut egui::Ui,
    label: &str,
    detail: &str,
    value: &str,
    color: Color32,
    t: Tokens,
) -> egui::Response {
    performance_heading_with_state(ui, label, detail, value, None, color, t)
}

/// [`performance_heading`] with one state chip beside the value, drawn only
/// when a source is not Live ("Counters: Cached"). The chip lives inside the
/// hero, so a state change never moves the fields below it.
pub fn performance_heading_with_state(
    ui: &mut egui::Ui,
    label: &str,
    detail: &str,
    value: &str,
    state: Option<(&str, Color32)>,
    color: Color32,
    t: Tokens,
) -> egui::Response {
    let response = hover_frame(ui, surface(ui, t, false), |ui| {
        ui.horizontal(|ui| {
            let width = ui.available_width();
            // Measure the value's real width first, so a short reading (most
            // Performance tiles) leaves the device name room to fit before it
            // truncates, instead of always reserving a fixed 37%/200px chunk.
            let value_galley =
                ui.painter()
                    .layout_no_wrap(value.into(), FontId::monospace(24.0), t.text);
            let chip = state.map(|(text, color)| {
                (
                    ui.painter().layout_no_wrap(
                        text.into(),
                        FontId::proportional(10.0),
                        t.ink(color),
                    ),
                    color,
                )
            });
            let chip_width = chip.as_ref().map_or(0.0, |(g, _)| g.size().x + 12.0 + 8.0);
            let value_width = (value_galley.size().x + 4.0 + chip_width).min(width * 0.6);
            ui.allocate_ui_with_layout(
                Vec2::new(
                    (width - value_width - 8.0).max(0.0),
                    if detail.is_empty() { 30.0 } else { 47.0 },
                ),
                Layout::top_down(Align::Min),
                |ui| {
                    ui.add(
                        egui::Label::new(RichText::new(label).size(18.0).strong().color(t.text))
                            .truncate(),
                    );
                    if !detail.is_empty() {
                        ui.add(
                            egui::Label::new(RichText::new(detail).size(11.0).color(t.text_muted))
                                .truncate(),
                        );
                    }
                },
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add(
                    egui::Label::new(RichText::new(value).size(24.0).monospace().color(t.text))
                        .truncate(),
                )
                .on_hover_text(value);
                if let Some((galley, chip_color)) = chip {
                    ui.add_space(8.0);
                    let size = galley.size() + Vec2::new(12.0, 4.0);
                    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                    ui.painter().rect_filled(
                        rect,
                        rect.height() / 2.0,
                        t.surface(theme::mix(t.panel_raised, chip_color, 0.18)),
                    );
                    ui.painter()
                        .galley(rect.min + Vec2::new(6.0, 2.0), galley, t.ink(chip_color));
                }
            });
        });
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 2.0), Sense::hover());
        ui.painter().rect_filled(rect, 1.0, color);
    })
    .response;
    ui.add_space(theme::space::S);
    response
}

/// Height for a detail view's big graph: what is left of the visible
/// viewport after the content above it and `below` px of fields under it,
/// clamped to `min..=max`. Measured from the content origin, so scrolling
/// never resizes the graph; at 1000x580 the fields under the graph stay above
/// the fold instead of the graph always taking a fixed 270 px.
pub fn fit_height(ui: &egui::Ui, below: f32, min: f32, max: f32) -> f32 {
    let viewport = ui.clip_rect().height();
    let above = ui.cursor().top() - ui.min_rect().top();
    (viewport - above - below).clamp(min, max)
}

/// Samples one Performance history holds, one per system sample (nominally
/// 1 s). Every history graph spans exactly this window, so 20 samples fill
/// the newest sixth of the plot instead of stretching across "-120 s".
pub const HISTORY_WINDOW: usize = 120;

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
        ("120 SECONDS", HISTORY_WINDOW),
        None,
    );
}

/// `window.1` is the number of sample slots the plot spans, right-aligned at
/// "now": pass [`HISTORY_WINDOW`], never a `VecDeque`'s `capacity()` (a clone's
/// capacity equals its length, which stretches a few samples across the whole
/// window). `unit_fmt`, when given, formats the top-left axis label with its
/// unit ("100%", "25.0 ms"); `None` keeps a plain number.
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
    // Data lives between a top strip (the unit axis label) and a bottom strip
    // ("-120 s" / "now"), so a spike never runs through the labels. Same
    // geometry as the Graphs page cards.
    let plot = rect.shrink2(Vec2::new(7.0, 17.0));
    for index in 0..=4 {
        let y = egui::lerp(plot.top()..=plot.bottom(), index as f32 / 4.0);
        painter.line_segment(
            [egui::pos2(plot.left(), y), egui::pos2(plot.right(), y)],
            Stroke::new(0.5, theme::mix(t.graph_bg, t.border, 0.65)),
        );
    }
    for index in 1..8 {
        let x = egui::lerp(plot.left()..=plot.right(), index as f32 / 8.0);
        painter.line_segment(
            [egui::pos2(x, plot.top()), egui::pos2(x, plot.bottom())],
            Stroke::new(0.5, theme::mix(t.graph_bg, t.border, 0.5)),
        );
    }
    if history.len() > 1 {
        let observed = history.iter().copied().fold(0.0_f32, f32::max);
        // Auto scale rounds up to a readable step ("25 ms", never "MAX 23.1").
        let maximum = fixed_max
            .unwrap_or_else(|| format::nice_top(observed * 1.05))
            .max(0.001);
        let denominator = (window.1.max(history.len()).max(2) - 1) as f32;
        let points = history
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let x = plot.right()
                    - ((history.len() - 1 - index) as f32 / denominator) * plot.width();
                let y = plot.bottom() - (value.clamp(0.0, maximum) / maximum) * plot.height();
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
            fill.colored_vertex(egui::pos2(second.x, plot.bottom()), Color32::TRANSPARENT);
            fill.colored_vertex(egui::pos2(first.x, plot.bottom()), Color32::TRANSPARENT);
            fill.add_triangle(base, base + 1, base + 2);
            fill.add_triangle(base, base + 2, base + 3);
        }
        painter.add(egui::Shape::mesh(fill));
        for run in points.split(Option::is_none) {
            let run: Vec<_> = run.iter().flatten().copied().collect();
            if run.len() > 1 {
                painter.add(egui::Shape::line(run, Stroke::new(1.75, t.ink(color))));
            }
        }
        painter.text(
            rect.left_top() + Vec2::new(7.0, 4.0),
            egui::Align2::LEFT_TOP,
            match unit_fmt {
                Some(format) => format(maximum),
                None => format!("{maximum:.0}"),
            },
            FontId::monospace(9.0),
            t.text_muted,
        );
        painter.text(
            rect.left_bottom() + Vec2::new(7.0, -4.0),
            egui::Align2::LEFT_BOTTOM,
            "-120 s",
            FontId::monospace(9.0),
            t.text_muted,
        );
        painter.text(
            rect.right_bottom() - Vec2::new(7.0, 4.0),
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
    sparkline_range(painter, rect, values, 0.0, max, color, t);
}

/// The y-range for a sparkline of a steady physical reading (temperature,
/// power): the observed min and max, widened to at least `min_span` around
/// their midpoint and padded by a tenth of the range, so a steady reading sits
/// mid-band instead of drawing a flat line along an edge. Never below zero when
/// every sample is non-negative. `None` without a finite sample.
pub fn padded_range(values: &[Option<f32>], min_span: f32) -> Option<(f32, f32)> {
    let mut finite = values.iter().flatten().copied().filter(|v| v.is_finite());
    let first = finite.next()?;
    let (mut lo, mut hi) = finite.fold((first, first), |(lo, hi), v| (lo.min(v), hi.max(v)));
    let non_negative = lo >= 0.0;
    let pad = (hi - lo) * 0.1;
    lo -= pad;
    hi += pad;
    if hi - lo < min_span {
        let mid = (hi + lo) / 2.0;
        lo = mid - min_span / 2.0;
        hi = mid + min_span / 2.0;
    }
    if non_negative && lo < 0.0 {
        hi -= lo;
        lo = 0.0;
    }
    Some((lo, hi))
}

/// [`sparkline`] over an explicit `lo..=hi` value range; values outside it are
/// clamped to the band edges.
pub fn sparkline_range(
    painter: &egui::Painter,
    rect: egui::Rect,
    values: impl Iterator<Item = Option<f32>>,
    lo: f32,
    hi: f32,
    color: Color32,
    t: Tokens,
) {
    let values: Vec<Option<f32>> = values.collect();
    if values.len() < 2 {
        return;
    }
    let span = (hi - lo).max(0.001);
    let denominator = (values.len() - 1).max(1) as f32;
    let points: Vec<Option<egui::Pos2>> = values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            value.filter(|v| v.is_finite()).map(|v| {
                let x = egui::lerp(rect.left()..=rect.right(), index as f32 / denominator);
                let y = rect.bottom() - ((v - lo).clamp(0.0, span) / span) * rect.height();
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

/// A Performance rail tile. `max` fixes the sparkline scale (100 for
/// percentages, so an idle disk never reads as a full-height spike); `None`
/// auto-scales to the visible samples (rates, temperatures). When `selected`
/// and `follow` are both set, the tile scrolls its enclosing scroll area just
/// enough to show it; callers pass `follow` only on the frame the selection
/// changed.
#[allow(clippy::too_many_arguments)]
pub fn rail_button(
    ui: &mut egui::Ui,
    selected: bool,
    follow: bool,
    label: &str,
    value: &str,
    history: &VecDeque<f32>,
    max: Option<f32>,
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
        let max = max.unwrap_or_else(|| {
            history
                .iter()
                .copied()
                .filter(|v| v.is_finite())
                .fold(1.0_f32, f32::max)
        });
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
    if selected && follow {
        // An instant jump: no animation frames, so no repaint run.
        response.scroll_to_me_animation(None, egui::style::ScrollAnimation::none());
    }
    response
        .on_hover_text(format!("{label}: {value}"))
        .clicked()
}

/// Content for one [`kpi_tile`] on the Overview page's KPI row.
pub struct Kpi<'a> {
    pub label: &'a str,
    pub value: &'a str,
    pub sub: &'a str,
    /// Drawn instead of `sub` when `sub` does not fit; empty for none.
    pub sub_short: &'a str,
    pub hover: &'a str,
    pub series: &'a [Option<f32>],
    pub max: Option<f32>,
    pub color: Color32,
    /// `Some` only when the tile is not Live (e.g. "Cached", "Partial", "Stale").
    pub state: Option<&'a str>,
}

/// Base height of a [`kpi_tile`], regardless of width.
pub const KPI_TILE_HEIGHT: f32 = 84.0;

/// A raised, fixed-height at-a-glance tile: a 12 px label row, a big value, an
/// 11 px caption, and a full-width sparkline band along the bottom.
#[cfg(test)]
pub fn kpi_tile(
    ui: &mut egui::Ui,
    kpi: &Kpi<'_>,
    settings: ThemeSettings,
    t: Tokens,
) -> egui::Response {
    kpi_tile_sized(ui, kpi, KPI_TILE_HEIGHT, settings, t)
}

/// [`kpi_tile`] at `height` (at least [`KPI_TILE_HEIGHT`]): every extra pixel
/// goes to the sparkline band, so a tall window shows more trend, not air.
pub fn kpi_tile_sized(
    ui: &mut egui::Ui,
    kpi: &Kpi<'_>,
    height: f32,
    settings: ThemeSettings,
    t: Tokens,
) -> egui::Response {
    let response = ui.allocate_response(
        Vec2::new(ui.available_width(), height.max(KPI_TILE_HEIGHT)),
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
        egui::pos2(content.left(), rect.top() + 61.0),
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
    let row1 = egui::Rect::from_min_size(content.min, Vec2::new(content.width(), 14.0));
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(row1)
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            ui.add(
                egui::Label::new(RichText::new(kpi.label).size(12.0).color(t.text_muted))
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
        egui::pos2(content.left(), rect.top() + 21.0),
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
        egui::pos2(content.left(), rect.top() + 45.0),
        egui::pos2(content.right(), rect.top() + 59.0),
    );
    let fits = |text: &str| {
        ui.painter()
            .layout_no_wrap(text.to_owned(), FontId::proportional(11.0), t.text_muted)
            .size()
            .x
            <= row3.width()
    };
    let caption = if !kpi.sub_short.is_empty() && !fits(kpi.sub) {
        kpi.sub_short
    } else {
        kpi.sub
    };
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(row3)
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            ui.add(
                egui::Label::new(RichText::new(caption).size(11.0).color(t.text_muted)).truncate(),
            );
        },
    );

    response.on_hover_text(kpi.hover)
}

/// A compact value tile: a sentence-case label with a state chip on the right
/// only when the reading is not Live, then the monospace value. Provenance and
/// caveats go in `hover`, never on a visible caption line. A `--` value is
/// drawn muted so a gap never reads as a measurement.
pub fn value_tile(
    ui: &mut egui::Ui,
    label: &str,
    value: &str,
    hover: &str,
    state: Option<&str>,
    banded: bool,
    t: Tokens,
) -> egui::Response {
    hover_frame(ui, surface(ui, t, banded), |ui| {
        ui.set_min_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 2.0;
        let width = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 15.0), Sense::hover());
        let mut label_right = rect.right();
        if let Some(state) = state {
            let galley =
                ui.painter()
                    .layout_no_wrap(state.into(), FontId::proportional(10.0), t.text_muted);
            let pill = egui::Rect::from_min_max(
                egui::pos2(rect.right() - galley.size().x - 12.0, rect.top()),
                rect.max,
            );
            ui.painter().rect_filled(
                pill,
                pill.height() / 2.0,
                t.surface(theme::mix(t.panel_raised, t.text_muted, 0.2)),
            );
            ui.painter().galley(
                egui::pos2(pill.left() + 6.0, pill.center().y - galley.size().y / 2.0),
                galley,
                t.text_muted,
            );
            label_right = pill.left() - theme::space::S;
        }
        paint_text(
            ui,
            egui::Rect::from_min_max(rect.min, egui::pos2(label_right, rect.bottom())),
            label,
            FontId::proportional(11.0),
            t.text_muted,
            Align::Min,
        );
        let muted = value == "--" || value.starts_with("-- ");
        ui.add(
            egui::Label::new(RichText::new(value).size(17.0).monospace().color(if muted {
                t.text_muted
            } else {
                t.text
            }))
            .truncate(),
        );
    })
    .response
    .on_hover_text(if hover.is_empty() {
        format!("{label}: {value}")
    } else {
        format!("{label}: {value}\n{hover}")
    })
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
            FontId::proportional(11.0),
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

/// `values` returns one `(display, hover)` pair per column. Most columns hover
/// their own display text; a column that wants to keep its cell short (e.g. a
/// freshness word with the observation age on hover) gives a longer hover string.
/// `columns` gives each column's header label plus its `(fraction, minimum)`
/// width; `flex_index` names which one is the flexible remainder column (its
/// fraction is ignored) so callers put the flex on the actual text column
/// (COMMAND / FILE, DISPLAY NAME) instead of whichever happens to sit last;
/// a short label like PID or FRESHNESS never earns the leftover width. Every
/// other column keeps the fixed width its fraction implies, so callers with
/// the same `N` but different meanings (e.g. Services with and without a
/// FRESHNESS column) pass their own widths instead of sharing one by column
/// count alone. The flex column is not user-resizable, matching how a
/// last-column remainder already behaves; every other column still drags.
#[allow(clippy::too_many_arguments)]
pub fn inventory_table<const N: usize>(
    ui: &mut egui::Ui,
    id: &str,
    columns: [(&str, f32, f32); N],
    flex_index: usize,
    row_count: usize,
    selected: Option<usize>,
    chip: Option<StateChipColumn<'_>>,
    mut values: impl FnMut(usize) -> [(String, String); N],
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
        for (index, &(_, fraction, minimum)) in columns.iter().enumerate() {
            table = table.column(if index == flex_index {
                egui_extras::Column::remainder()
                    .at_least(minimum)
                    .clip(true)
                    .resizable(false)
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
                for &(label, _, _) in &columns {
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
                    for (column, (value, hover)) in values(index).iter().enumerate() {
                        table_column(&mut row, t, |ui| {
                            let response = match chip {
                                Some((chip_column, color)) if chip_column == column => {
                                    state_chip(ui, value, color(value), t)
                                }
                                _ => table_label(
                                    ui,
                                    RichText::new(value.as_str()).size(12.0).color(t.text),
                                ),
                            };
                            if response.on_hover_text(hover.as_str()).clicked() {
                                clicked = Some(index);
                            }
                        });
                    }
                })
            });
    });
    clicked
}

/// `(column index, color for a state word)`: the inventory column drawn as
/// [`state_chip`] cells instead of plain text.
pub type StateChipColumn<'a> = (usize, &'a dyn Fn(&str) -> Color32);

/// A table-cell state chip: a colored dot and the state word on a faint tint
/// of the same color, so Running and Stopped read apart at a glance. The cell
/// stays one click target, like [`table_label`].
pub fn state_chip(ui: &mut egui::Ui, text: &str, color: Color32, t: Tokens) -> egui::Response {
    let response = ui.allocate_response(ui.available_size(), Sense::click());
    let font = FontId::proportional(11.0);
    let ink = t.ink(color);
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, ink);
    let width = (galley.size().x + 24.0).min(response.rect.width());
    let chip = egui::Rect::from_min_size(
        egui::pos2(response.rect.left(), response.rect.center().y - 10.0),
        Vec2::new(width, 20.0),
    );
    ui.painter().rect_filled(
        chip,
        10.0,
        t.surface(theme::mix(t.panel_raised, color, 0.16)),
    );
    ui.painter()
        .circle_filled(chip.left_center() + Vec2::new(9.0, 0.0), 3.0, ink);
    // Optically centered: the galley box carries descender room below the
    // cap height, so lift the text by one pixel.
    ui.painter_at(chip).galley(
        egui::pos2(
            chip.left() + 17.0,
            chip.center().y - galley.size().y * 0.5 - 1.0,
        ),
        galley,
        ink,
    );
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, ui.is_enabled(), text));
    paint_table_focus(ui, &response);
    response
}

pub fn push_history(history: &mut VecDeque<f32>, value: f32, limit: usize) {
    if history.len() == limit {
        history.pop_front();
    }
    history.push_back(value);
}

/// Column count for a responsive tile grid: 1 below 520 px, 2 below 760, 3 below
/// 1100, otherwise 4.
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

/// Grid columns for `cards` cards when the width allows `columns`: keep
/// `columns` when the cards fill every row; otherwise widen to the fewest
/// extra columns (each still at least `min_card` wide) that make every row
/// full, then fall back to fewer columns that divide the cards evenly. Only
/// when neither exists does the grid keep `columns` and a short last row.
pub fn balanced_columns(cards: usize, columns: usize, width: f32, min_card: f32) -> usize {
    let columns = columns.max(1);
    if cards <= columns || cards.is_multiple_of(columns) {
        return columns;
    }
    let fit = (width / min_card.max(1.0)).floor() as usize;
    if let Some(wider) = (columns + 1..=fit.min(cards)).find(|k| cards.is_multiple_of(*k)) {
        return wider;
    }
    (2..columns)
        .rev()
        .find(|k| cards.is_multiple_of(*k))
        .unwrap_or(columns)
}

/// Columns to allocate for a grid row holding `cards` of a `columns`-wide
/// grid. A full row uses every column. A short last row stretches its cards
/// only while each stays within 1.5x a full-row card (4 columns with one
/// missing, 3 with one missing); otherwise it keeps the full-row width and
/// sits left aligned, so a lone card never spans the whole pane.
pub fn row_columns(columns: usize, cards: usize) -> usize {
    let columns = columns.max(1);
    let cards = cards.clamp(1, columns);
    if cards == columns || columns as f32 / cards as f32 <= 1.5 {
        cards
    } else {
        columns
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

/// Height of the dialog header bar drawn by [`dialog_header`].
pub const DIALOG_HEADER: f32 = 36.0;

/// Dialog chrome: a header bar with a 16 px left-aligned title and a close
/// button. egui's own title bar centers a Heading-sized title, so dialogs use
/// `Window::title_bar(false)` with a zero-margin frame, call this first, and
/// pad their body with [`dialog_body`]. Returns true when close is clicked.
pub fn dialog_header(ui: &mut egui::Ui, title: &str, t: Tokens) -> bool {
    let (bar, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), DIALOG_HEADER),
        Sense::hover(),
    );
    let radius = ui.visuals().window_corner_radius;
    ui.painter().rect_filled(
        bar,
        egui::CornerRadius {
            nw: radius.nw,
            ne: radius.ne,
            sw: 0,
            se: 0,
        },
        ui.visuals().widgets.open.weak_bg_fill,
    );
    ui.painter()
        .hline(bar.x_range(), bar.bottom(), Stroke::new(1.0, t.border));
    let close =
        egui::Rect::from_center_size(bar.right_center() - Vec2::new(22.0, 0.0), Vec2::splat(26.0));
    let title_rect = egui::Rect::from_min_max(
        bar.min + Vec2::new(14.0, 0.0),
        egui::pos2(close.left() - 8.0, bar.bottom()),
    );
    paint_text(
        ui,
        title_rect,
        title,
        FontId::proportional(16.0),
        t.text,
        egui::Align::Min,
    );
    let response = ui
        .interact(close, ui.id().with("dialog_close"), Sense::click())
        .on_hover_text("Close");
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), "Close")
    });
    if response.hovered() {
        ui.painter().rect_filled(
            close,
            ui.visuals().widgets.hovered.corner_radius,
            ui.visuals().widgets.hovered.weak_bg_fill,
        );
    }
    Icon::Close.paint(
        ui.painter(),
        egui::Rect::from_center_size(close.center(), Vec2::splat(14.0)),
        if response.hovered() {
            t.text
        } else {
            t.text_muted
        },
    );
    response.clicked()
}

/// Dialog body under [`dialog_header`], with the standard dialog padding.
pub fn dialog_body<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .inner_margin(egui::Margin {
            left: 14,
            right: 14,
            top: 10,
            bottom: 12,
        })
        .show(ui, add)
        .inner
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_cell_partial_rounds_zero_to_a_muted_plus_and_keeps_nonzero_precise() {
        use crate::gpu_activity::Usage;
        // P17: a partial reading that rounds to zero reads the same calm "0%"
        // convention as an exact zero, never the noisy "0.0%+".
        let (label, muted, _) = gpu_cell_parts(Usage::Partial(0.03), false);
        assert_eq!(label, "0%+");
        assert!(muted);
        // A partial reading that still rounds to a nonzero digit keeps its
        // precise lower-bound marker and stays unmuted.
        let (label, muted, _) = gpu_cell_parts(Usage::Partial(0.2), false);
        assert_eq!(label, "0.2%+");
        assert!(!muted);
        let (label, muted, _) = gpu_cell_parts(Usage::Partial(12.34), false);
        assert_eq!(label, "12.3%+");
        assert!(!muted);
    }

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
                                    sub_short: "",
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
    fn balanced_columns_never_leave_an_orphan_card_when_a_full_grid_fits() {
        // Three engine cards at a two-column width go three across.
        assert_eq!(balanced_columns(3, 2, 535.0, 170.0), 3);
        // Too narrow for three: keep two (and the short last row).
        assert_eq!(balanced_columns(3, 2, 400.0, 170.0), 2);
        // Already full rows, or fewer cards than columns: unchanged.
        assert_eq!(balanced_columns(4, 2, 535.0, 170.0), 2);
        assert_eq!(balanced_columns(2, 3, 900.0, 170.0), 3);
        // Six at four columns: three across divides evenly.
        assert_eq!(balanced_columns(6, 4, 900.0, 300.0), 3);
    }

    #[test]
    fn row_columns_stretches_a_short_row_only_within_one_and_a_half_widths() {
        assert_eq!(row_columns(4, 4), 4);
        assert_eq!(row_columns(4, 3), 3);
        assert_eq!(row_columns(4, 2), 4);
        assert_eq!(row_columns(4, 1), 4);
        assert_eq!(row_columns(3, 2), 2);
        assert_eq!(row_columns(3, 1), 3);
        assert_eq!(row_columns(2, 1), 2);
        assert_eq!(row_columns(1, 1), 1);
        for columns in 1..=4 {
            for cards in 1..=columns {
                let allocated = row_columns(columns, cards);
                assert!(columns as f32 / allocated as f32 <= 1.5);
            }
        }
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
                    [
                        ("NAME", 0.24, 140.0),
                        ("COMMAND", 0.34, 170.0),
                        ("SOURCE", 0.27, 175.0),
                        ("FRESHNESS", 0.15, 90.0),
                    ],
                    1,
                    20_000,
                    None,
                    None,
                    |index| {
                        formatted.push(index);
                        [
                            (format!("Fixture {index}"), format!("Fixture {index}")),
                            ("file".into(), "file".into()),
                            ("source".into(), "source".into()),
                            ("Cached".into(), "Cached".into()),
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

    /// 20 samples in a 120-sample window cover the newest sixth of the plot.
    /// A `.cloned()` history has capacity == len, which once stretched these
    /// 20 samples across the whole "-120 s" axis.
    #[test]
    fn a_short_history_fills_only_its_share_of_the_time_axis() {
        let ctx = egui::Context::default();
        let settings = ThemeSettings::default();
        crate::theme::install(&ctx, settings);
        let t = crate::theme::tokens(settings);
        let values: VecDeque<f32> = (0..20).map(|v| v as f32).collect();
        let cloned = values.clone();
        assert_eq!(cloned.capacity().max(cloned.len()), 20, "fixture premise");
        for pass in 0..2 {
            let mut rect = egui::Rect::NOTHING;
            let output = ctx.run_ui(egui::RawInput::default(), |ui| {
                rect = ui.available_rect_before_wrap();
                if pass == 0 {
                    history_graph(ui, &cloned, Color32::RED, 100.0, Some(100.0), t);
                } else {
                    history_graph_with_window(
                        ui,
                        &cloned,
                        Color32::RED,
                        100.0,
                        None,
                        t,
                        ("120 s", HISTORY_WINDOW),
                        Some(&|v: f32| format!("{v:.0}%")),
                    );
                }
            });
            let first_x = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Path(path) if !path.closed => Some(path),
                    _ => None,
                })
                .flat_map(|path| path.points.iter().map(|p| p.x))
                .fold(f32::MAX, f32::min);
            assert!(
                first_x >= rect.left() + rect.width() * 0.8,
                "pass {pass}: first point at x={first_x} in {rect:?}"
            );
        }
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
                                            rail_button(
                                                ui,
                                                false,
                                                false,
                                                "CPU",
                                                "37.2%",
                                                &VecDeque::new(),
                                                None,
                                                t.accent,
                                                t,
                                            );
                                        }
                                        2 => {
                                            value_tile(
                                                ui,
                                                "Uptime",
                                                "5d 03h 28m",
                                                "",
                                                None,
                                                false,
                                                t,
                                            );
                                        }
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
                                        5 => {
                                            status_pill(ui, "LIVE", t.good);
                                        }
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
                                            rail_button(
                                                ui,
                                                true,
                                                false,
                                                "CPU",
                                                "37.2%",
                                                &VecDeque::new(),
                                                None,
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
