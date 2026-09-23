use super::*;
use crate::diagnostics::{Provider, State, age};
use std::time::Instant;

/// Height of a Startup source chip: dot, name, count, and (when the source is
/// not a settled read) its freshness state suffixed on the chip itself.
const CHIP_HEIGHT: f32 = 26.0;

impl TrontopApp {
    pub(super) fn startup_inventory(&self, ui: &mut egui::Ui) {
        let t = self.colors();
        let now = Instant::now();
        let snapshot = &self.snapshot.startup;
        let total = snapshot.rows().count();
        startup_chip_row(ui, snapshot, now, t);
        ui.add_space(theme::space::S);
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
                let freshness = source.row_state(entry, now).to_string();
                let freshness_hover = format!(
                    "{freshness}, observed {}",
                    age(Some(entry.observed_at), now)
                );
                [
                    (entry.row.name.clone(), entry.row.name.clone()),
                    (entry.row.command.clone(), entry.row.command.clone()),
                    (source.source.name().into(), source.source.name().into()),
                    (freshness, freshness_hover),
                ]
            },
            t,
        );
        widgets::hover_label(ui, RichText::new(format!("{shown} matching of {total} retained entries. Read-only inventory; enabled/disabled state is not inferred.")).size(10.0).color(t.text_muted));
    }

    pub(super) fn service_inventory(&mut self, ui: &mut egui::Ui) {
        let t = self.colors();
        let now = Instant::now();
        self.service_controls(ui);
        ui.add_space(theme::space::M);
        let health = self.snapshot.diagnostics.get(Provider::Services);
        let state = health.state(Provider::Services, now);
        let freshness = match state {
            State::Live => "Live",
            State::Starting => "Starting",
            _ if health.last_success.is_some() => "Cached",
            _ => "Unavailable",
        };
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
            ["DISPLAY NAME", "SERVICE", "STATE", "PID", "FRESHNESS"],
            shown,
            selected,
            |index| {
                let row = rows[index];
                let (status, command_read) = self.service_status(row);
                let pid = if status.pid == 0 {
                    String::new()
                } else {
                    status.pid.to_string()
                };
                let pid_hover = if pid.is_empty() {
                    "No PID reported".to_string()
                } else {
                    format!("PID {pid}")
                };
                let freshness_value = if command_read {
                    "Command read".to_string()
                } else if freshness == "Live" && !self.service_is_fresh(row) {
                    "Pre-command".to_string()
                } else {
                    freshness.to_string()
                };
                [
                    (row.display_name.clone(), row.display_name.clone()),
                    (row.name.clone(), row.name.clone()),
                    (
                        status.state.label().to_string(),
                        status.state.label().to_string(),
                    ),
                    (pid, pid_hover),
                    (freshness_value.clone(), freshness_value),
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

/// Short chip label for a Startup source: the identical [`crate::startup::Source::name`]
/// for the three that already fit, and a shorter form of the two longest
/// names so five equal-width chips fit one row down to a 1000 px window.
pub(super) fn chip_label(source: crate::startup::Source) -> &'static str {
    use crate::startup::Source;
    match source {
        Source::MachineRun32 => "32-bit Run key",
        Source::UserFolder => "User Startup folder",
        other => other.name(),
    }
}

/// One line of source chips, always exactly [`CHIP_HEIGHT`] tall regardless of
/// how many sources need their state spelled out: a fixed-height row keeps the
/// table below it from moving as freshness changes. Five equal-width columns
/// (not auto-sized-to-content chips) mean the row's total width is always
/// exactly `ui.available_width()`, so it can never overflow the page at any
/// supported window size, regardless of how long a freshness suffix gets.
fn startup_chip_row(
    ui: &mut egui::Ui,
    snapshot: &crate::startup::Snapshot,
    now: Instant,
    t: Tokens,
) {
    ui.columns(snapshot.sources.len(), |columns| {
        for (cell, source) in columns.iter_mut().zip(&snapshot.sources) {
            startup_chip(cell, source, now, t);
        }
    });
}

/// A dot, the source name (its own text run, so it stays independently
/// searchable), and an entry count, filling this column's full width. Live,
/// Empty and Absent are all settled, complete reads and stay plain; anything
/// else (Cached, Partial, Starting, Unavailable) tints the dot, border and
/// count, and spells out the state on the chip itself. Both runs are wrapped
/// to a share of the column width, so a long name or state word clips inside
/// the chip instead of ever pushing past it. The full "N entries / Complete
/// Ns ago" detail is always on hover, never on the chip.
fn startup_chip(
    ui: &mut egui::Ui,
    source: &crate::startup::SourceSnapshot,
    now: Instant,
    t: Tokens,
) {
    use crate::startup::State as SourceState;
    let state = source.state(now);
    let settled = matches!(
        state,
        SourceState::Live | SourceState::Empty | SourceState::Absent
    );
    let raw = match state {
        SourceState::Live | SourceState::Empty | SourceState::Absent => t.good,
        SourceState::Starting => t.text_muted,
        SourceState::Unavailable => t.danger,
        SourceState::Partial | SourceState::Cached => ui.visuals().warn_fg_color,
    };
    let dot = t.ink(raw);
    let name = chip_label(source.source);
    let suffix = if settled {
        format!(" {}", source.entries.len())
    } else {
        format!(" {}  \u{b7}  {}", source.entries.len(), state.label())
    };
    let font = FontId::proportional(11.0);
    // Inset the whole chip a hair from the column's true edge: a glyph's
    // rendered ink can extend a fraction past its logical advance width, and
    // painting flush against the ambient clip would slice that sliver off.
    let rect = egui::Rect::from_min_size(
        ui.cursor().min,
        Vec2::new((ui.available_width() - 1.0).max(1.0), CHIP_HEIGHT),
    );
    let response = ui.allocate_rect(rect, Sense::hover());
    let fill = if response.hovered() {
        ui.visuals().widgets.hovered.weak_bg_fill
    } else {
        t.panel_raised
    };
    ui.painter().rect(
        rect,
        CHIP_HEIGHT / 2.0,
        fill,
        Stroke::new(1.0, if settled { t.border } else { dot }),
        egui::StrokeKind::Inside,
    );
    ui.painter()
        .circle_filled(rect.left_center() + Vec2::new(11.0, 0.0), 3.0, dot);
    // The count-and-state suffix keeps a fixed share of the column so it
    // never gets crowded out; the name takes what's left.
    let usable = (rect.width() - 28.0).max(20.0);
    let suffix_width = usable.min(if settled { 26.0 } else { 78.0 });
    let name_width = (usable - suffix_width).max(10.0);
    let name_rect = egui::Rect::from_min_size(
        rect.left_top() + Vec2::new(20.0, 0.0),
        Vec2::new(name_width, rect.height()),
    );
    widgets::paint_text(
        ui,
        name_rect,
        name,
        font.clone(),
        if settled { t.text } else { dot },
        Align::Min,
    );
    let suffix_rect = egui::Rect::from_min_size(
        name_rect.right_top(),
        Vec2::new(suffix_width, rect.height()),
    );
    widgets::paint_text(
        ui,
        suffix_rect,
        &suffix,
        font,
        if settled { t.text_muted } else { dot },
        Align::Min,
    );
    let hover = format!(
        "{}: {} entries / Complete {}{}",
        source.source.name(),
        source.entries.len(),
        age(source.last_complete, now),
        if source.retention_limited {
            " / Retention limit reached"
        } else {
            ""
        }
    );
    response.on_hover_text(hover);
}
