use super::*;
use crate::gpu_sensors::{AdapterSensors, SensorHistory};

impl TrontopApp {
    pub(super) fn gpu_sensor_performance(&self, ui: &mut egui::Ui) {
        let t = self.colors();
        if self.page != Page::Sensors {
            widgets::performance_heading(
                ui,
                "GPU sensors",
                "Read-only NVIDIA driver telemetry",
                "NVML",
                t.secondary,
                t,
            );
        }
        let snapshot = &self.snapshot.gpu_sensors;
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
                widgets::hover_frame(ui, widgets::surface(ui, t, index % 2 == 1), |ui| {
                    ui.set_min_width(ui.available_width());
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
                        RichText::new(if snapshot.using_cached {
                            format!(
                                "Cached reading / last success {}",
                                crate::diagnostics::age(
                                    snapshot.last_success,
                                    std::time::Instant::now()
                                )
                            )
                        } else if snapshot.adapters.is_empty() {
                            "No readings / fields stay visible".into()
                        } else {
                            format!(
                                "NVIDIA adapter {index} / live sample / {:.2} ms collection",
                                snapshot.query_millis
                            )
                        })
                        .size(10.0)
                        .color(t.text_muted),
                    )
                    .on_hover_text(
                        snapshot
                            .error
                            .as_deref()
                            .unwrap_or("Read-only sensor provider"),
                    );
                });
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
                for row in details.chunks(2) {
                    ui.columns(2, |columns| {
                        for (column, (label, value)) in columns.iter_mut().zip(row) {
                            widgets::metric(column, label, value, t);
                        }
                    });
                    ui.add_space(5.0);
                }
                let memory_note = if adapter.memory.is_none() {
                    "VRAM is unavailable for this adapter."
                } else if adapter.memory_includes_reserved {
                    "Legacy VRAM reading includes driver reservations."
                } else {
                    "VRAM used excludes driver reservations."
                };
                widgets::hover_label(
                    ui,
                    RichText::new(format!(
                        "Fan % is the intended speed, not measured RPM. {memory_note}"
                    ))
                    .size(10.0)
                    .color(t.text_muted),
                );
                if adapter.uuid.is_none() {
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
        widgets::hover_label(ui, RichText::new("Unavailable means unsupported or inaccessible, not zero. CPU and storage sensors are not connected yet.")
            .size(11.0).color(t.text_muted));
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
                        "{:.2} / {:.2} GiB",
                        used as f64 / 1_073_741_824.0,
                        total as f64 / 1_073_741_824.0
                    )
                },
            ),
        ),
    ]
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
