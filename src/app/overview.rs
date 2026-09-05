use super::*;
use crate::diagnostics::Provider;
use std::time::Instant;

impl TrontopApp {
    pub(super) fn overview_page(&mut self, ui: &mut egui::Ui) {
        let health = self.snapshot.diagnostics.get(Provider::System);
        let now = Instant::now();
        let subtitle = format!(
            "{} / last usable system sample {} / background telemetry",
            health.state(Provider::System, now).label(),
            crate::diagnostics::age(health.last_success, now)
        );
        self.page_header(ui, "Overview", &subtitle, false);
        let t = self.colors();
        egui::ScrollArea::vertical().id_salt("overview_scroll").auto_shrink([false, false]).show(ui, |ui| {
            let has_sample = self.seen_generation > 0;
            let cpu = if has_sample { format::percent(self.snapshot.cpu_percent) } else { "--".into() };
            let ram = if self.snapshot.memory_total_bytes > 0 { format::percent(memory_percent(&self.snapshot)) } else { "--".into() };
            let gpu = if self.snapshot.gpu.available { format::percent(self.snapshot.gpu.utilization_percent) } else { "--".into() };
            ui.columns(4, |cols| {
                widgets::stat_card(&mut cols[0], "CPU", &cpu, "Whole-machine load", t.accent, self.theme, t);
                widgets::stat_card(&mut cols[1], "MEMORY", &ram, "Physical memory used", t.secondary, self.theme, t);
                widgets::stat_card(&mut cols[2], "GPU ACTIVITY", &gpu, "Windows GPU Engine counters", t.secondary, self.theme, t);
                widgets::stat_card(&mut cols[3], "UPTIME", &if has_sample { format::duration(self.snapshot.uptime_seconds) } else { "--".into() }, &format!("{} processes", self.snapshot.process_count), t.good, self.theme, t);
            });
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                widgets::section_label(ui, "Temperatures & power", t);
                let state = self.snapshot.diagnostics.get(Provider::GpuSensors).state(Provider::GpuSensors, Instant::now());
                widgets::status_pill(ui, if self.snapshot.gpu_sensors.using_cached { "Cached" } else { state.label() }, diagnostics::state_color(state, t));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("All sensors").clicked() { self.page = Page::Sensors; }
                });
            });
            let placeholder = crate::gpu_sensors::AdapterSensors { name: "GPU sensor fields".into(), ..Default::default() };
            let adapters = if self.snapshot.gpu_sensors.adapters.is_empty() { std::slice::from_ref(&placeholder) } else { &self.snapshot.gpu_sensors.adapters };
            for (index, adapter) in adapters.iter().enumerate() {
                widgets::hover_frame(ui, widgets::surface(ui, t, index % 2 == 1), |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.add(egui::Label::new(RichText::new(&adapter.name).strong()).truncate()).on_hover_text(&adapter.name);
                    ui.columns(3, |cols| {
                        widgets::metric(&mut cols[0], "TEMPERATURE", &adapter.temperature_c.map_or_else(|| "-- °C".into(), |v| format!("{v} °C")), t);
                        widgets::metric(&mut cols[1], "BOARD POWER", &adapter.power_w.map_or_else(|| "-- W".into(), |v| format!("{v:.1} W")), t);
                        widgets::metric(&mut cols[2], "FAN TARGET", &adapter.fan_percent.map_or_else(|| "-- %".into(), |v| format!("{v}%")), t);
                    });
                });
                ui.add_space(4.0);
            }
            widgets::hover_label(ui, RichText::new("CPU and drive temperature coverage is in progress. Missing readings are never estimated.").size(11.0).color(t.text_muted));
            ui.add_space(12.0);
            ui.columns(2, |cols| {
                widgets::hover_label(&mut cols[0], RichText::new("CPU history").strong().size(14.0));
                widgets::history_graph(&mut cols[0], &self.cpu_history, t.accent, 145.0, Some(100.0), t);
                widgets::hover_label(&mut cols[1], RichText::new("Memory history").strong().size(14.0));
                widgets::history_graph(&mut cols[1], &self.memory_history, t.secondary, 145.0, Some(100.0), t);
            });
            ui.add_space(12.0);
            ui.columns(2, |cols| {
                widgets::section_label(&mut cols[0], "Storage & network", t);
                let rate = |present, value| if has_sample && present { format::rate(value) } else { "-- B/s".into() };
                let has_disks = !self.snapshot.disks.is_empty();
                let has_networks = !self.snapshot.networks.is_empty();
                widgets::detail_row(&mut cols[0], "Disk read", &rate(has_disks, self.snapshot.disks.iter().map(|d| d.read_bytes_per_sec).sum()), t);
                widgets::detail_row(&mut cols[0], "Disk write", &rate(has_disks, self.snapshot.disks.iter().map(|d| d.write_bytes_per_sec).sum()), t);
                widgets::detail_row(&mut cols[0], "Network receive", &rate(has_networks, self.snapshot.networks.iter().map(|n| n.received_bytes_per_sec).sum()), t);
                widgets::detail_row(&mut cols[0], "Network send", &rate(has_networks, self.snapshot.networks.iter().map(|n| n.transmitted_bytes_per_sec).sum()), t);
                widgets::section_label(&mut cols[1], "Memory headroom", t);
                let bytes = |value| if has_sample { format::bytes(value) } else { "--".into() };
                widgets::detail_row(&mut cols[1], "Available RAM", &bytes(self.snapshot.memory_available_bytes), t);
                widgets::detail_row(&mut cols[1], "Swap used", &bytes(self.snapshot.swap_used_bytes), t);
                widgets::detail_row(&mut cols[1], "Swap capacity", &bytes(self.snapshot.swap_total_bytes), t);
                if cols[1].button("Performance details").clicked() { self.page = Page::Performance; }
            });
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                widgets::section_label(ui, "Heaviest processes", t);
                if ui.button("Open processes").clicked() { self.page = Page::Processes; }
            });
            ui.columns(2, |cols| {
                top_processes(&mut cols[0], &self.snapshot.processes, false, t);
                top_processes(&mut cols[1], &self.snapshot.processes, true, t);
            });
            ui.add_space(12.0);
            ui.horizontal_wrapped(|ui| {
                for provider in Provider::ALL {
                    let state = self.snapshot.diagnostics.get(provider).state(provider, Instant::now());
                    widgets::status_pill(ui, &format!("{}: {}", provider.name(), state.label()), diagnostics::state_color(state, t));
                }
                if ui.button("Provider details").clicked() { self.show_diagnostics = true; }
            });
        });
    }

    pub(super) fn sensors_page(&mut self, ui: &mut egui::Ui) {
        self.page_header(
            ui,
            "Hardware sensors",
            "Temperature, power, clocks and fans. Missing readings keep their fields.",
            false,
        );
        egui::ScrollArea::vertical().id_salt("all_sensors_scroll").auto_shrink([false, false]).show(ui, |ui| {
            self.provider_notice(ui, Provider::GpuSensors);
            self.gpu_sensor_performance(ui);
            let t = self.colors();
            widgets::section_label(ui, "CPU & motherboard", t);
            widgets::detail_row(ui, "CPU temperature", "-- °C", t);
            widgets::hover_label(ui, RichText::new("A CPU hardware-sensor provider is not connected. ACPI thermal zones are not assumed to be CPU package readings.").size(11.0).color(t.text_muted));
            ui.add_space(10.0);
            widgets::section_label(ui, "Drive temperatures", t);
            if self.snapshot.disks.is_empty() { widgets::detail_row(ui, "Drive sensor", "-- °C", t); }
            for disk in &self.snapshot.disks { widgets::detail_row(ui, &disk.mount, "-- °C", t); }
            widgets::hover_label(ui, RichText::new("Drive temperature sampling is not connected yet. The SSD capability probe succeeded; runtime integration is still in progress.").size(11.0).color(t.text_muted));
        });
    }
}

fn top_processes(ui: &mut egui::Ui, rows: &[ProcessRow], memory: bool, t: Tokens) {
    widgets::hover_label(
        ui,
        RichText::new(if memory { "By memory" } else { "By CPU" }).strong(),
    );
    let mut top: Vec<&ProcessRow> = Vec::with_capacity(6);
    for row in rows {
        let before = top
            .iter()
            .position(|other| {
                if memory {
                    row.memory_bytes > other.memory_bytes
                } else {
                    row.cpu_percent > other.cpu_percent
                }
            })
            .unwrap_or(top.len());
        if before < 5 {
            top.insert(before, row);
            top.truncate(5);
        }
    }
    // Reserve the same five row slots while the first snapshot is pending.
    for index in 0..5 {
        widgets::hover_frame(ui, widgets::surface(ui, t, index % 2 == 1), |ui| {
            ui.set_min_width(ui.available_width());
            let row = top.get(index);
            ui.add(
                egui::Label::new(RichText::new(row.map_or("--", |r| &r.name)).strong()).truncate(),
            );
            let value = row.map_or_else(
                || "PID -- / --".into(),
                |r| {
                    format!(
                        "PID {} / {}",
                        r.pid,
                        if memory {
                            format::bytes(r.memory_bytes)
                        } else {
                            format::percent(r.cpu_percent)
                        }
                    )
                },
            );
            widgets::hover_label(ui, RichText::new(value).size(11.0).monospace());
        });
        ui.add_space(4.0);
    }
}
