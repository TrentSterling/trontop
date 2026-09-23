//! Overview: an at-a-glance dashboard that fits a 1000x580 window. Five KPI
//! tiles, a Thermals and power band, then the top processes by CPU and by
//! memory. Deep per-signal graphs live only on the Graphs page; every series
//! here is read from existing histories, never polled from the UI thread.
use super::graphs::{SeriesView, Signal};
use super::*;
use crate::diagnostics::{Provider, State};
use std::time::Instant;

/// Base height of a Thermals and power tile: label row, value, then a
/// full-width sparkline band. Taller windows grow the band.
pub(super) const COMPACT_TILE_HEIGHT: f32 = 70.0;
/// One Top CPU / Top memory row: the text line with its 2 px share bar
/// drawn inside the same rect, just above the row bottom.
pub(super) const PROCESS_ROW_HEIGHT: f32 = 24.0;
/// Top process rows: always five (reserved while the first snapshot is
/// pending), more when a taller window leaves room above the fold.
const TOP_ROWS: usize = 5;
const TOP_ROWS_MAX: usize = 10;
/// Below this content width the KPI row splits into 3 then 2 tiles.
const KPI_ONE_ROW_WIDTH: f32 = 700.0;
/// A section header: its 20 px line plus `space::S` below it.
const SECTION_HEADER: f32 = 26.0;
/// The one-line gap row under the thermal tiles.
const GAP_ROW: f32 = 22.0;
/// The "data sources degraded" footer line.
const FOOTER_LINE: f32 = 18.0;
/// Core strip cells: 18 px at the base size, up to 28 px on tall windows.
const CORE_CELL: f32 = 18.0;
const CORE_CELL_MAX: f32 = 28.0;
const CORE_GAP: f32 = 2.0;
const CORE_MIN_WIDTH: f32 = 12.0;
/// The most extra sparkline height one KPI or thermal tile takes.
const KPI_GROWTH_MAX: f32 = 180.0;
const THERMAL_GROWTH_MAX: f32 = 150.0;

/// What the Overview stacks vertically at the current width, before sizing.
#[derive(Clone, Copy, Debug)]
pub(super) struct Shape {
    pub kpi_rows: usize,
    pub thermal_rows: usize,
    pub gaps: bool,
    pub stacked_lists: bool,
    /// Rows of the core strip; 0 draws the one-line "Waiting" gap row.
    pub core_rows: usize,
    pub degraded: bool,
}

/// Sizes for one Overview frame: every pixel of spare height is spent on
/// more process rows, then larger core cells, then taller sparkline bands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Plan {
    pub kpi_height: f32,
    pub thermal_height: f32,
    pub slots: usize,
    pub core_cell: f32,
}

impl Plan {
    const BASE: Plan = Plan {
        kpi_height: widgets::KPI_TILE_HEIGHT,
        thermal_height: COMPACT_TILE_HEIGHT,
        slots: TOP_ROWS,
        core_cell: CORE_CELL,
    };

    /// The page height `shape` takes with these sizes, matching the layout
    /// in [`TrontopApp::overview_page`] exactly.
    pub(super) fn height(&self, shape: &Shape) -> f32 {
        use theme::space::{GAP, L, M, XS};
        let rows = |count: usize, height: f32, gap: f32| {
            if count == 0 {
                0.0
            } else {
                count as f32 * height + (count - 1) as f32 * gap
            }
        };
        let kpis = rows(shape.kpi_rows, self.kpi_height, GAP);
        let mut thermals = SECTION_HEADER + rows(shape.thermal_rows, self.thermal_height, GAP);
        if shape.gaps {
            thermals += GAP_ROW + if shape.thermal_rows > 0 { XS } else { 0.0 };
        }
        let list = SECTION_HEADER + self.slots as f32 * PROCESS_ROW_HEIGHT;
        let lists = if shape.stacked_lists {
            2.0 * list + L
        } else {
            list
        };
        let cores = SECTION_HEADER
            + if shape.core_rows == 0 {
                GAP_ROW
            } else {
                rows(shape.core_rows, self.core_cell, CORE_GAP)
            };
        let footer = if shape.degraded { M + FOOTER_LINE } else { 0.0 };
        kpis + L + thermals + L + lists + L + cores + footer + XS
    }

    pub(super) fn fit(viewport: f32, shape: &Shape) -> Plan {
        let mut plan = Plan::BASE;
        // One pixel of slack: content exactly as tall as the viewport still
        // shows a scroll bar.
        let mut spare = viewport - 1.0 - plan.height(shape);
        if spare <= 0.0 {
            return plan;
        }
        let per_slot = PROCESS_ROW_HEIGHT * if shape.stacked_lists { 2.0 } else { 1.0 };
        let more = ((spare / per_slot).floor() as usize).min(TOP_ROWS_MAX - TOP_ROWS);
        plan.slots += more;
        spare -= more as f32 * per_slot;
        if shape.core_rows > 0 {
            let grow = (spare / shape.core_rows as f32)
                .floor()
                .clamp(0.0, CORE_CELL_MAX - CORE_CELL);
            plan.core_cell += grow;
            spare -= grow * shape.core_rows as f32;
        }
        // The rest: KPI bands take a little more than half, thermal bands the
        // rest; each is capped so a huge window never draws a wall of line.
        let (kpi_share, thermal_share) = if shape.thermal_rows > 0 {
            (spare * 0.55, spare * 0.45)
        } else {
            (spare, 0.0)
        };
        plan.kpi_height += (kpi_share / shape.kpi_rows.max(1) as f32)
            .floor()
            .clamp(0.0, KPI_GROWTH_MAX);
        if shape.thermal_rows > 0 {
            plan.thermal_height += (thermal_share / shape.thermal_rows as f32)
                .floor()
                .clamp(0.0, THERMAL_GROWTH_MAX);
        }
        plan
    }
}

/// Rows the core strip wraps into at `width`: cells never narrower than
/// [`CORE_MIN_WIDTH`]. 0 when no logical processor is known yet.
pub(super) fn core_rows(count: usize, width: f32) -> usize {
    if count == 0 {
        return 0;
    }
    let fit = (((width + CORE_GAP) / (CORE_MIN_WIDTH + CORE_GAP)).floor() as usize).max(1);
    count.div_ceil(fit)
}

/// One Overview tile, owned so it can be built before the scroll area borrows
/// the app.
pub(super) struct Tile {
    pub label: String,
    pub value: String,
    pub sub: String,
    pub hover: String,
    pub series: Vec<Option<f32>>,
    pub max: Option<f32>,
    /// `Some(span)` scales the sparkline to the observed min and max widened
    /// to at least `span` (thermal tiles); `None` uses `0..max`.
    pub span: Option<f32>,
    pub color: Color32,
    /// `Some` only when the tile is not Live.
    pub state: Option<&'static str>,
}

impl Tile {
    fn kpi(&self) -> widgets::Kpi<'_> {
        widgets::Kpi {
            label: &self.label,
            value: &self.value,
            sub: &self.sub,
            hover: &self.hover,
            series: &self.series,
            max: self.max,
            color: self.color,
            state: self.state,
        }
    }
}

/// A signal that produced no value: listed on one compact row, never a tile.
pub(super) struct Gap {
    pub label: String,
    /// Two or three words for the row's chip when this is the only gap.
    pub short: &'static str,
    pub reason: String,
}

/// Sum of live physical-disk read + write bytes/s. The flag is true when some
/// disk's read or write counter had no live value, so the total is a lower
/// bound. `None` when no disk has a live reading at all.
pub(super) fn disk_throughput(
    disks: &crate::disk_activity::Snapshot,
    now: Instant,
) -> Option<(f64, bool)> {
    use crate::disk_activity::Metric;
    let mut total = None;
    let mut missing = false;
    for device in &disks.devices {
        for metric in [Metric::Read, Metric::Write] {
            match device.readings[metric as usize].live(disks, now) {
                Some(value) => *total.get_or_insert(0.0) += value,
                None => missing = true,
            }
        }
    }
    total.map(|total| (total, missing))
}

/// Receive and send bytes/s summed over the sampler's adapters (already
/// deduplicated by interface alias). `None` before the first usable system
/// sample or when no adapter is reported.
pub(super) fn network_throughput(snapshot: &SystemSnapshot) -> Option<(f64, f64)> {
    if snapshot.networks.is_empty()
        || snapshot
            .diagnostics
            .get(Provider::System)
            .last_success
            .is_none()
    {
        return None;
    }
    Some(snapshot.networks.iter().fold((0.0, 0.0), |(rx, tx), n| {
        (
            rx + n.received_bytes_per_sec,
            tx + n.transmitted_bytes_per_sec,
        )
    }))
}

fn pct1(value: f32) -> String {
    format!("{value:.1}%")
}

fn gib(bytes: u64) -> f64 {
    bytes as f64 / 1_073_741_824.0
}

/// A finite-only series from a plain history; NaN entries are gaps.
fn finite(history: &VecDeque<f32>) -> Vec<Option<f32>> {
    history
        .iter()
        .map(|v| v.is_finite().then_some(*v))
        .collect()
}

/// A short chip for a Graphs state label, or `None` when it is Live.
fn chip(state: &'static str) -> Option<&'static str> {
    match state {
        "Live" => None,
        "Partial" => Some("Partial"),
        state if state.starts_with("Cached") => Some("Cached"),
        "Starting" | "Warming" => Some("Starting"),
        _ => Some("Stale"),
    }
}

/// "TEAM TM8FP6002T" from "TEAM TM8FP6002T", "WDC WD60EZAX" from
/// "WDC WD60EZAX-00C8VB0": the vendor word plus the model, without suffixes.
pub(super) fn drive_short_name(name: &str) -> String {
    let mut words = name.split_whitespace();
    match (words.next(), words.next()) {
        (Some(vendor), Some(model)) => {
            format!("{vendor} {}", model.split('-').next().unwrap_or(model))
        }
        (Some(vendor), None) => vendor.to_owned(),
        _ => "Drive".to_owned(),
    }
}

fn peak(values: &[Option<f32>]) -> Option<f32> {
    values.iter().flatten().copied().reduce(f32::max)
}

/// Element-wise maximum of equally timed series, aligned at their newest point.
fn max_series(series: Vec<Vec<Option<f32>>>) -> Vec<Option<f32>> {
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

impl TrontopApp {
    pub(super) fn overview_page(&mut self, ui: &mut egui::Ui) {
        self.page_header(ui, "Overview", "", false);
        self.graphs.ensure_sample(&self.snapshot);
        let t = self.colors();
        let now = self.graphs.now();
        let kpis = self.overview_kpis(now, t);
        let (thermals, gaps) = self.overview_thermals(now, t);
        let top_cpu = top_indices(&self.snapshot.processes, |row| row.cpu_percent as f64);
        let top_memory = top_indices(&self.snapshot.processes, |row| row.memory_bytes as f64);
        let degraded: Vec<&'static str> = Provider::ALL
            .into_iter()
            .filter(|provider| {
                matches!(
                    self.snapshot
                        .diagnostics
                        .get(*provider)
                        .state(*provider, now),
                    State::Partial | State::Stale | State::Unavailable
                )
            })
            .map(Provider::name)
            .collect();
        let settings = self.theme;
        let mut open_page = None;
        let mut open_cores = false;
        let mut select_pid = None;
        let mut show_diagnostics = false;
        let processes = &self.snapshot.processes;
        let icons = &mut self.process_icons;
        let cores = &self.snapshot.cpu;
        let memory_total = self.snapshot.memory_total_bytes;
        let viewport = ui.available_height();
        egui::ScrollArea::vertical()
            .id_salt("overview_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(theme::space::GAP, 0.0);
                let width = ui.available_width();
                let columns = if width >= KPI_ONE_ROW_WIDTH {
                    kpis.len()
                } else {
                    3
                };
                let per_row = if width >= KPI_ONE_ROW_WIDTH {
                    5
                } else if width >= 520.0 {
                    3
                } else {
                    2
                };
                // Balance rows (6 tiles read as 3 + 3, never 5 + 1).
                let thermal_rows = thermals.len().div_ceil(per_row);
                let per_row = thermals.len().div_ceil(thermal_rows.max(1)).max(1);
                let plan = Plan::fit(
                    viewport,
                    &Shape {
                        kpi_rows: kpis.len().div_ceil(columns),
                        thermal_rows,
                        gaps: !gaps.is_empty(),
                        stacked_lists: width < 480.0,
                        core_rows: core_rows(cores.logical_cores.min(256), width),
                        degraded: !degraded.is_empty(),
                    },
                );

                // Row 1: KPI tiles.
                tile_grid(ui, &kpis, columns, |ui, tile| {
                    widgets::kpi_tile_sized(ui, &tile.kpi(), plan.kpi_height, settings, t);
                });
                ui.add_space(theme::space::L);

                // Row 2: Thermals and power.
                if widgets::section_header(ui, "Thermals and power", Some("Sensor details"), t) {
                    open_page = Some(Page::Sensors);
                }
                tile_grid(ui, &thermals, per_row, |ui, tile| {
                    compact_tile(ui, tile, plan.thermal_height, settings, t);
                });
                if !gaps.is_empty() {
                    if !thermals.is_empty() {
                        ui.add_space(theme::space::XS);
                    }
                    let names = gaps
                        .iter()
                        .map(|gap| gap.label.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    let reasons = gaps
                        .iter()
                        .map(|gap| format!("{}: {}", gap.label, gap.reason))
                        .collect::<Vec<_>>()
                        .join("\n");
                    let short = if gaps.len() == 1 {
                        gaps[0].short.to_owned()
                    } else {
                        format!("{} missing", gaps.len())
                    };
                    widgets::gap_row(ui, &format!("No reading: {names}"), &short, &reasons, t);
                }
                ui.add_space(theme::space::L);

                // Row 3: top processes, five rows or as many as fit above the fold.
                let slots = plan.slots;
                let mut lists = |ui: &mut egui::Ui, memory: bool| {
                    let link = memory.then_some("All processes");
                    if widgets::section_header(
                        ui,
                        if memory { "Top memory" } else { "Top CPU" },
                        link,
                        t,
                    ) {
                        open_page = Some(Page::Processes);
                    }
                    let indices = if memory { &top_memory } else { &top_cpu };
                    // Keep all five slots while the first snapshot is pending.
                    for slot in 0..slots {
                        let row = indices.get(slot).map(|&index| &processes[index]);
                        let (value, fraction) = match row {
                            Some(row) if memory => (
                                format::bytes(row.memory_bytes),
                                if memory_total > 0 {
                                    row.memory_bytes as f32 / memory_total as f32
                                } else {
                                    0.0
                                },
                            ),
                            Some(row) => (pct1(row.cpu_percent), row.cpu_percent / 100.0),
                            None => ("--".to_owned(), 0.0),
                        };
                        let color = if memory { t.secondary } else { t.accent };
                        if process_row(ui, icons, row, &value, fraction, color, t)
                            && let Some(row) = row
                        {
                            select_pid = Some(row.pid);
                        }
                    }
                };
                if width >= 480.0 {
                    ui.columns(2, |columns| {
                        columns[0].spacing_mut().item_spacing.y = 0.0;
                        columns[1].spacing_mut().item_spacing.y = 0.0;
                        lists(&mut columns[0], false);
                        lists(&mut columns[1], true);
                    });
                } else {
                    lists(ui, false);
                    ui.add_space(theme::space::L);
                    lists(ui, true);
                }
                ui.add_space(theme::space::L);

                // Below the fold: one compact heat strip, not 24 graphs.
                if widgets::section_header(ui, "Cores", Some("All cores"), t) {
                    open_cores = true;
                }
                core_strip(ui, cores, plan.core_cell, t);
                if !degraded.is_empty() {
                    ui.add_space(theme::space::M);
                    ui.allocate_ui_with_layout(
                        Vec2::new(width, FOOTER_LINE),
                        Layout::left_to_right(Align::Center),
                        |ui| {
                            ui.set_min_height(FOOTER_LINE);
                            ui.spacing_mut().item_spacing.x = theme::space::S;
                            let count = degraded.len();
                            widgets::hover_label(
                                ui,
                                RichText::new(format!(
                                    "{count} data source{} degraded.",
                                    if count == 1 { "" } else { "s" }
                                ))
                                .size(11.0)
                                .color(t.text_muted),
                            )
                            .on_hover_text(degraded.join("\n"));
                            if ui
                                .link(RichText::new("Details").size(11.0).color(t.ink(t.accent)))
                                .clicked()
                            {
                                show_diagnostics = true;
                            }
                        },
                    );
                }
                ui.add_space(theme::space::XS);
            });
        if let Some(pid) = select_pid {
            self.selected_pid = Some(pid);
            self.page = Page::Processes;
        }
        if let Some(page) = open_page {
            self.page = page;
        }
        if open_cores {
            self.graphs.show_all_cores();
            self.page = Page::Graphs;
        }
        self.show_diagnostics |= show_diagnostics;
    }

    /// CPU, Memory, GPU, Disk and Network, in that order.
    pub(super) fn overview_kpis(&self, now: Instant, t: Tokens) -> Vec<Tile> {
        let s = &self.snapshot;
        let has_sample = self.seen_generation > 0;
        let clock = self
            .graphs
            .series(Signal::CpuClockAverage)
            .filter(|view| view.state == "Live")
            .and_then(|view| view.current);
        let cpu = Tile {
            label: "CPU".into(),
            value: if has_sample {
                pct1(s.cpu_percent)
            } else {
                "--".into()
            },
            sub: match clock {
                Some(mhz) => format!("avg {:.2} GHz", mhz / 1000.0),
                None if s.cpu.logical_cores > 0 => {
                    format!("{} logical processors", s.cpu.logical_cores)
                }
                None => String::new(),
            },
            hover: "Whole-machine CPU load, from Windows processor time. The line shows the last two minutes; the clock is the Windows performance-state average across all logical processors.".into(),
            series: finite(&self.cpu_history),
            max: Some(100.0),
            span: None,
            color: t.accent,
            state: None,
        };

        let memory = Tile {
            label: "Memory".into(),
            value: if s.memory_total_bytes > 0 {
                pct1(memory_percent(s))
            } else {
                "--".into()
            },
            sub: if s.memory_total_bytes > 0 {
                format!(
                    "{:.1} of {:.1} GB",
                    gib(s.memory_used_bytes),
                    gib(s.memory_total_bytes)
                )
            } else {
                String::new()
            },
            hover: format!(
                "Physical memory in use: {} of {}. {} available to applications.",
                format::bytes(s.memory_used_bytes),
                format::bytes(s.memory_total_bytes),
                format::bytes(s.memory_available_bytes)
            ),
            series: finite(&self.memory_history),
            max: Some(100.0),
            span: None,
            color: t.secondary,
            state: None,
        };

        let reading = s.gpu.reading();
        let activity = self.graphs.series(Signal::GpuActivity);
        let partial = reading.value().is_some() && reading.exact().is_none();
        let vram = s
            .gpu
            .adapters
            .iter()
            .filter(|adapter| adapter.description.as_ref().is_some_and(|d| !d.software))
            .max_by(|a, b| {
                a.activity
                    .value()
                    .unwrap_or(-1.0)
                    .total_cmp(&b.activity.value().unwrap_or(-1.0))
            })
            .and_then(|adapter| {
                let total = adapter.description.as_ref()?.dedicated_video;
                let used = adapter.memory[0].value?;
                (total > 0).then_some((used, total))
            })
            .or_else(|| {
                s.gpu_sensors
                    .adapters
                    .iter()
                    .filter(|_| !s.gpu_sensors.using_cached)
                    .find_map(|adapter| adapter.memory)
            });
        let gpu = Tile {
            label: "GPU".into(),
            value: reading.value().map_or_else(
                || "--".into(),
                |v| format!("{}{}", pct1(v), if partial { "+" } else { "" }),
            ),
            sub: vram.map_or_else(String::new, |(used, total)| {
                format!("VRAM {:.1} of {:.1} GB", gib(used), gib(total))
            }),
            hover: format!(
                "{}\nWindows PDH GPU Engine counters, busiest engine across adapters.",
                reading.explanation()
            ),
            series: activity
                .as_ref()
                .map_or_else(|| finite(&self.gpu_history), |view| view.values.clone()),
            max: Some(100.0),
            span: None,
            color: theme::mix(t.accent, t.secondary, 0.5),
            state: if partial { Some("Partial") } else { None },
        };

        let disks = disk_throughput(&s.physical_disks, now);
        let busiest = s
            .physical_disks
            .devices
            .iter()
            .filter_map(|device| {
                device.readings[crate::disk_activity::Metric::Active as usize]
                    .live(&s.physical_disks, now)
            })
            .reduce(f64::max);
        let disk = Tile {
            label: "Disk".into(),
            value: disks.map_or_else(
                || "--".into(),
                |(total, partial)| {
                    format!("{}{}", format::rate(total), if partial { "+" } else { "" })
                },
            ),
            sub: busiest.map_or_else(String::new, |active| {
                format!("busiest disk {active:.0}% active")
            }),
            hover: match disks {
                Some((_, true)) => "Physical disks, read + write. Some disk counters are not reporting, so + marks a lower bound. Windows PDH PhysicalDisk counters.".into(),
                Some(_) => "Physical disks, read + write. Windows PDH PhysicalDisk counters.".into(),
                None => "No live physical-disk counters right now. Missing is not zero.".into(),
            },
            series: finite(&self.overview_disk_total),
            max: None,
            span: None,
            color: t.good,
            state: disks.and_then(|(_, partial)| partial.then_some("Partial")),
        };

        let network = network_throughput(s);
        let net = Tile {
            label: "Network".into(),
            value: network.map_or_else(|| "--".into(), |(rx, tx)| format::rate(rx + tx)),
            sub: network.map_or_else(String::new, |(rx, tx)| {
                format!("in {} \u{b7} out {}", format::rate(rx), format::rate(tx))
            }),
            hover: if network.is_some() {
                format!(
                    "Receive + send across {}. In is receive, out is send.",
                    s.networks
                        .iter()
                        .map(|n| n.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            } else {
                "No network adapter reported yet.".into()
            },
            series: finite(&self.overview_net_total),
            max: None,
            span: None,
            color: t.secondary,
            state: None,
        };
        vec![cpu, memory, gpu, disk, net]
    }

    /// One tile per measured thermal or power signal; everything that produced
    /// no value is a [`Gap`] instead. Power is never summed into a system total.
    pub(super) fn overview_thermals(&self, now: Instant, t: Tokens) -> (Vec<Tile>, Vec<Gap>) {
        let s = &self.snapshot;
        let mut tiles = Vec::new();
        let mut gaps = Vec::new();

        // CPU package temperature and power come only from the sensor bridge.
        let bridge = &self.specs_view.bridge;
        let fresh = bridge.collected_at.is_some_and(|at| {
            now.saturating_duration_since(at) <= crate::specs::BRIDGE_STALE_AFTER
        });
        let bridge_reading = |key: &crate::specs::LiveKey| {
            fresh
                .then(|| {
                    bridge
                        .readings
                        .iter()
                        .find(|r| &r.key == key && r.value.is_finite())
                })
                .flatten()
        };
        let package = crate::specs::LiveKey::CpuPackageTemperature;
        let power = crate::specs::LiveKey::Sensor {
            id: crate::specs::cpu::PACKAGE_POWER.into(),
        };
        for (key, label, span) in [
            (&package, "CPU temperature", 10.0),
            (&power, "CPU power", 20.0),
        ] {
            if let Some(reading) = bridge_reading(key) {
                tiles.push(Tile {
                    label: label.into(),
                    value: reading.unit.format(reading.value),
                    sub: String::new(),
                    hover: format!("{}: {}", reading.source, reading.label),
                    series: Vec::new(),
                    max: None,
                    span: Some(span),
                    color: t.accent,
                    state: None,
                });
            }
        }
        if bridge_reading(&package).is_none() {
            let (short, reason) = match (&bridge.status, fresh) {
                _ if self.specs.is_none() => ("Not checked", "not checked yet. Trontop reads CPU temperature only from LibreHardwareMonitor, OpenHardwareMonitor or HWiNFO when one is already running; open Sensor details to look for one".to_owned()),
                (crate::specs::Value::Known(_), true) => (
                    "Not reported",
                    "the running sensor provider does not report it".to_owned(),
                ),
                (crate::specs::Value::Known(_), false) => (
                    "Provider stopped",
                    "the sensor provider stopped responding".to_owned(),
                ),
                (crate::specs::Value::Unavailable(reason), _) => (
                    "Needs sensor app",
                    format!("needs LibreHardwareMonitor or HWiNFO running ({reason})"),
                ),
            };
            gaps.push(Gap {
                label: "CPU temperature".into(),
                short,
                reason,
            });
        }

        // GPU temperature and board power from NVML, as graphed on Graphs.
        let identified = s
            .gpu_sensors
            .adapters
            .iter()
            .filter(|adapter| adapter.uuid.is_some())
            .count();
        for adapter in &s.gpu_sensors.adapters {
            let Some(uuid) = adapter.uuid.as_deref() else {
                continue;
            };
            let suffix = if identified > 1 {
                format!(
                    " {}",
                    adapter
                        .name
                        .trim_start_matches("NVIDIA ")
                        .trim_start_matches("GeForce ")
                )
            } else {
                String::new()
            };
            for (signal, label, span, unit) in [
                (Signal::GpuTemperature(uuid), "GPU temperature", 10.0, "°C"),
                (Signal::GpuPower(uuid), "GPU power", 20.0, "W"),
            ] {
                let Some(SeriesView {
                    values,
                    current: Some(current),
                    state,
                    ..
                }) = self.graphs.series(signal)
                else {
                    continue;
                };
                let format = |v: f32| {
                    if unit == "W" {
                        format!("{v:.0} W")
                    } else {
                        format!("{v:.0} °C")
                    }
                };
                tiles.push(Tile {
                    label: format!("{label}{suffix}"),
                    value: format(current),
                    sub: peak(&values).map_or_else(String::new, |p| format!("peak {}", format(p))),
                    hover: format!(
                        "{}: {} (NVIDIA driver telemetry).{}",
                        adapter.name,
                        if unit == "W" {
                            "board power"
                        } else {
                            "GPU temperature"
                        },
                        if unit == "W" {
                            " GPU power only; not whole-system power."
                        } else {
                            ""
                        }
                    ),
                    series: values,
                    max: None,
                    span: Some(span),
                    color: t.secondary,
                    state: chip(state),
                });
            }
        }
        if s.gpu_sensors.adapters.is_empty()
            && let Some(error) = &s.gpu_sensors.error
        {
            gaps.push(Gap {
                label: "GPU temperature".into(),
                short: "Not reported",
                reason: error.clone(),
            });
        }

        // Drives: one tile per drive with any sensor value, hottest sensor first.
        for drive in s.storage_sensors.drives.iter().filter(|d| d.present) {
            let short = drive_short_name(&drive.device.name);
            let sensors: Vec<_> = drive
                .temperatures
                .sensors
                .iter()
                .filter_map(|sensor| sensor.celsius.map(|c| (sensor.index, c)))
                .collect();
            let Some(hottest) = sensors.iter().map(|(_, c)| *c).max() else {
                gaps.push(Gap {
                    label: short,
                    short: "No sensor",
                    reason: drive.error.as_ref().map_or_else(
                        || "drive does not report temperature".to_owned(),
                        |error| format!("drive does not report temperature ({error})"),
                    ),
                });
                continue;
            };
            let series = max_series(
                sensors
                    .iter()
                    .filter_map(|(index, _)| {
                        self.graphs
                            .series(Signal::DriveTemperature(&drive.device.id, *index))
                            .map(|view| view.values)
                    })
                    .collect(),
            );
            let mut hover = format!("{}\n", drive.device.name);
            for (index, celsius) in &sensors {
                hover.push_str(&format!("Sensor {index}: {celsius} °C\n"));
            }
            if let Some(warning) = drive.temperatures.warning {
                hover.push_str(&format!("Warning at {warning} °C. "));
            }
            hover.push_str("Windows storage temperature.");
            tiles.push(Tile {
                label: short,
                value: format!("{hottest} °C"),
                sub: peak(&series).map_or_else(String::new, |p| format!("peak {p:.0} °C")),
                hover,
                series,
                max: None,
                span: Some(10.0),
                color: t.good,
                state: chip(drive.status(now)),
            });
        }
        (tiles, gaps)
    }
}

/// Tiles in rows of `columns` with an 8 px gap; the last row stretches, so a
/// grid never ends in an orphan.
fn tile_grid(
    ui: &mut egui::Ui,
    tiles: &[Tile],
    columns: usize,
    render: impl FnMut(&mut egui::Ui, &Tile),
) {
    if tiles.is_empty() {
        return;
    }
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing = Vec2::splat(theme::space::GAP);
        widgets::fill_last_row(ui, tiles, columns, render);
    });
}

/// Indices of the heaviest processes by `key` (at most [`TOP_ROWS_MAX`]), heaviest first.
fn top_indices(rows: &[ProcessRow], key: impl Fn(&ProcessRow) -> f64) -> Vec<usize> {
    let mut top: Vec<usize> = Vec::with_capacity(TOP_ROWS_MAX + 1);
    for (index, row) in rows.iter().enumerate() {
        let value = key(row);
        let before = top
            .iter()
            .position(|&other| value > key(&rows[other]))
            .unwrap_or(top.len());
        if before < TOP_ROWS_MAX {
            top.insert(before, index);
            top.truncate(TOP_ROWS_MAX);
        }
    }
    top
}

/// A muted rounded chip painted at the right edge of `rect`; returns its left.
fn paint_chip(ui: &egui::Ui, rect: egui::Rect, text: &str, t: Tokens) -> f32 {
    let galley = ui
        .painter()
        .layout_no_wrap(text.into(), FontId::proportional(10.0), t.text_muted);
    let size = Vec2::new(galley.size().x + 12.0, 16.0);
    let chip = egui::Rect::from_min_size(
        egui::pos2(rect.right() - size.x, rect.center().y - size.y / 2.0),
        size,
    );
    ui.painter().rect_filled(
        chip,
        size.y / 2.0,
        t.surface(theme::mix(t.panel_raised, t.text_muted, 0.28)),
    );
    ui.painter().galley(
        egui::pos2(chip.left() + 6.0, chip.center().y - galley.size().y / 2.0),
        galley,
        t.text_muted,
    );
    chip.left()
}

/// The full-width sparkline band of a tile `rect` whose value line ends at
/// `top`: under the value, inside the card padding.
pub(super) fn spark_band(rect: egui::Rect, top: f32) -> egui::Rect {
    let pad = theme::CARD_PAD;
    egui::Rect::from_min_max(
        egui::pos2(rect.left() + f32::from(pad.left), rect.top() + top),
        egui::pos2(
            rect.right() - f32::from(pad.right),
            rect.bottom() - f32::from(pad.bottom),
        ),
    )
}

/// The value line of a thermal tile ends here; its band starts 4 px lower.
const COMPACT_BAND_TOP: f32 = 50.0;

/// A Thermals and power tile, at least [`COMPACT_TILE_HEIGHT`] tall: label
/// (with the peak or a state chip on the right), a 20 px value and a
/// full-width sparkline band below it, scaled around the observed range so a
/// steady reading sits mid-band. Hover has provenance.
fn compact_tile(
    ui: &mut egui::Ui,
    tile: &Tile,
    height: f32,
    settings: ThemeSettings,
    t: Tokens,
) -> egui::Response {
    let response = ui.allocate_response(
        Vec2::new(ui.available_width(), height.max(COMPACT_TILE_HEIGHT)),
        Sense::hover(),
    );
    let rect = response.rect;
    if ui.is_rect_visible(rect) {
        let fill = if response.contains_pointer() {
            t.surface(theme::mix(theme::raised_color(settings), t.row_hover, 0.5))
        } else {
            theme::raised_color(settings)
        };
        ui.painter().rect(
            rect,
            settings.roundness,
            fill,
            Stroke::new(1.0, t.border),
            egui::StrokeKind::Inside,
        );
        let pad = theme::CARD_PAD;
        let content = egui::Rect::from_min_max(
            rect.min + Vec2::new(f32::from(pad.left), f32::from(pad.top)),
            rect.max - Vec2::new(f32::from(pad.right), f32::from(pad.bottom)),
        );
        let spark = spark_band(rect, COMPACT_BAND_TOP);
        if spark.width() > 2.0 && spark.height() > 2.0 {
            let (lo, hi) = match tile.span {
                Some(span) => widgets::padded_range(&tile.series, span).unwrap_or((0.0, span)),
                None => (
                    0.0,
                    tile.max.unwrap_or_else(|| {
                        format::nice_top(tile.series.iter().flatten().copied().fold(0.0, f32::max))
                    }),
                ),
            };
            widgets::sparkline_range(
                ui.painter(),
                spark,
                tile.series.iter().copied(),
                lo,
                hi,
                tile.color,
                t,
            );
        }
        let row1 = egui::Rect::from_min_size(content.min, Vec2::new(content.width(), 12.0));
        let mut label_right = row1.right();
        if let Some(state) = tile.state {
            label_right = paint_chip(ui, row1, state, t) - 6.0;
        } else if !tile.sub.is_empty() {
            let galley = ui.painter().layout_no_wrap(
                tile.sub.clone(),
                FontId::proportional(10.0),
                t.text_muted,
            );
            // Only when it leaves the label at least half the row.
            if galley.size().x < row1.width() * 0.5 {
                label_right = row1.right() - galley.size().x - 6.0;
                ui.painter().galley(
                    egui::pos2(
                        row1.right() - galley.size().x,
                        row1.center().y - galley.size().y / 2.0,
                    ),
                    galley,
                    t.text_muted,
                );
            }
        }
        widgets::paint_text(
            ui,
            egui::Rect::from_min_max(row1.min, egui::pos2(label_right, row1.bottom())),
            &tile.label,
            FontId::proportional(11.0),
            t.text_muted,
            Align::Min,
        );
        widgets::paint_text(
            ui,
            egui::Rect::from_min_max(
                egui::pos2(content.left(), rect.top() + 20.0),
                egui::pos2(content.right(), rect.top() + 46.0),
            ),
            &tile.value,
            FontId::monospace(20.0),
            t.text,
            Align::Min,
        );
    }
    response.on_hover_text(&tile.hover)
}

/// Where one Top CPU / Top memory row draws its parts, all inside `rect`.
pub(super) struct RowLayout {
    /// The text line: the row minus the strip kept for the share bar.
    pub line: egui::Rect,
    pub icon: egui::Rect,
    pub name: egui::Rect,
    pub value_left: f32,
    /// The full-share extent of the bar: from the name's left edge to the
    /// value's right edge, 2 px tall, 1 px above the row bottom.
    pub track: egui::Rect,
}

impl RowLayout {
    pub(super) fn new(rect: egui::Rect, value_width: f32) -> Self {
        let line =
            egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), rect.bottom() - 4.0));
        let icon = egui::Rect::from_center_size(
            egui::pos2(line.left() + 12.0, line.center().y),
            Vec2::splat(16.0),
        );
        let value_right = line.right() - 6.0;
        let value_left = value_right - value_width;
        let name = egui::Rect::from_min_max(
            egui::pos2(icon.right() + 8.0, line.top()),
            egui::pos2(value_left - 10.0, line.bottom()),
        );
        let track = egui::Rect::from_min_max(
            egui::pos2(name.left(), rect.bottom() - 3.0),
            egui::pos2(value_right, rect.bottom() - 1.0),
        );
        Self {
            line,
            icon,
            name,
            value_left,
            track,
        }
    }

    /// The filled part of the share bar for `fraction` (0 to 1), at least
    /// 2 px wide so a tiny nonzero share stays visible.
    pub(super) fn bar(&self, fraction: f32) -> egui::Rect {
        egui::Rect::from_min_size(
            self.track.min,
            Vec2::new(
                (self.track.width() * fraction.clamp(0.0, 1.0)).max(2.0),
                self.track.height(),
            ),
        )
    }
}

/// One Top CPU / Top memory row: icon, name, right-aligned value and a 2 px
/// share bar proportional to `fraction`, inside the row's own rect. PID and path show on hover; returns true
/// when the row was clicked. `None` draws a reserved placeholder slot.
fn process_row(
    ui: &mut egui::Ui,
    icons: &mut crate::process_icons::Cache,
    row: Option<&ProcessRow>,
    value: &str,
    fraction: f32,
    color: Color32,
    t: Tokens,
) -> bool {
    let sense = if row.is_some() {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), PROCESS_ROW_HEIGHT), sense);
    if !ui.is_rect_visible(rect) {
        return false;
    }
    if row.is_some() && response.hovered() {
        ui.painter().rect_filled(rect, 3.0, t.row_hover);
    }
    let value_galley = ui
        .painter()
        .layout_no_wrap(value.into(), FontId::monospace(12.0), t.text);
    let layout = RowLayout::new(rect, value_galley.size().x);
    if let Some(row) = row {
        // A child Ui that does not allocate in the parent: a scope here would
        // move the list cursor back up to the icon's bottom edge.
        let mut icon_ui = ui.new_child(egui::UiBuilder::new().max_rect(layout.icon));
        icons.paint(
            &mut icon_ui,
            row.executable.as_deref(),
            16.0,
            t.text_muted,
            Sense::hover(),
        );
    }
    ui.painter().galley(
        egui::pos2(
            layout.value_left,
            layout.line.center().y - value_galley.size().y / 2.0,
        ),
        value_galley,
        t.text,
    );
    widgets::paint_text(
        ui,
        layout.name,
        row.map_or("--", |r| r.name.as_str()),
        FontId::proportional(12.0),
        if row.is_some() { t.text } else { t.text_muted },
        Align::Min,
    );
    let fraction = fraction.clamp(0.0, 1.0);
    if fraction > 0.0 {
        ui.painter()
            .rect_filled(layout.bar(fraction), 1.0, t.ink(color));
    }
    match row {
        Some(row) => {
            let clicked = response.clicked();
            response.on_hover_text(format!(
                "{}\nPID {}{}\nClick to select it on Processes.",
                row.name,
                row.pid,
                row.executable
                    .as_ref()
                    .map_or_else(String::new, |path| format!("\n{}", path.display()))
            ));
            clicked
        }
        None => false,
    }
}

/// Every logical processor as one equal cell, shaded by load. Wraps into more
/// rows when cells would be narrower than 12 px. The exact value is on hover.
fn core_strip(ui: &mut egui::Ui, cpu: &crate::model::CpuInfo, cell_height: f32, t: Tokens) {
    let count = cpu.logical_cores.min(256);
    if count == 0 {
        widgets::gap_row(
            ui,
            "Logical processors",
            "Waiting",
            "Waiting for the first system sample.",
            t,
        );
        return;
    }
    const GAP: f32 = CORE_GAP;
    let cell_height = cell_height.clamp(CORE_CELL, CORE_CELL_MAX);
    let width = ui.available_width();
    let rows = core_rows(count, width);
    let per_row = count.div_ceil(rows);
    let height = rows as f32 * cell_height + (rows - 1) as f32 * GAP;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let cell_width = (width - GAP * (per_row - 1) as f32) / per_row as f32;
    let cell = |index: usize| {
        let (row, column) = (index / per_row, index % per_row);
        egui::Rect::from_min_size(
            egui::pos2(
                rect.left() + column as f32 * (cell_width + GAP),
                rect.top() + row as f32 * (cell_height + GAP),
            ),
            Vec2::new(cell_width, cell_height),
        )
    };
    let usage = |index: usize| {
        cpu.logical_usage
            .get(index)
            .copied()
            .flatten()
            .filter(|v: &f32| v.is_finite())
    };
    let ink = t.ink(t.accent);
    for index in 0..count {
        let bounds = cell(index);
        match usage(index) {
            Some(value) => {
                let load = (value / 100.0).clamp(0.0, 1.0);
                ui.painter().rect_filled(
                    bounds,
                    2.0,
                    theme::mix(t.graph_bg, ink, 0.12 + 0.88 * load),
                );
            }
            None => {
                ui.painter().rect(
                    bounds,
                    2.0,
                    t.graph_bg,
                    Stroke::new(1.0, t.border),
                    egui::StrokeKind::Inside,
                );
            }
        }
    }
    if let Some(pointer) = response.hover_pos()
        && let Some(index) =
            (0..count).find(|&index| cell(index).expand(GAP / 2.0).contains(pointer))
    {
        ui.painter().rect_stroke(
            cell(index),
            2.0,
            Stroke::new(1.0, t.text),
            egui::StrokeKind::Inside,
        );
        response.on_hover_text(match usage(index) {
            Some(value) => format!("CPU {index}: {}", pct1(value)),
            None => format!("CPU {index}: not reported"),
        });
    }
}
