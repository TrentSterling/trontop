use super::*;
use crate::diagnostics::{Provider, State, age};
use std::time::Instant;

/// Height of a Startup source chip: dot, name, count, and (when the source is
/// not a settled read) its freshness state suffixed on the chip itself.
const CHIP_HEIGHT: f32 = 26.0;

/// Startup table columns: `(header, fraction, minimum)`, last column flexible.
const STARTUP_COLUMNS_WITH_FRESHNESS: [(&str, f32, f32); 4] = [
    ("NAME", 0.24, 140.0),
    ("COMMAND / FILE", 0.34, 170.0),
    ("SOURCE", 0.27, 175.0),
    ("FRESHNESS", 0.15, 90.0),
];
/// Same three columns, widths rebalanced across the space FRESHNESS gave up.
const STARTUP_COLUMNS: [(&str, f32, f32); 3] = [
    ("NAME", 0.28, 140.0),
    ("COMMAND / FILE", 0.40, 170.0),
    ("SOURCE", 0.32, 175.0),
];
const SERVICES_COLUMNS_WITH_FRESHNESS: [(&str, f32, f32); 5] = [
    ("DISPLAY NAME", 0.22, 130.0),
    ("SERVICE", 0.28, 150.0),
    ("STATE", 0.14, 80.0),
    ("PID", 0.10, 56.0),
    ("FRESHNESS", 0.14, 90.0),
];
/// DISPLAY NAME / SERVICE / STATE / PID, widths rebalanced across the space
/// FRESHNESS gave up.
const SERVICES_COLUMNS: [(&str, f32, f32); 4] = [
    ("DISPLAY NAME", 0.32, 150.0),
    ("SERVICE", 0.38, 180.0),
    ("STATE", 0.16, 90.0),
    ("PID", 0.14, 70.0),
];

const STARTUP_CAVEAT: &str = "Read-only inventory; enabled/disabled state is not inferred.";
const SERVICES_CAVEAT: &str = "Select a row for confirmed controls.";

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
        // The FRESHNESS column only earns its place when it would say something
        // other than "Live" for every row; an all-fresh table gives that width
        // back to NAME, COMMAND / FILE and SOURCE instead.
        let all_live = rows
            .iter()
            .all(|(source, entry)| source.row_state(entry, now) == "Live");
        // COMMAND / FILE is the flexible column on both variants, at index 1;
        // NAME and SOURCE (and FRESHNESS, when present) keep fixed widths.
        if all_live {
            widgets::inventory_table(
                ui,
                "startup_grid",
                STARTUP_COLUMNS,
                1,
                shown,
                None,
                None,
                |index| {
                    let (source, entry) = rows[index];
                    [
                        (entry.row.name.clone(), entry.row.name.clone()),
                        (entry.row.command.clone(), entry.row.command.clone()),
                        (source.source.name().into(), source.source.name().into()),
                    ]
                },
                t,
            );
        } else {
            widgets::inventory_table(
                ui,
                "startup_grid",
                STARTUP_COLUMNS_WITH_FRESHNESS,
                1,
                shown,
                None,
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
        }
        let footer = if needle.is_empty() {
            format!("{total} entries")
        } else {
            format!("{shown} shown")
        };
        widgets::hover_label(ui, RichText::new(footer).size(10.0).color(t.text_muted))
            .on_hover_text(STARTUP_CAVEAT);
    }

    pub(super) fn service_inventory(&mut self, ui: &mut egui::Ui) {
        let t = self.colors();
        let now = Instant::now();
        self.service_controls(ui);
        ui.add_space(theme::space::XS);
        // Running is the calm, good state; Stopped recedes; anything in
        // between (starting, stopping, paused) is the one to notice.
        let warn = ui.visuals().warn_fg_color;
        let state_color = move |state: &str| match state {
            "Running" => t.good,
            "Stopped" => t.text_muted,
            _ => warn,
        };
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
            .filter(|row| {
                !self.services_running_only
                    || self.service_status(row).0.state == crate::service_control::State::Running
            })
            .collect::<Vec<_>>();
        let shown = rows.len();
        let selected = rows
            .iter()
            .position(|row| self.selected_service.as_ref() == Some(&row.name));
        // Precompute each row's freshness once: it decides whether the
        // FRESHNESS column earns its place, then feeds the table itself.
        let freshness_values = rows
            .iter()
            .map(|row| {
                let (_, command_read) = self.service_status(row);
                if command_read {
                    "Command read".to_string()
                } else if freshness == "Live" && !self.service_is_fresh(row) {
                    "Pre-command".to_string()
                } else {
                    freshness.to_string()
                }
            })
            .collect::<Vec<_>>();
        let all_live = freshness_values.iter().all(|value| value == "Live");
        // DISPLAY NAME is the flexible column on both variants, at index 0;
        // SERVICE, STATE, PID (and FRESHNESS, when present) keep fixed
        // widths, so PID never inherits the leftover width.
        let clicked = if all_live {
            widgets::inventory_table(
                ui,
                "services_grid",
                SERVICES_COLUMNS,
                0,
                shown,
                selected,
                Some((2, &state_color)),
                |index| {
                    let row = rows[index];
                    let (status, _) = self.service_status(row);
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
                    [
                        (row.display_name.clone(), row.display_name.clone()),
                        (row.name.clone(), row.name.clone()),
                        (
                            status.state.label().to_string(),
                            status.state.label().to_string(),
                        ),
                        (pid, pid_hover),
                    ]
                },
                t,
            )
        } else {
            widgets::inventory_table(
                ui,
                "services_grid",
                SERVICES_COLUMNS_WITH_FRESHNESS,
                0,
                shown,
                selected,
                Some((2, &state_color)),
                |index| {
                    let row = rows[index];
                    let (status, _) = self.service_status(row);
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
                    let freshness_value = freshness_values[index].clone();
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
            )
        };
        if let Some(index) = clicked {
            self.selected_service = Some(rows[index].name.clone());
        }
        let footer = if needle.is_empty() && !self.services_running_only {
            format!("{} services", self.snapshot.services.len())
        } else {
            format!("{shown} of {} shown", self.snapshot.services.len())
        };
        widgets::hover_label(ui, RichText::new(footer).size(10.0).color(t.text_muted))
            .on_hover_text(SERVICES_CAVEAT);
    }
}

/// Short chip label for a Startup source, none of which truncate at a 1000 px
/// window; the full [`crate::startup::Source::name`] is always on the chip's hover.
pub(super) fn chip_label(source: crate::startup::Source) -> &'static str {
    use crate::startup::Source;
    match source {
        Source::UserRun => "User Run",
        Source::MachineRun => "Machine Run",
        Source::MachineRun32 => "32-bit Run",
        Source::UserFolder => "User folder",
        Source::MachineFolder => "Machine folder",
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
