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
                .status(&row.name, inventory_at, self.graphs.now())
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
                .state(crate::diagnostics::Provider::Services, self.graphs.now())
                == crate::diagnostics::State::Live
    }

    fn selected_service_row(&self) -> Option<&crate::model::ServiceRow> {
        self.snapshot
            .services
            .iter()
            .find(|row| Some(&row.name) == self.selected_service.as_ref())
    }

    /// One 34 px toolbar: [Start][Stop][Restart], the selected service (or a
    /// muted prompt), then right-aligned a status only when there is one to
    /// report, the Running only filter and [Refresh]. Identity and inventory
    /// age live in hover text on the name and Refresh.
    pub(super) fn service_controls(&mut self, ui: &mut egui::Ui) {
        let t = self.colors();
        let now = self.graphs.now();
        let row = self.selected_service_row().cloned();
        let health = self
            .snapshot
            .diagnostics
            .get(crate::diagnostics::Provider::Services);
        let refresh_hover = format!(
            "{} services / Complete {} ago. Read every service again.",
            self.snapshot.services.len(),
            crate::diagnostics::age(health.last_success, now)
        );
        let history_blocked = row
            .as_ref()
            .is_some_and(|row| !self.service_observations.can_track(&row.name));
        let width = ui.available_width();
        ui.allocate_ui_with_layout(
            Vec2::new(width, 34.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.spacing_mut().item_spacing.x = theme::space::S;
                for action in Action::ALL {
                    let refusal = row
                        .as_ref()
                        .and_then(|row| action.refusal(self.service_status(row).0));
                    let enabled = row.as_ref().is_some_and(|row| {
                        self.service_is_fresh(row)
                            && self.service_observations.can_track(&row.name)
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
                                Vec2::ZERO,
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
                        .on_hover_text(refusal.unwrap_or(
                            "Start, stop and restart ask for confirmation first. Start may start required dependencies; Stop never stops dependents; Restart can leave a service stopped if Start fails.",
                        ))
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
                ui.add_space(theme::space::M);
                let available = self.service_controller.available();
                // One muted message at a time: the selection (or its prompt)
                // while controls work, or a single "Controls unavailable"
                // with the reason on hover when they cannot.
                if available || row.is_some() {
                    let name = row.as_ref().map_or_else(
                        || "Select a service".to_string(),
                        |row| row.display_name.clone(),
                    );
                    let name_hover = row.as_ref().map_or_else(
                        || "Select a row below to enable Start, Stop and Restart.".to_string(),
                        |row| {
                            let (status, command_read) = self.service_status(row);
                            format!(
                                "{} / {} / {}{}",
                                row.name,
                                status.state.label(),
                                if status.pid == 0 {
                                    "no PID".into()
                                } else {
                                    format!("PID {}", status.pid)
                                },
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
                    ui.add(
                        egui::Label::new(RichText::new(name).size(12.0).color(
                            if row.is_some() {
                                t.text
                            } else {
                                t.text_muted
                            },
                        ))
                        .truncate(),
                    )
                    .on_hover_text(name_hover);
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let refresh = ui
                        .add_enabled_ui(
                            self.sampler.is_some() && !self.service_controller.busy(),
                            |ui| {
                                widgets::icon_button(
                                    ui,
                                    Icon::Restart,
                                    "Refresh",
                                    Vec2::ZERO,
                                    t.panel_raised,
                                    t,
                                )
                            },
                        )
                        .inner
                        .on_hover_text(&refresh_hover);
                    if refresh.clicked()
                        && let Some(sampler) = &self.sampler
                    {
                        sampler.request_service_refresh();
                    }
                    ui.checkbox(
                        &mut self.services_running_only,
                        RichText::new("Running only").size(12.0),
                    )
                    .on_hover_text("Show only services that are running now.");
                    ui.add_space(theme::space::S);
                    let status = if history_blocked {
                        Some((
                            "Command history is full. Refresh first.".to_string(),
                            t.text_muted,
                            "Command history for this service is full or incomplete. Refresh the list before sending it another command.".to_string(),
                        ))
                    } else if let Some(event) = &self.service_event {
                        let phase =
                            format!("{}: {} - {}", event.action.label(), event.name, event.phase);
                        let detail = event.error.clone().unwrap_or_else(|| {
                            if event.done {
                                "Target state observed; inventory refresh requested.".into()
                            } else {
                                "Windows is handling the command. Closing Trontop cannot undo an accepted request.".into()
                            }
                        });
                        let color = if event.error.is_some() {
                            ui.visuals().warn_fg_color
                        } else {
                            t.text_muted
                        };
                        Some((phase.clone(), color, format!("{phase}\n{detail}")))
                    } else if !available {
                        Some((
                            "Controls unavailable".to_string(),
                            t.text_muted,
                            "The service command worker is unavailable, so Start, Stop and Restart are off. The list stays readable.".to_string(),
                        ))
                    } else {
                        None
                    };
                    if let Some((text, color, hover)) = status {
                        ui.add(
                            egui::Label::new(RichText::new(text).size(11.0).color(color))
                                .truncate(),
                        )
                        .on_hover_text(hover);
                    }
                });
            },
        );
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
        let closed = widgets::action_dialog(ctx, "Confirm service command", 430.0, t, |ui| {
            widgets::hover_label(
                ui,
                RichText::new(format!("{} this service?", request.action.label()))
                    .size(18.0)
                    .strong()
                    .color(t.text),
            );
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
                if ui.button("Cancel").clicked() {
                    self.pending_service = None;
                }
                if widgets::action_button_enabled(
                    ui,
                    RichText::new("Confirm command"),
                    Vec2::new(145.0, 28.0),
                    if request.action == Action::Start {
                        t.accent_dim
                    } else {
                        theme::mix(t.panel, t.danger, 0.35)
                    },
                    t,
                    valid,
                )
                .clicked()
                {
                    let result = self.service_controller.submit(request.clone());
                    self.service_event = Some(crate::service_control::Event {
                        name: request.name.clone(),
                        action: request.action,
                        phase: if result.is_ok() {
                            "Queued"
                        } else {
                            "Not submitted"
                        },
                        observed: None,
                        command_at: None,
                        done: result.is_err(),
                        error: result.err(),
                    });
                    self.pending_service = None;
                }
            });
        });
        if closed {
            open = false;
        }
        if !open {
            self.pending_service = None;
        }
    }
}
