//! Live values on the System page. Providers emit keys; the UI resolves them
//! against the latest sampler snapshot and sensor-bridge readings. Resolution
//! is pure data lookup: no native calls, no locks, safe on the render thread.
use super::model::Value;
use crate::format;
use crate::model::SystemSnapshot;
use std::time::{Duration, Instant};

/// Bridge readings older than this are not presented as live.
pub const BRIDGE_STALE_AFTER: Duration = Duration::from_secs(15);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum LiveKey {
    // Resolved from the sampler's SystemSnapshot.
    /// Whole-machine CPU busy percentage.
    CpuUsage,
    /// Interval-average effective clock over all logical processors.
    CpuClockAverage,
    CpuClockFastest,
    /// One logical processor, by Windows processor group and number.
    CpuCoreClock {
        group: u16,
        number: u32,
    },
    /// In-use physical memory, of total.
    MemoryUsed,
    /// Commit charge, of commit limit.
    MemoryCommit,
    Uptime,
    /// NVML-backed GPU sensors (see GpuRef for matching).
    Gpu {
        adapter: GpuRef,
        metric: GpuMetric,
    },
    /// Windows storage temperature for a disk device interface path, exactly as
    /// SetupDi returns it for GUID_DEVINTERFACE_DISK (compared ignoring ASCII case).
    DriveTemperature {
        interface: String,
    },
    /// Receive/transmit rate for the sampler's network row with this exact name
    /// (the interface alias, e.g. "Ethernet").
    NetworkThroughput {
        interface: String,
    },

    // Published only by the sensor bridge (src/specs/bridge.rs).
    CpuPackageTemperature,
    /// Bridge-defined CPU core index, as the external provider numbers it.
    CpuCoreTemperature {
        index: u32,
    },
    MotherboardTemperature,
    /// Any other bridge sensor, by a stable bridge-defined identifier.
    Sensor {
        id: String,
    },
}

/// Selects one entry of `SystemSnapshot::gpu_sensors.adapters`: the
/// `ordinal`-th adapter (0-based) whose name equals `name` (trimmed, ASCII
/// case-insensitive). DXGI and NVML report the same marketing name on NVIDIA.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GpuRef {
    pub name: String,
    pub ordinal: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GpuMetric {
    Temperature,
    Power,
    CoreClock,
    MemoryClock,
    /// NVML's intended fan speed, not a tachometer reading.
    FanTarget,
    MemoryUsed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiveUnit {
    Celsius,
    Rpm,
    Volts,
    Watts,
    Megahertz,
    Percent,
}

impl LiveUnit {
    pub fn format(self, value: f64) -> String {
        match self {
            Self::Celsius => format!("{value:.0} °C"),
            Self::Rpm => format!("{value:.0} RPM"),
            Self::Volts => format!("{value:.3} V"),
            Self::Watts => format!("{value:.1} W"),
            Self::Megahertz => format!("{value:.0} MHz"),
            Self::Percent => format!("{value:.0}%"),
        }
    }
}

/// One reading from an already-running external sensor provider.
#[derive(Clone, Debug, PartialEq)]
pub struct BridgeReading {
    pub key: LiveKey,
    /// The provider's own sensor name, e.g. "CPU Package".
    pub label: String,
    pub value: f64,
    pub unit: LiveUnit,
    /// Provider name, e.g. "LibreHardwareMonitor (WMI)".
    pub source: String,
}

/// Everything the bridge's fast worker publishes.
#[derive(Clone, Debug, PartialEq)]
pub struct BridgeReadings {
    pub readings: Vec<BridgeReading>,
    /// Known("<provider names>") while at least one provider answers, otherwise
    /// the reason no provider is available. Used for missing bridge keys.
    pub status: Value,
    /// Set by the worker when the poll that produced these readings started.
    pub collected_at: Option<Instant>,
    /// The bridge's hint for the next poll; the worker clamps it to 1..=60 s.
    pub retry_after: Option<Duration>,
}

impl Default for BridgeReadings {
    fn default() -> Self {
        Self {
            readings: Vec::new(),
            status: Value::unavailable("sensor bridge has not polled yet"),
            collected_at: None,
            retry_after: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LiveValue {
    pub value: Value,
    /// Present only for fresh temperature readings; drives color bands.
    pub celsius: Option<f32>,
    /// "Provider: sensor label" for bridge readings; None for sampler values.
    pub source: Option<String>,
}

impl LiveValue {
    fn known(text: String) -> Self {
        Self {
            value: Value::Known(text),
            celsius: None,
            source: None,
        }
    }

    fn celsius(celsius: f32) -> Self {
        Self {
            value: Value::Known(LiveUnit::Celsius.format(celsius as f64)),
            celsius: Some(celsius),
            source: None,
        }
    }

    fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            value: Value::unavailable(reason),
            celsius: None,
            source: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TempBand {
    Normal,
    Warm,
    Hot,
}

/// Coloring thresholds only; not device limits and never an alarm.
pub fn band(key: &LiveKey, celsius: f32) -> TempBand {
    let (warm, hot) = match key {
        LiveKey::DriveTemperature { .. } => (50.0, 60.0),
        _ => (70.0, 85.0),
    };
    if celsius >= hot {
        TempBand::Hot
    } else if celsius >= warm {
        TempBand::Warm
    } else {
        TempBand::Normal
    }
}

/// Sampler-owned keys resolve from `snapshot`; if that yields Unavailable, a
/// bridge reading with the identical key is used instead. Bridge-only keys
/// resolve from `bridge` alone.
pub fn resolve(
    key: &LiveKey,
    snapshot: &SystemSnapshot,
    bridge: &BridgeReadings,
    now: Instant,
) -> LiveValue {
    let sampled = from_snapshot(key, snapshot, now);
    match sampled {
        Some(value) if value.value.is_known() => value,
        Some(value) => from_bridge(key, bridge, now).unwrap_or(value),
        None => from_bridge(key, bridge, now).unwrap_or_else(|| {
            LiveValue::unavailable(match &bridge.status {
                Value::Known(_) if bridge_fresh(bridge, now) => {
                    "the running sensor provider does not report this reading".to_string()
                }
                Value::Known(_) => "sensor provider stopped responding".to_string(),
                Value::Unavailable(reason) => reason.clone(),
            })
        }),
    }
}

fn bridge_fresh(bridge: &BridgeReadings, now: Instant) -> bool {
    bridge
        .collected_at
        .is_some_and(|at| now.saturating_duration_since(at) <= BRIDGE_STALE_AFTER)
}

fn from_bridge(key: &LiveKey, bridge: &BridgeReadings, now: Instant) -> Option<LiveValue> {
    if !bridge_fresh(bridge, now) {
        return None;
    }
    let reading = bridge
        .readings
        .iter()
        .find(|reading| &reading.key == key && reading.value.is_finite())?;
    let mut value = match reading.unit {
        LiveUnit::Celsius => LiveValue::celsius(reading.value as f32),
        unit => LiveValue::known(unit.format(reading.value)),
    };
    value.source = Some(format!("{}: {}", reading.source, reading.label));
    Some(value)
}

/// None means the key is bridge-only.
fn from_snapshot(key: &LiveKey, snapshot: &SystemSnapshot, now: Instant) -> Option<LiveValue> {
    let waiting = || LiveValue::unavailable("waiting for the first sample");
    Some(match key {
        LiveKey::CpuUsage => {
            if snapshot.sequence == 0 {
                waiting()
            } else {
                LiveValue::known(format::percent(snapshot.cpu_percent))
            }
        }
        LiveKey::CpuClockAverage | LiveKey::CpuClockFastest => match &snapshot.cpu.clocks {
            Some(clocks) => LiveValue::known(format!(
                "{:.0} MHz",
                if *key == LiveKey::CpuClockAverage {
                    clocks.average_mhz
                } else {
                    clocks.fastest_mhz
                }
            )),
            None if snapshot.sequence == 0 => waiting(),
            None => LiveValue::unavailable("CPU clock counters are unavailable"),
        },
        LiveKey::CpuCoreClock { group, number } => {
            let processor = snapshot.cpu.clocks.as_ref().and_then(|clocks| {
                clocks
                    .processors
                    .iter()
                    .find(|p| p.group == *group && p.number == *number)
            });
            match processor.and_then(|p| p.mhz) {
                Some(mhz) => LiveValue::known(format!("{mhz:.0} MHz")),
                None if snapshot.sequence == 0 => waiting(),
                None => LiveValue::unavailable("no clock reading for this logical processor"),
            }
        }
        LiveKey::MemoryUsed => {
            if snapshot.memory_total_bytes == 0 {
                waiting()
            } else {
                LiveValue::known(format!(
                    "{} of {} ({})",
                    format::bytes(snapshot.memory_used_bytes),
                    format::bytes(snapshot.memory_total_bytes),
                    format::percent(
                        snapshot.memory_used_bytes as f32 / snapshot.memory_total_bytes as f32
                            * 100.0
                    )
                ))
            }
        }
        LiveKey::MemoryCommit => match snapshot.memory_details {
            Some(details) if details.commit_limit_bytes > 0 => LiveValue::known(format!(
                "{} of {}",
                format::bytes(details.commit_bytes),
                format::bytes(details.commit_limit_bytes)
            )),
            _ if snapshot.sequence == 0 => waiting(),
            _ => LiveValue::unavailable("Windows commit counters are unavailable"),
        },
        LiveKey::Uptime => {
            if snapshot.sequence == 0 {
                waiting()
            } else {
                LiveValue::known(format::duration(snapshot.uptime_seconds))
            }
        }
        LiveKey::Gpu { adapter, metric } => gpu(snapshot, adapter, *metric),
        LiveKey::DriveTemperature { interface } => drive(snapshot, interface, now),
        LiveKey::NetworkThroughput { interface } => {
            match snapshot.networks.iter().find(|n| &n.name == interface) {
                Some(row) => LiveValue::known(format!(
                    "Rx {} / Tx {}",
                    format::rate(row.received_bytes_per_sec),
                    format::rate(row.transmitted_bytes_per_sec)
                )),
                None if snapshot.sequence == 0 => waiting(),
                None => LiveValue::unavailable("interface not present in network counters"),
            }
        }
        LiveKey::CpuPackageTemperature
        | LiveKey::CpuCoreTemperature { .. }
        | LiveKey::MotherboardTemperature
        | LiveKey::Sensor { .. } => return None,
    })
}

fn gpu(snapshot: &SystemSnapshot, adapter: &GpuRef, metric: GpuMetric) -> LiveValue {
    let sensors = &snapshot.gpu_sensors;
    let Some(found) = sensors
        .adapters
        .iter()
        .filter(|a| a.name.trim().eq_ignore_ascii_case(adapter.name.trim()))
        .nth(adapter.ordinal as usize)
    else {
        return LiveValue::unavailable(match &sensors.error {
            _ if sensors.attempted_at.is_none() => "waiting for the first GPU sensor sample",
            Some(_) => "GPU sensor provider unavailable (NVIDIA NVML)",
            None => "no NVML sensor data for this adapter",
        });
    };
    let text = match metric {
        GpuMetric::Temperature => found
            .temperature_c
            .map(|v| LiveUnit::Celsius.format(v as f64)),
        GpuMetric::Power => found.power_w.map(|v| LiveUnit::Watts.format(v as f64)),
        GpuMetric::CoreClock => found.graphics_clock_mhz.map(|v| format!("{v} MHz")),
        GpuMetric::MemoryClock => found.memory_clock_mhz.map(|v| format!("{v} MHz")),
        GpuMetric::FanTarget => found.fan_percent.map(|v| format!("{:.1}%", v as f32)),
        GpuMetric::MemoryUsed => found
            .memory
            .map(|(used, total)| format!("{} of {}", format::bytes(used), format::bytes(total))),
    };
    if metric == GpuMetric::Temperature
        && !sensors.using_cached
        && let Some(celsius) = found.temperature_c
    {
        return LiveValue::celsius(celsius as f32);
    }
    match text {
        Some(text) if sensors.using_cached => LiveValue::known(format!("{text} (cached)")),
        Some(text) => LiveValue::known(text),
        None => LiveValue::unavailable("not supported by this adapter or driver"),
    }
}

fn drive(snapshot: &SystemSnapshot, interface: &str, now: Instant) -> LiveValue {
    let storage = &snapshot.storage_sensors;
    let Some(drive) = storage
        .drives
        .iter()
        .find(|d| d.device.id.eq_ignore_ascii_case(interface))
    else {
        return LiveValue::unavailable(if storage.inventory_at.is_none() {
            "waiting for the drive sensor inventory"
        } else {
            "Windows reports no temperature interface for this drive"
        });
    };
    // Index 0 may be a composite sensor; prefer it, otherwise the first reading.
    let celsius = drive
        .temperatures
        .sensors
        .iter()
        .find(|s| s.index == 0 && s.celsius.is_some())
        .or_else(|| {
            drive
                .temperatures
                .sensors
                .iter()
                .find(|s| s.celsius.is_some())
        })
        .and_then(|s| s.celsius);
    match celsius {
        Some(value) if drive.live(now) => LiveValue::celsius(value as f32),
        Some(value) if drive.last_success.is_some() => LiveValue::known(format!(
            "{} (cached)",
            LiveUnit::Celsius.format(value as f64)
        )),
        _ => LiveValue::unavailable(drive.error.as_ref().map_or_else(
            || "no temperature reading yet".to_string(),
            ToString::to_string,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bridge(readings: Vec<BridgeReading>, at: Option<Instant>) -> BridgeReadings {
        BridgeReadings {
            readings,
            status: Value::known("Fixture provider"),
            collected_at: at,
            retry_after: None,
        }
    }

    #[test]
    fn bridge_only_keys_explain_absence_and_expire() {
        let now = Instant::now();
        let snapshot = SystemSnapshot::default();
        let none = resolve(
            &LiveKey::CpuPackageTemperature,
            &snapshot,
            &BridgeReadings::default(),
            now,
        );
        assert_eq!(
            none.value.reason(),
            Some("sensor bridge has not polled yet")
        );
        let reading = BridgeReading {
            key: LiveKey::CpuPackageTemperature,
            label: "CPU Package".into(),
            value: 61.4,
            unit: LiveUnit::Celsius,
            source: "Fixture provider".into(),
        };
        let fresh = resolve(
            &LiveKey::CpuPackageTemperature,
            &snapshot,
            &bridge(vec![reading.clone()], Some(now)),
            now,
        );
        assert_eq!(fresh.value, Value::known("61 °C"));
        assert_eq!(fresh.celsius, Some(61.4));
        assert_eq!(
            fresh.source.as_deref(),
            Some("Fixture provider: CPU Package")
        );
        let stale = resolve(
            &LiveKey::CpuPackageTemperature,
            &snapshot,
            &bridge(vec![reading], Some(now)),
            now + BRIDGE_STALE_AFTER + Duration::from_secs(1),
        );
        assert_eq!(
            stale.value.reason(),
            Some("sensor provider stopped responding")
        );
        let unsupported = resolve(
            &LiveKey::MotherboardTemperature,
            &snapshot,
            &bridge(Vec::new(), Some(now)),
            now,
        );
        assert!(
            unsupported
                .value
                .reason()
                .unwrap()
                .contains("does not report")
        );
    }

    #[test]
    fn sampler_keys_never_invent_values_before_the_first_sample() {
        let now = Instant::now();
        let snapshot = SystemSnapshot::default();
        for key in [
            LiveKey::CpuUsage,
            LiveKey::CpuClockAverage,
            LiveKey::MemoryUsed,
            LiveKey::Uptime,
            LiveKey::NetworkThroughput {
                interface: "Ethernet".into(),
            },
        ] {
            let value = resolve(&key, &snapshot, &BridgeReadings::default(), now);
            assert!(!value.value.is_known(), "{key:?} invented a value");
        }
        let drive = resolve(
            &LiveKey::DriveTemperature {
                interface: r"\\?\fixture".into(),
            },
            &snapshot,
            &BridgeReadings::default(),
            now,
        );
        assert_eq!(
            drive.value.reason(),
            Some("waiting for the drive sensor inventory")
        );
    }

    #[test]
    fn gpu_matching_uses_name_and_ordinal_and_marks_cached_values() {
        let now = Instant::now();
        let mut snapshot = SystemSnapshot::default();
        snapshot.gpu_sensors.attempted_at = Some(now);
        snapshot.gpu_sensors.adapters = (0..2)
            .map(|index| crate::gpu_sensors::AdapterSensors {
                name: "Fixture GPU".into(),
                temperature_c: Some(40 + index),
                ..Default::default()
            })
            .collect();
        let key = |ordinal| LiveKey::Gpu {
            adapter: GpuRef {
                name: " fixture gpu ".into(),
                ordinal,
            },
            metric: GpuMetric::Temperature,
        };
        let bridge = BridgeReadings::default();
        assert_eq!(
            resolve(&key(1), &snapshot, &bridge, now).celsius,
            Some(41.0)
        );
        assert!(!resolve(&key(2), &snapshot, &bridge, now).value.is_known());
        let power = LiveKey::Gpu {
            adapter: GpuRef {
                name: "Fixture GPU".into(),
                ordinal: 0,
            },
            metric: GpuMetric::Power,
        };
        assert_eq!(
            resolve(&power, &snapshot, &bridge, now).value.reason(),
            Some("not supported by this adapter or driver")
        );
        snapshot.gpu_sensors.using_cached = true;
        let cached = resolve(&key(0), &snapshot, &bridge, now);
        assert_eq!(cached.value, Value::known("40 °C (cached)"));
        assert_eq!(cached.celsius, None);
    }

    #[test]
    fn unavailable_sampler_values_fall_back_to_an_identical_bridge_key() {
        let now = Instant::now();
        let key = LiveKey::DriveTemperature {
            interface: "fixture-drive".into(),
        };
        let reading = BridgeReading {
            key: key.clone(),
            label: "Fixture drive".into(),
            value: 52.0,
            unit: LiveUnit::Celsius,
            source: "Fixture provider".into(),
        };
        let value = resolve(
            &key,
            &SystemSnapshot::default(),
            &bridge(vec![reading], Some(now)),
            now,
        );
        assert_eq!(value.celsius, Some(52.0));
        assert_eq!(band(&key, 52.0), TempBand::Warm);
        assert_eq!(
            band(&LiveKey::CpuPackageTemperature, 52.0),
            TempBand::Normal
        );
        assert_eq!(band(&LiveKey::CpuPackageTemperature, 90.0), TempBand::Hot);
    }
}
