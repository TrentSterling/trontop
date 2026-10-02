use super::*;
use crate::startup::control::{Action, Key, Request, State};

impl TrontopApp {
    fn selected_startup_row(
        &self,
    ) -> Option<(std::borrow::Cow<'_, crate::model::StartupRow>, bool)> {
        let (source, entry) = self
            .snapshot
            .startup
            .rows()
            .find(|(_, entry)| Some(&Key::of(&entry.row)) == self.selected_startup.as_ref())?;
        let (row, _, fresh) = self
            .startup_observations
            .view(source, entry, self.graphs.now());
        Some((row, fresh))
    }
    pub(super) fn startup_controls(&mut self, ui: &mut egui::Ui) {
        let t = self.colors();
        let selected = self.selected_startup_row();
        let ready = self.startup_controller.ready();
        let editable = selected.as_ref().is_some_and(|(row, fresh)| {
            *fresh
                && row
                    .control
                    .as_ref()
                    .is_some_and(|control| control.editable())
        });
        let mut staged = None;
        let mut undo_clicked = false;
        ui.allocate_ui_with_layout(Vec2::new(ui.available_width(), 32.0), Layout::left_to_right(Align::Center), |ui| {
            for (label, action, state) in [("Enable", Action::Enable, State::Disabled), ("Disable", Action::Disable, State::Enabled)] {
                let enabled = ready && editable && selected.as_ref().is_some_and(|(row, _)| row.control.as_ref().is_some_and(|control| control.approval.state() == state));
                if ui.add_enabled(enabled, egui::Button::new(label)).on_disabled_hover_text("Select a fresh entry with a recognized Windows approval state.").clicked() {
                    staged = Some(action);
                }
            }
            let undo = selected.as_ref().is_some_and(|(row, _)| self.startup_observations.can_undo(row));
            if ui.add_enabled(ready && editable && undo, egui::Button::new("Undo")).on_hover_text("Restore the exact approval record from this entry's last change in this session.").clicked() {
                undo_clicked = true;
            }
            ui.add_space(theme::space::S);
            let name = selected.as_ref().map_or("Select a startup entry", |(row, _)| row.name.as_str());
            ui.add(egui::Label::new(RichText::new(name).color(t.text_muted)).truncate());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add_enabled(self.sampler.is_some() && !self.startup_controller.busy(), egui::Button::new("Refresh")).on_hover_text("Read all five startup sources again in the background.").clicked()
                    && let Some(sampler) = &self.sampler { sampler.request_startup_refresh(); }
                if self.startup_controller.busy() { ui.label(RichText::new("Changing startup...").color(t.text_muted)); }
            });
        });
        if let Some((row, _)) = selected {
            if undo_clicked {
                self.pending_startup = self.startup_observations.undo(&row);
            } else if let Some(action) = staged {
                self.pending_startup = Some(Request::new(row.into_owned(), action));
            }
        }
    }
    pub(super) fn poll_startup_command(&mut self) {
        if let Some(outcome) = self.startup_controller.poll() {
            self.startup_observations.apply(&outcome);
            self.message = Some(match &outcome.result {
                Ok(_) => (
                    format!(
                        "{}: {}. Approval verified; takes effect at your next sign-in.",
                        outcome.request.action.label(),
                        outcome.request.target.name
                    ),
                    false,
                ),
                Err(error) => (
                    format!("{}: {}", outcome.request.target.name, error.message),
                    true,
                ),
            });
            if let Some(sampler) = &self.sampler {
                sampler.request_startup_refresh();
            }
        }
        self.startup_observations.reconcile(&self.snapshot.startup);
    }
    pub(super) fn confirm_startup_command(&mut self, ctx: &egui::Context) {
        let Some(request) = self.pending_startup.take() else {
            return;
        };
        let t = self.colors();
        let current = self
            .snapshot
            .startup
            .rows()
            .find(|(_, entry)| Key::of(&entry.row) == Key::of(&request.target))
            .is_some_and(|(source, entry)| {
                let (row, _, fresh) =
                    self.startup_observations
                        .view(source, entry, self.graphs.now());
                fresh
                    && row.command == request.target.command
                    && row.control == request.target.control
            });
        let mut cancel = false;
        let mut submitted = false;
        let closed = widgets::action_dialog(ctx, "Confirm startup change", 510.0, t, |ui| {
            widgets::identity_card(ui, &request.target.name, request.target.source.name(), t);
            ui.add(
                egui::Label::new(
                    RichText::new(&request.target.command)
                        .size(11.0)
                        .color(t.text_muted),
                )
                .truncate(),
            )
            .on_hover_text(&request.target.command);
            let label = match &request.action {
                Action::Enable => "Allow this registration at the next sign-in?",
                Action::Disable => "Disable this registration at the next sign-in?",
                Action::Restore(_) => "Restore this entry's previous approval record?",
            };
            widgets::hover_label(ui, RichText::new(label).strong().color(t.text));
            widgets::hover_label(
                ui,
                RichText::new(
                    "The command or shortcut is preserved. Programs already running stay open.",
                )
                .color(t.text_muted),
            );
            if matches!(
                request.target.source,
                crate::startup::Source::MachineRun
                    | crate::startup::Source::MachineRun32
                    | crate::startup::Source::MachineFolder
            ) {
                widgets::hover_label(ui, RichText::new("Machine registration: this change can affect other users and requires existing write access.").color(ui.visuals().warn_fg_color));
            }
            let valid = current && request.validate().is_ok() && self.startup_controller.ready();
            if !current {
                widgets::hover_label(
                    ui,
                    RichText::new("Entry or approval changed. Refresh and review it again.")
                        .color(t.ink(t.danger)),
                );
            } else if let Err(error) = request.validate() {
                widgets::hover_label(ui, RichText::new(error).color(t.ink(t.danger)));
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                cancel = ui.button("Cancel").clicked();
                if ui
                    .add_enabled(valid, egui::Button::new(request.action.label()))
                    .clicked()
                {
                    match self.startup_controller.submit(request.clone()) {
                        Ok(()) => submitted = true,
                        Err(error) => self.message = Some((error, true)),
                    }
                }
            });
        });
        if !(closed || cancel || submitted) {
            self.pending_startup = Some(request);
        }
    }
}
