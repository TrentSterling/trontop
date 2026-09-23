//! One graph-first dashboard backed only by the existing background providers.
use super::*;
use std::time::{Duration, Instant};
mod history;
mod wall;
pub(super) use history::disk_short_name as disk_label;
use history::{Group, History, WINDOW};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Style {
    #[default]
    Lines,
    Bars,
}

/// A Graphs series the Overview tiles read. Overview never keeps a second copy
/// of this history and never polls hardware itself.
#[derive(Clone, Copy)]
pub(super) enum Signal<'a> {
    /// Busiest GPU engine across adapters, including partial (lower-bound) points.
    GpuActivity,
    CpuClockAverage,
    /// NVML adapter temperature, by adapter UUID.
    GpuTemperature(&'a str),
    /// NVML board power, by adapter UUID.
    GpuPower(&'a str),
    /// One drive temperature sensor, by device interface id and sensor index.
    DriveTemperature(&'a str, u16),
}

/// Read-only view of one series: recent values (gaps are `None`), the latest
/// measured value and its state label.
pub(super) struct SeriesView {
    pub values: Vec<Option<f32>>,
    pub current: Option<f32>,
    pub state: &'static str,
}

#[derive(Default)]
pub(super) struct Dashboard {
    history: History,
    style: Style,
    filter: Option<Group>,
    /// Performance > CPU shows the per-core grid instead of the Total CPU graph.
    pub(super) cpu_all_cores: bool,
    pub(super) gpu_selected: Option<crate::gpu_adapters::Key>,
    // Reference path for geometry and same-binary CPU comparisons only.
    #[cfg(test)]
    pub(super) draw_all_rows: bool,
    #[cfg(test)]
    pub(super) laid_out_cards: usize,
    /// Cards the current tab composes (measured, not folded), for tests.
    #[cfg(test)]
    pub(super) wall_cards: usize,
    #[cfg(test)]
    pub(super) fixed_now: Option<Instant>,
}
impl Dashboard {
    /// Performance > GPU: the selected adapter's per-engine or memory charts.
    /// Per-engine-instance charts appear only here, never on the wall.
    pub(super) fn adapter_charts(
        &self,
        ui: &mut egui::Ui,
        key: crate::gpu_adapters::Key,
        memory: bool,
        t: Tokens,
    ) {
        let now = self.now();
        let all: Vec<_> = self
            .history
            .charts
            .iter()
            .filter(|c| match c.id {
                history::Id::Adapter(k, metric) => {
                    k == key && if memory { metric < 3 } else { metric >= 4 }
                }
                _ => false,
            })
            .collect();
        let measured: Vec<_> = all.iter().filter(|c| wall::measured(c, now)).collect();
        if measured.len() < all.len() {
            let missing: Vec<_> = all
                .iter()
                .filter(|c| !wall::measured(c, now))
                .map(|c| c.title.as_str())
                .collect();
            widgets::gap_row(
                ui,
                &format!(
                    "{} {} with no value in the last 2 minutes",
                    missing.len(),
                    if missing.len() == 1 {
                        "counter"
                    } else {
                        "counters"
                    }
                ),
                "Not reported",
                &missing.join(", "),
                t,
            );
        }
        // Individual engine instances that stayed silent for the whole window
        // fold into one muted line instead of flooding the tab with flat
        // cards. VRAM cards (memory) are never idle-folded: a 0-byte reading
        // is still a real value.
        let (active, idle): (Vec<_>, Vec<_>) = if memory {
            (measured, Vec::new())
        } else {
            measured
                .into_iter()
                .partition(|c| wall::window_max(c, now).unwrap_or(0.0) > 0.0)
        };
        // "3D / engine 0" reads as a "3D" card with an "engine 0" chip.
        let charts: Vec<_> = active
            .iter()
            .map(|c| {
                let mut card = wall::Card::single(c);
                if let Some((kind, instance)) = c.title.split_once(" / ") {
                    card.title = kind.to_owned();
                    card.device = Some(instance.to_owned());
                }
                card
            })
            .collect();
        let width = ui.available_width();
        let cols = widgets::tile_grid_columns(width);
        let mut height = 0.0;
        for (row, chunk) in charts.chunks(cols).enumerate() {
            let size = Vec2::new(width, height);
            if row == 0
                || ui.is_rect_visible(egui::Rect::from_min_size(ui.next_widget_position(), size))
            {
                let response = ui.push_id(("gpu-charts", key, memory, row), |ui| {
                    ui.set_width(width);
                    ui.spacing_mut().item_spacing.x = theme::space::GAP;
                    // A short last row keeps its siblings' width unless
                    // stretching stays within 1.5x; never a lone wide card.
                    ui.columns(widgets::row_columns(cols, chunk.len()), |columns| {
                        for (index, (column, card)) in columns.iter_mut().zip(chunk).enumerate() {
                            column.push_id(&card.key, |ui| {
                                card_view(ui, card, now, self.style, (row + index) % 2 == 1, t)
                            });
                        }
                    });
                });
                if row == 0 {
                    height = response.response.rect.height();
                }
            } else {
                ui.allocate_space(size);
            }
            ui.add_space(theme::space::GAP);
        }
        if !idle.is_empty() {
            let names: Vec<&str> = idle.iter().map(|c| c.title.as_str()).collect();
            widgets::gap_row(
                ui,
                &format!(
                    "{} idle {}",
                    idle.len(),
                    if idle.len() == 1 { "engine" } else { "engines" }
                ),
                "Idle",
                &names.join(", "),
                t,
            );
        }
    }
    /// Adapters that produced a measured wall value in the window and are not
    /// folded as idle on the Graphs wall (an unused iGPU), in history order.
    /// Software adapters and silent counter identities never count.
    pub(super) fn active_adapters(&self) -> Vec<crate::gpu_adapters::Key> {
        let now = self.now();
        let idle = wall::idle_adapter_keys(&self.history, now);
        let mut keys = Vec::new();
        for chart in &self.history.charts {
            if let history::Id::Adapter(key, _) = chart.id
                && chart.wall
                && !idle.contains(&key)
                && !keys.contains(&key)
                && wall::measured(chart, now)
            {
                keys.push(key);
            }
        }
        keys
    }
    /// Performance > network: one adapter's Receive and Send as two lines
    /// with a legend on a shared rate axis, the same history and drawing as
    /// the Graphs network card.
    pub(super) fn network_graph(&self, ui: &mut egui::Ui, name: &str, height: f32, t: Tokens) {
        self.two_line_graph(
            ui,
            [
                (history::Id::Network(name.to_owned(), 0), "Receive"),
                (history::Id::Network(name.to_owned(), 1), "Send"),
            ],
            (
                "Traffic history",
                "No sample of this adapter has been recorded yet.",
            ),
            name,
            height,
            t,
        );
    }

    /// Performance > volume: Read and Write as two lines with a legend on a
    /// shared, rounded rate axis, drawn like the network graph.
    pub(super) fn volume_graph(&self, ui: &mut egui::Ui, mount: &str, height: f32, t: Tokens) {
        self.two_line_graph(
            ui,
            [
                (history::Id::Volume(mount.to_owned(), 0), "Read"),
                (history::Id::Volume(mount.to_owned(), 1), "Write"),
            ],
            (
                "Throughput history",
                "No sample of this volume has been recorded yet.",
            ),
            mount,
            height,
            t,
        );
    }

    fn two_line_graph(
        &self,
        ui: &mut egui::Ui,
        lines: [(history::Id, &'static str); 2],
        (gap_title, gap_reason): (&str, &str),
        title: &str,
        height: f32,
        t: Tokens,
    ) {
        let now = self.now();
        let series: Vec<_> = lines
            .iter()
            .filter_map(|(id, label)| {
                self.history
                    .chart(id)
                    .map(|chart| wall::Series { chart, label })
            })
            .collect();
        if series.is_empty() {
            widgets::gap_row(ui, gap_title, "Starting", gap_reason, t);
            return;
        }
        let mut card = wall::Card::single(series[0].chart);
        card.title = title.to_owned();
        card.combine = wall::Combine::Sum;
        card.series = series;
        let colors = [t.secondary, t.accent];
        plot(ui, &card, now, self.style, &colors, t, height);
    }

    pub(super) fn now(&self) -> Instant {
        #[cfg(test)]
        if let Some(now) = self.fixed_now {
            return now;
        }
        Instant::now()
    }
    pub(super) fn ensure_sample(&mut self, snapshot: &SystemSnapshot) {
        if self.history.charts.is_empty() {
            self.sample(snapshot, Instant::now());
        }
    }

    pub(super) fn cpu_grid(&self, ui: &mut egui::Ui, logical_count: usize, t: Tokens) {
        if logical_count == 0 {
            widgets::hover_label(ui, "Waiting for logical processor inventory");
            return;
        }
        let count = logical_count.min(256);
        let cols = grid_columns(
            count,
            (((ui.available_width() + 6.0) / 112.0).floor() as usize).clamp(1, 8),
        );
        let width = ui.available_width();
        let now = Instant::now();
        #[cfg(test)]
        let now = self.fixed_now.unwrap_or(now);
        let mut row_height = 0.0;
        for first in (0..count).step_by(cols) {
            let size = Vec2::new(width, row_height);
            let visible = first == 0
                || ui.is_rect_visible(egui::Rect::from_min_size(ui.next_widget_position(), size));
            #[cfg(test)]
            let visible = visible || self.draw_all_rows;
            if !visible {
                ui.allocate_space(size);
                ui.add_space(4.0);
                continue;
            }
            let response = ui.push_id(("cpu-grid", first), |ui| {
                ui.set_width(width);
                // A trailing partial row stretches to the full width instead
                // of leaving an orphan gap.
                ui.columns(cols.min(count - first), |columns| {
                    for (offset, column) in columns.iter_mut().enumerate() {
                        let index = first + offset;
                        let chart = self.history.chart(&history::Id::Cpu(index));
                        column.push_id(index, |ui| {
                            widgets::hover_frame(ui, widgets::surface(ui, t, (first / cols + offset) % 2 == 1).inner_margin(5), |ui| {
                                ui.set_min_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(format!("CPU {index}")).size(10.0).strong().color(t.text));
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        let value = chart.map_or_else(|| "--".into(), |c| format!("{}{}", if c.state(now) == "Live" { "" } else { "~" }, c.value_label()));
                                        ui.add(egui::Label::new(RichText::new(value).size(10.0).monospace().color(t.text)).truncate())
                                            .on_hover_text("Logical processor busy time. ~ means the value is retained, not a fresh measurement.");
                                    });
                                });
                                if let Some(chart) = chart {
                                    plot(ui, &wall::Card::single(chart), now, self.style, &[t.accent], t, 72.0);
                                } else {
                                    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 72.0), Sense::hover());
                                    ui.painter().rect_filled(rect, 4.0, t.graph_bg);
                                    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "No data", FontId::proportional(10.0), t.text_muted);
                                }
                            });
                        });
                    }
                });
            });
            if first == 0 {
                row_height = response.response.rect.height();
            }
            ui.add_space(4.0);
        }
        if logical_count > count {
            widgets::hover_label(
                ui,
                format!("Showing {count} of {logical_count} logical processors (graph budget)."),
            );
        }
    }

    pub(super) fn sample(&mut self, snapshot: &SystemSnapshot, now: Instant) {
        self.history.sample(snapshot, now);
    }

    pub(super) fn series(&self, signal: Signal<'_>) -> Option<SeriesView> {
        let id = match signal {
            Signal::GpuActivity => history::Id::Activity("GPU activity".into()),
            Signal::CpuClockAverage => history::Id::CpuClock(0),
            Signal::GpuTemperature(uuid) => history::Id::Gpu(uuid.into(), 0),
            Signal::GpuPower(uuid) => history::Id::Gpu(uuid.into(), 1),
            Signal::DriveTemperature(drive, sensor) => {
                history::Id::Temperature(drive.into(), sensor)
            }
        };
        let now = self.now();
        let chart = self.history.chart(&id)?;
        Some(SeriesView {
            values: self.history.recent(&id, now).unwrap_or_default(),
            current: chart.current,
            state: chart.state(now),
        })
    }

    /// Open the Graphs page's All cores tab (the Overview "All cores" link).
    pub(super) fn show_all_cores(&mut self) {
        self.filter = Some(Group::Cores);
    }
}

impl TrontopApp {
    /// `(short chip text, full reason)` when the sensor bridge publishes no CPU
    /// temperature. Graphs never substitutes an ACPI zone or a guess.
    fn cpu_temperature_gap(&self) -> Option<(&'static str, String)> {
        let bridge = &self.specs_view.bridge;
        let fresh = bridge.collected_at.is_some_and(|at| {
            self.graphs.now().saturating_duration_since(at) <= crate::specs::BRIDGE_STALE_AFTER
        });
        let published = fresh
            && bridge.readings.iter().any(|r| {
                r.key == crate::specs::LiveKey::CpuPackageTemperature && r.value.is_finite()
            });
        if published {
            return None;
        }
        const SOURCES: &str = "Trontop reads CPU temperature only from an already-running LibreHardwareMonitor, OpenHardwareMonitor or HWiNFO (read-only). It never substitutes an ACPI thermal zone or invents a value, and installs no driver. Click for Hardware sensors.";
        Some(match (&bridge.status, fresh) {
            _ if self.specs.is_none() => (
                "CPU temp: not checked",
                format!("CPU temperature: not checked yet. {SOURCES}"),
            ),
            (crate::specs::Value::Known(_), true) => (
                "CPU temp: not reported",
                format!("CPU temperature: the running sensor app does not report it. {SOURCES}"),
            ),
            (crate::specs::Value::Known(_), false) => (
                "CPU temp: sensor app stopped",
                format!("CPU temperature: the sensor app stopped responding. {SOURCES}"),
            ),
            (crate::specs::Value::Unavailable(reason), _) => (
                "CPU temp: no sensor app",
                format!("CPU temperature: no sensor app is running ({reason}). {SOURCES}"),
            ),
        })
    }

    pub(super) fn graphs_page(&mut self, ui: &mut egui::Ui) {
        let t = self.colors();
        let now = self.graphs.now();
        if self.graphs.history.charts.is_empty() {
            self.graphs.sample(&self.snapshot, now);
        }
        self.page_header(
            ui,
            "Graphs",
            "Your machine, on one timeline. Hover a graph to inspect a reading.",
            false,
        );
        let previous_filter = self.graphs.filter;
        let cpu_temperature = self.cpu_temperature_gap();
        let mut chip_placed = false;
        let mut open_sensors = false;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = theme::space::XS;
            ui.spacing_mut().button_padding = Vec2::new(8.0, 4.0);
            tab(ui, &mut self.graphs.filter, None, "Everything");
            for group in Group::ALL {
                tab(ui, &mut self.graphs.filter, Some(group), group.label());
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                style_toggle(ui, &mut self.graphs.style, t);
                if let Some((short, reason)) = cpu_temperature
                    .as_ref()
                    .filter(|_| matches!(self.graphs.filter, None | Some(Group::Thermal)))
                {
                    let needed = ui
                        .painter()
                        .layout_no_wrap((*short).into(), FontId::proportional(10.0), t.text_muted)
                        .size()
                        .x
                        + 24.0;
                    if ui.available_width() >= needed {
                        ui.add_space(theme::space::S);
                        open_sensors |= gap_chip(ui, short, reason, t);
                        chip_placed = true;
                    }
                }
            });
        });
        ui.add_space(theme::space::M);
        let page_width = ui.available_width();
        let mut scroll = egui::ScrollArea::vertical()
            .id_salt("graph_wall_scroll")
            .auto_shrink([false, false]);
        if self.graphs.filter != previous_filter {
            scroll = scroll.vertical_scroll_offset(0.0);
        }
        #[cfg(test)]
        let mut laid_out = 0;
        #[cfg(test)]
        let draw_all_rows = self.graphs.draw_all_rows;
        #[cfg(test)]
        let mut wall_cards = 0;
        scroll.show(ui, |ui| {
            if self.graphs.filter == Some(Group::Cores) {
                self.graphs.cpu_grid(ui, self.snapshot.cpu.logical_cores, t);
                return;
            }
            let graphs = &self.graphs;
            let wall = wall::compose(&graphs.history, graphs.filter, now);
            #[cfg(test)]
            {
                wall_cards = wall.cards.len();
            }
            // Columns follow the page width, not the width left beside a
            // scroll bar, so a long tab and a short one share one grid.
            let count = widgets::tile_grid_columns(page_width);
            if wall.cards.is_empty() {
                widgets::gap_row(
                    ui,
                    if graphs.history.charts.is_empty() {
                        "Waiting for the first samples"
                    } else {
                        "Nothing in this category is reporting"
                    },
                    "No data",
                    "Only measured values are graphed. Other categories keep running.",
                    t,
                );
            }
            let mut row_height = 0.0;
            let row_width = ui.available_width();
            let rows = wall_rows(&wall.cards, count, graphs.filter == Some(Group::Storage));
            for (row, chunk) in rows.into_iter().enumerate() {
                // Every card has the same single-line anatomy, so one measured
                // row height places every offscreen row exactly.
                let row_size = Vec2::new(row_width, row_height);
                let visible = row == 0
                    || ui.is_rect_visible(egui::Rect::from_min_size(
                        ui.next_widget_position(),
                        row_size,
                    ));
                #[cfg(test)]
                let visible = visible || draw_all_rows;
                if visible {
                    let response = ui.push_id(("graph-row", row), |ui| {
                        // Do not let pixel rounding in an earlier row grow
                        // later columns cumulatively at fractional UI scale.
                        ui.set_width(row_width);
                        ui.spacing_mut().item_spacing.x = theme::space::GAP;
                        // A short last row stretches: no orphan cards.
                        ui.columns(chunk.len(), |columns| {
                            for (index, (column, card)) in
                                columns.iter_mut().zip(chunk).enumerate()
                            {
                                column.push_id(&card.key, |ui| {
                                    card_view(
                                        ui,
                                        card,
                                        now,
                                        graphs.style,
                                        (row + index) % 2 == 1,
                                        t,
                                    );
                                });
                            }
                        });
                    });
                    if row == 0 {
                        row_height = response.response.rect.height();
                    }
                    #[cfg(test)]
                    {
                        laid_out += chunk.len();
                    }
                } else {
                    // scope/push_id and allocate_space each consume one auto
                    // ID, keeping later rows' hover IDs and geometry stable.
                    ui.allocate_space(row_size);
                }
                // Rows sit one grid gap apart, the same as columns.
                ui.add_space((theme::space::GAP - ui.spacing().item_spacing.y).max(0.0));
            }
            let footer = |ui: &mut egui::Ui, text: String, hover: String| {
                ui.add(
                    egui::Label::new(RichText::new(text).size(11.0).color(t.text_muted))
                        .truncate(),
                )
                .on_hover_text(hover);
            };
            for (line, hover) in &wall.idle_adapters {
                footer(ui, line.clone(), hover.clone());
            }
            if !wall.idle.is_empty() {
                footer(
                    ui,
                    format!("Idle engines: {}", wall.idle.join(", ")),
                    format!(
                        "These GPU engine types stayed at exactly 0.0% for the whole 2 minute window, so they are listed here instead of drawing flat cards:\n{}",
                        wall.idle.join("\n")
                    ),
                );
            }
            if !wall.unreported.is_empty() {
                let n = wall.unreported.len();
                footer(
                    ui,
                    format!(
                        "{n} {} not reported",
                        if n == 1 { "signal" } else { "signals" }
                    ),
                    format!(
                        "Left off the wall because they produced no value in the last 2 minutes:\n{}",
                        wall.unreported.join("\n")
                    ),
                );
            }
            let thermal_view = matches!(graphs.filter, None | Some(Group::Thermal));
            if !chip_placed
                && thermal_view
                && let Some((short, reason)) = &cpu_temperature
            {
                open_sensors |= gap_chip(ui, short, reason, t);
            }
            if graphs.history.omitted > 0 {
                footer(
                    ui,
                    format!(
                        "Graph budget: 512 series. {} more are not charted.",
                        graphs.history.omitted
                    ),
                    "Trontop keeps at most 512 histories so the wall stays fast.".into(),
                );
            }
        });
        #[cfg(test)]
        {
            self.graphs.laid_out_cards = laid_out;
            self.graphs.wall_cards = wall_cards;
        }
        if open_sensors {
            self.page = Page::Sensors;
        }
    }
}

/// One category tab. A framed (selected or hovered) button is one stroke
/// width larger on every side than a frameless one, which dropped the active
/// tab's text and every later tab by 1 px. A fixed outer size that fits the
/// framed button keeps every label on one baseline and one position.
fn tab(ui: &mut egui::Ui, filter: &mut Option<Group>, value: Option<Group>, label: &str) {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let text = ui
        .painter()
        .layout_no_wrap(label.into(), font, Color32::PLACEHOLDER)
        .size();
    let pad = ui.spacing().button_padding;
    let stroke = ui
        .visuals()
        .widgets
        .active
        .bg_stroke
        .width
        .max(ui.visuals().widgets.hovered.bg_stroke.width)
        .max(ui.visuals().selection.stroke.width);
    let size = Vec2::new(
        text.x + pad.x * 2.0 + stroke * 2.0,
        (text.y + pad.y * 2.0 + stroke * 2.0).max(ui.spacing().interact_size.y),
    );
    let response = ui.add(egui::Button::selectable(*filter == value, label).min_size(size.ceil()));
    if response.clicked() && *filter != value {
        *filter = value;
    }
}

/// Split wall cards into grid rows of at most `count`. On the Disks tab each
/// disk starts a new row and its four cards form one block: a single row at
/// four columns, otherwise two rows of two, so a disk never wraps into the
/// next disk's row.
fn wall_rows<'c, 'a>(
    cards: &'c [wall::Card<'a>],
    count: usize,
    per_device: bool,
) -> Vec<&'c [wall::Card<'a>]> {
    if !per_device {
        return cards.chunks(count.max(1)).collect();
    }
    let mut rows = Vec::new();
    let mut rest = cards;
    while let Some(first) = rest.first() {
        let block = rest.iter().take_while(|c| c.device == first.device).count();
        let (device, tail) = rest.split_at(block);
        let columns = if count >= 4 { 4 } else { count.min(2) }.max(1);
        rows.extend(device.chunks(columns));
        rest = tail;
    }
    rows
}

/// Compact Lines / Bars segmented control.
fn style_toggle(ui: &mut egui::Ui, style: &mut Style, t: Tokens) {
    egui::Frame::new()
        .fill(t.graph_bg)
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(ui.visuals().widgets.inactive.corner_radius)
        .inner_margin(egui::Margin::same(2))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.spacing_mut().button_padding = Vec2::new(7.0, 2.0);
            // right_to_left parents add Bars first so Lines stays on the left.
            let reversed = ui.layout().prefer_right_to_left();
            let order = if reversed {
                [(Style::Bars, "Bars"), (Style::Lines, "Lines")]
            } else {
                [(Style::Lines, "Lines"), (Style::Bars, "Bars")]
            };
            ui.horizontal(|ui| {
                for (value, label) in order {
                    ui.selectable_value(style, value, RichText::new(label).size(12.0));
                }
            });
        });
}

/// Columns for `count` equal tiles when at most `fit` fit side by side: the
/// largest divisor of `count` that still uses at least half the room, so 24
/// cores lay out 6 x 4 rather than 7 + 7 + 7 + 3. Counts with no such divisor
/// (primes) use every column and stretch the last row instead.
fn grid_columns(count: usize, fit: usize) -> usize {
    let fit = fit.clamp(1, count.max(1));
    (fit.div_ceil(2)..=fit)
        .rev()
        .find(|cols| count.is_multiple_of(*cols))
        .unwrap_or(fit)
}

/// A muted, clickable gap chip; returns true when clicked.
fn gap_chip(ui: &mut egui::Ui, short: &str, reason: &str, t: Tokens) -> bool {
    ui.add(
        egui::Button::new(RichText::new(short).size(10.0).color(t.text_muted))
            .fill(theme::mix(t.panel_raised, t.text_muted, 0.12))
            .stroke(Stroke::NONE)
            .corner_radius(10.0)
            .small(),
    )
    .on_hover_text(reason)
    .clicked()
}

/// A right-aligned pill ending at `right`; returns its left edge.
#[allow(clippy::too_many_arguments)]
fn paint_pill(
    ui: &egui::Ui,
    right: f32,
    center_y: f32,
    text: &str,
    size: f32,
    max_width: f32,
    fill: Color32,
    color: Color32,
) -> f32 {
    let pad = 6.0;
    let mut job =
        egui::text::LayoutJob::simple_singleline(text.into(), FontId::proportional(size), color);
    job.wrap.max_width = (max_width - pad * 2.0).max(8.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
    let pill = egui::Rect::from_min_max(
        egui::pos2(
            right - galley.size().x - pad * 2.0,
            center_y - galley.size().y / 2.0 - 2.0,
        ),
        egui::pos2(right, center_y + galley.size().y / 2.0 + 2.0),
    );
    ui.painter().rect_filled(pill, pill.height() / 2.0, fill);
    ui.painter().galley(
        egui::pos2(pill.left() + pad, center_y - galley.size().y / 2.0),
        galley,
        color,
    );
    pill.left()
}

/// Short state chip text and color, or `None` for Live.
fn state_chip(state: &str, t: Tokens) -> Option<(&'static str, Color32)> {
    match state {
        "Live" => None,
        "Partial" => Some(("Partial", t.secondary)),
        "Cached" => Some(("Cached", t.text_muted)),
        "Starting" | "Warming" => Some(("Starting", t.text_muted)),
        _ => Some(("Stale", t.danger)),
    }
}

fn series_colors(card: &wall::Card<'_>, t: Tokens) -> Vec<Color32> {
    if card.series.len() > 1 {
        if card.group == Group::Network {
            // Receive/Send share one color rule with Performance > Wi-Fi:
            // receive is the secondary token, send is the accent token.
            return vec![t.secondary, t.accent];
        }
        return vec![
            t.accent,
            t.secondary,
            theme::mix(t.accent, t.secondary, 0.5),
        ];
    }
    vec![match card.group {
        Group::System | Group::Cores => t.accent,
        Group::Memory | Group::Thermal | Group::Network => t.secondary,
        Group::Gpu => theme::mix(t.accent, t.secondary, 0.5),
        Group::Storage => t.good,
    }]
}

/// Plot height inside a wall card. With the title, value and padding a card
/// stays at or under 150 px, so a 1000x580 window shows two full rows and
/// part of a third.
pub(super) const CARD_PLOT_HEIGHT: f32 = 80.0;

/// Card anatomy: a 13 px title with a device chip, a 21 px value with a state
/// chip only when not Live, then the plot. Provenance lives in hover text.
fn card_view(
    ui: &mut egui::Ui,
    card: &wall::Card<'_>,
    now: Instant,
    style: Style,
    banded: bool,
    t: Tokens,
) {
    let colors = series_colors(card, t);
    widgets::hover_frame(
        ui,
        widgets::surface(ui, t, banded).inner_margin(theme::CARD_PAD),
        |ui| {
            ui.set_min_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = theme::space::XS;
            let width = ui.available_width();
            let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 17.0), Sense::hover());
            let mut title_right = rect.right();
            if let Some(device) = &card.device {
                title_right = paint_pill(
                    ui,
                    rect.right(),
                    rect.center().y,
                    device,
                    9.5,
                    (width * 0.5).max(48.0),
                    t.surface(theme::mix(t.panel_raised, t.text_muted, 0.2)),
                    t.text_muted,
                ) - theme::space::S;
            }
            widgets::paint_text(
                ui,
                egui::Rect::from_min_max(rect.min, egui::pos2(title_right, rect.bottom())),
                &card.title,
                FontId::proportional(13.0),
                t.text,
                Align::Min,
            );
            response.on_hover_text(card.hover(now));
            let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 26.0), Sense::hover());
            let mut value_right = rect.right();
            if let Some((label, color)) = state_chip(card.state(now), t) {
                value_right = paint_pill(
                    ui,
                    rect.right(),
                    rect.center().y,
                    label,
                    10.0,
                    80.0,
                    t.surface(theme::mix(t.panel_raised, color, 0.18)),
                    t.ink(color),
                ) - theme::space::S;
            }
            let value = card.value_label();
            widgets::paint_text(
                ui,
                egui::Rect::from_min_max(rect.min, egui::pos2(value_right, rect.bottom())),
                &value,
                FontId::monospace(21.0),
                if value == "--" { t.text_muted } else { t.text },
                Align::Min,
            );
            response.on_hover_text(
                "Only measured values enter the graph. Gaps mark missing or stale samples; hollow rings and a trailing + mark partial (lower-bound) samples. Cached values are never extended into fake history.",
            );
            plot(ui, card, now, style, &colors, t, CARD_PLOT_HEIGHT);
        },
    );
}

/// "S0 44  S1 44  S2 41 °C" style legend entries, or labels only.
fn legend_entries(card: &wall::Card<'_>, with_values: bool) -> Vec<String> {
    let celsius = matches!(card.series[0].chart.unit, history::Unit::Celsius);
    let last = card.series.len() - 1;
    card.series
        .iter()
        .enumerate()
        .map(|(index, series)| {
            if !with_values {
                return series.label.to_owned();
            }
            let value = match (series.chart.current, celsius) {
                (None, _) => "--".to_owned(),
                (Some(v), true) if index != last => format!("{v:.0}"),
                (Some(_), _) => series.chart.value_label(),
            };
            format!("{} {value}", series.label)
        })
        .collect()
}

fn plot(
    ui: &mut egui::Ui,
    card: &wall::Card<'_>,
    now: Instant,
    style: Style,
    colors: &[Color32],
    t: Tokens,
    height: f32,
) {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
    // Offscreen cards still occupy layout space, but emit no chart geometry.
    if !ui.is_rect_visible(rect) {
        return;
    }
    let painter = ui.painter_at(rect);
    painter.rect_filled(
        rect,
        5.0,
        if response.hovered() {
            theme::mix(t.graph_bg, t.row_hover, 0.45)
        } else {
            t.graph_bg
        },
    );
    let plot = rect.shrink2(Vec2::new(7.0, 19.0));
    let (low, high) = card.range(now);
    for index in 0..=3 {
        let y = egui::lerp(plot.top()..=plot.bottom(), index as f32 / 3.0);
        painter.line_segment(
            [egui::pos2(plot.left(), y), egui::pos2(plot.right(), y)],
            Stroke::new(0.5, theme::mix(t.graph_bg, t.border, 0.65)),
        );
    }
    for index in 1..4 {
        let x = egui::lerp(plot.left()..=plot.right(), index as f32 / 4.0);
        painter.line_segment(
            [egui::pos2(x, plot.top()), egui::pos2(x, plot.bottom())],
            Stroke::new(0.5, theme::mix(t.graph_bg, t.border, 0.5)),
        );
    }
    let position = |at: Instant, value: f32| {
        egui::pos2(
            plot.right()
                - now.saturating_duration_since(at).as_secs_f32() / WINDOW.as_secs_f32()
                    * plot.width(),
            plot.bottom() - (value - low) / (high - low) * plot.height(),
        )
    };
    let lanes = card.series.len();
    let fill_alpha = if lanes > 1 { 0.10 } else { 0.22 };
    let mut any_value = false;
    for (lane, series) in card.series.iter().enumerate() {
        let chart = series.chart;
        let ink = t.ink(colors[lane.min(colors.len() - 1)]);
        let visible: Vec<_> = chart
            .points
            .iter()
            .filter(|p| p.at <= now && now.duration_since(p.at) <= WINDOW)
            .collect();
        any_value |= visible.iter().any(|p| p.value.is_some());
        if style == Style::Bars {
            let slot = (plot.width() * chart.cadence.as_secs_f32() / WINDOW.as_secs_f32() * 0.72)
                .clamp(1.0, 12.0);
            let bar_width = (slot / lanes as f32).max(1.0);
            let baseline = position(now, 0.0).y;
            for point in &visible {
                if let Some(value) = point.value {
                    let p = position(point.at, value);
                    let right = p.x - bar_width * (lanes - 1 - lane) as f32;
                    let bar = egui::Rect::from_two_pos(
                        egui::pos2((right - bar_width).max(plot.left()), baseline),
                        egui::pos2(right, p.y),
                    );
                    painter.rect_filled(bar, 0.5, ink);
                    if value == 0.0 {
                        painter.circle_filled(egui::pos2(right, p.y), 1.0, ink);
                    }
                }
            }
            continue;
        }
        let mut fill = egui::Mesh::default();
        for pair in visible.windows(2) {
            let [a, b] = pair else {
                continue;
            };
            let (Some(av), Some(bv)) = (a.value, b.value) else {
                continue;
            };
            if b.at.saturating_duration_since(a.at) > chart.cadence * 3 {
                continue;
            }
            let a = position(a.at, av);
            let b = position(b.at, bv);
            let base = fill.vertices.len() as u32;
            fill.colored_vertex(a, ink.gamma_multiply(fill_alpha));
            fill.colored_vertex(b, ink.gamma_multiply(fill_alpha));
            fill.colored_vertex(egui::pos2(b.x, plot.bottom()), Color32::TRANSPARENT);
            fill.colored_vertex(egui::pos2(a.x, plot.bottom()), Color32::TRANSPARENT);
            fill.add_triangle(base, base + 1, base + 2);
            fill.add_triangle(base, base + 2, base + 3);
        }
        painter.add(egui::Shape::mesh(fill));
        for pair in visible.windows(2) {
            if let (Some(a), Some(b)) = (pair[0].value, pair[1].value)
                && pair[1].at.saturating_duration_since(pair[0].at) <= chart.cadence * 3
            {
                painter.line_segment(
                    [position(pair[0].at, a), position(pair[1].at, b)],
                    Stroke::new(1.5, ink),
                );
            }
        }
        for point in &visible {
            if let Some(value) = point.value {
                if point.partial {
                    // Lower-bound sample: hollow ring, so it never reads as exact.
                    painter.circle_stroke(position(point.at, value), 2.0, Stroke::new(1.0, ink));
                } else {
                    painter.circle_filled(position(point.at, value), 1.0, ink);
                }
            }
        }
    }
    let unit = card.series[0].chart.unit;
    let axis = painter.text(
        rect.left_top() + Vec2::new(7.0, 4.0),
        egui::Align2::LEFT_TOP,
        unit.axis_label(high),
        FontId::monospace(9.0),
        t.text_muted,
    );
    painter.text(
        rect.left_bottom() + Vec2::new(7.0, -4.0),
        egui::Align2::LEFT_BOTTOM,
        "-120 s",
        FontId::monospace(9.0),
        t.text_muted,
    );
    painter.text(
        rect.right_bottom() - Vec2::new(7.0, 4.0),
        egui::Align2::RIGHT_BOTTOM,
        "now",
        FontId::monospace(9.0),
        t.text_muted,
    );
    if lanes > 1 {
        // Legend in the top strip, right of the axis label.
        let room = rect.right() - 7.0 - (axis.right() + 10.0);
        let layout = |with_values: bool| {
            legend_entries(card, with_values)
                .into_iter()
                .map(|text| {
                    ui.fonts_mut(|f| f.layout_no_wrap(text, FontId::monospace(9.0), t.text_muted))
                })
                .collect::<Vec<_>>()
        };
        let width = |galleys: &[std::sync::Arc<egui::Galley>]| {
            galleys.iter().map(|g| g.size().x + 10.0).sum::<f32>()
                + 8.0 * (galleys.len() - 1) as f32
        };
        let mut galleys = layout(true);
        if width(&galleys) > room {
            galleys = layout(false);
        }
        if width(&galleys) <= room {
            let mut x = rect.right() - 7.0 - width(&galleys);
            let y = rect.top() + 4.0;
            for (lane, galley) in galleys.into_iter().enumerate() {
                let ink = t.ink(colors[lane.min(colors.len() - 1)]);
                let h = galley.size().y;
                painter.rect_filled(
                    egui::Rect::from_min_size(egui::pos2(x, y + h / 2.0 - 3.0), Vec2::splat(6.0)),
                    1.0,
                    ink,
                );
                x += 10.0;
                let w = galley.size().x;
                painter.galley(egui::pos2(x, y), galley, t.text_muted);
                x += w + 8.0;
            }
        }
    }
    if !any_value {
        painter.text(
            plot.center(),
            egui::Align2::CENTER_CENTER,
            "No data",
            FontId::proportional(10.0),
            t.text_muted,
        );
    }
    if let Some(pointer) = response.hover_pos() {
        let first = card.series[0].chart;
        let nearest = first
            .points
            .iter()
            .filter(|p| p.at <= now && now.duration_since(p.at) <= WINDOW)
            .min_by(|a, b| {
                let ax = (position(a.at, 0.0).x - pointer.x).abs();
                let bx = (position(b.at, 0.0).x - pointer.x).abs();
                ax.total_cmp(&bx)
            });
        if let Some(point) = nearest {
            let x = position(point.at, 0.0).x;
            painter.line_segment(
                [egui::pos2(x, plot.top()), egui::pos2(x, plot.bottom())],
                Stroke::new(1.0, t.text_muted),
            );
            let mut text = card.title.clone();
            for series in &card.series {
                // Each series' own sample nearest in time to the hovered one.
                let value = series
                    .chart
                    .points
                    .iter()
                    .min_by_key(|p| {
                        if p.at > point.at {
                            p.at - point.at
                        } else {
                            point.at - p.at
                        }
                    })
                    .and_then(|p| p.value);
                let value = value.map_or_else(
                    || "No exact measurement".into(),
                    |v| series.chart.unit.format(v),
                );
                if series.label.is_empty() {
                    text.push_str(&format!("\n{value}"));
                } else {
                    text.push_str(&format!("\n{}: {value}", series.chart.title));
                }
            }
            text.push_str(&format!(
                "\n{:.1} seconds ago",
                now.saturating_duration_since(point.at).as_secs_f32()
            ));
            response.on_hover_text(text);
        }
    }
}

/// Gauntlet render harness: choose a Graphs tab and style without clicking.
#[cfg(test)]
impl Dashboard {
    /// Tabs in toolbar order: `None` is Everything, then each group.
    pub(super) fn gauntlet_tabs() -> Vec<(Option<usize>, &'static str)> {
        std::iter::once((None, "Everything"))
            .chain(
                Group::ALL
                    .iter()
                    .enumerate()
                    .map(|(i, g)| (Some(i), g.label())),
            )
            .collect()
    }
    pub(super) fn set_gauntlet_view(&mut self, tab: Option<usize>, bars: bool) {
        self.filter = tab.map(|i| Group::ALL[i]);
        self.style = if bars { Style::Bars } else { Style::Lines };
    }
    pub(super) fn gauntlet_signal_count(&self) -> usize {
        self.history.charts.len()
    }
}

#[cfg(test)]
mod tests;
