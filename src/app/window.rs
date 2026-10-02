use super::*;
use crate::tray::TrayState;

impl TrontopApp {
    pub(super) fn request_window_close(&mut self, ctx: &egui::Context) {
        if self.close_authorized || self.closing_at.is_some() || self.hidden_to_tray {
            return;
        }
        if self.tray.as_ref().map(TrayController::state) != Some(TrayState::Ready) {
            self.message = Some((
                "System tray is not ready; the window stays open. Use About > Quit Trontop to exit."
                    .into(),
                true,
            ));
            ctx.request_repaint();
            return;
        }
        // Hiding is immediate even if a save is slow. The app and writer remain
        // alive; only explicit Quit uses the durable-save exit gate.
        self.capture_preferences(ctx);
        self.preferences.dispatch(true);
        self.hidden_to_tray = true;
        if let Some(sampler) = &self.sampler {
            sampler.set_ui_visible(false);
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
    }

    pub(super) fn show_window(&mut self, ctx: &egui::Context, focus: bool) {
        self.hidden_to_tray = false;
        if let Some(sampler) = &self.sampler {
            sampler.set_ui_visible(true);
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        if focus {
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        ctx.request_repaint();
    }

    pub(super) fn tray_logic(&mut self, ctx: &egui::Context) {
        if let Some(action) = self.tray.as_ref().and_then(TrayController::poll) {
            match action {
                TrayAction::Show => self.show_window(ctx, true),
                TrayAction::Quit => self.request_quit(ctx),
            }
        }
        if self.hidden_to_tray
            && !self.close_authorized
            && self.tray.as_ref().map(TrayController::state) != Some(TrayState::Ready)
        {
            // The worker repaints on failure transitions even while sample
            // repaints are disabled. Restore access without taking keyboard focus.
            self.show_window(ctx, false);
            self.message = Some((
                "System tray became unavailable; Trontop reopened so you can still reach it."
                    .into(),
                true,
            ));
        }
    }
}
