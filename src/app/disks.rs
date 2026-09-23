use super::*;
use crate::disk_activity::{METRICS, Metric, Snapshot};
use std::time::Instant;

#[derive(Default)]
pub(super) struct Histories {
    generation: Option<u64>,
    pub(super) values: HashMap<String, [VecDeque<f32>; METRICS]>,
}

impl Histories {
    pub(super) fn push(&mut self, snapshot: &Snapshot, now: Instant) {
        let new = self.generation != Some(snapshot.generation);
        self.generation = Some(snapshot.generation);
        self.values
            .retain(|id, _| snapshot.devices.iter().any(|d| &d.instance == id));
        for disk in &snapshot.devices {
            let history = self.values.entry(disk.instance.clone()).or_default();
            for (index, field) in history.iter_mut().enumerate() {
                let value = new
                    .then(|| disk.readings[index].live(snapshot, now))
                    .flatten()
                    .filter(|v| *v <= f32::MAX as f64)
                    .map_or(f32::NAN, |v| v as f32);
                widgets::push_history(field, value, HISTORY_LENGTH);
            }
        }
    }
}

impl TrontopApp {
    /// The model of the physical disk behind a PDH instance ("0 C: D:"),
    /// joined through the drive letters of the System page's storage
    /// partitions. Both sides come from Windows at the same moment; a disk
    /// without a letter, or a letter no storage group lists, has no model
    /// here rather than a guessed one.
    fn physical_disk_model(&self, instance: &str) -> Option<String> {
        let letters: Vec<char> = instance
            .split_whitespace()
            .filter(|word| word.len() == 2 && word.ends_with(':'))
            .filter_map(|word| word.chars().next())
            .collect();
        if letters.is_empty() {
            return None;
        }
        let section = self
            .specs_view
            .get(crate::specs::SectionId::Storage)?
            .section
            .as_ref()?;
        section
            .groups
            .iter()
            .find(|disk| {
                disk.items.iter().any(|item| match item {
                    crate::specs::Item::Group(partitions)
                        if partitions.title.starts_with("Partitions") =>
                    {
                        partitions.items.iter().any(|row| match row {
                            crate::specs::Item::Row(row) => letters
                                .iter()
                                .any(|letter| row.label.ends_with(&format!("({letter}:)"))),
                            crate::specs::Item::Group(_) => false,
                        })
                    }
                    _ => false,
                })
            })
            .map(|disk| disk.title.clone())
            .filter(|title| title != "Disk")
    }

    pub(super) fn physical_disks_performance(&mut self, ui: &mut egui::Ui) {
        let t = self.colors();
        // Bright editable accents need darker plot strokes on light surfaces.
        let graph_color = |color| {
            if self.theme.dark {
                color
            } else {
                theme::mix(color, t.text, 0.5)
            }
        };
        let snapshot = &self.snapshot.physical_disks;
        let now = Instant::now();
        if self.selected_physical_disk.is_none() {
            self.selected_physical_disk = snapshot.devices.first().map(|d| d.instance.clone());
        }
        let selected = snapshot
            .devices
            .iter()
            .find(|d| Some(&d.instance) == self.selected_physical_disk.as_ref());
        let title = selected.map_or_else(
            || "Physical disks".to_owned(),
            |d| super::graphs::disk_label(d.number, &d.instance),
        );
        let model = selected.and_then(|d| self.physical_disk_model(&d.instance));
        let value = selected.map_or_else(
            || "--".to_owned(),
            |d| Metric::Active.format(d.readings[Metric::Active as usize].value),
        );
        // A counter error is an active anomaly: one chip inside the hero (so
        // nothing below moves), with the reason on hover.
        widgets::performance_heading_with_state(
            ui,
            &title,
            model.as_deref().unwrap_or(""),
            &value,
            snapshot.error.map(|_| ("Partial", t.secondary)),
            t.good,
            t,
        )
        .on_hover_text(format!(
            "Active time of the selected disk.\n{}{}\nWindows PDH PhysicalDisk counters, \
             independent 1 second sampler. Provider sample {}. History holds 120 display \
             samples; gaps mean no fresh reading. Disk numbers can change after hotplug.",
            selected.map_or("No disk selected.", |d| d.instance.as_str()),
            snapshot
                .error
                .map_or_else(String::new, |error| format!("\n{error}")),
            crate::diagnostics::age(snapshot.at, now),
        ));
        // The rail lists every disk and selects it; a second tab row here
        // would repeat the rail and the hero title.
        // Active anomalies stay visible as one compact row; routine
        // provenance lives on the hero's hover.
        if selected.is_none() {
            let reason = if self.selected_physical_disk.is_some() {
                "The selected disk is no longer reported. Choose another disk."
            } else {
                "No physical disk instances reported yet."
            };
            widgets::gap_row(ui, "Physical disk", "Not reported", reason, t);
        }
        let history = selected.and_then(|d| self.physical_disk_history.values.get(&d.instance));
        for (row, metrics) in [&Metric::ALL[..3], &Metric::ALL[3..]]
            .into_iter()
            .enumerate()
        {
            ui.columns(metrics.len(), |columns| {
                for (column, &metric) in columns.iter_mut().zip(metrics) {
                    let reading = selected
                        .map(|d| d.readings[metric as usize])
                        .unwrap_or_default();
                    let state = reading.state(snapshot, now);
                    widgets::value_tile(
                        column,
                        metric.label(),
                        &metric.format(reading.value),
                        &format!(
                            "{}\nLast measured: {}",
                            metric.explanation(),
                            crate::diagnostics::age(reading.at, now)
                        ),
                        (state != "Live").then_some(state),
                        (row + metric as usize) % 2 == 1,
                        t,
                    );
                }
            });
            ui.add_space(theme::space::M);
        }
        let empty = VecDeque::new();
        widgets::section_header(ui, "Active time history", None, t);
        // Room below: the response and queue histories with their headers.
        let height = widgets::fit_height(ui, 118.0, 60.0, 180.0);
        widgets::history_graph_with_window(
            ui,
            history.map_or(&empty, |h| &h[Metric::Active as usize]),
            graph_color(t.good),
            height,
            Some(100.0),
            t,
            ("120 s", HISTORY_LENGTH),
            Some(&|v: f32| format!("{v:.0}%")),
        );
        ui.add_space(theme::space::M);
        // Measured before the columns: each column's own origin would hide
        // how far down the page the pair sits. 26 px is the header row.
        let small = widgets::fit_height(ui, 30.0, 56.0, 110.0);
        ui.columns(2, |columns| {
            let response_fmt = |v: f32| {
                if v >= 10.0 {
                    format!("{v:.0} ms")
                } else {
                    format!("{v:.1} ms")
                }
            };
            let queue_fmt = |v: f32| format!("{v:.0} req");
            for (column, (metric, label, color, unit)) in columns.iter_mut().zip([
                (
                    Metric::Response,
                    "Response time history",
                    t.accent,
                    &response_fmt as &dyn Fn(f32) -> String,
                ),
                (
                    Metric::Queue,
                    "Queue depth history",
                    t.secondary,
                    &queue_fmt as &dyn Fn(f32) -> String,
                ),
            ]) {
                widgets::section_header(column, label, None, t);
                widgets::history_graph_with_window(
                    column,
                    history.map_or(&empty, |h| &h[metric as usize]),
                    graph_color(color),
                    small,
                    None,
                    t,
                    ("120 s", HISTORY_LENGTH),
                    Some(unit),
                );
            }
        });
    }
}
