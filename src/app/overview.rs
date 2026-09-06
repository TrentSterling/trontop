use super::*;
use crate::diagnostics::Provider;
use std::time::Instant;

impl TrontopApp {
    pub(super) fn overview_page(&mut self, ui: &mut egui::Ui) {
        let health = self.snapshot.diagnostics.get(Provider::System);
        let now = self.graphs.now();
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
            let gpu = self.snapshot.gpu.reading().label();
            ui.columns(4, |cols| {
                widgets::stat_card(&mut cols[0], "CPU", &cpu, "Whole-machine load", t.accent, self.theme, t);
                widgets::stat_card(&mut cols[1], "MEMORY", &ram, "Physical memory used", t.secondary, self.theme, t);
                widgets::stat_card(&mut cols[2], "GPU ACTIVITY", &gpu, "Busiest Windows GPU engine", t.secondary, self.theme, t);
                widgets::stat_card(&mut cols[3], "UPTIME", &if has_sample { format::duration(self.snapshot.uptime_seconds) } else { "--".into() }, &format!("{} processes", self.snapshot.process_count), t.good, self.theme, t);
            });
            ui.add_space(8.0);
            self.graphs.ensure_sample(&self.snapshot);
            self.graphs.controls(ui, t);
            ui.add_space(6.0);
            widgets::section_label(ui, "Machine signals", t);
            self.graphs.overview_wall(ui, true, t);
            ui.add_space(8.0);
            widgets::section_label(ui, &format!("All {} logical processors", self.snapshot.cpu.logical_cores), t);
            self.graphs.cpu_grid(ui, self.snapshot.cpu.logical_cores, t);
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                widgets::section_label(ui, "Memory, engines, storage & sensors", t);
                if ui.small_button("Sensor details").clicked() { self.page = Page::Sensors; }
            });
            self.graphs.overview_wall(ui, false, t);
            widgets::hover_label(ui, RichText::new("Measured history only. Gaps mark missing samples. CPU temperature needs a compatible sensor provider.").size(11.0).color(t.text_muted));
            ui.add_space(8.0);
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
            let t = self.colors();
            let now = Instant::now();
            ui.horizontal_wrapped(|ui| {
                let gpu = self.snapshot.diagnostics.get(Provider::GpuSensors).state(Provider::GpuSensors, now);
                let drives = self.snapshot.storage_sensors.health(now).state(Provider::StorageSensors, now);
                widgets::status_pill(ui, &format!("GPU: {}", if self.snapshot.gpu_sensors.using_cached { "Cached" } else { gpu.label() }), diagnostics::state_color(gpu, t));
                widgets::status_pill(ui, &format!("Drives: {}", drives.label()), diagnostics::state_color(drives, t));
                widgets::status_pill(ui, "CPU: not connected", t.text_muted);
            });
            ui.add_space(8.0);
            widgets::section_label(ui, "CPU & motherboard", t);
            widgets::detail_row(ui, "CPU temperature", "-- °C", t);
            widgets::hover_label(ui, RichText::new("A CPU hardware-sensor provider is not connected. ACPI thermal zones are not assumed to be CPU package readings.").size(11.0).color(t.text_muted));
            ui.add_space(10.0);
            self.storage_sensor_cards(ui);
            ui.add_space(10.0);
            self.gpu_sensor_performance(ui);
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
