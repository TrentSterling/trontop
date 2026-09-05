use super::*;
use std::time::{Duration, Instant};

impl TrontopApp {
    pub(super) fn poll_preferences(&mut self, ctx: &egui::Context) {
        if let Some(loaded) = self.preferences.poll() {
            if let Some(memory) = loaded.memory {
                ctx.memory_mut(|current| *current = memory);
            }
            self.theme = loaded.theme;
            if let Some(library) = loaded.library {
                self.theme_studio.load_library(&library);
            }
            self.preferences_theme = self.theme;
            self.preferences_library_revision = self.theme_studio.revision();
            theme::install(ctx, self.theme);
        }
    }

    pub(super) fn capture_preferences(&mut self, ctx: &egui::Context) {
        if !self.preferences.enabled() || !self.preferences.can_edit() {
            return;
        }
        self.preferences.update(crate::preferences::Snapshot {
            theme: self.theme.encode(),
            library: self.theme_studio.encode_library(),
            memory: ctx.memory(Clone::clone),
        });
        self.preferences_theme = self.theme;
        self.preferences_library_revision = self.theme_studio.revision();
        self.next_memory_save = Instant::now() + Duration::from_secs(30);
    }

    pub(super) fn request_close(&mut self, ctx: &egui::Context) {
        if self.close_authorized {
            return;
        }
        if self.closing_at.is_none() {
            self.closing_at = Some(Instant::now());
            self.capture_preferences(ctx);
        }
        self.preferences.dispatch(true);
        self.finish_preferences_close(ctx);
        ctx.request_repaint();
    }

    fn finish_preferences_close(&mut self, ctx: &egui::Context) {
        if self.closing_at.is_some() && !self.preferences.pending() {
            // Before load succeeds no theme/library editing is permitted, so
            // there are no user preference changes to lose and no save to wait on.
            self.close_authorized = true;
            self.closing_at = None;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    pub(super) fn preferences_logic(&mut self, ctx: &egui::Context) {
        if ctx.input(|input| input.viewport().close_requested()) && !self.close_authorized {
            self.request_close(ctx);
            // Defer this OS close even if request_close just queued an authorized
            // programmatic close. eframe handles that Close on the next event.
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        if self.preferences.enabled()
            && self.preferences.can_edit()
            && self.closing_at.is_none()
            && Instant::now() >= self.next_memory_save
            && self.preferences.error().is_none()
        {
            self.capture_preferences(ctx);
        }
        self.preferences.dispatch(self.closing_at.is_some());
        self.finish_preferences_close(ctx);
        self.preferences.schedule(ctx);
    }

    pub(super) fn preferences_bar(&mut self, root: &mut egui::Ui) {
        if !self.preferences.enabled() {
            return;
        }
        let t = self.colors();
        egui::Panel::bottom("preferences_status").exact_size(30.0)
            .frame(egui::Frame::new().fill(t.panel).inner_margin(egui::Margin::symmetric(12, 0)))
            .show(root, |ui| {
                ui.horizontal_centered(|ui| {
                    let color = if self.preferences.error().is_some() { t.danger } else if self.preferences.pending() { t.secondary } else { t.text_muted };
                    ui.scope(|ui| widgets::status_pill(ui, self.preferences.status(), color)).response.on_hover_text(
                        self.preferences.error().unwrap_or("Theme, named palettes and UI memory save in this Windows account. Saving never waits on disk from the UI thread."));
                    if self.preferences.error().is_some() && ui.add_enabled(self.preferences.can_retry(), egui::Button::new("Retry save / load").small()).clicked() { self.preferences.retry(); }
                    if self.preferences.slow() {
                        widgets::hover_label(ui, RichText::new("Storage is taking longer; the app remains usable.").size(11.0).color(t.text_muted));
                    }
                });
            });
    }

    pub(super) fn preferences_close_dialog(&mut self, ctx: &egui::Context) {
        let Some(at) = self.closing_at else {
            return;
        };
        if self.preferences.error().is_none() && at.elapsed() < Duration::from_millis(250) {
            ctx.request_repaint_after(Duration::from_millis(250));
            return;
        }
        let t = self.colors();
        egui::Modal::new(egui::Id::new("preferences_close"))
          .frame(egui::Frame::new().fill(t.panel).stroke(Stroke::new(1.0, t.border))
              .corner_radius(10).inner_margin(egui::Margin::same(16)))
          .show(ctx, |ui| {
            ui.set_width(460.0_f32.min(ctx.content_rect().width() - 64.0));
            widgets::hover_label(ui, RichText::new("Settings are not saved yet").size(20.0).strong().color(t.text));
            ui.add_space(8.0);
            widgets::hover_label(ui, RichText::new(self.preferences.error().unwrap_or("Waiting for storage. This window will close when saving succeeds.")).color(t.text_muted));
            ui.add_space(8.0);
            widgets::hover_label(ui, RichText::new("Close anyway can lose the latest theme or named palettes. A write already in progress may still finish; it is not rolled back.").color(t.text_muted));
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if widgets::action_button(ui, "Keep open", Vec2::new(108.0, 30.0), t.accent_dim, t).clicked() { self.closing_at = None; }
                if ui.add_enabled(self.preferences.error().is_some() && self.preferences.can_retry(), egui::Button::new("Retry save").min_size(Vec2::new(100.0, 30.0))).clicked() { self.preferences.retry(); }
                if widgets::action_button(ui, "Close anyway", Vec2::new(128.0, 30.0), theme::mix(t.panel, t.danger, 0.2), t).clicked() {
                    self.close_authorized = true;
                    self.closing_at = None;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
        });
    }
}
