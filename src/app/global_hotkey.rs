//! App side of the global show/hide hotkey: acting on a press, applying a new
//! choice live, and the About-window control. The thread and the Win32 calls live
//! in `crate::hotkey`.
use super::*;
use crate::hotkey::{Action, Choice};

impl TrontopApp {
    /// Runs from `logic`, so a press is handled even while the window is hidden
    /// and `ui` is skipped. The listener repaints once per press; this never
    /// asks for a repaint timer of its own.
    pub(super) fn hotkey_logic(&mut self, ctx: &egui::Context) {
        if !self
            .hotkey
            .as_ref()
            .is_some_and(|hotkey| hotkey.take_press())
        {
            return;
        }
        let (minimized, focused) = ctx.input(|input| {
            let viewport = input.viewport();
            (viewport.minimized, viewport.focused)
        });
        // `hidden_to_tray` is the real "hidden" state. egui's `visible()` only
        // means "not minimized or occluded", so it cannot stand in for it.
        match crate::hotkey::action(
            !self.hidden_to_tray,
            minimized == Some(true),
            focused == Some(true),
        ) {
            // Exactly what the tray's Show does: Visible, Minimized(false), Focus.
            Action::Show => self.show_window(ctx, true),
            // Exactly what the title-bar X does, including its refusal (and
            // message) when the tray is not ready to hold the window.
            Action::Hide => self.request_window_close(ctx),
        }
    }

    /// Apply a choice from the UI and save it with the other preferences.
    pub(super) fn change_hotkey(&mut self, ctx: &egui::Context, choice: Choice) {
        self.apply_hotkey(choice);
        self.capture_preferences(ctx);
    }

    /// Re-register live (release the old chord, take the new one). Choosing the
    /// current combo again retries it, which recovers from "in use".
    pub(super) fn apply_hotkey(&mut self, choice: Choice) {
        self.hotkey_choice = choice;
        if let Some(hotkey) = &self.hotkey {
            hotkey.set(choice);
        }
    }

    /// The notice for the current choice, if its registration failed.
    pub(super) fn hotkey_notice(&self) -> Option<String> {
        let report = self.hotkey.as_ref()?.report();
        // A late answer for a previous choice must not describe this one.
        (report.choice == self.hotkey_choice)
            .then(|| report.notice())
            .flatten()
    }

    /// The About cell beside the tray state: label and the choice. It matches
    /// the 26 px rows around it so About keeps its height.
    pub(super) fn hotkey_cell(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, t: Tokens) {
        let editable = self.preferences.can_edit();
        let hover = "Press this anywhere to show Trontop, or to hide it to the tray when it is already in front. Works while Trontop runs, including from the tray.";
        let frame = egui::Frame::new()
            .fill(t.panel_raised)
            .stroke(Stroke::new(1.0, t.border))
            .corner_radius(ui.visuals().widgets.inactive.corner_radius)
            .inner_margin(egui::Margin::symmetric(10, 3));
        let mut chosen = None;
        widgets::hover_frame(ui, frame, |ui| {
            ui.set_min_width(ui.available_width());
            // An 18 px content row, like the painted rows beside it.
            ui.set_height(18.0);
            ui.spacing_mut().interact_size.y = 18.0;
            ui.spacing_mut().button_padding.y = 1.0;
            ui.horizontal_centered(|ui| {
                widgets::hover_label(
                    ui,
                    RichText::new("Show / hide hotkey")
                        .size(11.0)
                        .color(t.text_muted),
                )
                .on_hover_text(hover);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_enabled_ui(editable, |ui| {
                        egui::ComboBox::from_id_salt("global_hotkey_choice")
                            .selected_text(
                                RichText::new(self.hotkey_choice.label())
                                    .monospace()
                                    .size(11.0),
                            )
                            .width(112.0)
                            .show_ui(ui, |ui| {
                                for choice in Choice::ALL {
                                    let selected = choice == self.hotkey_choice;
                                    if ui.selectable_label(selected, choice.label()).clicked() {
                                        chosen = Some(choice);
                                    }
                                }
                            })
                            .response
                            .on_hover_text(hover)
                            .on_disabled_hover_text("Settings are still loading.");
                    });
                });
            });
        });
        if let Some(choice) = chosen {
            self.change_hotkey(ctx, choice);
        }
    }

    /// One line under the row, only while the chosen combo could not be taken.
    pub(super) fn hotkey_notice_line(&self, ui: &mut egui::Ui, t: Tokens) {
        if let Some(notice) = self.hotkey_notice() {
            widgets::hover_label(ui, RichText::new(notice).size(11.0).color(t.danger))
                .on_hover_text(
                    "Pick another combination, or Off. Selecting this one again retries it.",
                );
        }
    }
}
