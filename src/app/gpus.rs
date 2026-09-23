use super::*;

/// A per-adapter GPU reading older than this is missing, not current.
pub(super) const GPU_ADAPTER_STALE_SECS: f32 = 3.0;

impl TrontopApp {
    /// Adapters producing values, restricted to ones the snapshot still lists.
    fn gpu_active_adapters(&self) -> Vec<crate::gpu_adapters::Key> {
        let adapters = &self.snapshot.gpu.adapters;
        self.graphs
            .active_adapters()
            .into_iter()
            .filter(|key| adapters.iter().any(|a| a.key == *key))
            .collect()
    }

    /// The adapter Performance > GPU shows. With one adapter producing values
    /// (an idle iGPU, a software adapter or a silent counter identity does not
    /// count) there is nothing to pick: that adapter is shown. `None` when the
    /// snapshot lists no adapter.
    pub(super) fn gpu_shown_adapter(&self) -> Option<crate::gpu_adapters::Key> {
        let adapters = &self.snapshot.gpu.adapters;
        let first = adapters
            .iter()
            .find(|a| a.description.as_ref().is_some_and(|d| !d.software))
            .or(adapters.first())?
            .key;
        let active = self.gpu_active_adapters();
        Some(
            (active.len() == 1)
                .then(|| active[0])
                .or(self
                    .graphs
                    .gpu_selected
                    .filter(|key| adapters.iter().any(|a| a.key == *key)))
                .unwrap_or(first),
        )
    }

    /// The one GPU reading Performance shows, in both the rail tile and the
    /// device header: the shown adapter's busiest engine, or the machine-wide
    /// reading when Windows lists no adapter. An adapter sample older than
    /// [`GPU_ADAPTER_STALE_SECS`] is missing on both, never current on one.
    pub(super) fn gpu_performance_reading(&self) -> crate::gpu_activity::Usage {
        let Some(key) = self.gpu_shown_adapter() else {
            return self.snapshot.gpu.reading();
        };
        let now = self.graphs.now();
        self.snapshot
            .gpu
            .adapters
            .iter()
            .find(|a| a.key == key)
            .filter(|adapter| {
                adapter.sampled_at.is_some_and(|at| {
                    now.saturating_duration_since(at).as_secs_f32() <= GPU_ADAPTER_STALE_SECS
                })
            })
            .map_or(crate::gpu_activity::Usage::Unavailable, |adapter| {
                adapter.activity
            })
    }

    pub(super) fn gpu_adapters_page(&mut self, ui: &mut egui::Ui) {
        let t = self.colors();
        self.graphs.ensure_sample(&self.snapshot);
        let adapters = &self.snapshot.gpu.adapters;
        let active = self.gpu_active_adapters();
        let picker = active.len() > 1 || (active.is_empty() && adapters.len() > 1);
        let Some(key) = self.gpu_shown_adapter() else {
            return;
        };
        self.graphs.gpu_selected = Some(key);
        let adapters = &self.snapshot.gpu.adapters;
        let adapter = adapters.iter().find(|a| a.key == key).unwrap();
        let now = self.graphs.now();
        let activity = self.gpu_performance_reading();
        // Counter identity and status are provenance, not something to read at
        // a glance; they live on the picker's hover, not their own text line.
        if picker {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Adapter").color(t.text_muted));
                egui::ComboBox::from_id_salt("gpu-adapter-selector")
                    .width((ui.available_width() - 8.0).clamp(140.0, 420.0))
                    .selected_text(adapter.name())
                    .show_ui(ui, |ui| {
                        for adapter in adapters {
                            ui.selectable_value(
                                &mut self.graphs.gpu_selected,
                                Some(adapter.key),
                                format!("{} [{}]", adapter.name(), adapter.key.label()),
                            )
                            .on_hover_text(adapter.key.label());
                        }
                    })
                    .response
                    .on_hover_text(format!("{}  |  {}", key.label(), activity.status()));
            });
            ui.add_space(theme::space::M);
        }
        widgets::performance_heading(ui, &adapter.name(), "", &activity.label(), t.accent, t)
            .on_hover_text(format!(
                "Busiest engine: {}
Windows GPU Engine and GPU Adapter Memory counters.",
                activity.explanation()
            ));
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
                    let value = adapter.memory[metric].label(now);
                    widgets::value_tile(
                        column,
                        ["Dedicated used", "Shared used", "Committed"][metric],
                        &value,
                        [
                            "Dedicated video memory in use (Windows GPU Adapter Memory). ~ marks a retained reading.",
                            "System memory in use by this adapter. Shared capacity is a limit, not reserved RAM. ~ marks a retained reading.",
                            "Committed memory is not an extra amount to add to dedicated or shared usage. ~ marks a retained reading.",
                        ][metric],
                        None,
                        metric % 2 == 1,
                        t,
                    );
                }
            });
        }
        ui.add_space(8.0);
        widgets::section_header(ui, "Engine histories", None, t);
        if adapter.engines.is_empty() {
            widgets::hover_label(
                ui,
                "No Windows engine counters reported for this adapter. Memory fields remain available.",
            );
        }
        self.graphs.adapter_charts(ui, key, false, t);
        widgets::section_header(ui, "Memory histories", None, t);
        self.graphs.adapter_charts(ui, key, true, t);
        if let Some(description) = &adapter.description {
            ui.add_space(4.0);
            // DXGI capacity of the logical adapter, all nodes.
            widgets::section_header(ui, "Capacity", None, t);
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
        if let Some(error) = &adapter.memory_error {
            widgets::hover_label(ui, error);
        }
        self.provider_notice(ui, crate::diagnostics::Provider::GpuActivity);
    }
}
