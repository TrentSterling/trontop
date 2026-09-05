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
        widgets::performance_heading(
            ui,
            "Physical disks",
            "Windows PDH / independent 1 second sampler",
            snapshot.state(now).label(),
            t.good,
            t,
        );
        ui.horizontal_wrapped(|ui| {
            for disk in &snapshot.devices {
                let response = ui
                    .selectable_label(
                        Some(&disk.instance) == self.selected_physical_disk.as_ref(),
                        format!("Disk {}", disk.number),
                    )
                    .on_hover_text(format!(
                        "Windows instance: {}\nNot a persistent hardware identifier.",
                        disk.instance
                    ));
                if response.clicked() {
                    self.selected_physical_disk = Some(disk.instance.clone());
                }
            }
        });
        ui.add_space(6.0);
        // Fixed fields remain visible during warmup, partial failure and removal.
        let detail = selected.map_or_else(
            || {
                if self.selected_physical_disk.is_some() {
                    "Selected disk is no longer reported. Choose another disk above."
                } else {
                    "No physical disk instances reported yet. Metrics remain in place below."
                }
            },
            |d| d.instance.as_str(),
        );
        ui.add(egui::Label::new(RichText::new(detail).color(t.text_muted).size(11.0)).truncate())
            .on_hover_text(detail);
        ui.add_space(8.0);
        let history = selected.and_then(|d| self.physical_disk_history.values.get(&d.instance));
        for metrics in [&Metric::ALL[..3], &Metric::ALL[3..]] {
            ui.columns(metrics.len(), |columns| {
                for (column, &metric) in columns.iter_mut().zip(metrics) {
                    let reading = selected
                        .map(|d| d.readings[metric as usize])
                        .unwrap_or_default();
                    widgets::hover_frame(
                        column,
                        widgets::surface(column, t, metric as usize % 2 == 1),
                        |ui| {
                            ui.spacing_mut().item_spacing.y = 3.0;
                            ui.set_min_width(ui.available_width());
                            ui.add(
                                egui::Label::new(
                                    RichText::new(metric.label()).size(11.0).color(t.text_muted),
                                )
                                .truncate(),
                            )
                            .on_hover_text(metric.format(reading.value));
                            ui.add(
                                egui::Label::new(
                                    RichText::new(metric.format(reading.value))
                                        .size(18.0)
                                        .monospace()
                                        .color(t.text),
                                )
                                .truncate(),
                            );
                            ui.add(
                                egui::Label::new(
                                    RichText::new(reading.state(snapshot, now))
                                        .size(10.0)
                                        .color(t.text_muted),
                                )
                                .truncate(),
                            )
                            .on_hover_text(format!(
                                "Last measured: {}",
                                crate::diagnostics::age(reading.at, now)
                            ));
                        },
                    )
                    .response
                    .on_hover_text(metric.explanation());
                }
            });
            ui.add_space(8.0);
        }
        let empty = VecDeque::new();
        widgets::hover_label(
            ui,
            RichText::new("Active time (%)")
                .size(11.0)
                .color(t.text_muted),
        );
        widgets::history_graph_with_window(
            ui,
            history.map_or(&empty, |h| &h[0]),
            graph_color(t.good),
            110.0,
            Some(100.0),
            t,
            ("120 SAMPLES", HISTORY_LENGTH),
        );
        ui.add_space(8.0);
        ui.columns(2, |columns| {
            for (column, (index, label, color)) in columns.iter_mut().zip([
                (1, "Response time (ms)", t.accent),
                (2, "Queue depth (requests)", t.secondary),
            ]) {
                widgets::hover_label(column, RichText::new(label).size(11.0).color(t.text_muted));
                widgets::history_graph_with_window(
                    column,
                    history.map_or(&empty, |h| &h[index]),
                    graph_color(color),
                    70.0,
                    None,
                    t,
                    ("120 SAMPLES", HISTORY_LENGTH),
                );
            }
        });
        ui.add_space(8.0);
        widgets::detail_row(
            ui,
            "Provider sample",
            &crate::diagnostics::age(snapshot.at, now),
            t,
        );
        let note = snapshot.error.unwrap_or("History holds 120 display samples, nominally 1 second apart. Gaps mean no fresh reading. Disk numbers can change after hotplug. Volume capacity and temperatures have separate views.");
        widgets::hover_label(ui, RichText::new(note).size(11.0).color(t.text_muted));
    }
}
