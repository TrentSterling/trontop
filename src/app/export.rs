use super::*;
use crate::export::{Capture, Format, Outcome};

impl TrontopApp {
    pub(super) fn export_window(&mut self, ctx: &egui::Context) {
        if !self.show_export {
            return;
        }
        let t = self.colors();
        if self.exporter.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
        let mut open = true;
        // One region for the whole body, not a second nested scroll area that
        // squeezes the "Limited details" box: the window sizes itself to its
        // (now short, one-sentence-per-block) content instead of scrolling it.
        // A `vscroll(true)`/wrapping `ScrollArea` here under-sizes the window
        // and stays stuck there; this shape does not.
        egui::Window::new("Export snapshot").open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .default_width(580.0).resizable(false).collapsible(false)
            .frame(egui::Frame::window(&ctx.style_of(ctx.theme())).inner_margin(theme::CARD_PAD))
            .show(ctx, |ui| {
                widgets::hover_label(ui, RichText::new("Take the data with you").size(22.0).strong().color(t.text));
                widgets::hover_label(ui, "A fixed capture when you press Save as; live sampling continues.");
                ui.add_space(theme::space::L);
                ui.add_enabled_ui(!self.exporter.busy(), |ui| {
                    widgets::control_row(ui, "Format", false, t, |ui| {
                        ui.selectable_value(&mut self.export_options.format, Format::Json, "JSON snapshot");
                        ui.selectable_value(&mut self.export_options.format, Format::Csv, "CSV processes");
                    });
                    ui.add_space(theme::space::S);
                    widgets::control_row(ui, "Details", true, t, |ui| {
                        ui.checkbox(&mut self.export_options.private_details, "Include private details");
                    });
                });
                ui.add_space(theme::space::L);
                let (scope, scope_hover) = if self.export_options.format == Format::Json {
                    (
                        "Includes system, process, disk, network, sensor, user, Startup and Service data.",
                        "Provider freshness travels with the readings.",
                    )
                } else {
                    (
                        "One row per process, with raw counters and units.",
                        "Missing GPU state is flagged; CSV text is escaped for spreadsheet import.",
                    )
                };
                widgets::hover_label(ui, scope).on_hover_text(scope_hover);
                widgets::hover_label(ui, RichText::new("All sampler rows, not just the filtered view.").size(11.0).color(t.text_muted))
                    .on_hover_text("No chart history, subtree totals or service-command annotations are included.");
                ui.add_space(theme::space::L);
                widgets::hover_frame(ui, widgets::surface(ui, t, false), |ui| {
                    widgets::hover_label(ui, RichText::new(if self.export_options.private_details { "Private details included" } else { "Limited details; not anonymous" }).strong().color(t.text));
                    let (body, body_hover) = if self.export_options.private_details {
                        (
                            "Includes accounts, host, paths, commands, adapters and hardware IDs.",
                            "Review before sharing outside this machine.",
                        )
                    } else {
                        (
                            "Excludes accounts, host, paths, commands and hardware IDs.",
                            "Process, service and device names still identify installed software; this is not anonymous.",
                        )
                    };
                    widgets::hover_label(ui, body).on_hover_text(body_hover);
                });
                ui.add_space(theme::space::L);
                let (state, detail) = if self.exporter.busy() {
                    ("In progress", "Choose a file in Windows Save As, or cancel that dialog. File writing runs in the background.".into())
                } else { match &self.export_result {
                    Some(Outcome::Saved { path, bytes, sequence }) => ("Saved", format!("Sample {sequence} / {} / {}", format::bytes(*bytes), path.display())),
                    Some(Outcome::Cancelled) => ("Cancelled", "No export was saved.".into()),
                    Some(Outcome::Failed(error)) => ("Failed", error.clone()),
                    None if self.snapshot.sequence == 0 => ("Waiting for sample", "The fields remain here. Save becomes available after the first system sample.".into()),
                    None if !self.exporter.available() => ("Unavailable", "Native export is unavailable in this session.".into()),
                    None => ("Ready", format!("{} process rows in sample {}. Nothing is written until you choose a destination.", self.snapshot.processes.len(), self.snapshot.sequence)),
                }};
                widgets::inventory_status(ui, "Export status", state, &detail, t.text, t, true);
                ui.add_space(theme::space::L);
                ui.horizontal(|ui| {
                    if ui.button("Close panel").clicked() { self.show_export = false; }
                    let enabled = !self.exporter.busy() && self.exporter.available() && self.snapshot.sequence > 0;
                    if widgets::action_button_enabled(ui, RichText::new("Save as..."), Vec2::new(140.0, 28.0), t.accent_dim, t, enabled).clicked() {
                        let capture = Capture::new(self.snapshot.clone(), self.export_options);
                        self.export_result = self.exporter.submit(capture, ctx.clone()).err().map(Outcome::Failed);
                    }
                });
                widgets::hover_label(ui, RichText::new("Closing this panel does not cancel an export in progress.").size(10.0).color(t.text_muted))
                    .on_hover_text("Nothing is uploaded or opened automatically.");
            });
        if !open {
            self.show_export = false;
        }
    }
}
