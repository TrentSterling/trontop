//! Bounded, timestamped chart data. No OS calls and no synthetic runtime samples.
use super::*;
use crate::diagnostics::{Provider, State};

const MAX_SERIES: usize = 512;
const MAX_POINTS: usize = 128;
pub(super) const WINDOW: Duration = Duration::from_secs(120);

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub(super) enum Id {
    System(u8),
    Memory(u8),
    Cpu(usize),
    CpuClock(u8),
    Activity(String),
    Adapter(crate::gpu_adapters::Key, u64),
    Gpu(String, u8),
    Temperature(String, u16),
    Disk(String, usize),
    Network(String, u8),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Group {
    System,
    Memory,
    Thermal,
    Gpu,
    Storage,
    Network,
    Cores,
}
impl Group {
    pub(super) const ALL: [Self; 7] = [
        Self::System,
        Self::Memory,
        Self::Thermal,
        Self::Gpu,
        Self::Storage,
        Self::Network,
        Self::Cores,
    ];
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::System => "Load",
            Self::Memory => "Memory",
            Self::Thermal => "Temperatures & power",
            Self::Gpu => "GPU",
            Self::Storage => "Disks",
            Self::Network => "Network",
            Self::Cores => "All cores",
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum Unit {
    Percent,
    Celsius,
    Watts,
    Mhz,
    Gib,
    Rate,
    Millis,
    Count,
}
impl Unit {
    pub(super) fn format(self, value: f32) -> String {
        match self {
            Self::Percent => format!("{value:.1}%"),
            Self::Celsius => format!("{value:.0} °C"),
            Self::Watts => format!("{value:.1} W"),
            Self::Mhz => format!("{value:.0} MHz"),
            Self::Gib => format!("{value:.2} GiB"),
            Self::Rate => crate::format::rate(value as f64),
            Self::Millis => format!("{value:.2} ms"),
            // Counts (processes, queue depth) are whole numbers: never "369.0".
            Self::Count => format!("{:.0}", value.round()),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Point {
    pub at: Instant,
    pub value: Option<f32>,
    /// A real lower bound from the readable counters, drawn distinctly.
    pub partial: bool,
}

pub(super) struct Chart {
    pub id: Id,
    pub title: String,
    pub detail: String,
    pub group: Group,
    pub unit: Unit,
    pub points: VecDeque<Point>,
    pub maximum: Option<f32>,
    pub current: Option<f32>,
    pub state: &'static str,
    pub measured_at: Option<Instant>,
    pub last_seen: Instant,
    pub cadence: Duration,
    pub partial: bool,
}

impl Chart {
    pub(super) fn state(&self, now: Instant) -> &'static str {
        if self.last_seen < now && now.duration_since(self.last_seen) > Duration::from_secs(3) {
            return "Cached / not reporting";
        }
        if self.state == "Live"
            && self
                .measured_at
                .is_some_and(|at| now.saturating_duration_since(at) > self.cadence * 3)
        {
            "Cached"
        } else {
            self.state
        }
    }
    pub(super) fn value_label(&self) -> String {
        self.current.map_or_else(
            || "Unavailable".into(),
            |v| {
                format!(
                    "{}{}",
                    self.unit.format(v),
                    if self.partial { "+" } else { "" }
                )
            },
        )
    }
    /// Values inside the window, oldest first, for a sparkline. A step longer
    /// than three cadences becomes a `None` break, so a missing stretch stays a
    /// visible gap instead of being bridged. Partial (lower-bound) points keep
    /// their measured value.
    pub(super) fn recent(&self, now: Instant) -> Vec<Option<f32>> {
        let mut values = Vec::with_capacity(self.points.len());
        let mut previous: Option<Instant> = None;
        for point in self
            .points
            .iter()
            .filter(|p| p.at <= now && now.duration_since(p.at) <= WINDOW)
        {
            if previous.is_some_and(|at| point.at.saturating_duration_since(at) > self.cadence * 3)
            {
                values.push(None);
            }
            values.push(point.value);
            previous = Some(point.at);
        }
        values
    }
    pub(super) fn range(&self, now: Instant) -> (f32, f32) {
        let mut low = 0.0_f32;
        let mut high = 1.0_f32;
        for value in self
            .points
            .iter()
            .filter(|p| now.saturating_duration_since(p.at) <= WINDOW)
            .filter_map(|p| p.value)
        {
            low = low.min(value);
            high = high.max(value);
        }
        let high = self.maximum.unwrap_or(high * 1.15).max(high);
        let high = match self.unit {
            // Integer quantities get an integer axis top, e.g. 450 rather than 447.3.
            Unit::Count if self.maximum.is_none() => whole_axis_top(high),
            _ => high,
        };
        (low.min(0.0), high)
    }
}

/// Round an axis top up to a whole, readable step (1, 5, 50, 500, ...).
pub(super) fn whole_axis_top(value: f32) -> f32 {
    if !value.is_finite() || value <= 1.0 {
        return 1.0;
    }
    let step = (10_f32.powf(value.log10().floor()) / 2.0).max(1.0);
    (value / step).ceil() * step
}

#[derive(Default)]
pub(super) struct History {
    pub charts: Vec<Chart>,
    pub omitted: usize,
    index: HashMap<Id, usize>,
}

struct Field<'a> {
    id: Id,
    title: &'a str,
    detail: &'a str,
    group: Group,
    unit: Unit,
    value: Option<f32>,
    at: Option<Instant>,
    state: &'static str,
    maximum: Option<f32>,
    cadence: Duration,
    partial: bool,
}

impl History {
    pub(super) fn chart(&self, id: &Id) -> Option<&Chart> {
        self.index.get(id).map(|&index| &self.charts[index])
    }

    /// A chart's recent values by identity, oldest first (see [`Chart::recent`]).
    pub(super) fn recent(&self, id: &Id, now: Instant) -> Option<Vec<Option<f32>>> {
        self.chart(id).map(|chart| chart.recent(now))
    }

    fn field(&mut self, field: Field<'_>, now: Instant) {
        let index = if let Some(&index) = self.index.get(&field.id) {
            index
        } else {
            if self.charts.len() >= MAX_SERIES {
                self.omitted += 1;
                return;
            }
            let index = self.charts.len();
            self.index.insert(field.id.clone(), index);
            self.charts.push(Chart {
                id: field.id,
                title: field.title.into(),
                detail: field.detail.into(),
                group: field.group,
                unit: field.unit,
                points: VecDeque::new(),
                maximum: field.maximum,
                current: None,
                state: field.state,
                measured_at: None,
                last_seen: now,
                cadence: field.cadence,
                partial: false,
            });
            index
        };
        let chart = &mut self.charts[index];
        chart.last_seen = now;
        chart.title = field.title.into();
        chart.detail = field.detail.into();
        chart.maximum = field.maximum;
        chart.state = field.state;
        let measured = field.value.filter(|v| v.is_finite());
        if measured.is_some() {
            chart.partial = field.partial;
            chart.current = measured;
        } else if chart.current.is_some() {
            chart.state = "Cached";
        }
        let at = field.at.filter(|at| *at <= now);
        let graphable = matches!(field.state, "Live" | "Partial");
        if graphable && measured.is_some() {
            chart.measured_at = at;
        }
        if let Some(at) = at
            && chart.points.back().is_none_or(|last| at > last.at)
        {
            chart.points.push_back(Point {
                at,
                value: graphable.then_some(measured).flatten(),
                partial: field.partial,
            });
        }
        while chart.points.len() > MAX_POINTS
            || chart
                .points
                .front()
                .is_some_and(|p| now.saturating_duration_since(p.at) > WINDOW)
        {
            chart.points.pop_front();
        }
    }

    pub(super) fn sample(&mut self, s: &SystemSnapshot, now: Instant) {
        self.charts
            .retain(|chart| now.saturating_duration_since(chart.last_seen) <= WINDOW);
        self.index.clear();
        for (index, chart) in self.charts.iter().enumerate() {
            self.index.insert(chart.id.clone(), index);
        }
        self.omitted = 0;
        let system = s.diagnostics.get(Provider::System);
        let system_live = matches!(
            system.state(Provider::System, now),
            State::Live | State::Partial
        );
        let system_state = if system_live {
            "Live"
        } else if system.last_success.is_some() {
            "Cached"
        } else {
            "Starting"
        };
        let system_values = [
            (
                "CPU usage",
                "Whole-machine CPU load",
                Unit::Percent,
                Some(s.cpu_percent),
                Some(100.0),
            ),
            (
                "Memory usage",
                "Physical memory used",
                Unit::Percent,
                (s.memory_total_bytes > 0)
                    .then(|| s.memory_used_bytes as f32 / s.memory_total_bytes as f32 * 100.0),
                Some(100.0),
            ),
            (
                "Processes",
                "Live process count",
                Unit::Count,
                Some(s.process_count as f32),
                None,
            ),
            (
                "Available RAM",
                "Physical memory available to applications",
                Unit::Gib,
                (s.memory_total_bytes > 0)
                    .then(|| s.memory_available_bytes as f32 / 1_073_741_824.0),
                Some(s.memory_total_bytes as f32 / 1_073_741_824.0),
            ),
        ];
        for (index, (title, detail, unit, value, maximum)) in system_values.into_iter().enumerate()
        {
            self.field(
                Field {
                    id: Id::System(index as u8),
                    title,
                    detail,
                    group: if index == 3 {
                        Group::Memory
                    } else {
                        Group::System
                    },
                    unit,
                    value: system.last_success.and(value),
                    at: system.last_success,
                    state: system_state,
                    maximum,
                    cadence: Duration::from_secs(1),
                    partial: false,
                },
                now,
            );
        }
        let memory_health = s.diagnostics.get(Provider::MemoryCounters);
        let clock_health = s.diagnostics.get(Provider::CpuClock);
        let clock_state = if clock_health.state(Provider::CpuClock, now) == State::Live {
            "Live"
        } else if s.cpu.clocks.is_some() {
            "Cached"
        } else {
            clock_health.state(Provider::CpuClock, now).label()
        };
        for (index, title, value) in [
            (
                0,
                "CPU average clock",
                s.cpu.clocks.as_ref().map(|v| v.average_mhz),
            ),
            (
                1,
                "CPU fastest clock",
                s.cpu.clocks.as_ref().map(|v| v.fastest_mhz),
            ),
        ] {
            self.field(
                Field {
                    id: Id::CpuClock(index),
                    title,
                    detail: "Windows performance-state interval / per-processor nominal clocks",
                    group: Group::System,
                    unit: Unit::Mhz,
                    value: value.map(|v| v as f32),
                    at: clock_health.last_attempt,
                    state: clock_state,
                    maximum: None,
                    cadence: Duration::from_secs(1),
                    partial: false,
                },
                now,
            );
        }
        // Keep other devices chartable on many-core machines, within the shared
        // 512-series budget. Processor indices never depend on current load/sort.
        for index in 0..s.cpu.logical_cores.min(256) {
            let value = s
                .cpu
                .logical_usage
                .get(index)
                .copied()
                .flatten()
                .filter(|v| v.is_finite() && (0.0..=100.0).contains(v));
            self.field(
                Field {
                    id: Id::Cpu(index),
                    title: &format!("CPU {index}"),
                    detail: "Logical processor busy time / 0-100% / Windows sysinfo",
                    group: Group::Cores,
                    unit: Unit::Percent,
                    value: system.last_success.and(value),
                    at: system.last_attempt,
                    state: if value.is_none() {
                        "Unavailable"
                    } else {
                        system_state
                    },
                    maximum: Some(100.0),
                    cadence: Duration::from_secs(1),
                    partial: false,
                },
                now,
            );
        }
        let memory_live = memory_health.state(Provider::MemoryCounters, now) == State::Live;
        let gib = |value: u64| value as f32 / 1_073_741_824.0;
        let memory = s.memory_details;
        let memory_values = [
            (
                "Commit charge",
                "Committed virtual memory; not page-file occupancy",
                memory.map(|m| gib(m.commit_bytes)),
                Unit::Gib,
                memory.map(|m| gib(m.commit_limit_bytes)),
            ),
            (
                "Commit pressure",
                "Committed / current commit limit",
                memory.and_then(|m| m.pressure()),
                Unit::Percent,
                Some(100.0),
            ),
            (
                "System cache",
                "Standby pages + system working set",
                memory.map(|m| gib(m.system_cache_bytes)),
                Unit::Gib,
                None,
            ),
            (
                "Paged pool",
                "Kernel paged-pool allocation",
                memory.map(|m| gib(m.kernel_paged_bytes)),
                Unit::Gib,
                None,
            ),
            (
                "Nonpaged pool",
                "Kernel nonpaged-pool allocation",
                memory.map(|m| gib(m.kernel_nonpaged_bytes)),
                Unit::Gib,
                None,
            ),
        ];
        for (index, (title, detail, value, unit, maximum)) in memory_values.into_iter().enumerate()
        {
            self.field(
                Field {
                    id: Id::Memory(index as u8),
                    title,
                    detail,
                    group: Group::Memory,
                    unit,
                    value: memory_health.last_success.and(value),
                    at: memory_health.last_attempt,
                    state: if memory_live {
                        "Live"
                    } else if memory.is_some() {
                        "Cached"
                    } else {
                        "Unavailable"
                    },
                    maximum,
                    cadence: Duration::from_secs(1),
                    partial: false,
                },
                now,
            );
        }
        let activity_health = s.diagnostics.get(Provider::GpuActivity);
        let activity_live = matches!(
            activity_health.state(Provider::GpuActivity, now),
            State::Live | State::Partial
        );
        for (name, usage) in std::iter::once(("GPU activity", s.gpu.reading())).chain(
            s.gpu
                .engine_utilization
                .iter()
                .map(|(name, value)| (name.as_str(), *value)),
        ) {
            let partial = usage.value().is_some() && usage.exact().is_none();
            self.field(
                Field {
                    id: Id::Activity(name.into()),
                    title: name,
                    detail: "Busiest engine, all adapters",
                    group: if name == "GPU activity" {
                        Group::System
                    } else {
                        Group::Gpu
                    },
                    unit: Unit::Percent,
                    value: usage.value(),
                    at: activity_health.last_attempt,
                    state: if !activity_live {
                        "Unavailable"
                    } else if partial {
                        "Partial"
                    } else if usage.exact().is_some() {
                        "Live"
                    } else {
                        "Unavailable"
                    },
                    maximum: Some(100.0),
                    cadence: Duration::from_secs(1),
                    partial,
                },
                now,
            );
        }
        for adapter in &s.gpu.adapters {
            let detail = format!("{} / {}", adapter.name(), adapter.key.label());
            for (metric, title) in [
                "Dedicated GPU memory",
                "Shared GPU memory",
                "GPU committed memory",
            ]
            .iter()
            .enumerate()
            {
                let value = &adapter.memory[metric];
                self.field(
                    Field {
                        id: Id::Adapter(adapter.key, metric as u64),
                        title,
                        detail: &detail,
                        group: Group::Gpu,
                        unit: Unit::Gib,
                        value: value.value.map(|v| v as f32 / 1_073_741_824.),
                        at: value.last_attempt,
                        state: value.state(now),
                        maximum: None,
                        cadence: Duration::from_secs(1),
                        partial: false,
                    },
                    now,
                );
            }
            for (index, title, usage) in
                std::iter::once((3, "Busiest engine".to_string(), adapter.activity)).chain(
                    adapter.engines.iter().map(|e| {
                        (
                            4 + u64::from(e.number),
                            format!("{} / engine {}", e.kind, e.number),
                            e.usage,
                        )
                    }),
                )
            {
                let partial = usage.value().is_some() && usage.exact().is_none();
                self.field(
                    Field {
                        id: Id::Adapter(adapter.key, index),
                        title: &title,
                        detail: &detail,
                        group: Group::Gpu,
                        unit: Unit::Percent,
                        value: usage.value(),
                        at: adapter.sampled_at,
                        state: if partial {
                            "Partial"
                        } else if usage.exact().is_some() {
                            "Live"
                        } else {
                            "Unavailable"
                        },
                        maximum: Some(100.),
                        cadence: Duration::from_secs(1),
                        partial,
                    },
                    now,
                );
            }
        }
        let gpu = &s.gpu_sensors;
        let gpu_live = !gpu.using_cached
            && gpu
                .last_success
                .is_some_and(|at| now.saturating_duration_since(at) <= Duration::from_secs(3));
        for (index, adapter) in gpu.adapters.iter().enumerate() {
            let identity = adapter
                .uuid
                .clone()
                .unwrap_or_else(|| format!("unidentified:{index}"));
            let values = [
                (
                    "GPU temperature",
                    adapter.temperature_c.map(|v| v as f32),
                    Unit::Celsius,
                    Group::Thermal,
                    Some(110.0),
                ),
                (
                    "Board power",
                    adapter.power_w,
                    Unit::Watts,
                    Group::Thermal,
                    None,
                ),
                (
                    "Graphics clock",
                    adapter.graphics_clock_mhz.map(|v| v as f32),
                    Unit::Mhz,
                    Group::Gpu,
                    None,
                ),
                (
                    "Memory clock",
                    adapter.memory_clock_mhz.map(|v| v as f32),
                    Unit::Mhz,
                    Group::Gpu,
                    None,
                ),
                (
                    "Fan target",
                    adapter.fan_percent.map(|v| v as f32),
                    Unit::Percent,
                    Group::Gpu,
                    Some(100.0),
                ),
                (
                    "VRAM used",
                    adapter.memory.map(|v| v.0 as f32 / 1_073_741_824.0),
                    Unit::Gib,
                    Group::Gpu,
                    adapter.memory.map(|v| v.1 as f32 / 1_073_741_824.0),
                ),
            ];
            for (metric, (title, value, unit, group, maximum)) in values.into_iter().enumerate() {
                let detail = format!(
                    "{}{}",
                    adapter.name,
                    match metric {
                        4 => " / requested fan speed, not measured RPM",
                        5 if adapter.memory_includes_reserved => " / includes driver reservations",
                        _ => " / NVIDIA driver telemetry",
                    }
                );
                self.field(
                    Field {
                        id: Id::Gpu(identity.clone(), metric as u8),
                        title,
                        detail: &detail,
                        group,
                        unit,
                        value,
                        at: adapter.uuid.as_ref().and(gpu.sampled_at),
                        state: if adapter.uuid.is_none() {
                            "No stable sensor identity"
                        } else if value.is_none() {
                            "Unavailable"
                        } else if gpu_live {
                            "Live"
                        } else {
                            "Cached"
                        },
                        maximum,
                        cadence: Duration::from_secs(1),
                        partial: false,
                    },
                    now,
                );
            }
        }
        for drive in &s.storage_sensors.drives {
            for sensor in &drive.temperatures.sensors {
                self.field(
                    Field {
                        id: Id::Temperature(drive.device.id.clone(), sensor.index),
                        title: &format!("Drive temperature / sensor {}", sensor.index),
                        detail: &drive.device.name,
                        group: Group::Thermal,
                        unit: Unit::Celsius,
                        value: sensor.celsius.map(|v| v as f32),
                        at: drive.last_attempt,
                        state: if sensor.celsius.is_none() {
                            "Unavailable"
                        } else {
                            drive.status(now)
                        },
                        maximum: Some(100.0),
                        cadence: Duration::from_secs(5),
                        partial: false,
                    },
                    now,
                );
            }
            if drive.temperatures.sensors.is_empty() {
                self.field(
                    Field {
                        id: Id::Temperature(drive.device.id.clone(), u16::MAX),
                        title: "Drive temperature",
                        detail: &drive.device.name,
                        group: Group::Thermal,
                        unit: Unit::Celsius,
                        value: None,
                        at: drive.last_attempt,
                        state: drive.status(now),
                        maximum: Some(100.0),
                        cadence: Duration::from_secs(5),
                        partial: false,
                    },
                    now,
                );
            }
        }
        for disk in &s.physical_disks.devices {
            for metric in crate::disk_activity::Metric::ALL {
                let index = metric as usize;
                let reading = disk.readings[index];
                self.field(
                    Field {
                        id: Id::Disk(disk.instance.clone(), index),
                        title: &format!("Disk {} / {}", disk.number, metric.label()),
                        detail: metric.explanation(),
                        group: Group::Storage,
                        unit: [
                            Unit::Percent,
                            Unit::Millis,
                            Unit::Count,
                            Unit::Rate,
                            Unit::Rate,
                        ][index],
                        value: reading.value.map(|v| v as f32),
                        at: s.physical_disks.at,
                        state: reading.state(&s.physical_disks, now),
                        maximum: (index == 0).then_some(100.0),
                        cadence: Duration::from_secs(1),
                        partial: false,
                    },
                    now,
                );
            }
        }
        for network in &s.networks {
            for (index, (title, value)) in [
                ("Receive", network.received_bytes_per_sec),
                ("Send", network.transmitted_bytes_per_sec),
            ]
            .into_iter()
            .enumerate()
            {
                self.field(
                    Field {
                        id: Id::Network(network.name.clone(), index as u8),
                        title,
                        detail: &network.name,
                        group: Group::Network,
                        unit: Unit::Rate,
                        value: system.last_success.map(|_| value as f32),
                        at: system.last_success,
                        state: system_state,
                        maximum: None,
                        cadence: Duration::from_secs(1),
                        partial: false,
                    },
                    now,
                );
            }
        }
    }
}
