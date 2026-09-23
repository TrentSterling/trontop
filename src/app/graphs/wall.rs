//! Compose history charts into Graphs wall cards: one device per card, up to
//! three series per card, and signals that never produced a value folded into
//! one footer line instead of empty cards. Pure data; no drawing, no OS calls.
use super::history::{Chart, Group, History, Id, WINDOW};
use super::*;

/// Most series one card draws.
pub(super) const MAX_SERIES: usize = 3;

/// How a multi-series card's headline value is derived from measured series.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Combine {
    /// The first series (for example average clock, or a single signal).
    First,
    /// The hottest sensor.
    Max,
    /// Read plus write, receive plus send, paged plus nonpaged.
    Sum,
}

pub(super) struct Series<'a> {
    pub chart: &'a Chart,
    /// Short legend label ("R", "Receive", "S0"); empty on single-series cards.
    pub label: &'static str,
}

pub(super) struct Card<'a> {
    pub key: String,
    pub title: String,
    pub device: Option<String>,
    pub group: Group,
    pub combine: Combine,
    pub series: Vec<Series<'a>>,
    order: (usize, u8, usize, u8),
}

impl<'a> Card<'a> {
    /// One chart as a card, for per-adapter pages outside the wall.
    pub(super) fn single(chart: &'a Chart) -> Self {
        Self {
            key: format!("{:?}", chart.id),
            title: chart.title.clone(),
            device: None,
            group: chart.group,
            combine: Combine::First,
            series: vec![Series { chart, label: "" }],
            order: (0, 0, 0, 0),
        }
    }

    /// Headline value and whether it is a lower bound (a partial sample, or a
    /// sum missing one of its series). `None` when no series has a value.
    pub(super) fn value(&self) -> Option<(f32, bool)> {
        let values: Vec<_> = self
            .series
            .iter()
            .filter_map(|s| s.chart.current.map(|v| (v, s.chart.partial)))
            .collect();
        let partial = values.iter().any(|(_, partial)| *partial);
        match self.combine {
            Combine::First => self.series[0]
                .chart
                .current
                .map(|v| (v, self.series[0].chart.partial)),
            Combine::Max => values
                .iter()
                .map(|(v, _)| *v)
                .reduce(f32::max)
                .map(|v| (v, partial)),
            Combine::Sum => (!values.is_empty()).then(|| {
                (
                    values.iter().map(|(v, _)| *v).sum(),
                    partial || values.len() < self.series.len(),
                )
            }),
        }
    }

    pub(super) fn value_label(&self) -> String {
        let unit = self.series[0].chart.unit;
        self.value().map_or_else(
            || "--".into(),
            |(v, partial)| format!("{}{}", unit.format(v), if partial { "+" } else { "" }),
        )
    }

    /// The least healthy series state, so one stale sensor is never hidden
    /// behind a live one.
    pub(super) fn state(&self, now: Instant) -> &'static str {
        self.series
            .iter()
            .map(|s| s.chart.state(now))
            .find(|state| *state != "Live")
            .unwrap_or("Live")
    }

    /// Shared axis for every series in the card.
    pub(super) fn range(&self, now: Instant) -> (f32, f32) {
        self.series
            .iter()
            .map(|s| s.chart.range(now))
            .reduce(|a, b| (a.0.min(b.0), a.1.max(b.1)))
            .unwrap_or((0.0, 1.0))
    }

    /// Full provenance for the title's hover text.
    pub(super) fn hover(&self, now: Instant) -> String {
        let mut text = self.title.clone();
        if let Some(device) = &self.device {
            text.push_str(&format!(" / {device}"));
        }
        let mut details: Vec<&str> = Vec::new();
        for series in &self.series {
            if !details.contains(&series.chart.detail.as_str()) {
                details.push(&series.chart.detail);
            }
        }
        for detail in details {
            text.push('\n');
            text.push_str(detail);
        }
        if self.series.len() > 1 {
            for series in &self.series {
                text.push_str(&format!(
                    "\n{}: {} ({})",
                    series.chart.title,
                    series.chart.value_label(),
                    series.chart.state(now)
                ));
            }
            text.push_str(match self.combine {
                Combine::First => "\nHeadline: first series.",
                Combine::Max => "\nHeadline: hottest sensor.",
                Combine::Sum => "\nHeadline: total of the series. + means a series is missing.",
            });
        }
        text.push_str(&format!("\nState: {}", self.state(now)));
        text
    }
}

#[derive(Default)]
pub(super) struct Wall<'a> {
    pub cards: Vec<Card<'a>>,
    /// Engine types whose highest value in the window was exactly 0.
    pub idle: Vec<String>,
    /// Everything only: adapters that stayed idle for the whole window (see
    /// `idle_adapter_keys`), as `(footer line, hover with the measured
    /// values)`. The GPU tab still graphs them.
    pub idle_adapters: Vec<(String, String)>,
    /// One line per folded card: what it is and why it is not shown.
    pub unreported: Vec<String>,
}

/// A value was measured inside the window. Never-measured signals (ghost
/// adapter LUIDs, missing drive sensors) and ones silent for 120 s fold away.
pub(super) fn measured(chart: &Chart, now: Instant) -> bool {
    chart
        .points
        .iter()
        .any(|p| p.value.is_some() && p.at <= now && now.duration_since(p.at) <= WINDOW)
}

/// The highest value a chart reported inside the window, or `None` when it
/// never reported one. Used to fold engines that reported but stayed at
/// exactly 0.0 away from busy ones, both on the wall and on a single
/// adapter's Performance > GPU engine list.
pub(super) fn window_max(chart: &Chart, now: Instant) -> Option<f32> {
    chart
        .points
        .iter()
        .filter(|p| p.at <= now && now.duration_since(p.at) <= WINDOW)
        .filter_map(|p| p.value)
        .reduce(f32::max)
}

/// Card identity, title, legend label, headline rule and in-group section.
fn place(chart: &Chart) -> (String, String, &'static str, Combine, u8) {
    let single = |section| {
        (
            format!("{:?}", chart.id),
            chart.title.clone(),
            "",
            Combine::First,
            section,
        )
    };
    match &chart.id {
        Id::CpuClock(index) => (
            "cpu-clock".into(),
            "CPU clock".into(),
            if *index == 0 { "Avg" } else { "Max" },
            Combine::First,
            1,
        ),
        Id::Activity(name) if name == "GPU activity" => single(2),
        Id::System(_) => single(0),
        Id::Memory(index @ (3 | 4)) => (
            "kernel-pools".into(),
            "Kernel pools".into(),
            if *index == 3 { "Paged" } else { "Nonpaged" },
            Combine::Sum,
            1,
        ),
        Id::Memory(_) => single(0),
        Id::Adapter(key, metric) => (
            format!("adapter-{key:?}-{metric}"),
            chart.title.clone(),
            "",
            Combine::First,
            // History order keeps each adapter's cards together.
            0,
        ),
        Id::Gpu(uuid, metric @ (2 | 3)) => (
            format!("gpu-clocks-{uuid}"),
            "GPU clocks".into(),
            if *metric == 2 { "Core" } else { "Mem" },
            Combine::First,
            2,
        ),
        Id::Gpu(_, _) => single(if chart.group == Group::Gpu { 2 } else { 0 }),
        Id::Activity(name) => (
            format!("engine-{name}"),
            format!("{name} engines"),
            "",
            Combine::First,
            3,
        ),
        Id::Temperature(drive, sensor) => (
            format!("drive-{drive}"),
            "Drive temperature".into(),
            match sensor {
                0 => "S0",
                1 => "S1",
                2 => "S2",
                _ => "S",
            },
            Combine::Max,
            1,
        ),
        Id::Disk(instance, metric @ (3 | 4)) => (
            format!("disk-io-{instance}"),
            "Read / write".into(),
            if *metric == 3 { "R" } else { "W" },
            Combine::Sum,
            0,
        ),
        // Section is unused for disks: [`compose`] orders them per device.
        Id::Disk(..) => single(0),
        Id::Network(name, direction) => (
            format!("net-{name}"),
            "Network traffic".into(),
            if *direction == 0 { "Receive" } else { "Send" },
            Combine::Sum,
            0,
        ),
        Id::Cpu(_) => single(0),
    }
}

/// Reading order of one disk's cards: throughput first, then how busy the
/// device is, how long requests take and how many are waiting.
fn disk_rank(metric: usize) -> u8 {
    match metric {
        3 | 4 => 0,
        0 => 1,
        1 => 2,
        _ => 3,
    }
}

/// Cards for one Graphs tab. `None` is Everything, which leaves out the
/// per-core grid (it has its own tab) and the per-type "All GPUs" engine
/// cards (the GPU tab has them), and folds fully idle adapters into a line.
pub(super) fn compose(history: &History, filter: Option<Group>, now: Instant) -> Wall<'_> {
    let mut cards: Vec<Card<'_>> = Vec::new();
    let mut index = HashMap::<String, usize>::new();
    // Each adapter's cards stay together, busiest engine first.
    let mut adapters = HashMap::<crate::gpu_adapters::Key, usize>::new();
    // Each disk's cards stay together, in first-seen device order.
    let mut disks = HashMap::<String, usize>::new();
    let everything = filter.is_none();
    let idle_adapters: Vec<crate::gpu_adapters::Key> = if everything {
        idle_adapter_keys(history, now)
    } else {
        Vec::new()
    };
    for (position, chart) in history.charts.iter().enumerate() {
        let included = match filter {
            None => {
                chart.group != Group::Cores
                    && !matches!(&chart.id, Id::Activity(name) if name != "GPU activity")
                    && !matches!(&chart.id, Id::Adapter(key, _) if idle_adapters.contains(key))
            }
            Some(group) => chart.group == group,
        };
        if !chart.wall || !included {
            continue;
        }
        let (key, title, label, combine, section) = place(chart);
        let group_rank = Group::ALL
            .iter()
            .position(|g| *g == chart.group)
            .unwrap_or(0);
        let slot = *index.entry(key.clone()).or_insert_with(|| {
            cards.push(Card {
                key,
                title,
                device: chart.device.clone(),
                group: chart.group,
                combine,
                series: Vec::new(),
                order: match chart.id {
                    Id::Adapter(key, metric) => (
                        group_rank,
                        section,
                        *adapters.entry(key).or_insert(position),
                        u8::from(metric != 3),
                    ),
                    Id::Disk(ref instance, metric) => {
                        let next = disks.len();
                        (
                            group_rank,
                            0,
                            *disks.entry(instance.clone()).or_insert(next),
                            disk_rank(metric),
                        )
                    }
                    _ => (group_rank, section, position, 0),
                },
            });
            cards.len() - 1
        });
        cards[slot].series.push(Series { chart, label });
    }
    let mut wall = Wall::default();
    for key in idle_adapters {
        wall.idle_adapters
            .push(idle_adapter_line(history, key, now));
    }
    for mut card in cards {
        let first = card.series[0].chart;
        card.series.retain(|s| measured(s.chart, now));
        if card.series.is_empty() {
            wall.unreported.push(unreported_line(&card, first, now));
            continue;
        }
        card.series.truncate(MAX_SERIES);
        if card.series.len() == 1 {
            card.series[0].label = "";
        }
        let idle_engine = matches!(&card.series[0].chart.id, Id::Activity(name) if name != "GPU activity")
            && window_max(card.series[0].chart, now) == Some(0.0);
        if idle_engine {
            if let Id::Activity(name) = &card.series[0].chart.id {
                wall.idle.push(name.clone());
            }
            continue;
        }
        wall.cards.push(card);
    }
    wall.cards.sort_by_key(|card| card.order);
    wall
}

/// The three wall signals of one adapter: busiest engine, dedicated VRAM and
/// shared memory.
const ADAPTER_WALL_METRICS: [(u64, &str); 3] = [
    (3, "Busiest engine"),
    (0, "Dedicated VRAM"),
    (1, "Shared memory"),
];

/// Highest busiest-engine percent an adapter may reach in the window and
/// still fold as idle on Everything.
const IDLE_BUSY_PERCENT: f32 = 1.0;
/// Most dedicated or shared memory (GiB) an idle adapter may hold: 64 MiB,
/// a desktop compositor surface or two, not a workload.
const IDLE_MEMORY_GIB: f32 = 1.0 / 16.0;

/// Wall adapters whose busiest engine stayed under 1% and whose dedicated
/// and shared memory stayed under 64 MiB for the whole window (an iGPU the
/// desktop is not using). A signal that never reported keeps the adapter on
/// the wall: that is a gap, not idleness.
fn idle_adapter_keys(history: &History, now: Instant) -> Vec<crate::gpu_adapters::Key> {
    let mut keys: Vec<crate::gpu_adapters::Key> = Vec::new();
    for chart in &history.charts {
        if let Id::Adapter(key, 3) = chart.id
            && chart.wall
            && !keys.contains(&key)
            && ADAPTER_WALL_METRICS.iter().all(|(metric, _)| {
                let limit = if *metric == 3 {
                    IDLE_BUSY_PERCENT
                } else {
                    IDLE_MEMORY_GIB
                };
                history
                    .chart(&Id::Adapter(key, *metric))
                    .and_then(|c| c.wall.then(|| window_max(c, now)).flatten())
                    .is_some_and(|max| max < limit)
            })
        {
            keys.push(key);
        }
    }
    keys
}

/// The folded line with the measured window peaks, never rounded to a
/// flattering zero: "Intel Graphics idle (0% busy, 0 B used)" only when
/// every value was exactly 0, otherwise "Intel Graphics near idle (peak 0.2%
/// busy, up to 776 KiB used)". The hover lists every value.
fn idle_adapter_line(
    history: &History,
    key: crate::gpu_adapters::Key,
    now: Instant,
) -> (String, String) {
    let busiest = history.chart(&Id::Adapter(key, 3));
    let name = busiest
        .and_then(|c| c.device.clone())
        .unwrap_or_else(|| "GPU".into());
    let peak = |metric| {
        history
            .chart(&Id::Adapter(key, metric))
            .and_then(|c| window_max(c, now))
            .unwrap_or(0.0)
    };
    let busy = peak(3);
    let memory_bytes = (f64::from(peak(0)) + f64::from(peak(1))) * 1_073_741_824.0;
    let line = if busy == 0.0 && memory_bytes == 0.0 {
        format!("{name} idle (0% busy, 0 B used)")
    } else {
        format!(
            "{name} near idle (peak {} busy, up to {} used)",
            super::history::Unit::Percent.format(busy),
            crate::format::bytes(memory_bytes.round() as u64)
        )
    };
    let mut hover = format!(
        "{name} stayed under 1% busy and under 64 MiB of dedicated and shared memory for the whole 2 minute window, so it is one line here instead of three flat cards. The GPU tab still graphs it. Used is dedicated peak plus shared peak."
    );
    if let Some(detail) = busiest.map(|c| c.detail.as_str()) {
        hover.push_str(&format!("\n{detail}"));
    }
    for (metric, title) in ADAPTER_WALL_METRICS {
        if let Some(chart) = history.chart(&Id::Adapter(key, metric)) {
            hover.push_str(&format!(
                "\n{title}: {} now, {} highest",
                chart.value_label(),
                window_max(chart, now).map_or_else(|| "--".into(), |v| chart.unit.format(v))
            ));
        }
    }
    (line, hover)
}

/// "Drive temperature / WDC WD60EZAX: never reported a value (...)".
fn unreported_line(card: &Card<'_>, chart: &Chart, now: Instant) -> String {
    let reason = if chart.current.is_some() {
        "no new value in the last 2 minutes"
    } else {
        match chart.state(now) {
            "Starting" | "Warming" => "waiting for the first sample",
            "No stable sensor identity" => "no stable sensor identity",
            _ => "never reported a value",
        }
    };
    let device = card
        .device
        .as_ref()
        .map_or_else(String::new, |d| format!(" / {d}"));
    format!("{}{device}: {reason} ({})", card.title, chart.detail)
}
