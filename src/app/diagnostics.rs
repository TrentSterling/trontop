use super::*;
use crate::diagnostics::{Provider, State, age, support_report};
use std::time::Instant;

/// Window id of the About dialog.
pub(super) const ABOUT_WINDOW: &str = "about_window";

/// Version table row pitch, frame included.
const ABOUT_ROW: f32 = 28.0;

impl TrontopApp {
    pub(super) fn diagnostics_window(&mut self, ctx: &egui::Context) {
        if !self.show_diagnostics {
            return;
        }
        let t = self.colors();
        let mut close = false;
        let screen = ctx.content_rect();
        let width = 620.0_f32.min(screen.width() - 48.0);
        // Everything fits at 1000x580 with the licenses folded; opening them
        // scrolls the body, never the header.
        let body_height = (screen.height() - 48.0 - widgets::DIALOG_HEADER).max(200.0);
        widgets::dialog_window(ctx, "About Trontop", width)
            .id(egui::Id::new(ABOUT_WINDOW))
            .default_width(width)
            .default_height(body_height + widgets::DIALOG_HEADER)
            .max_height(body_height + widgets::DIALOG_HEADER)
            .resizable(false)
            .show(ctx, |ui| {
                ui.set_width(width);
                close = widgets::dialog_header(ui, "About Trontop", t);
                egui::ScrollArea::vertical()
                    .id_salt("about_body")
                    .max_height(body_height)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        widgets::dialog_body(ui, |ui| self.about_body(ui, ctx, t))
                    })
                    .settled(ui, widgets::VERTICAL);
            });
        if close {
            self.show_diagnostics = false;
        }
    }

    fn about_body(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, t: Tokens) {
        ui.spacing_mut().item_spacing.y = theme::space::S;
        ui.horizontal(|ui| {
            widgets::tront_mark(ui, self.theme, 30.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                widgets::hover_label(
                    ui,
                    RichText::new("Trontop").size(16.0).strong().color(t.text),
                );
                widgets::hover_label(
                    ui,
                    RichText::new("Built by Trent Sterling / tront.xyz")
                        .size(12.0)
                        .color(t.text_muted),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Copy support report")
                    .on_hover_text("Build and provider status only. No process names, commands, paths, account or host names, GPU IDs or addresses. Nothing is uploaded.")
                    .clicked()
                {
                    ctx.copy_text(support_report(&self.snapshot.diagnostics, Instant::now()));
                    self.message = Some(("Support report copied. Nothing uploaded.".into(), false));
                }
            });
        });
        ui.add_space(theme::space::XS);
        let tray = self.tray.as_ref().map(TrayController::state);
        let tray_reason = match tray {
            None => {
                "No notification-area icon in this session: its background worker did not start. Trontop keeps working without it."
            }
            Some(crate::tray::TrayState::Unavailable) => {
                "Windows did not accept the notification-area icon, or its background worker stopped. Trontop keeps working without it."
            }
            Some(crate::tray::TrayState::Stopped) => {
                "The notification-area icon was removed while Trontop closes."
            }
            Some(_) => "Notification-area icon with a live CPU meter.",
        };
        ui.scope(|ui| {
            // 26 px rows plus 2 px spacing: a 28 px pitch.
            ui.spacing_mut().item_spacing.y = ABOUT_ROW - 26.0;
            about_row(ui, "Version", env!("CARGO_PKG_VERSION"), None, t);
            about_row(ui, "Build", env!("TRONTOP_BUILD_ID"), None, t);
            about_row(ui, "Target", env!("TRONTOP_BUILD_TARGET"), None, t);
            about_row(
                ui,
                "Profile",
                if cfg!(debug_assertions) {
                    "Debug"
                } else {
                    "Optimized release"
                },
                None,
                t,
            );
            about_row(
                ui,
                "System tray",
                tray.unwrap_or(crate::tray::TrayState::Unavailable).label(),
                Some(tray_reason),
                t,
            );
        });
        ui.add_space(theme::space::S);
        widgets::section_label(ui, "Provider health", t);
        let now = self.graphs.now();
        let columns = if ui.available_width() >= 440.0 { 2 } else { 1 };
        let gap = theme::space::M;
        let cell = (ui.available_width() - gap * (columns - 1) as f32) / columns as f32;
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            for chunk in Provider::ALL.chunks(columns) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for &provider in chunk {
                        self.provider_cell(ui, provider, cell, now, t);
                    }
                });
            }
        });
        ui.add_space(theme::space::XS);
        let log_hover = "Keeps up to 32 local failure records; nothing is uploaded. Rust panics, native-runner errors and GPU recovery events, without raw messages or memory dumps. Forced exits, hangs and native crashes may leave no record, and file access failures can prevent logging.";
        ui.horizontal(|ui| {
            widgets::hover_label(ui, RichText::new("Local failure log").size(11.0).color(t.text_muted))
                .on_hover_text(log_hover);
            widgets::hover_label(ui, RichText::new(crate::failure::LOCATION_HINT).monospace().size(11.0).color(t.text))
                .on_hover_text(log_hover);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Copy log location").on_hover_text(log_hover).clicked() {
                    ctx.copy_text(crate::failure::LOCATION_HINT.into());
                    self.message = Some(("Log location copied. Paste into Explorer; the file exists only after a recorded failure.".into(), false));
                }
            });
        });
        ui.add_space(theme::space::XS);
        egui::CollapsingHeader::new("Trontop license and attribution").show(ui, |ui| {
            ui.label("Source available. Apache 2.0 + Commons Clause 1.0.");
            ui.label("Free personal and workplace use. Redistribution must retain the notices; sales are restricted by the license.");
            ui.hyperlink_to("Source and complete terms", "https://github.com/TrentSterling/trontop");
            if ui.button("Copy Trontop license and credit").clicked() {
                ctx.copy_text(format!("{}\n{}", include_str!("../../NOTICE"), include_str!("../../LICENSE")));
            }
        });
        egui::CollapsingHeader::new("Third-party licenses").show(ui, |ui| {
            if ui.button("Copy all third-party notices").clicked() {
                ctx.copy_text(include_str!("../../THIRD_PARTY_NOTICES.txt").into());
            }
            widgets::hover_label(ui, RichText::new("Renderer: egui-wgpu (MIT)").strong());
            ui.label(include_str!("../../vendor/egui-wgpu/LICENSE-MIT"));
            ui.add_space(theme::space::M);
            widgets::hover_label(
                ui,
                RichText::new("CPU ABI reference: System Informer (MIT)").strong(),
            );
            ui.label(include_str!("../../docs/SYSTEM_INFORMER_NOTICE.txt"));
        });
        // Breathing room so the last control is never flush with the frame.
        ui.add_space(theme::space::L);
    }

    /// One 24 px provider cell: name and a state pill; details on hover.
    fn provider_cell(
        &self,
        ui: &mut egui::Ui,
        provider: Provider,
        width: f32,
        now: Instant,
        t: Tokens,
    ) {
        let health = self.snapshot.diagnostics.get(provider);
        let state = health.state(provider, now);
        let full = provider.name();
        let short = full.split(" / ").next().unwrap_or(full);
        let timing = health.query_millis.map_or_else(
            || {
                if provider == Provider::StorageSensors {
                    "Query times shown per drive".into()
                } else {
                    "Not sampled".into()
                }
            },
            |ms| format!("{ms:.3} ms query"),
        );
        let coverage = health
            .coverage
            .map_or_else(String::new, |(present, total)| {
                format!(", {present} of {total} readable")
            });
        let mut hover = format!(
            "{full}: {}\nLast attempt {}\nLast usable {}\n{timing}{coverage}",
            state.label(),
            age(health.last_attempt, now),
            age(health.last_success, now),
        );
        if let Some(issue) = health.issue {
            hover.push('\n');
            hover.push_str(issue.description());
        }
        let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 24.0), Sense::hover());
        ui.painter().rect_filled(
            rect,
            ui.visuals().widgets.inactive.corner_radius,
            if response.hovered() {
                ui.visuals().widgets.hovered.weak_bg_fill
            } else {
                t.panel_raised
            },
        );
        let color = state_color(state, t);
        let pill = ui.painter().layout_no_wrap(
            state.label().into(),
            FontId::proportional(10.0),
            theme::ink(color, ui.visuals().dark_mode),
        );
        let pill_rect = egui::Rect::from_min_size(
            egui::pos2(
                rect.right() - 8.0 - pill.size().x - 12.0,
                rect.center().y - 9.0,
            ),
            Vec2::new(pill.size().x + 12.0, 18.0),
        );
        ui.painter().rect_filled(
            pill_rect,
            9.0,
            theme::text_surface(
                theme::mix(ui.visuals().window_fill, color, 0.15),
                ui.visuals().dark_mode,
            ),
        );
        ui.painter()
            .galley(pill_rect.center() - pill.size() / 2.0, pill, t.text);
        widgets::paint_text(
            ui,
            egui::Rect::from_min_max(
                rect.min + Vec2::new(8.0, 0.0),
                egui::pos2(pill_rect.left() - 6.0, rect.bottom()),
            ),
            short,
            FontId::proportional(12.0),
            t.text,
            Align::Min,
        );
        response.on_hover_text(hover);
    }

    /// Nothing while the provider is fresh; a single plain sentence when its
    /// readings are stale or unavailable, with the age on hover. Cards
    /// already carry their own state chips, so there is no status pill or
    /// "Last usable data" row here.
    pub(super) fn provider_notice(&self, ui: &mut egui::Ui, provider: Provider) {
        let health = self.snapshot.diagnostics.get(provider);
        let now = self.graphs.now();
        let state = health.state(provider, now);
        if !matches!(state, State::Stale | State::Unavailable) {
            return;
        }
        let t = self.colors();
        ui.add_space(theme::space::S);
        widgets::hover_label(
            ui,
            RichText::new(
                "No fresh reading: values shown are cached, and missing fields are not zero.",
            )
            .size(11.0)
            .color(t.text_muted),
        )
        .on_hover_text(format!(
            "{}: {}. Last usable data {}.",
            provider.name(),
            state.label(),
            age(health.last_success, now)
        ));
        ui.add_space(theme::space::M);
    }
}

/// A 26 px About-window row: label, value, hover text with the full value
/// (or `reason`, which says why a value is what it is).
fn about_row(ui: &mut egui::Ui, label: &str, value: &str, reason: Option<&str>, t: Tokens) {
    let frame = egui::Frame::new()
        .fill(t.panel_raised)
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(ui.visuals().widgets.inactive.corner_radius)
        .inner_margin(egui::Margin::symmetric(10, 3));
    widgets::hover_frame(ui, frame, |ui| {
        ui.set_min_width(ui.available_width());
        let width = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 18.0), Sense::hover());
        let label_width = (width * 0.4).min(140.0);
        widgets::paint_text(
            ui,
            egui::Rect::from_min_max(
                rect.min,
                egui::pos2(rect.left() + label_width, rect.bottom()),
            ),
            label,
            FontId::proportional(11.0),
            t.text_muted,
            Align::Min,
        );
        widgets::paint_text(
            ui,
            egui::Rect::from_min_max(egui::pos2(rect.left() + label_width, rect.top()), rect.max),
            value,
            FontId::monospace(11.0),
            t.text,
            Align::Max,
        );
    })
    .response
    .on_hover_text(reason.map_or_else(
        || format!("{label}: {value}"),
        |reason| format!("{label}: {value}. {reason}"),
    ));
}

pub(super) fn state_color(state: State, t: Tokens) -> Color32 {
    match state {
        State::Live => t.good,
        State::Unavailable => t.danger,
        State::Partial | State::Stale => t.text,
        State::Starting => t.text_muted,
    }
}
