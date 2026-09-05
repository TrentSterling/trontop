use super::*;
use crate::service_control::{Action, Request, Status};
use std::time::Instant;

impl TrontopApp {
    pub(super) fn service_status(&self, row: &crate::model::ServiceRow) -> (Status, bool) {
        let inventory_at = self
            .snapshot
            .diagnostics
            .get(crate::diagnostics::Provider::Services)
            .last_success;
        if let Some(status) =
            self.service_observations
                .status(&row.name, inventory_at, Instant::now())
        {
            return (status, true);
        }
        (row.status, false)
    }

    pub(super) fn service_is_fresh(&self, row: &crate::model::ServiceRow) -> bool {
        let inventory_at = self
            .snapshot
            .diagnostics
            .get(crate::diagnostics::Provider::Services)
            .last_success;
        if self.service_observations.uncertain(&row.name, inventory_at) {
            return false; // A pre-command inventory cannot resolve an unknown outcome.
        }
        self.service_status(row).1
            || self
                .snapshot
                .diagnostics
                .get(crate::diagnostics::Provider::Services)
                .state(crate::diagnostics::Provider::Services, Instant::now())
                == crate::diagnostics::State::Live
    }

    fn selected_service_row(&self) -> Option<&crate::model::ServiceRow> {
        self.snapshot
            .services
            .iter()
            .find(|row| Some(&row.name) == self.selected_service.as_ref())
    }

    pub(super) fn service_controls(&mut self, ui: &mut egui::Ui) {
        let t = self.colors();
        let row = self.selected_service_row().cloned();
        let title = row
            .as_ref()
            .map_or("Select a service", |row| row.display_name.as_str());
        let mut detail = row.as_ref().map_or_else(
            || "Commands use current Windows permissions; no automatic elevation.".into(),
            |row| {
                let (status, command_read) = self.service_status(row);
                format!(
                    "{} / {} / PID {}{}",
                    row.name,
                    status.state.label(),
                    status.pid,
                    if command_read {
                        " / latest command read"
                    } else if !self.service_is_fresh(row) {
                        " / cached inventory; refresh before controlling"
                    } else {
                        ""
                    }
                )
            },
        );
        if row
            .as_ref()
            .is_some_and(|row| !self.service_observations.can_track(&row.name))
        {
            detail = "Command history is full or incomplete. Refresh list before another command to this service.".into();
        }
        widgets::inventory_status(ui, title, "Service control", &detail, t.text, t, true);
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            for action in Action::ALL {
                let refusal = row
                    .as_ref()
                    .and_then(|row| action.refusal(self.service_status(row).0));
                let enabled = row.as_ref().is_some_and(|row| {
                    self.service_is_fresh(row) && self.service_observations.can_track(&row.name)
                }) && refusal.is_none()
                    && !self.service_controller.busy()
                    && self.service_controller.available();
                let icon = match action {
                    Action::Start => Icon::Startup,
                    Action::Stop => Icon::Stop,
                    Action::Restart => Icon::Restart,
                };
                let response = ui
                    .add_enabled_ui(enabled, |ui| {
                        widgets::icon_button(
                            ui,
                            icon,
                            action.label(),
                            Vec2::new(95.0, 28.0),
                            if action == Action::Stop {
                                theme::mix(t.panel, t.danger, 0.16)
                            } else {
                                t.panel_raised
                            },
                            t,
                        )
                    })
                    .inner;
                if response
                    .on_hover_text(
                        refusal
                            .unwrap_or("Review a confirmation before any service command is sent."),
                    )
                    .clicked()
                    && let Some(row) = &row
                {
                    self.pending_service = Some(Request {
                        name: row.name.clone(),
                        display_name: row.display_name.clone(),
                        expected: self.service_status(row).0,
                        action,
                        staged_at: Instant::now(),
                    });
                }
            }
            if ui
                .add_enabled(
                    self.sampler.is_some() && !self.service_controller.busy(),
                    egui::Button::new("Refresh list"),
                )
                .clicked()
                && let Some(sampler) = &self.sampler
            {
                sampler.request_service_refresh();
            }
        });
        ui.add_space(6.0);
        let (title, phase, detail, color) = if let Some(event) = &self.service_event {
            (format!("{}: {}", event.action.label(), event.name), event.phase,
                event.error.clone().unwrap_or_else(|| if event.done {
                    "Target state observed; inventory refresh requested.".into()
                } else { "Windows is handling the command. Closing Trontop cannot undo an accepted request.".into() }),
                if event.error.is_some() { t.danger } else { t.text })
        } else {
            ("Command status".into(), if self.service_controller.available() { "Ready" } else { "Unavailable" },
                if self.service_controller.available() { "Start may start required dependencies. Stop never recursively stops dependents; Restart can leave a service stopped if Start fails." }
                else { "The service command worker is unavailable; inventory remains readable." }.into(), t.text_muted)
        };
        widgets::inventory_status(ui, &title, phase, &detail, color, t, false);
    }

    pub(super) fn poll_service_command(&mut self) {
        self.service_observations.reconcile(
            self.snapshot
                .diagnostics
                .get(crate::diagnostics::Provider::Services)
                .last_success,
        );
        if let Some(event) = self.service_controller.poll() {
            self.service_observations.record(&event);
            if event.done
                && let Some(sampler) = &self.sampler
            {
                sampler.request_service_refresh();
            }
            self.service_event = Some(event);
        }
    }

    pub(super) fn confirm_service_command(&mut self, ctx: &egui::Context) {
        let Some(request) = self.pending_service.clone() else {
            return;
        };
        let t = self.colors();
        let same = self
            .snapshot
            .services
            .iter()
            .find(|row| row.name == request.name)
            .is_some_and(|row| {
                let status = self.service_status(row).0;
                self.service_is_fresh(row)
                    && status.state == request.expected.state
                    && status.pid == request.expected.pid
            });
        let valid = same
            && self.service_observations.can_track(&request.name)
            && request.validate().is_ok()
            && self.service_controller.available()
            && !self.service_controller.busy();
        let mut open = true;
        egui::Window::new("Confirm service command").open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO).collapsible(false).resizable(false)
            .show(ctx, |ui| {
                ui.set_width(430.0);
                widgets::hover_label(ui, RichText::new(format!("{} this service?", request.action.label())).size(18.0).strong().color(t.text));
                widgets::identity_card(ui, &request.display_name, &request.name, t);
                widgets::detail_row(ui, "Reported state", request.expected.state.label(), t);
                widgets::hover_label(ui, RichText::new(match request.action {
                    Action::Start => "Windows may start required dependencies. Disabled startup settings are not changed.",
                    Action::Stop => "Stopping a service can interrupt applications, network connections or system functions. Running dependents will not be stopped automatically.",
                    Action::Restart => "Stop, wait for Stopped, then Start. This interrupts service users and can leave the service stopped if Start fails or Trontop closes midway.",
                }).color(t.text));
                widgets::hover_label(ui, RichText::new(if valid {
                    "Current state and host PID are rechecked on the service handle. Confirmation expires in 30 seconds."
                } else if !self.service_observations.can_track(&request.name) {
                    "Command history is full or incomplete. Cancel and refresh the service list."
                } else { "The confirmation expired or the service changed/is unavailable. Cancel and select again." }).size(11.0).color(t.text));
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() { self.pending_service = None; }
                    if widgets::action_button_enabled(ui, RichText::new("Confirm command"), Vec2::new(145.0, 28.0),
                        if request.action == Action::Start { t.accent_dim } else { theme::mix(t.panel, t.danger, 0.35) }, t, valid).clicked() {
                        let result = self.service_controller.submit(request.clone());
                        self.service_event = Some(crate::service_control::Event {
                            name: request.name.clone(), action: request.action, phase: if result.is_ok() { "Queued" } else { "Not submitted" },
                            observed: None, command_at: None, done: result.is_err(), error: result.err(),
                        });
                        self.pending_service = None;
                    }
                });
            });
        if !open {
            self.pending_service = None;
        }
    }
}
