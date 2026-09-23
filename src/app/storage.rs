use super::graphs::Signal;
use super::*;
use std::time::Instant;

impl TrontopApp {
    /// One compact card per drive that reports a sensor; a drive with no
    /// sensor at all is a [`widgets::gap_row`], never a card with an empty
    /// plot.
    pub(super) fn storage_sensor_cards(&self, ui: &mut egui::Ui) {
        let t = self.colors();
        let now = Instant::now();
        let snapshot = &self.snapshot.storage_sensors;
        let header = ui.scope(|ui| {
            widgets::section_header(ui, "Drives", None, t);
        });
        header
            .response
            .on_hover_text("Windows storage metadata / 5 s sampling / 60 s retry after errors.");
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
            widgets::gap_row(
                ui,
                "Drives",
                "No inventory",
                if snapshot.inventory_at.is_some() {
                    "No supported drive sensor inventory is available."
                } else {
                    "Waiting for the first background inventory. No temperatures are estimated."
                },
                t,
            );
            return;
        }
        for (index, drive) in snapshot.drives.iter().enumerate() {
            if drive.temperatures.sensors.is_empty() {
                let reason = drive.error.as_ref().map_or_else(
                    || "The storage driver does not report a temperature sensor.".to_owned(),
                    ToString::to_string,
                );
                widgets::gap_row(ui, &drive.device.name, "Not reported", &reason, t);
                continue;
            }
            self.drive_card(ui, drive, now, index % 2 == 1, t);
            ui.add_space(theme::space::S);
        }
    }

    fn drive_card(
        &self,
        ui: &mut egui::Ui,
        drive: &crate::storage_sensors::DriveReading,
        now: Instant,
        banded: bool,
        t: Tokens,
    ) {
        let status = drive.status(now);
        let chip = drive_chip(status, t);
        let card =
            widgets::hover_frame(
                ui,
                widgets::surface(ui, t, banded).inner_margin(theme::CARD_PAD),
                |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.spacing_mut().item_spacing.y = theme::space::XS;
                    // A fixed-height row so a missing chip (the common Live case)
                    // never shifts the sensor row beneath it.
                    ui.allocate_ui_with_layout(
                        Vec2::new(ui.available_width(), 22.0),
                        Layout::left_to_right(Align::Center),
                        |ui| {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&drive.device.name)
                                        .size(13.0)
                                        .strong()
                                        .color(t.text),
                                )
                                .truncate(),
                            );
                            if let Some((label, color)) = chip {
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    widgets::status_pill(ui, label, color);
                                });
                            }
                        },
                    );
                    ui.horizontal(|ui| {
                        for sensor in &drive.temperatures.sensors {
                            ui.label(
                                RichText::new(format!("S{}", sensor.index))
                                    .size(10.0)
                                    .color(t.text_muted),
                            );
                            ui.add(
                            egui::Label::new(
                                RichText::new(celsius(sensor.celsius))
                                    .monospace()
                                    .size(11.0)
                                    .color(t.text),
                            )
                            .truncate(),
                        )
                        .on_hover_text(format!(
                            "Upper threshold: {}\nLower threshold: {}\nThreshold notifications: {}",
                            celsius(sensor.over_threshold),
                            celsius(sensor.under_threshold),
                            if sensor.event { "enabled" } else { "not enabled" }
                        ));
                            ui.add_space(theme::space::M);
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let (rect, _) =
                                ui.allocate_exact_size(Vec2::new(48.0, 18.0), Sense::hover());
                            let series = hottest_series(self, drive);
                            widgets::sparkline(
                                ui.painter(),
                                rect,
                                series.into_iter(),
                                100.0,
                                t.good,
                                t,
                            );
                        });
                    });
                },
            );
        let mut hover = format!(
            "{}\nLast usable reading: {}",
            drive.device.name,
            crate::diagnostics::age(drive.last_success, now)
        );
        if let Some(ms) = drive.query_millis {
            hover.push_str(&format!("\n{ms:.2} ms query"));
        }
        hover.push_str(&format!(
            "\nDevice warning {} / critical {}",
            celsius(drive.temperatures.warning),
            celsius(drive.temperatures.critical)
        ));
        hover.push_str(&match &drive.error {
            Some(error) => format!("\n{error}"),
            None => "\nRead-only / no driver installation".to_owned(),
        });
        card.response.on_hover_text(hover);
    }
}

/// Chip vocabulary restricted to the three words the sensors page uses
/// everywhere else: no chip at all for Live, otherwise Cached or Stale.
fn drive_chip(status: &str, t: Tokens) -> Option<(&'static str, Color32)> {
    match status {
        "Live" => None,
        "Cached" => Some(("Cached", t.text_muted)),
        _ => Some(("Stale", t.danger)),
    }
}

fn celsius(value: Option<i16>) -> String {
    value.map_or_else(|| "-- °C".into(), |value| format!("{value} °C"))
}

/// Element-wise maximum across a drive's sensor histories, so the sparkline
/// always traces the hottest reading at each point in time.
fn hottest_series(
    app: &TrontopApp,
    drive: &crate::storage_sensors::DriveReading,
) -> Vec<Option<f32>> {
    let series: Vec<Vec<Option<f32>>> = drive
        .temperatures
        .sensors
        .iter()
        .filter_map(|sensor| {
            app.graphs
                .series(Signal::DriveTemperature(&drive.device.id, sensor.index))
                .map(|view| view.values)
        })
        .collect();
    let length = series.iter().map(Vec::len).max().unwrap_or(0);
    (0..length)
        .map(|back| {
            series
                .iter()
                .filter_map(|values| {
                    values
                        .len()
                        .checked_sub(length - back)
                        .and_then(|index| values[index])
                })
                .reduce(f32::max)
        })
        .collect()
}
