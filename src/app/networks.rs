//! Network Performance reuses the System inventory and existing traffic history.
use super::*;
use crate::specs::{Group, Item, LiveKey, Row, SectionId, SectionState};

fn adapter_group<'a>(section: &'a crate::specs::Section, interface: &str) -> Option<&'a Group> {
    section.groups.iter().find(|group| match &group.live {
        Some(LiveKey::NetworkThroughput { interface: name }) => name == interface,
        None => group.title == interface && row(group, "Adapter").is_some(),
        _ => false,
    })
}

fn row<'a>(group: &'a Group, label: &str) -> Option<&'a Row> {
    group.items.iter().find_map(|item| match item {
        Item::Row(row) if row.label == label => Some(row),
        _ => None,
    })
}

impl TrontopApp {
    pub(super) fn network_performance(&mut self, ui: &mut egui::Ui, index: usize) {
        // Also poll on the selection frame, before the following logic pass.
        self.poll_specs(ui.ctx());
        let t = self.colors();
        let now = self.graphs.now();
        self.graphs.ensure_sample(&self.snapshot);
        let Some(network) = self.snapshot.networks.get(index) else {
            widgets::gap_row(
                ui,
                "Network adapter",
                "No longer present",
                "This adapter is no longer reported. Choose another device on the left.",
                t,
            );
            return;
        };
        let entry = self.specs_view.get(SectionId::Network).cloned();
        let group = entry
            .as_ref()
            .and_then(|entry| entry.section.as_ref())
            .and_then(|section| adapter_group(section, &network.name));
        let description = group
            .and_then(|group| row(group, "Adapter"))
            .and_then(|row| row.value.text())
            .unwrap_or("Network adapter");
        widgets::performance_heading(
            ui,
            &network.name,
            description,
            &format::rate(network.received_bytes_per_sec + network.transmitted_bytes_per_sec),
            t.secondary,
            t,
        )
        .on_hover_text(format!(
            "{description}\nDownload plus upload throughput for this adapter."
        ));
        let height = widgets::fit_height(ui, 172.0, 120.0, 360.0);
        self.graphs.network_graph(ui, &network.name, height, t);
        ui.add_space(theme::space::M);
        ui.columns(4, |columns| {
            for (index, (column, (label, value, hover))) in columns
                .iter_mut()
                .zip([
                    (
                        "Download",
                        format::rate(network.received_bytes_per_sec),
                        "Bytes downloaded (received) per second.",
                    ),
                    (
                        "Upload",
                        format::rate(network.transmitted_bytes_per_sec),
                        "Bytes uploaded (sent) per second.",
                    ),
                    (
                        "Downloaded",
                        format::bytes(network.total_received_bytes),
                        "Total downloaded since Windows started counting, as reported by Windows.",
                    ),
                    (
                        "Uploaded",
                        format::bytes(network.total_transmitted_bytes),
                        "Total uploaded since Windows started counting, as reported by Windows.",
                    ),
                ])
                .enumerate()
            {
                widgets::value_tile(column, label, &value, hover, None, index % 2 == 1, t);
            }
        });
        ui.add_space(theme::space::M);
        let title = match entry.as_ref().map(|entry| entry.health.state) {
            Some(SectionState::Complete) => "Adapter details".into(),
            Some(SectionState::Collecting) if group.is_some() => {
                "Adapter details / Updating".into()
            }
            Some(SectionState::Slow | SectionState::Unavailable | SectionState::Stopped)
                if group.is_some() =>
            {
                "Adapter details / Cached".into()
            }
            Some(state) => format!("Adapter details / {}", state.label()),
            None => "Adapter details / Reading".into(),
        };
        if widgets::section_header(ui, &title, Some("System details"), t) {
            self.page = Page::System;
            self.system_section = SectionId::Network;
        }
        if let Some(group) = group {
            let provenance = entry.as_ref().and_then(|entry| entry.health.collected_at).map_or_else(
                || "Adapter inventory has no recorded read time.".into(),
                |at| format!("{}. Adapter configuration refreshes independently of traffic readings.", super::system::read_ago(at, now)),
            );
            ui.columns(2, |columns| {
                for (column, labels) in columns
                    .iter_mut()
                    .zip([["Type", "Status"], ["Link speed", "MTU"]])
                {
                    for (index, label) in labels.into_iter().enumerate() {
                        let mut detail = row(group, label)
                            .cloned()
                            .unwrap_or_else(|| Row::unavailable(label, "not reported by Windows"));
                        detail.note = Some(provenance.clone());
                        self.spec_row(column, &detail, index % 2 == 1, now, t);
                    }
                }
            });
        } else {
            let waiting = entry.as_ref().is_none_or(|entry| entry.section.is_none());
            widgets::gap_row(
                ui,
                "Adapter configuration",
                if waiting { "Reading" } else { "Not reported" },
                if waiting {
                    "Waiting for adapter inventory. Traffic readings continue independently."
                } else {
                    "The inventory has no exact match for this interface. Open System details for available adapters and provider status."
                },
                t,
            );
        }
    }
}
