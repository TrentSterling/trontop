use super::*;
use crate::diagnostics::{Provider, State, age};
use std::time::Instant;

impl TrontopApp {
    pub(super) fn startup_inventory(&self, ui: &mut egui::Ui) {
        use crate::startup::State as SourceState;
        let t = self.colors();
        let now = Instant::now();
        let snapshot = &self.snapshot.startup;
        let total = snapshot.rows().count();
        let (checked, sources) = snapshot.coverage();
        let health = self.snapshot.diagnostics.get(Provider::Startup);
        let state = health.state(Provider::Startup, now);
        widgets::inventory_status(
            ui,
            "Startup inventory",
            state.label(),
            &format!(
                "{total} retained entries / {checked} of {sources} sources fully checked. Cached is not current; Observed is from an incomplete read.{}",
                health
                    .issue
                    .map_or(String::new(), |issue| format!(" {}", issue.description()))
            ),
            diagnostics::state_color(state, t),
            t,
            false,
        );
        ui.add_space(6.0);
        ui.columns(2, |columns| {
            for (index, source) in snapshot.sources.iter().enumerate() {
                let ui = &mut columns[index % 2];
                let state = source.state(now);
                let color = match state {
                    SourceState::Live | SourceState::Empty | SourceState::Absent => t.good,
                    SourceState::Unavailable => t.danger,
                    SourceState::Starting => t.text_muted,
                    _ => t.text,
                };
                let detail = format!(
                    "{} entries / Complete {}{}",
                    source.entries.len(),
                    age(source.last_complete, now),
                    if source.retention_limited {
                        " / Retention limit reached"
                    } else {
                        ""
                    }
                );
                widgets::inventory_status(
                    ui,
                    source.source.name(),
                    state.label(),
                    &detail,
                    color,
                    t,
                    index % 2 == 1,
                );
                ui.add_space(4.0);
            }
        });
        ui.add_space(6.0);
        let needle = self.secondary_query.trim().to_lowercase();
        let rows = snapshot
            .rows()
            .filter(|(source, entry)| {
                needle.is_empty()
                    || entry.row.name.to_lowercase().contains(&needle)
                    || entry.row.command.to_lowercase().contains(&needle)
                    || source.source.name().to_lowercase().contains(&needle)
            })
            .collect::<Vec<_>>();
        let shown = rows.len();
        widgets::inventory_table(
            ui,
            "startup_grid",
            ["NAME", "COMMAND / FILE", "SOURCE", "FRESHNESS"],
            shown,
            None,
            |index| {
                let (source, entry) = rows[index];
                [
                    entry.row.name.clone(),
                    entry.row.command.clone(),
                    source.source.name().into(),
                    format!(
                        "{} / {}",
                        source.row_state(entry, now),
                        age(Some(entry.observed_at), now)
                    ),
                ]
            },
            t,
        );
        widgets::hover_label(ui, RichText::new(format!("{shown} matching of {total} retained entries. Read-only inventory; enabled/disabled state is not inferred.")).size(10.0).color(t.text_muted));
    }

    pub(super) fn service_inventory(&mut self, ui: &mut egui::Ui) {
        let t = self.colors();
        let now = Instant::now();
        let health = self.snapshot.diagnostics.get(Provider::Services);
        let state = health.state(Provider::Services, now);
        let freshness = match state {
            State::Live => "Live",
            State::Starting => "Starting",
            _ if health.last_success.is_some() => "Cached",
            _ => "Unavailable",
        };
        let explanation = match freshness {
            "Live" => "Reported states reflect the last complete read.",
            "Cached" => "Showing the last complete list; reported service states may have changed.",
            "Starting" => "Waiting for the first inventory; the table stays in place.",
            _ => "No complete service inventory is available. Nothing is assumed stopped.",
        };
        widgets::inventory_status(
            ui,
            "Service inventory",
            freshness,
            &format!(
                "{} entries / Complete {}. {explanation}{}",
                self.snapshot.services.len(),
                age(health.last_success, now),
                health
                    .issue
                    .map_or(String::new(), |issue| format!(" {}", issue.description()))
            ),
            diagnostics::state_color(state, t),
            t,
            false,
        );
        ui.add_space(6.0);
        self.service_controls(ui);
        ui.add_space(12.0);
        let needle = self.secondary_query.trim().to_lowercase();
        let rows = self
            .snapshot
            .services
            .iter()
            .filter(|row| {
                needle.is_empty()
                    || row.name.to_lowercase().contains(&needle)
                    || row.display_name.to_lowercase().contains(&needle)
            })
            .collect::<Vec<_>>();
        let shown = rows.len();
        let selected = rows
            .iter()
            .position(|row| self.selected_service.as_ref() == Some(&row.name));
        let clicked = widgets::inventory_table(
            ui,
            "services_grid",
            [
                "DISPLAY NAME",
                "SERVICE",
                "REPORTED STATE / PID",
                "FRESHNESS",
            ],
            shown,
            selected,
            |index| {
                let row = rows[index];
                let (status, command_read) = self.service_status(row);
                [
                    row.display_name.clone(),
                    row.name.clone(),
                    format!(
                        "{} | {}",
                        status.state.label(),
                        if status.pid == 0 {
                            "-".into()
                        } else {
                            status.pid.to_string()
                        }
                    ),
                    if command_read {
                        "Command read"
                    } else if freshness == "Live" && !self.service_is_fresh(row) {
                        "Pre-command"
                    } else {
                        freshness
                    }
                    .into(),
                ]
            },
            t,
        );
        if let Some(index) = clicked {
            self.selected_service = Some(rows[index].name.clone());
        }
        widgets::hover_label(
            ui,
            RichText::new(format!(
                "{shown} matching of {} retained services. Select a row for confirmed controls.",
                self.snapshot.services.len()
            ))
            .size(10.0)
            .color(t.text_muted),
        );
    }
}
