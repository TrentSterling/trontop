use super::*;

impl TrontopApp {
    pub(super) fn gpu_adapters_page(&mut self, ui: &mut egui::Ui) {
        let t = self.colors();
        self.graphs.ensure_sample(&self.snapshot);
        let adapters = &self.snapshot.gpu.adapters;
        let selected = self
            .graphs
            .gpu_selected
            .filter(|key| adapters.iter().any(|a| a.key == *key))
            .unwrap_or_else(|| {
                adapters
                    .iter()
                    .find(|a| a.description.as_ref().is_some_and(|d| !d.software))
                    .unwrap_or(&adapters[0])
                    .key
            });
        self.graphs.gpu_selected = Some(selected);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Adapter").color(t.text_muted));
            egui::ComboBox::from_id_salt("gpu-adapter-selector")
                .width((ui.available_width() - 8.0).clamp(140.0, 420.0))
                .selected_text(adapters.iter().find(|a| a.key == selected).unwrap().name())
                .show_ui(ui, |ui| {
                    for adapter in adapters {
                        ui.selectable_value(
                            &mut self.graphs.gpu_selected,
                            Some(adapter.key),
                            format!("{} [{}]", adapter.name(), adapter.key.label()),
                        )
                        .on_hover_text(adapter.key.label());
                    }
                });
        });
        let key = self.graphs.gpu_selected.unwrap();
        let adapter = adapters.iter().find(|a| a.key == key).unwrap();
        let now = self.graphs.now();
        let age = adapter
            .sampled_at
            .map(|at| now.saturating_duration_since(at).as_secs_f32());
        let activity = if age.is_some_and(|v| v <= 3.0) {
            adapter.activity
        } else {
            crate::gpu_activity::Usage::Unavailable
        };
        ui.add_space(8.0);
        widgets::performance_heading(
            ui,
            &adapter.name(),
            "Windows adapter activity and memory",
            &activity.label(),
            t.accent,
            t,
        );
        ui.add(
            egui::Label::new(
                RichText::new(format!("{}  |  {}", adapter.key.label(), activity.status()))
                    .size(10.)
                    .color(t.text_muted),
            )
            .wrap(),
        );
        if adapter.description.as_ref().is_some_and(|d| d.software) {
            widgets::hover_label(ui, "Software adapter; not a physical graphics card.");
        } else if adapter.description.is_none() {
            widgets::hover_label(
                ui,
                "Windows counter identity has no matching DXGI description. Hardware type and capacity are unknown.",
            );
        } else if !adapter.description_current {
            widgets::hover_label(
                ui,
                "Adapter description is cached; DXGI inventory is not current.",
            );
        }
        ui.add_space(8.0);
        let columns = if ui.available_width() >= 480.0 { 3 } else { 1 };
        for chunk in [0, 1, 2].chunks(columns) {
            ui.columns(columns, |columns| {
                for (column, &metric) in columns.iter_mut().zip(chunk) {
                    widgets::metric_banded(
                        column,
                        ["DEDICATED USED", "SHARED USED", "COMMITTED"][metric],
                        &adapter.memory[metric].label(now),
                        metric % 2 == 1,
                        t,
                    );
                }
            });
        }
        ui.add_space(8.0);
        widgets::section_label(ui, "Engine histories", t);
        if adapter.engines.is_empty() {
            widgets::hover_label(
                ui,
                "No Windows engine counters reported for this adapter. Memory fields remain available.",
            );
        }
        self.graphs.adapter_charts(ui, key, false, t);
        widgets::section_label(ui, "Memory histories", t);
        self.graphs.adapter_charts(ui, key, true, t);
        if let Some(description) = &adapter.description {
            ui.add_space(4.0);
            widgets::section_label(ui, "DXGI capacity (logical adapter, all nodes)", t);
            for (label, value) in [
                ("Dedicated video memory", description.dedicated_video),
                ("Dedicated system memory", description.dedicated_system),
                ("Shared system memory limit", description.shared_limit),
            ] {
                widgets::detail_row(ui, label, &format::bytes(value), t);
            }
            widgets::detail_row(
                ui,
                "Vendor / device",
                &format!(
                    "{:04X} / {:04X}",
                    description.vendor_id, description.device_id
                ),
                t,
            );
        }
        ui.add_space(8.0);
        widgets::hover_label(
            ui,
            "Usage: Windows GPU Adapter Memory. Shared capacity is a limit, not reserved RAM. Committed memory is not an extra amount to add to dedicated/shared usage. ~ marks a retained reading.",
        );
        if let Some(error) = &adapter.memory_error {
            widgets::hover_label(ui, error);
        }
        self.provider_notice(ui, crate::diagnostics::Provider::GpuActivity);
    }
}
