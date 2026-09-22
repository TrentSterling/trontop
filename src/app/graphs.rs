//! One graph-first dashboard backed only by the existing background providers.
use super::*;
use std::time::{Duration, Instant};
mod history;
use history::{Chart, Group, History, WINDOW};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Style {
    #[default]
    Lines,
    Bars,
}

#[derive(Default)]
pub(super) struct Dashboard {
    history: History,
    style: Style,
    filter: Option<Group>,
    pub(super) cpu_total: bool,
    pub(super) gpu_selected: Option<crate::gpu_adapters::Key>,
    // Reference path for geometry and same-binary CPU comparisons only.
    #[cfg(test)]
    pub(super) draw_all_rows: bool,
    #[cfg(test)]
    pub(super) laid_out_cards: usize,
    #[cfg(test)]
    pub(super) fixed_now: Option<Instant>,
}
impl Dashboard {
    pub(super) fn adapter_charts(
        &self,
        ui: &mut egui::Ui,
        key: crate::gpu_adapters::Key,
        memory: bool,
        t: Tokens,
    ) {
        let charts: Vec<_> = self
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
        let cols = if ui.available_width() >= 480.0 { 2 } else { 1 };
        let width = ui.available_width();
        let mut height = 0.0;
        let now = self.now();
        for (row, chunk) in charts.chunks(cols).enumerate() {
            let size = Vec2::new(width, height);
            if row == 0
                || ui.is_rect_visible(egui::Rect::from_min_size(ui.next_widget_position(), size))
            {
                let response = ui.push_id(("gpu-charts", key, memory, row), |ui| {
                    ui.set_width(width);
                    ui.columns(cols, |columns| {
                        for (index, (column, chart)) in columns.iter_mut().zip(chunk).enumerate() {
                            column.push_id(&chart.id, |ui| {
                                card(ui, chart, now, self.style, (row + index) % 2 == 1, t)
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
            ui.add_space(6.0);
        }
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

    pub(super) fn controls(&mut self, ui: &mut egui::Ui, t: Tokens) {
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.style, Style::Lines, "Lines");
            ui.selectable_value(&mut self.style, Style::Bars, "Bars");
            widgets::hover_label(
                ui,
                RichText::new(format!(
                    "{} signals / 120 seconds / hover to inspect",
                    self.history.charts.len()
                ))
                .size(11.0)
                .color(t.text_muted),
            );
        });
    }

    pub(super) fn cpu_grid(&self, ui: &mut egui::Ui, logical_count: usize, t: Tokens) {
        if logical_count == 0 {
            widgets::hover_label(ui, "Waiting for logical processor inventory");
            return;
        }
        let count = logical_count.min(256);
        let cols = (((ui.available_width() + 6.0) / 112.0).floor() as usize)
            .clamp(1, 8)
            .min(count);
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
                ui.columns(cols, |columns| {
                    for (offset, column) in columns.iter_mut().enumerate().take(count - first) {
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
                                    plot(ui, chart, now, self.style, t.accent, t, 72.0);
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

    // Share timestamped series with Graphs; Overview never polls hardware or
    // creates a second copy of history. A clipped row reserves space only.
    pub(super) fn overview_wall(&self, ui: &mut egui::Ui, headline: bool, t: Tokens) {
        let is_headline = |chart: &&Chart| {
            matches!(
                chart.id,
                history::Id::System(_) | history::Id::Activity(_) | history::Id::CpuClock(_)
            ) && chart.group == Group::System
                || matches!(
                    chart.id,
                    history::Id::Gpu(_, 0 | 1) | history::Id::Network(_, _)
                )
        };
        let charts: Vec<_> = self
            .history
            .charts
            .iter()
            .filter(|chart| chart.group != Group::Cores)
            .filter(|chart| is_headline(chart) == headline)
            .collect();
        let cols = ((ui.available_width() / 250.0).floor() as usize).clamp(1, 5);
        let width = ui.available_width();
        let mut height = 0.0;
        let now = Instant::now();
        #[cfg(test)]
        let now = self.fixed_now.unwrap_or(now);
        for (row, chunk) in charts.chunks(cols).enumerate() {
            let size = Vec2::new(width, height);
            let visible = row == 0
                || ui.is_rect_visible(egui::Rect::from_min_size(ui.next_widget_position(), size));
            #[cfg(test)]
            let visible = visible || self.draw_all_rows;
            if visible {
                let response = ui.push_id(("overview-signals", headline, row), |ui| {
                    ui.set_width(width);
                    ui.columns(cols, |columns| {
                        for (index, (column, chart)) in columns.iter_mut().zip(chunk).enumerate() {
                            column.push_id(&chart.id, |ui| {
                                card(ui, chart, now, self.style, (row + index) % 2 == 1, t)
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
            ui.add_space(6.0);
        }
    }
    pub(super) fn sample(&mut self, snapshot: &SystemSnapshot, now: Instant) {
        self.history.sample(snapshot, now);
    }
}

impl TrontopApp {
    pub(super) fn graphs_page(&mut self, ui: &mut egui::Ui) {
        let t = self.colors();
        let now = Instant::now();
        #[cfg(test)]
        let now = self.graphs.fixed_now.unwrap_or(now);
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
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.graphs.style, Style::Lines, "Lines");
            ui.selectable_value(&mut self.graphs.style, Style::Bars, "Bars");
            ui.separator();
            ui.selectable_value(&mut self.graphs.filter, None, "Everything");
            for group in Group::ALL {
                ui.selectable_value(&mut self.graphs.filter, Some(group), group.label());
            }
        });
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            widgets::status_pill(ui, "120 seconds", t.secondary);
            widgets::hover_label(ui, RichText::new("CPU temperature: not graphed; live on Hardware sensors when a provider runs").size(11.0).color(t.text_muted))
                .on_hover_text("CPU package/core temperature comes only from an already-running LibreHardwareMonitor, OpenHardwareMonitor or HWiNFO (read-only). We do not substitute an ACPI thermal zone or invent a temperature. No driver is installed by Trontop.");
            if ui.small_button("Sensor details").clicked() { self.page = Page::Sensors; }
        });
        ui.add_space(8.0);
        let mut scroll = egui::ScrollArea::vertical()
            .id_salt("graph_wall_scroll")
            .auto_shrink([false, false]);
        if self.graphs.filter != previous_filter {
            scroll = scroll.vertical_scroll_offset(0.0);
        }
        scroll.show(ui, |ui| {
            if self.graphs.filter == Some(Group::Cores) {
                self.graphs.cpu_grid(ui, self.snapshot.cpu.logical_cores, t);
                return;
            }
            let count = columns(ui.available_width());
            // A continuous wall avoids mostly empty rows at section boundaries.
            let charts: Vec<_> = Group::ALL
                .into_iter()
                .filter(|group| self.graphs.filter.is_none_or(|filter| filter == *group))
                .flat_map(|group| {
                    self.graphs
                        .history
                        .charts
                        .iter()
                        .filter(move |chart| chart.group == group)
                })
                .collect();
            if charts.is_empty() {
                widgets::hover_label(
                    ui,
                    RichText::new(
                        "No reporting devices in this category. Other graphs keep running.",
                    )
                    .size(11.0)
                    .color(t.text_muted),
                );
            }
            let mut row_height = 0.0;
            let row_width = ui.available_width();
            #[cfg(test)]
            {
                self.graphs.laid_out_cards = 0;
            }
            for (row, chunk) in charts.chunks(count).enumerate() {
                // All card captions are single-line. Measure one real row
                // each frame so font scale, theme margins and pixel rounding
                // remain authoritative, without cached guessed dimensions.
                let row_size = Vec2::new(row_width, row_height);
                let visible = row == 0
                    || ui.is_rect_visible(egui::Rect::from_min_size(
                        ui.next_widget_position(),
                        row_size,
                    ));
                #[cfg(test)]
                let visible = visible || self.graphs.draw_all_rows;
                if visible {
                    let response = ui.push_id(("graph-row", row), |ui| {
                        // Do not let pixel rounding in an earlier row grow
                        // later columns cumulatively at fractional UI scale.
                        ui.set_width(row_width);
                        ui.columns(count, |columns| {
                            for (index, (column, chart)) in
                                columns.iter_mut().zip(chunk).enumerate()
                            {
                                column.push_id(&chart.id, |ui| {
                                    card(
                                        ui,
                                        chart,
                                        now,
                                        self.graphs.style,
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
                        self.graphs.laid_out_cards += chunk.len();
                    }
                } else {
                    // scope/push_id and allocate_space each consume one auto
                    // ID, keeping later rows' hover IDs and geometry stable.
                    ui.allocate_space(row_size);
                }
                ui.add_space(8.0);
            }
            if self.graphs.history.omitted > 0 {
                widgets::hover_label(
                    ui,
                    RichText::new(format!(
                        "Dashboard limit: 512 series. {} additional fields are not charted.",
                        self.graphs.history.omitted
                    ))
                    .color(t.text),
                );
            }
        });
    }
}

fn columns(width: f32) -> usize {
    ((width + 8.0) / 300.0).floor().clamp(1.0, 4.0) as usize
}

fn card(ui: &mut egui::Ui, chart: &Chart, now: Instant, style: Style, banded: bool, t: Tokens) {
    let color = match chart.group {
        Group::System => t.accent,
        Group::Cores => t.accent,
        Group::Memory => t.secondary,
        Group::Thermal => t.secondary,
        Group::Gpu => theme::mix(t.accent, t.secondary, 0.5),
        Group::Storage => t.good,
        Group::Network => t.secondary,
    };
    widgets::hover_frame(ui, widgets::surface(ui, t, banded), |ui| {
        ui.set_min_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 3.0;
        ui.add(
            egui::Label::new(
                RichText::new(&chart.title)
                    .size(13.0)
                    .strong()
                    .color(t.text),
            )
            .truncate(),
        )
        .on_hover_text(&chart.title);
        ui.add(
            egui::Label::new(RichText::new(&chart.detail).size(10.0).color(t.text_muted))
                .truncate(),
        )
        .on_hover_text(&chart.detail);
        ui.horizontal(|ui| {
            ui.add(egui::Label::new(RichText::new(chart.value_label()).size(21.0).monospace().color(t.text)).truncate());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add(egui::Label::new(RichText::new(chart.state(now)).size(10.0).color(t.text_muted)).truncate())
                    .on_hover_text("Only measured values enter the graph. Gaps indicate missing, partial or stale samples. Cached values are not extended into fake history.");
            });
        });
        plot(ui, chart, now, style, color, t, 106.0);
    });
}

fn plot(
    ui: &mut egui::Ui,
    chart: &Chart,
    now: Instant,
    style: Style,
    color: Color32,
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
    let (low, high) = chart.range(now);
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
    let visible: Vec<_> = chart
        .points
        .iter()
        .filter(|p| p.at <= now && now.duration_since(p.at) <= WINDOW)
        .collect();
    let ink = t.ink(color);
    if style == Style::Bars {
        let bar_width = (plot.width() * chart.cadence.as_secs_f32() / WINDOW.as_secs_f32() * 0.72)
            .clamp(1.0, 12.0);
        let baseline = position(now, 0.0).y;
        for point in &visible {
            if let Some(value) = point.value {
                let p = position(point.at, value);
                let bar = egui::Rect::from_two_pos(
                    egui::pos2((p.x - bar_width).max(plot.left()), baseline),
                    p,
                );
                painter.rect_filled(bar, 0.5, ink);
                if value == 0.0 {
                    painter.circle_filled(p, 1.0, ink);
                }
            }
        }
    } else {
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
            fill.colored_vertex(a, ink.gamma_multiply(0.22));
            fill.colored_vertex(b, ink.gamma_multiply(0.22));
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
                painter.circle_filled(position(point.at, value), 1.0, ink);
            }
        }
    }
    painter.text(
        rect.left_top() + Vec2::new(7.0, 4.0),
        egui::Align2::LEFT_TOP,
        chart.unit.format(high),
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
    if !visible.iter().any(|p| p.value.is_some()) {
        painter.text(
            plot.center(),
            egui::Align2::CENTER_CENTER,
            if rect.width() < 200.0 {
                "No data"
            } else {
                "No measured samples yet"
            },
            FontId::proportional(11.0),
            t.text_muted,
        );
    }
    if let Some(pointer) = response.hover_pos()
        && let Some(point) = visible.iter().min_by(|a, b| {
            let ax = (position(a.at, 0.0).x - pointer.x).abs();
            let bx = (position(b.at, 0.0).x - pointer.x).abs();
            ax.total_cmp(&bx)
        })
    {
        let x = position(point.at, 0.0).x;
        painter.line_segment(
            [egui::pos2(x, plot.top()), egui::pos2(x, plot.bottom())],
            Stroke::new(1.0, t.text_muted),
        );
        response.on_hover_text(format!(
            "{}\n{}\n{:.1} seconds ago",
            chart.title,
            point
                .value
                .map_or_else(|| "No exact measurement".into(), |v| chart.unit.format(v)),
            now.saturating_duration_since(point.at).as_secs_f32()
        ));
    }
}

#[cfg(test)]
mod tests;
