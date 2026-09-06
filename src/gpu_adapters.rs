//! Windows adapter identity and retained telemetry. Never sums process VRAM.
use crate::gpu_activity::{EngineInstance, Usage};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

#[cfg(windows)]
mod native;
#[cfg(windows)]
pub use native::Sampler;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Key {
    pub high: u32,
    pub low: u32,
    pub physical: u32,
}
impl Key {
    pub fn parse(name: &str) -> Option<Self> {
        let (luid, physical) = name.strip_prefix("luid_")?.split_once("_phys_")?;
        let (high, low) = luid.split_once('_')?;
        Some(Self {
            high: u32::from_str_radix(high.strip_prefix("0x")?, 16).ok()?,
            low: u32::from_str_radix(low.strip_prefix("0x")?, 16).ok()?,
            physical: physical.parse().ok()?,
        })
    }
    pub fn label(self) -> String {
        format!(
            "{:08X}:{:08X} / node {}",
            self.high, self.low, self.physical
        )
    }
}

#[derive(Clone, Debug)]
pub struct Description {
    pub name: String,
    pub vendor_id: u32,
    pub device_id: u32,
    pub dedicated_video: u64,
    pub dedicated_system: u64,
    pub shared_limit: u64,
    pub software: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Bytes {
    pub value: Option<u64>,
    pub last_success: Option<Instant>,
    pub last_attempt: Option<Instant>,
    pub live: bool,
}
impl Bytes {
    pub fn record(&mut self, value: Option<u64>, now: Instant) {
        self.last_attempt = Some(now);
        self.live = value.is_some();
        if let Some(value) = value {
            self.value = Some(value);
            self.last_success = Some(now);
        }
    }
    pub fn state(&self, now: Instant) -> &'static str {
        if self.live
            && self
                .last_success
                .is_some_and(|at| now.saturating_duration_since(at) <= Duration::from_secs(3))
        {
            "Live"
        } else if self.value.is_some() {
            "Cached"
        } else {
            "Unavailable"
        }
    }
    pub fn label(&self, now: Instant) -> String {
        self.value.map_or_else(
            || "--".into(),
            |value| {
                format!(
                    "{}{}",
                    if self.state(now) == "Live" { "" } else { "~" },
                    crate::format::bytes(value)
                )
            },
        )
    }
}

#[derive(Clone, Debug)]
pub struct Engine {
    pub number: u32,
    pub kind: String,
    pub usage: Usage,
}

#[derive(Clone, Debug, Default)]
pub struct Adapter {
    pub key: Key,
    pub description: Option<Description>,
    pub description_current: bool,
    pub activity: Usage,
    pub engines: Vec<Engine>,
    /// Dedicated Usage, Shared Usage, Total Committed; bytes, whole physical adapter.
    pub memory: [Bytes; 3],
    pub sampled_at: Option<Instant>,
    pub last_seen: Option<Instant>,
    pub memory_error: Option<String>,
}
impl Adapter {
    pub fn name(&self) -> String {
        self.description.as_ref().map_or_else(
            || format!("Windows adapter {}", self.key.label()),
            |d| d.name.clone(),
        )
    }
}

/// Preserve physical engine indices, including two engines with the same kind.
pub fn aggregate<'a>(
    readings: impl IntoIterator<Item = (&'a EngineInstance, Usage)>,
    incomplete: bool,
) -> Vec<Adapter> {
    let mut groups = BTreeMap::<Key, BTreeMap<u32, Engine>>::new();
    for (id, usage) in readings {
        let Some((adapter, engine)) = id.physical.split_once("_eng_") else {
            continue;
        };
        let Some(key) = Key::parse(adapter) else {
            continue;
        };
        let Ok(number) = engine.parse::<u32>() else {
            continue;
        };
        groups
            .entry(key)
            .or_default()
            .entry(number)
            .and_modify(|e| e.usage = e.usage.sum(usage))
            .or_insert_with(|| Engine {
                number,
                kind: id.kind.clone(),
                usage,
            });
    }
    groups
        .into_iter()
        .map(|(key, engines)| {
            let engines: Vec<_> = engines
                .into_values()
                .map(|mut e| {
                    e.usage = e.usage.bounded();
                    if incomplete && let Some(value) = e.usage.value() {
                        e.usage = Usage::Partial(value);
                    }
                    e
                })
                .collect();
            let activity = engines
                .iter()
                .map(|e| e.usage)
                .reduce(Usage::peak)
                .unwrap_or_default();
            Adapter {
                key,
                engines,
                activity,
                ..Default::default()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn adapter_identity_canonicalizes_hex_without_merging_nodes_or_engines() {
        assert_eq!(
            Key::parse("luid_0x00000000_0x0000ABCD_phys_0"),
            Key::parse("luid_0x0_0xabcd_phys_0")
        );
        assert_ne!(
            Key::parse("luid_0x0_0xabcd_phys_0"),
            Key::parse("luid_0x0_0xabcd_phys_1")
        );
        for name in [
            "pid_1_luid_0x0_0x1_phys_0",
            "luid_0x0_0x1_phys_0_extra",
            "luid_0x0_0x1_phys_x",
        ] {
            assert!(Key::parse(name).is_none());
        }
        let names = [
            "pid_1_luid_0x0_0xA_phys_0_eng_0_engtype_3D",
            "pid_2_luid_0x0_0xa_phys_0_eng_0_engtype_3D",
            "pid_1_luid_0x0_0xa_phys_0_eng_1_engtype_3D",
            "pid_1_luid_0x0_0xb_phys_0_eng_0_engtype_3D",
        ];
        let ids: Vec<_> = names
            .iter()
            .map(|s| EngineInstance::parse(s).unwrap())
            .collect();
        let adapters = aggregate(
            ids.iter().zip([20., 30., 80., 10.].map(Usage::Measured)),
            false,
        );
        assert_eq!(adapters.len(), 2);
        assert_eq!(adapters[0].engines.len(), 2);
        assert_eq!(adapters[0].engines[0].usage, Usage::Measured(50.));
        assert_eq!(adapters[0].activity, Usage::Measured(80.));
        assert_eq!(adapters[1].activity, Usage::Measured(10.));
        assert_eq!(
            aggregate(ids.iter().zip([20.; 4].map(Usage::Measured)), true)[0].activity,
            Usage::Partial(40.)
        );
    }
    #[test]
    fn memory_preserves_zero_and_failed_values_without_freshening_them() {
        let now = Instant::now();
        let mut value = Bytes::default();
        value.record(None, now);
        assert_eq!(value.label(now), "--");
        value.record(Some(0), now);
        assert_eq!(value.state(now), "Live");
        value.record(None, now + Duration::from_secs(1));
        assert_eq!(value.value, Some(0));
        assert_eq!(value.last_success, Some(now));
        assert_eq!(value.state(now), "Cached");
        value.record(Some(12_000_000_000), now + Duration::from_secs(2));
        assert_eq!(value.state(now + Duration::from_secs(6)), "Cached");
    }
}
