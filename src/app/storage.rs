use super::*;
use std::time::Instant;

impl TrontopApp {
    pub(super) fn storage_sensor_cards(&self, ui: &mut egui::Ui) {
        let t = self.colors();
        let snapshot = &self.snapshot.storage_sensors;
        widgets::section_label(ui, "Drive temperatures", t);
        widgets::hover_label(
            ui,
            RichText::new("Windows storage metadata / 5 s sampling / 60 s retry after errors")
                .size(11.0)
                .color(t.text_muted),
        );
        if let Some(error) = &snapshot.inventory_error {
            widgets::hover_label(
                ui,
                RichText::new(format!(
                    "Inventory: {error}. Previous device fields are retained."
                ))
                .size(11.0)
                .color(t.text),
            );
        }
        if snapshot.drives.is_empty() {
            widgets::detail_row(ui, "Drive sensor", "-- °C", t);
            widgets::hover_label(
                ui,
                RichText::new(if snapshot.inventory_at.is_some() {
                    "No supported drive sensor inventory is available."
                } else {
                    "Waiting for the first background inventory. No temperatures are estimated."
                })
                .size(11.0)
                .color(t.text_muted),
            );
        }
        for (index, drive) in snapshot.drives.iter().enumerate() {
            let now = Instant::now();
            widgets::hover_frame(ui, widgets::surface(ui, t, index % 2 == 1), |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    widgets::status_pill(
                        ui,
                        drive.status(now),
                        if drive.live(now) { t.good } else { t.text },
                    );
                    ui.add(
                        egui::Label::new(RichText::new(&drive.device.name).strong().color(t.text))
                            .truncate(),
                    )
                    .on_hover_text(&drive.device.name);
                });
                ui.horizontal(|ui| {
                    widgets::hover_label(
                        ui,
                        RichText::new(format!(
                            "Last usable reading: {}",
                            crate::diagnostics::age(drive.last_success, now)
                        ))
                        .size(10.0)
                        .color(t.text_muted),
                    );
                    if let Some(ms) = drive.query_millis {
                        widgets::hover_label(
                            ui,
                            RichText::new(format!("/ {ms:.2} ms query"))
                                .size(10.0)
                                .color(t.text_muted),
                        );
                    }
                });
                if drive.temperatures.sensors.is_empty() {
                    storage_row(ui, "Sensor", "-- °C", false, t);
                }
                for (index, sensor) in drive.temperatures.sensors.iter().enumerate() {
                    let response = storage_row(
                        ui,
                        &format!("Sensor {}", sensor.index),
                        &celsius(sensor.celsius),
                        index % 2 == 1,
                        t,
                    );
                    response.on_hover_text(format!("Device-reported sensor index, not a CPU core or mount letter. Index 0 may be composite.\nUpper threshold: {}\nLower threshold: {}\nThreshold notifications: {}", celsius(sensor.over_threshold), celsius(sensor.under_threshold), if sensor.event { "enabled" } else { "not enabled" }));
                }
                ui.horizontal(|ui| {
                    widgets::hover_label(
                        ui,
                        RichText::new(format!(
                            "Device warning {} / critical {}",
                            celsius(drive.temperatures.warning),
                            celsius(drive.temperatures.critical)
                        ))
                        .size(10.0)
                        .color(t.text_muted),
                    )
                    .on_hover_text(
                        "Limits reported by the device. Trontop does not change thresholds.",
                    );
                });
                // A fixed one-line status slot prevents ordinary refreshes from
                // moving the sensor rows. Errors get their full text on hover.
                let note = drive.error.as_ref().map_or_else(
                    || "Read-only / no driver installation".into(),
                    ToString::to_string,
                );
                ui.add(
                    egui::Label::new(RichText::new(&note).size(10.0).color(t.text_muted))
                        .truncate(),
                )
                .on_hover_text(note);
            });
            ui.add_space(6.0);
        }
    }
}

fn celsius(value: Option<i16>) -> String {
    value.map_or_else(|| "-- °C".into(), |value| format!("{value} °C"))
}

fn storage_row(
    ui: &mut egui::Ui,
    name: &str,
    value: &str,
    banded: bool,
    t: Tokens,
) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 27.0), Sense::hover());
    let fill = if response.hovered() {
        ui.visuals().widgets.hovered.weak_bg_fill
    } else if banded {
        ui.visuals().faint_bg_color
    } else {
        t.panel_raised
    };
    ui.painter().rect_filled(rect, 3.0, fill);
    ui.painter().text(
        rect.left_center() + Vec2::new(4.0, 0.0),
        egui::Align2::LEFT_CENTER,
        name,
        FontId::proportional(12.0),
        t.text,
    );
    ui.painter().text(
        rect.right_center() - Vec2::new(4.0, 0.0),
        egui::Align2::RIGHT_CENTER,
        value,
        FontId::monospace(16.0),
        t.text,
    );
    response
}
