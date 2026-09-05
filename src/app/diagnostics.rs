use super::*;
use crate::diagnostics::{Provider, State, age, support_report};
use std::time::Instant;

impl TrontopApp {
    pub(super) fn diagnostics_window(&mut self, ctx: &egui::Context) {
        if !self.show_diagnostics {
            return;
        }
        let t = self.colors();
        let mut open = true;
        egui::Window::new("About Trontop")
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .default_width(620.0)
            .default_height((ctx.content_rect().height() - 120.0).max(240.0))
            .max_height((ctx.content_rect().height() - 100.0).max(240.0))
            .vscroll(true).resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    widgets::tront_mark(ui, t.accent, t.secondary, 40.0);
                    ui.vertical(|ui| {
                        widgets::hover_label(ui, RichText::new("Trontop").size(24.0).strong());
                        widgets::hover_label(ui, "Built by Trent Sterling / tront.xyz");
                    });
                });
                ui.add_space(10.0);
                widgets::detail_row(ui, "Version", env!("CARGO_PKG_VERSION"), t);
                widgets::detail_row(ui, "Build", env!("TRONTOP_BUILD_ID"), t);
                widgets::detail_row(ui, "Target", env!("TRONTOP_BUILD_TARGET"), t);
                widgets::detail_row(ui, "Profile", if cfg!(debug_assertions) { "Debug" } else { "Optimized release" }, t);
                ui.add_space(10.0);
                if ui.button("Copy support report").clicked() {
                    ctx.copy_text(support_report(&self.snapshot.diagnostics, Instant::now()));
                    self.message = Some(("Support report copied. Nothing uploaded.".into(), false));
                }
                widgets::hover_label(ui, RichText::new("Build and provider status only. No process names, commands, paths, account/host names, GPU IDs or addresses.").size(11.0).color(t.text_muted));
                ui.add_space(12.0);
                widgets::section_label(ui, "Local failure log", t);
                widgets::hover_frame(ui, widgets::surface(ui, t, true), |ui| {
                    ui.set_min_width(ui.available_width());
                    widgets::hover_label(ui, RichText::new("Keeps up to 32 local failure records. Nothing uploaded.").size(12.0).strong());
                    widgets::hover_label(ui, RichText::new(crate::failure::LOCATION_HINT).monospace().size(11.0));
                    if ui.button("Copy log location").clicked() {
                        ctx.copy_text(crate::failure::LOCATION_HINT.into());
                        self.message = Some(("Log location copied. Paste into Explorer; the file exists only after a recorded failure.".into(), false));
                    }
                    widgets::hover_label(ui, RichText::new("Rust panics, native-runner errors and GPU recovery events; no raw messages or memory dumps. Forced exits, hangs and native crashes may leave no record. File access failures can also prevent logging.").size(11.0).color(t.text_muted));
                });
                egui::CollapsingHeader::new("Renderer license (egui-wgpu / MIT)").show(ui, |ui| {
                    ui.label(include_str!("../../vendor/egui-wgpu/LICENSE-MIT"));
                });
                ui.add_space(12.0);
                widgets::section_label(ui, "Provider health", t);
                let now = Instant::now();
                for (index, provider) in Provider::ALL.into_iter().enumerate() {
                    let health = self.snapshot.diagnostics.get(provider);
                    let state = health.state(provider, now);
                    widgets::hover_frame(ui, widgets::surface(ui, t, index % 2 == 1), |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.horizontal(|ui| {
                            widgets::hover_label(ui, RichText::new(provider.name()).strong().size(13.0));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                widgets::status_pill(ui, state.label(), state_color(state, t));
                            });
                        });
                        ui.horizontal_wrapped(|ui| {
                            widgets::hover_label(ui, RichText::new(format!("Attempt {}", age(health.last_attempt, now))).size(11.0).monospace());
                            widgets::hover_label(ui, RichText::new(format!("Usable {}", age(health.last_success, now))).size(11.0).monospace());
                        });
                        let timing = health.query_millis.map_or_else(|| if provider == Provider::StorageSensors { "Query times shown per drive".into() } else { "Not sampled".into() }, |ms| format!("{ms:.3} ms query"));
                        let coverage = health.coverage.map_or_else(String::new, |(present, total)| format!(" / {present} of {total} readable"));
                        widgets::hover_label(ui, RichText::new(format!("{timing}{coverage}")).size(11.0).color(t.text_muted));
                        if let Some(issue) = health.issue { widgets::hover_label(ui, RichText::new(issue.description()).size(11.0)); }
                    });
                    ui.add_space(5.0);
                }
            });
        self.show_diagnostics &= open;
    }

    pub(super) fn provider_notice(&self, ui: &mut egui::Ui, provider: Provider) {
        let health = self.snapshot.diagnostics.get(provider);
        let now = Instant::now();
        let state = health.state(provider, now);
        let t = self.colors();
        widgets::hover_frame(ui, widgets::surface(ui, t, true), |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                widgets::status_pill(ui, state.label(), state_color(state, t));
                widgets::hover_label(
                    ui,
                    RichText::new(format!(
                        "Last usable data: {}",
                        age(health.last_success, now)
                    ))
                    .size(11.0),
                );
            });
            if matches!(state, State::Stale | State::Unavailable) {
                widgets::hover_label(ui, RichText::new("No fresh reading. Cached values are not live; missing fields are not zero.").size(11.0));
            }
        });
        ui.add_space(8.0);
    }
}

pub(super) fn state_color(state: State, t: Tokens) -> Color32 {
    match state {
        State::Live => t.good,
        State::Unavailable => t.danger,
        State::Partial | State::Stale => t.text,
        State::Starting => t.text_muted,
    }
}
