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
        let now = self.graphs.now();
        ui.spacing_mut().item_spacing.y = 0.0;
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
                            if let Some(gap) = self.cpu_temperature_gap(now) {
                                widgets::status_pill(ui, gap.label, t.text_muted)
                                    .on_hover_text(gap.hover);
                            }
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
        ui.add_space(theme::space::M);
        egui::ScrollArea::vertical()
            .id_salt("all_sensors_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Vertical rhythm is explicit on this page: no implicit item
                // spacing between blocks, only the shared scale below.
                ui.spacing_mut().item_spacing.y = 0.0;
                self.gpu_sensor_performance(ui);
                ui.add_space(theme::space::L);
                self.storage_sensor_cards(ui);
                ui.add_space(theme::space::L);
                self.bridge_sensor_cards(ui, now);
                ui.add_space(theme::space::L);
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
                &format!("No sensor app is running ({reason}).\n{CPU_TEMP_SOURCES}"),
                t,
            );
            return;
        };
        let fresh = bridge.collected_at.is_some_and(|at| {
            now.saturating_duration_since(at) <= crate::specs::BRIDGE_STALE_AFTER
        });
        // The same row rhythm as System's spec rows.
        ui.spacing_mut().item_spacing.y = 3.0;
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
        let placeholder = AdapterSensors {
            name: "Hardware sensors unavailable".into(),
            ..Default::default()
        };
        let adapters = if snapshot.adapters.is_empty() {
            std::slice::from_ref(&placeholder)
        } else {
            &snapshot.adapters
        };
        let provenance = |index: usize| {
            let mut text = if snapshot.using_cached {
                format!(
                    "Cached reading / last success {}",
                    crate::diagnostics::age(snapshot.last_success, self.graphs.now())
                )
            } else if snapshot.adapters.is_empty() {
                "No readings; the fields stay in place.".into()
            } else {
                format!(
                    "NVIDIA adapter {index} / live sample / {:.2} ms collection",
                    snapshot.query_millis
                )
            };
            text.push('\n');
            text.push_str(
                snapshot
                    .error
                    .as_deref()
                    .unwrap_or("NVML, read-only driver telemetry."),
            );
            text
        };
        if !on_sensors_page {
            let hottest = snapshot
                .adapters
                .iter()
                .filter_map(|a| a.temperature_c)
                .max();
            // One adapter (the common case): its name is the hero subline, so
            // no second name heading repeats it below.
            widgets::performance_heading_with_state(
                ui,
                "GPU sensors",
                &adapters[0].name,
                &hottest.map_or_else(|| "--".into(), |v| format!("{v} °C")),
                snapshot.using_cached.then_some(("Cached", t.text_muted)),
                t.secondary,
                t,
            )
            .on_hover_text(provenance(0));
        }
        // The Sensors page stacks drives and CPU sensors under the GPU, so its
        // plots are shorter; Performance > GPU sensors has the pane to itself.
        // Tall windows (1600x1000) get the full plot back.
        let plot_height = if on_sensors_page && ui.available_height() < 760.0 {
            78.0
        } else {
            106.0
        };
        for (index, adapter) in adapters.iter().enumerate() {
            ui.push_id(("sensor_adapter", adapter.uuid.as_deref(), index), |ui| {
                // Explicit vertical rhythm: the block looks the same on
                // Sensors and on Performance whatever the parent spacing.
                ui.spacing_mut().item_spacing.y = 0.0;
                if index > 0 {
                    ui.add_space(theme::space::L);
                }
                let mut hover = provenance(index);
                if adapter.uuid.is_none() {
                    hover.push_str(
                        "\nHistory unavailable: the driver did not expose a stable adapter ID.",
                    );
                }
                // Provenance is hover text on the adapter's header; a second
                // adapter on Performance gets its own header, the first one is
                // already named by the hero.
                if on_sensors_page || index > 0 {
                    let header = ui.scope(|ui| {
                        widgets::section_header(ui, &adapter.name, None, t);
                    });
                    header.response.on_hover_text(hover);
                } else {
                    ui.add_space(theme::space::M);
                }
                if let Some(error) = &adapter.error {
                    widgets::hover_label(ui, RichText::new(error).color(t.text));
                    ui.add_space(theme::space::S);
                }
                let history = adapter
                    .uuid
                    .as_ref()
                    .and_then(|uuid| self.sensor_history.get(uuid));
                ui.scope(|ui| {
                    ui.spacing_mut().item_spacing.x = theme::space::GAP;
                    ui.columns(2, |columns| {
                        sensor_card(
                            &mut columns[0],
                            "GPU temperature",
                            adapter.temperature_c.map(|v| format!("{v} °C")),
                            "GPU die sensor, read-only NVML.",
                            history,
                            SensorPlot {
                                power: false,
                                height: plot_height,
                            },
                            self.theme,
                            t,
                        );
                        sensor_card(
                            &mut columns[1],
                            "Board power",
                            adapter.power_w.map(|v| format!("{v:.1} W")),
                            "Driver-reported board draw, read-only NVML.",
                            history,
                            SensorPlot {
                                power: true,
                                height: plot_height,
                            },
                            self.theme,
                            t,
                        );
                    });
                });
                ui.add_space(theme::space::GAP);
                let details = sensor_details(adapter);
                let memory_note = if adapter.memory.is_none() {
                    "VRAM is unavailable for this adapter."
                } else if adapter.memory_includes_reserved {
                    "Legacy VRAM reading includes driver reservations; the total is the \
                     adapter's full memory."
                } else {
                    "VRAM used excludes driver reservations; the total is the adapter's full \
                     memory."
                };
                let per_row = if ui.available_width() >= 640.0 { 4 } else { 2 };
                ui.scope(|ui| {
                    ui.spacing_mut().item_spacing.x = theme::space::GAP;
                    for (row_index, row) in details.chunks(per_row).enumerate() {
                        if row_index > 0 {
                            ui.add_space(theme::space::GAP);
                        }
                        ui.columns(per_row, |columns| {
                            for (index, (column, (label, value))) in
                                columns.iter_mut().zip(row).enumerate()
                            {
                                render_metric(column, index, label, value, memory_note, t);
                            }
                        });
                    }
                });
            });
        }
    }
}

fn sensor_details(adapter: &AdapterSensors) -> [(&'static str, String); 4] {
    // A value the driver does not report is a muted "--" with the reason on
    // hover, never a word that reads like a measurement.
    let clock = |value: Option<u32>| value.map_or_else(|| "--".into(), |v| format!("{v} MHz"));
    [
        ("Graphics clock", clock(adapter.graphics_clock_mhz)),
        ("Memory clock", clock(adapter.memory_clock_mhz)),
        (
            "Fan target",
            adapter
                .fan_percent
                .map_or_else(|| "--".into(), |v| format!("{:.1}%", v as f32)),
        ),
        (
            "VRAM used",
            adapter.memory.map_or_else(
                || "--".into(),
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

/// One of the four GPU value tiles. Caveats (fan target is not RPM, what VRAM
/// used includes) and the reason for a gap live on hover.
fn render_metric(
    ui: &mut egui::Ui,
    index: usize,
    label: &str,
    value: &str,
    memory_note: &str,
    t: Tokens,
) {
    let mut hover = match label {
        "Fan target" => "Fan % is the intended speed, not measured RPM.".to_owned(),
        "VRAM used" => memory_note.to_owned(),
        _ => "Read-only NVML driver telemetry.".to_owned(),
    };
    if value == "--" {
        hover.push_str("\nNot reported by the driver for this adapter.");
    }
    widgets::value_tile(ui, label, value, &hover, None, index % 2 == 1, t);
}

/// Which reading a GPU sensor card plots, and how tall its plot is.
#[derive(Clone, Copy)]
struct SensorPlot {
    power: bool,
    height: f32,
}

/// Card anatomy: a 13 px title with a right-aligned "peak" chip, the 21 px
/// value (a muted "--" when the driver reports nothing), then the plot. The
/// source and caveats live on hover; there is no subtitle or footer line.
#[allow(clippy::too_many_arguments)]
fn sensor_card(
    ui: &mut egui::Ui,
    title: &str,
    value: Option<String>,
    source: &str,
    history: Option<&SensorHistory>,
    plot: SensorPlot,
    settings: ThemeSettings,
    t: Tokens,
) {
    let power = plot.power;
    widgets::hover_frame(
        ui,
        widgets::surface(ui, t, power).inner_margin(theme::CARD_PAD),
        |ui| {
            ui.set_min_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = theme::space::XS;
            let width = ui.available_width();
            let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 17.0), Sense::hover());
            let peak = history.and_then(|h| h.peak(power)).map(|v| {
                if power {
                    format!("peak {v:.1} W")
                } else {
                    format!("peak {v:.0} °C")
                }
            });
            let mut title_right = rect.right();
            if let Some(peak) = &peak {
                let galley = ui.painter().layout_no_wrap(
                    peak.clone(),
                    FontId::proportional(9.5),
                    t.text_muted,
                );
                let pill = egui::Rect::from_min_max(
                    egui::pos2(
                        rect.right() - galley.size().x - 12.0,
                        rect.center().y - galley.size().y / 2.0 - 2.0,
                    ),
                    egui::pos2(rect.right(), rect.center().y + galley.size().y / 2.0 + 2.0),
                );
                ui.painter().rect_filled(
                    pill,
                    pill.height() / 2.0,
                    t.surface(theme::mix(t.panel_raised, t.text_muted, 0.2)),
                );
                ui.painter().galley(
                    egui::pos2(pill.left() + 6.0, pill.center().y - galley.size().y / 2.0),
                    galley,
                    t.text_muted,
                );
                title_right = pill.left() - theme::space::S;
            }
            widgets::paint_text(
                ui,
                egui::Rect::from_min_max(rect.min, egui::pos2(title_right, rect.bottom())),
                title,
                FontId::proportional(13.0),
                t.text,
                Align::Min,
            );
            response.on_hover_text(format!(
                "{title}\n{source}{}",
                peak.as_deref().map_or_else(String::new, |p| format!(
                    "\nHighest in the last 2 minutes: {}",
                    &p[5..]
                ))
            ));
            let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 26.0), Sense::hover());
            let text = value.as_deref().unwrap_or("--");
            widgets::paint_text(
                ui,
                rect,
                text,
                FontId::monospace(21.0),
                if value.is_some() {
                    t.text
                } else {
                    t.text_muted
                },
                Align::Min,
            );
            if value.is_none() {
                response.on_hover_text("Not reported by the driver for this adapter.");
            }
            let color = if power { t.accent } else { t.secondary };
            sensor_graph(ui, history, plot, color, settings, t);
        },
    );
}

fn sensor_graph(
    ui: &mut egui::Ui,
    history: Option<&SensorHistory>,
    plot_spec: SensorPlot,
    color: Color32,
    settings: ThemeSettings,
    t: Tokens,
) {
    let power = plot_spec.power;
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), plot_spec.height),
        Sense::hover(),
    );
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
    let plot = rect.shrink2(Vec2::new(7.0, 19.0));
    for row in 0..=3 {
        let y = egui::lerp(plot.top()..=plot.bottom(), row as f32 / 3.0);
        painter.line_segment(
            [egui::pos2(plot.left(), y), egui::pos2(plot.right(), y)],
            Stroke::new(0.5, theme::mix(t.graph_bg, t.border, 0.65)),
        );
    }
    let peak = history.and_then(|h| h.peak(power));
    // Readable axis tops: GPU temperature shares Graphs' one rule with drive
    // temperature (a familiar 0-100 band, unless a reading runs hot), power
    // rounds up to a whole step ("150 W", never "131.6").
    let maximum = if power {
        format::nice_top(peak.unwrap_or(1.0) * 1.15)
    } else {
        format::celsius_axis_top(peak.unwrap_or(0.0))
    };
    let unit = if power { "W" } else { "°C" };
    for (anchor, offset, text) in [
        (
            egui::Align2::LEFT_TOP,
            rect.left_top() + Vec2::new(7.0, 4.0),
            format!("{maximum:.0} {unit}"),
        ),
        (
            egui::Align2::LEFT_BOTTOM,
            rect.left_bottom() + Vec2::new(7.0, -4.0),
            "-120 s".into(),
        ),
        (
            egui::Align2::RIGHT_BOTTOM,
            rect.right_bottom() - Vec2::new(7.0, 4.0),
            "now".into(),
        ),
    ] {
        painter.text(offset, anchor, text, FontId::monospace(9.0), t.text_muted);
    }
    let Some(history) = history else {
        return;
    };
    let ink = t.ink(color);
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
                let top = ink.gamma_multiply(0.22);
                fill.colored_vertex(prev, top);
                fill.colored_vertex(position, top);
                fill.colored_vertex(egui::pos2(position.x, plot.bottom()), Color32::TRANSPARENT);
                fill.colored_vertex(egui::pos2(prev.x, plot.bottom()), Color32::TRANSPARENT);
                fill.add_triangle(0, 1, 2);
                fill.add_triangle(0, 2, 3);
                painter.add(egui::Shape::mesh(fill));
                painter.line_segment([prev, position], Stroke::new(1.5, ink));
            } else {
                painter.circle_filled(position, 1.5, ink);
            }
            previous = Some((position, point.at));
        }
    }
    response.on_hover_text(format!(
        "120-second history; gaps mean no reading. Scale 0 to {maximum:.0} {unit}."
    ));
}

/// Why no CPU temperature is shown, in the one wording every page uses
/// (Overview, Graphs, Hardware sensors, System).
pub(super) struct CpuTempGap {
    /// "CPU temp: needs sensor app": chips, pills and gap lines.
    pub label: &'static str,
    /// "Needs sensor app": a value cell whose row already says CPU temperature.
    pub short: &'static str,
    /// The reason plus which sensor apps Trontop can read.
    pub hover: String,
}

/// The sensor apps named in every CPU temperature gap tooltip.
pub(super) const CPU_TEMP_SOURCES: &str = "Trontop reads CPU temperature only from HWiNFO \
     (with Shared Memory Support on) or LibreHardwareMonitor / OpenHardwareMonitor while one \
     is already running. It never substitutes an ACPI thermal zone or invents a value, and \
     installs no driver.";

impl TrontopApp {
    /// `None` while a fresh CPU package temperature is published; otherwise
    /// the shared gap wording for the bridge state at `now`.
    pub(super) fn cpu_temperature_gap(&self, now: Instant) -> Option<CpuTempGap> {
        let bridge = &self.specs_view.bridge;
        let fresh = bridge.collected_at.is_some_and(|at| {
            now.saturating_duration_since(at) <= crate::specs::BRIDGE_STALE_AFTER
        });
        let published = fresh
            && bridge.readings.iter().any(|r| {
                r.key == crate::specs::LiveKey::CpuPackageTemperature && r.value.is_finite()
            });
        if published {
            return None;
        }
        let (label, short, reason) = match (&bridge.status, fresh) {
            _ if self.specs.is_none() => (
                "CPU temp: not checked yet",
                "Not checked yet",
                "Sensor apps have not been checked yet.".to_owned(),
            ),
            (Value::Known(_), true) => (
                "CPU temp: not in sensor app",
                "Not in sensor app",
                "The running sensor app does not publish a CPU package temperature.".to_owned(),
            ),
            (Value::Known(_), false) => (
                "CPU temp: sensor app stopped",
                "Sensor app stopped",
                "The sensor app stopped responding.".to_owned(),
            ),
            (Value::Unavailable(reason), _) => (
                "CPU temp: needs sensor app",
                "Needs sensor app",
                format!("No sensor app is running ({reason})."),
            ),
        };
        Some(CpuTempGap {
            label,
            short,
            hover: format!("{reason}\n{CPU_TEMP_SOURCES}"),
        })
    }
}
