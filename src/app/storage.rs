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
        // Drives with sensors form a card grid (one column below 760 px, then
        // the shared breakpoints); drives without any sensor follow as compact
        // gap rows, so a silent drive never leaves a hole in the grid.
        let (cards, silent): (Vec<_>, Vec<_>) = snapshot
            .drives
            .iter()
            .enumerate()
            .partition(|(_, drive)| !drive.temperatures.sensors.is_empty());
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(theme::space::GAP, 0.0);
            let width = ui.available_width();
            let columns = if width >= 760.0 {
                widgets::tile_grid_columns(width).min(cards.len().max(1))
            } else {
                1
            };
            for (row_index, row) in cards.chunks(columns.max(1)).enumerate() {
                if row_index > 0 {
                    ui.add_space(theme::space::GAP);
                }
                // A short last row stretches to fill the width: no orphans.
                ui.columns(row.len(), |cells| {
                    for (cell, (index, drive)) in cells.iter_mut().zip(row) {
                        self.drive_card(cell, drive, now, index % 2 == 1, t);
                    }
                });
            }
        });
        if !silent.is_empty() && !cards.is_empty() {
            ui.add_space(theme::space::S);
        }
        for (_, drive) in silent {
            let reason = drive.error.as_ref().map_or_else(
                || "The storage driver does not report a temperature sensor.".to_owned(),
                ToString::to_string,
            );
            widgets::gap_row(ui, &drive.device.name, "Not reported", &reason, t);
        }
    }

    /// Drive card: the name (and a Cached or Stale chip) over the per-sensor
    /// values on the left 55 percent; the right 45 percent is a sparkline of
    /// the hottest sensor at the card's full inner height.
    fn drive_card(
        &self,
        ui: &mut egui::Ui,
        drive: &crate::storage_sensors::DriveReading,
        now: Instant,
        banded: bool,
        t: Tokens,
    ) {
        const NAME_HEIGHT: f32 = 17.0;
        const VALUES_HEIGHT: f32 = 22.0;
        const INNER_HEIGHT: f32 = NAME_HEIGHT + theme::space::XS + VALUES_HEIGHT;
        let status = drive.status(now);
        let chip = drive_chip(status, t);
        let card = widgets::hover_frame(
            ui,
            widgets::surface(ui, t, banded).inner_margin(theme::CARD_PAD),
            |ui| {
                ui.set_min_width(ui.available_width());
                let (inner, _) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), INNER_HEIGHT),
                    Sense::hover(),
                );
                let split = inner.left() + inner.width() * 0.55;
                let left = egui::Rect::from_min_max(
                    inner.min,
                    egui::pos2(split - theme::space::M, inner.bottom()),
                );
                let spark = egui::Rect::from_min_max(egui::pos2(split, inner.top()), inner.max);

                // Name row, with the chip right-aligned inside the left half.
                let name_row =
                    egui::Rect::from_min_size(left.min, Vec2::new(left.width(), NAME_HEIGHT));
                let mut name_right = name_row.right();
                if let Some((label, color)) = chip {
                    let galley = ui.painter().layout_no_wrap(
                        label.to_owned(),
                        FontId::proportional(10.0),
                        color,
                    );
                    let pill = egui::Rect::from_min_size(
                        egui::pos2(
                            name_row.right() - galley.size().x - 12.0,
                            name_row.center().y - 8.0,
                        ),
                        Vec2::new(galley.size().x + 12.0, 16.0),
                    );
                    ui.painter().rect_filled(
                        pill,
                        8.0,
                        t.surface(theme::mix(t.panel_raised, color, 0.2)),
                    );
                    ui.painter().galley(
                        egui::pos2(pill.left() + 6.0, pill.center().y - galley.size().y / 2.0),
                        galley,
                        color,
                    );
                    name_right = pill.left() - theme::space::S;
                }
                widgets::paint_text(
                    ui,
                    egui::Rect::from_min_max(
                        name_row.min,
                        egui::pos2(name_right, name_row.bottom()),
                    ),
                    &drive.device.name,
                    FontId::proportional(13.0),
                    t.text,
                    Align::Min,
                );

                // One value per sensor: a muted "S0" label then the reading.
                let values = egui::Rect::from_min_max(
                    egui::pos2(left.left(), left.bottom() - VALUES_HEIGHT),
                    left.max,
                );
                let mut x = values.left();
                for sensor in &drive.temperatures.sensors {
                    let label = format!("S{}", sensor.index);
                    let label_width = ui
                        .painter()
                        .layout_no_wrap(label.clone(), FontId::proportional(10.0), t.text_muted)
                        .size()
                        .x;
                    let value = celsius(sensor.celsius);
                    let value_width = ui
                        .painter()
                        .layout_no_wrap(value.clone(), FontId::monospace(15.0), t.text)
                        .size()
                        .x;
                    let width = label_width + theme::space::XS + value_width;
                    // A narrow card clips its last values rather than
                    // painting them over the sparkline.
                    if x >= values.right() {
                        break;
                    }
                    let cell = egui::Rect::from_min_size(
                        egui::pos2(x, values.top()),
                        Vec2::new(width + 2.0, values.height()),
                    )
                    .intersect(values);
                    widgets::paint_text(
                        ui,
                        egui::Rect::from_min_size(
                            cell.min,
                            Vec2::new(label_width + 1.0, cell.height()),
                        ),
                        &label,
                        FontId::proportional(10.0),
                        t.text_muted,
                        Align::Min,
                    );
                    widgets::paint_text(
                        ui,
                        egui::Rect::from_min_max(
                            egui::pos2(cell.left() + label_width + theme::space::XS, cell.top()),
                            cell.max,
                        ),
                        &value,
                        FontId::monospace(15.0),
                        if sensor.celsius.is_some() {
                            t.text
                        } else {
                            t.text_muted
                        },
                        Align::Min,
                    );
                    ui.interact(
                        cell,
                        ui.id().with(("drive_sensor", sensor.index)),
                        Sense::hover(),
                    )
                    .on_hover_text(format!(
                        "Sensor {}: {value}\nUpper threshold: {}\nLower threshold: {}\n\
                         Threshold notifications: {}",
                        sensor.index,
                        celsius(sensor.over_threshold),
                        celsius(sensor.under_threshold),
                        if sensor.event {
                            "enabled"
                        } else {
                            "not enabled"
                        }
                    ));
                    x = cell.right() + theme::space::L;
                }

                // Sparkline of the hottest sensor on a fixed 0 to 100 °C scale.
                ui.painter().rect_filled(spark, 4.0, t.graph_bg);
                widgets::sparkline(
                    ui.painter(),
                    spark.shrink2(Vec2::new(4.0, 3.0)),
                    hottest_series(self, drive).into_iter(),
                    100.0,
                    t.good,
                    t,
                );
                ui.painter().text(
                    spark.left_top() + Vec2::new(5.0, 3.0),
                    egui::Align2::LEFT_TOP,
                    "100 °C",
                    FontId::monospace(9.0),
                    t.text_muted,
                );
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
        hover.push_str("\nTrend: hottest sensor, 0 to 100 °C.");
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
