//! Hardware sensors page: GPU first (the only reading that is always live),
//! then drives, then CPU and motherboard behind whatever provider is running.
//! Deep per-signal history lives only on Graphs; this page shows snapshot
//! values with compact gaps instead of giant "Unavailable" cards.
use super::*;
use crate::diagnostics::Provider;
use crate::gpu_sensors::{AdapterSensors, SensorHistory};
use crate::specs::{LiveKey, Value};
use std::time::Instant;

impl TrontopApp {
    pub(super) fn sensors_page(&mut self, ui: &mut egui::Ui) {
        let t = self.colors();
        let now = Instant::now();
        // A fixed-height row: an unbounded right_to_left child otherwise
        // claims the rest of the page height and centers the chips in the
        // middle of the window instead of beside the intro text.
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), 20.0),
            Layout::left_to_right(Align::Min),
            |ui| {
                ui.add(
                    egui::Label::new(
                        RichText::new(self.page.intro())
                            .size(11.0)
                            .color(t.text_muted),
                    )
                    .selectable(false),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let bridge = &self.specs_view.bridge;
                    let fresh = bridge.collected_at.is_some_and(|at| {
                        now.saturating_duration_since(at) <= crate::specs::BRIDGE_STALE_AFTER
                    });
                    match (&bridge.status, fresh) {
                        (Value::Known(providers), true) => {
                            widgets::status_pill(ui, &format!("CPU: {providers}"), t.good);
                        }
                        (Value::Known(_), false) => {
                            widgets::status_pill(ui, "CPU: provider stopped", t.danger);
                        }
                        (Value::Unavailable(_), _) => {
                            widgets::status_pill(ui, "CPU: no sensor provider", t.text_muted);
                        }
                    }
                    let drives = self
                        .snapshot
                        .storage_sensors
                        .health(now)
                        .state(Provider::StorageSensors, now);
                    widgets::status_pill(
                        ui,
                        &format!("Drives: {}", drives.label()),
                        diagnostics::state_color(drives, t),
                    );
                    let gpu = self
                        .snapshot
                        .diagnostics
                        .get(Provider::GpuSensors)
                        .state(Provider::GpuSensors, now);
                    widgets::status_pill(
                        ui,
                        &format!(
                            "GPU: {}",
                            if self.snapshot.gpu_sensors.using_cached {
                                "Cached"
                            } else {
                                gpu.label()
                            }
                        ),
                        diagnostics::state_color(gpu, t),
                    );
                });
            },
        );
        ui.add_space(theme::space::S);
        egui::ScrollArea::vertical()
            .id_salt("all_sensors_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.gpu_sensor_performance(ui);
                ui.add_space(theme::space::L);
                self.storage_sensor_cards(ui);
                ui.add_space(theme::space::L);
                self.bridge_sensor_cards(ui, now);
            });
    }

    /// CPU and motherboard readings from the specs sensor bridge. One compact
    /// gap row when no provider is running, instead of four "Unavailable"
    /// rows and a paragraph of provider jargon.
    fn bridge_sensor_cards(&self, ui: &mut egui::Ui, now: Instant) {
        let t = self.colors();
        let bridge = &self.specs_view.bridge;
        let header = ui.scope(|ui| {
            widgets::section_header(ui, "CPU & motherboard", None, t);
        });
        let Value::Known(providers) = &bridge.status else {
            let Value::Unavailable(reason) = &bridge.status else {
                unreachable!("Value has only Known and Unavailable variants")
            };
            header
                .response
                .on_hover_text("Read-only values published by a running sensor provider.");
            widgets::gap_row(
                ui,
                "CPU and motherboard sensors",
                "Needs sensor app",
                &format!(
                    "{reason}. Trontop reads CPU and motherboard temperature only from an \
                     already-running LibreHardwareMonitor, OpenHardwareMonitor or HWiNFO \
                     (shared memory). It never substitutes an ACPI thermal zone or invents a \
                     value, and installs no driver."
                ),
                t,
            );
            return;
        };
        let fresh = bridge.collected_at.is_some_and(|at| {
            now.saturating_duration_since(at) <= crate::specs::BRIDGE_STALE_AFTER
        });
        header.response.on_hover_text(format!(
            "Read-only values published by {providers}. {} readings in total; every one is \
             listed on System > Sensor Sources. ACPI thermal zones are never shown as CPU \
             temperature.{}",
            bridge.readings.len(),
            if fresh {
                ""
            } else {
                " The provider stopped responding; these are the last readings received."
            }
        ));
        let keys = [
            ("CPU package temperature", LiveKey::CpuPackageTemperature),
            ("Motherboard temperature", LiveKey::MotherboardTemperature),
            (
                "CPU package power",
                LiveKey::Sensor {
                    id: crate::specs::cpu::PACKAGE_POWER.into(),
                },
            ),
            (
                "CPU core voltage",
                LiveKey::Sensor {
                    id: crate::specs::cpu::CORE_VOLTAGE.into(),
                },
            ),
        ];
        for (index, (label, key)) in keys.iter().enumerate() {
            let (text, color, hover) = self.live_parts(key, now, t);
            super::system::spec_text_row(
                ui,
                label,
                (&text, color),
                Some(&hover),
                index % 2 == 1,
                t,
            );
        }
        let mut cores = bridge
            .readings
            .iter()
            .filter(|r| matches!(r.key, LiveKey::CpuCoreTemperature { .. }))
            .collect::<Vec<_>>();
        cores.sort_by_key(|r| match r.key {
            LiveKey::CpuCoreTemperature { index } => index,
            _ => u32::MAX,
        });
        if !cores.is_empty() {
            ui.add_space(theme::space::S);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::splat(theme::space::XS);
                for reading in &cores {
                    let index = match reading.key {
                        LiveKey::CpuCoreTemperature { index } => index,
                        _ => 0,
                    };
                    widgets::hover_frame(
                        ui,
                        widgets::surface(ui, t, false).inner_margin(egui::Margin::symmetric(6, 3)),
                        |ui| {
                            ui.label(
                                RichText::new(format!(
                                    "C{index} {}",
                                    reading.unit.format(reading.value)
                                ))
                                .monospace()
                                .size(10.0)
                                .color(t.text),
                            );
                        },
                    );
                }
            });
        }
    }

    pub(super) fn gpu_sensor_performance(&self, ui: &mut egui::Ui) {
        let t = self.colors();
        let snapshot = &self.snapshot.gpu_sensors;
        let on_sensors_page = self.page == Page::Sensors;
        if !on_sensors_page {
            let hottest = snapshot
                .adapters
                .iter()
                .filter_map(|a| a.temperature_c)
                .max();
            widgets::performance_heading(
                ui,
                "GPU sensors",
                "NVML / read-only driver telemetry",
                &hottest.map_or_else(|| "-- °C".into(), |v| format!("{v} °C")),
                t.secondary,
                t,
            );
        }
        let placeholder = AdapterSensors {
            name: "Hardware sensors unavailable".into(),
            ..Default::default()
        };
        let adapters = if snapshot.adapters.is_empty() {
            std::slice::from_ref(&placeholder)
        } else {
            &snapshot.adapters
        };
        for (index, adapter) in adapters.iter().enumerate() {
            ui.push_id(("sensor_adapter", adapter.uuid.as_deref(), index), |ui| {
                let sample_line = if snapshot.using_cached {
                    format!(
                        "Cached reading / last success {}",
                        crate::diagnostics::age(snapshot.last_success, std::time::Instant::now())
                    )
                } else if snapshot.adapters.is_empty() {
                    "No readings / fields stay visible".into()
                } else {
                    format!(
                        "NVIDIA adapter {index} / live sample / {:.2} ms collection",
                        snapshot.query_millis
                    )
                };
                if on_sensors_page {
                    // A routine live-sample line is provenance, not a caveat: it
                    // moves to hover. A cached or empty reading is an active
                    // anomaly, so it stays on the header's hover too, kept out
                    // of the visible flow either way to cut page clutter.
                    let mut hover = sample_line.clone();
                    hover.push('\n');
                    hover.push_str(
                        snapshot
                            .error
                            .as_deref()
                            .unwrap_or("Read-only sensor provider"),
                    );
                    if adapter.uuid.is_none() {
                        hover.push_str(
                            "\nHistory unavailable: the driver did not expose a stable adapter ID.",
                        );
                    }
                    let header = ui.scope(|ui| {
                        widgets::section_header(ui, &adapter.name, None, t);
                    });
                    header.response.on_hover_text(hover);
                } else {
                    // Performance > GPU sensors keeps its original two-line
                    // header; this rail view is out of P7's scope.
                    ui.add(
                        egui::Label::new(
                            RichText::new(&adapter.name)
                                .size(17.0)
                                .strong()
                                .color(t.text),
                        )
                        .truncate(),
                    )
                    .on_hover_text(&adapter.name);
                    widgets::hover_label(
                        ui,
                        RichText::new(sample_line).size(10.0).color(t.text_muted),
                    )
                    .on_hover_text(
                        snapshot
                            .error
                            .as_deref()
                            .unwrap_or("Read-only sensor provider"),
                    );
                }
                if let Some(error) = &adapter.error {
                    widgets::hover_label(ui, RichText::new(error).color(t.text));
                }
                ui.add_space(8.0);
                let history = adapter
                    .uuid
                    .as_ref()
                    .and_then(|uuid| self.sensor_history.get(uuid));
                ui.columns(2, |columns| {
                    sensor_card(
                        &mut columns[0],
                        "GPU temperature",
                        temperature(adapter.temperature_c),
                        "GPU die sensor",
                        history,
                        false,
                        self.theme,
                        t,
                    );
                    sensor_card(
                        &mut columns[1],
                        "Board power",
                        watts(adapter.power_w),
                        "Driver-reported board draw",
                        history,
                        true,
                        self.theme,
                        t,
                    );
                });
                ui.add_space(8.0);
                let details = sensor_details(adapter);
                let memory_note = if adapter.memory.is_none() {
                    "VRAM is unavailable for this adapter."
                } else if adapter.memory_includes_reserved {
                    "Legacy VRAM reading includes driver reservations."
                } else {
                    "VRAM used excludes driver reservations."
                };
                if on_sensors_page {
                    // One row of four: the Sensors page is wide enough for it.
                    ui.columns(4, |columns| {
                        for (column, (label, value)) in columns.iter_mut().zip(details.iter()) {
                            render_metric(column, label, value, memory_note, t);
                        }
                    });
                } else {
                    // The narrower Performance rail keeps its original 2x2
                    // grid, out of P7's scope.
                    for row in details.chunks(2) {
                        ui.columns(2, |columns| {
                            for (column, (label, value)) in columns.iter_mut().zip(row) {
                                render_metric(column, label, value, memory_note, t);
                            }
                        });
                        ui.add_space(5.0);
                    }
                }
                if adapter.uuid.is_none() && !on_sensors_page {
                    widgets::hover_label(
                        ui,
                        RichText::new(
                            "History unavailable: the driver did not expose a stable adapter ID.",
                        )
                        .size(10.0)
                        .color(t.text_muted),
                    );
                }
                ui.add_space(18.0);
            });
        }
    }
}

fn temperature(value: Option<u32>) -> String {
    value.map_or_else(|| "Unavailable".into(), |v| format!("{v} °C"))
}

fn watts(value: Option<f32>) -> String {
    value.map_or_else(|| "Unavailable".into(), |v| format!("{v:.1} W"))
}

fn sensor_details(adapter: &AdapterSensors) -> [(&'static str, String); 4] {
    let clock =
        |value: Option<u32>| value.map_or_else(|| "Unavailable".into(), |v| format!("{v} MHz"));
    [
        ("GRAPHICS CLOCK", clock(adapter.graphics_clock_mhz)),
        ("MEMORY CLOCK", clock(adapter.memory_clock_mhz)),
        (
            "FAN TARGET",
            adapter
                .fan_percent
                .map_or_else(|| "Unavailable".into(), |v| format!("{v}%")),
        ),
        (
            "VRAM USED / TOTAL",
            adapter.memory.map_or_else(
                || "Unavailable".into(),
                |(used, total)| {
                    format!(
                        "{:.1} / {:.1} GiB",
                        used as f64 / 1_073_741_824.0,
                        total as f64 / 1_073_741_824.0
                    )
                },
            ),
        ),
    ]
}

/// Dispatches one of the four GPU metric cells: Fan target and VRAM carry
/// their old visible caveat line as hover instead; the rest use the plain
/// metric anatomy unchanged.
fn render_metric(ui: &mut egui::Ui, label: &str, value: &str, memory_note: &str, t: Tokens) {
    match label {
        "FAN TARGET" => metric_with_hover(
            ui,
            label,
            value,
            "Fan % is the intended speed, not measured RPM.",
            t,
        ),
        "VRAM USED / TOTAL" => metric_with_hover(ui, label, value, memory_note, t),
        _ => widgets::metric(ui, label, value, t),
    }
}

/// Same anatomy as [`widgets::metric`], with a caller-chosen hover instead of
/// the generic "label: value" text: where a caveat that used to sit on its
/// own visible line now lives.
fn metric_with_hover(ui: &mut egui::Ui, label: &str, value: &str, hover: &str, t: Tokens) {
    widgets::hover_frame(ui, widgets::surface(ui, t, false), |ui| {
        ui.set_min_width(ui.available_width());
        ui.add(
            egui::Label::new(RichText::new(label).size(10.0).strong().color(t.text_muted))
                .truncate(),
        );
        ui.add(
            egui::Label::new(RichText::new(value).size(17.0).monospace().color(t.text)).truncate(),
        );
    })
    .response
    .on_hover_text(hover.to_string());
}

#[allow(clippy::too_many_arguments)]
fn sensor_card(
    ui: &mut egui::Ui,
    title: &str,
    value: String,
    detail: &str,
    history: Option<&SensorHistory>,
    power: bool,
    settings: ThemeSettings,
    t: Tokens,
) {
    widgets::hover_frame(ui, widgets::surface(ui, t, power), |ui| {
        ui.set_min_width(ui.available_width());
        widgets::hover_label(
            ui,
            RichText::new(title).size(12.0).strong().color(t.text_muted),
        );
        ui.add(
            egui::Label::new(RichText::new(value).size(25.0).monospace().color(t.text)).truncate(),
        );
        widgets::hover_label(ui, RichText::new(detail).size(10.0).color(t.text_muted));
        ui.add_space(6.0);
        let color = if power { t.accent } else { t.secondary };
        sensor_graph(ui, history, power, color, settings, t);
        let peak = history.and_then(|h| h.peak(power));
        let peak = peak.map_or_else(
            || "No readings".into(),
            |v| {
                if power {
                    format!("Peak {v:.1} W")
                } else {
                    format!("Peak {v:.0} °C")
                }
            },
        );
        widgets::hover_label(
            ui,
            RichText::new(format!("{peak} · last 2 min"))
                .size(10.0)
                .color(t.text_muted),
        );
    });
}

fn sensor_graph(
    ui: &mut egui::Ui,
    history: Option<&SensorHistory>,
    power: bool,
    color: Color32,
    settings: ThemeSettings,
    t: Tokens,
) {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 106.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(
        rect,
        settings.roundness.min(6.0),
        if response.hovered() {
            t.row_hover
        } else {
            t.graph_bg
        },
    );
    let plot = rect.shrink2(Vec2::new(6.0, 12.0));
    for row in 1..4 {
        let y = egui::lerp(plot.top()..=plot.bottom(), row as f32 / 4.0);
        painter.line_segment(
            [egui::pos2(plot.left(), y), egui::pos2(plot.right(), y)],
            Stroke::new(0.5, t.border),
        );
    }
    if let Some(history) = history {
        let maximum = if power {
            history.peak(true).unwrap_or(1.0).max(1.0) * 1.15
        } else {
            history.peak(false).unwrap_or(100.0).max(100.0)
        };
        if let Some(last) = history.points.back() {
            let mut previous: Option<(egui::Pos2, std::time::Instant)> = None;
            for point in &history.points {
                let Some(value) = (if power {
                    point.power_w
                } else {
                    point.temperature_c
                }) else {
                    previous = None;
                    continue;
                };
                let age = last.at.duration_since(point.at).as_secs_f32();
                let position = egui::pos2(
                    plot.right() - (age / 120.0).clamp(0.0, 1.0) * plot.width(),
                    plot.bottom() - (value / maximum).clamp(0.0, 1.0) * plot.height(),
                );
                if let Some((prev, at)) = previous
                    && point.at.duration_since(at).as_secs_f32() <= 3.0
                {
                    let mut fill = egui::Mesh::default();
                    let top = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 42);
                    fill.colored_vertex(prev, top);
                    fill.colored_vertex(position, top);
                    fill.colored_vertex(
                        egui::pos2(position.x, plot.bottom()),
                        Color32::TRANSPARENT,
                    );
                    fill.colored_vertex(egui::pos2(prev.x, plot.bottom()), Color32::TRANSPARENT);
                    fill.add_triangle(0, 1, 2);
                    fill.add_triangle(0, 2, 3);
                    painter.add(egui::Shape::mesh(fill));
                    painter.line_segment([prev, position], Stroke::new(1.6, color));
                } else {
                    painter.circle_filled(position, 1.5, color);
                }
                previous = Some((position, point.at));
            }
        }
        response.on_hover_text(format!(
            "120-second history; gaps mean no reading. Scale 0 to {maximum:.0} {}.",
            if power { "W" } else { "°C" }
        ));
    }
}
