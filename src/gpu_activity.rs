//! GPU counter identity and missing-data semantics, independent of Windows/UI.
use std::collections::HashMap;

/// Engine type label for Windows engines that report no `engtype_` name.
pub const UNNAMED_ENGINE: &str = "Unnamed";

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Usage {
    #[default]
    Unavailable,
    Warming,
    Unreported,
    Measured(f32),
    Partial(f32),
}

impl Usage {
    pub fn value(self) -> Option<f32> {
        match self {
            Self::Measured(value) | Self::Partial(value) => Some(value),
            _ => None,
        }
    }

    pub fn exact(self) -> Option<f32> {
        match self {
            Self::Measured(value) => Some(value),
            _ => None,
        }
    }

    pub fn label(self) -> String {
        match self {
            Self::Measured(value) => crate::format::percent(value),
            // Compact lower-bound marker; the status and tooltip spell it out.
            Self::Partial(value) => format!("{}+", crate::format::percent(value)),
            _ => "-- %".into(),
        }
    }

    pub fn explanation(self) -> &'static str {
        match self {
            Self::Measured(_) => {
                "Measured GPU activity. A process shows its busiest reported engine."
            }
            Self::Partial(_) => {
                "Partial GPU coverage: a counter was still warming up or unreadable, so + marks a lower bound from the readable counters."
            }
            Self::Warming => "GPU counters need another sample before a rate is available.",
            Self::Unreported => {
                "No GPU counter was reported for this process. This does not prove zero activity."
            }
            Self::Unavailable => "GPU counter data is unavailable or invalid. Missing is not zero.",
        }
    }

    pub fn status(self) -> &'static str {
        match self {
            Self::Measured(_) => "Measured engine activity",
            Self::Partial(_) => "Partial (lower bound)",
            Self::Warming => "Counters warming up",
            Self::Unreported => "No counter reported",
            Self::Unavailable => "Counter data unavailable",
        }
    }

    pub fn sum(self, other: Self) -> Self {
        self.combine(other, |a, b| a + b)
    }

    pub(crate) fn peak(self, other: Self) -> Self {
        self.combine(other, f32::max)
    }

    fn combine(self, other: Self, op: impl FnOnce(f32, f32) -> f32) -> Self {
        match (self, other) {
            (Self::Measured(a), Self::Measured(b)) => Self::Measured(op(a, b)),
            (a, b) => match (a.value(), b.value()) {
                (Some(a), Some(b)) => Self::Partial(op(a, b)),
                (Some(value), None) | (None, Some(value)) => Self::Partial(value),
                _ if a == Self::Warming && b == Self::Warming => Self::Warming,
                _ if a == Self::Unreported && b == Self::Unreported => Self::Unreported,
                _ => Self::Unavailable,
            },
        }
    }

    pub(crate) fn bounded(self) -> Self {
        match self {
            Self::Measured(value) => Self::Measured(value.clamp(0.0, 100.0)),
            Self::Partial(value) => Self::Partial(value.clamp(0.0, 100.0)),
            other => other,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EngineInstance {
    pub pid: u32,
    // Adapter LUID + physical adapter + engine number, NOT just "3D" or "Copy".
    pub physical: String,
    pub kind: String,
}

impl EngineInstance {
    pub fn parse(instance: &str) -> Option<Self> {
        let (pid, rest) = instance.strip_prefix("pid_")?.split_once('_')?;
        let (physical, kind) = rest.split_once("_engtype_")?;
        let luid = physical.strip_prefix("luid_")?;
        let (luid, engine) = luid.split_once("_phys_")?;
        let (high, low) = luid.split_once('_')?;
        u32::from_str_radix(high.strip_prefix("0x")?, 16).ok()?;
        u32::from_str_radix(low.strip_prefix("0x")?, 16).ok()?;
        let (adapter, engine) = engine.split_once("_eng_")?;
        adapter.parse::<u32>().ok()?;
        engine.parse::<u32>().ok()?;
        // Windows publishes some real engines with an empty `engtype_` suffix
        // (seen on NVIDIA RTX drivers). Rejecting them used to flag every sample
        // as partial coverage, so keep them under an explicit neutral label.
        Some(Self {
            pid: pid.parse().ok()?,
            physical: physical.into(),
            kind: if kind.is_empty() {
                UNNAMED_ENGINE.into()
            } else {
                kind.into()
            },
        })
    }
}

pub fn aggregate<'a>(
    readings: impl IntoIterator<Item = (&'a EngineInstance, Usage)>,
) -> (Usage, HashMap<u32, Usage>, Vec<(String, Usage)>) {
    let mut by_pid = HashMap::<u32, Usage>::new();
    let mut physical = HashMap::<&str, (&str, Usage)>::new();
    for (id, reading) in readings {
        by_pid
            .entry(id.pid)
            .and_modify(|value| *value = value.peak(reading))
            .or_insert(reading);
        physical
            .entry(&id.physical)
            .and_modify(|(_, value)| *value = value.sum(reading))
            .or_insert((&id.kind, reading));
    }
    let mut kinds = HashMap::<String, Usage>::new();
    let mut total: Option<Usage> = None;
    for (_, (kind, value)) in physical {
        let value = value.bounded();
        total = Some(total.map_or(value, |total| total.peak(value)));
        kinds
            .entry(kind.into())
            .and_modify(|peak| *peak = peak.peak(value))
            .or_insert(value);
    }
    let mut engines: Vec<_> = kinds.into_iter().collect();
    engines.sort_by(|a, b| {
        b.1.value()
            .unwrap_or(-1.0)
            .total_cmp(&a.1.value().unwrap_or(-1.0))
            .then_with(|| a.0.cmp(&b.0))
    });
    (total.unwrap_or_default(), by_pid, engines)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn id(pid: u32, low: u32, engine: u32, kind: &str) -> EngineInstance {
        EngineInstance::parse(&format!(
            "pid_{pid}_luid_0x00000000_0x{low:08x}_phys_0_eng_{engine}_engtype_{kind}"
        ))
        .unwrap()
    }
    #[test]
    fn busiest_physical_engine_is_not_sum_of_engine_types_or_adapters() {
        let ids = [
            id(1, 1, 0, "3D"),
            id(2, 1, 0, "3D"),
            id(1, 1, 1, "Copy"),
            id(1, 2, 0, "3D"),
        ];
        let (total, pids, engines) = aggregate(
            ids.iter()
                .zip([40.0, 20.0, 25.0, 50.0].map(Usage::Measured)),
        );
        assert_eq!(total, Usage::Measured(60.0));
        assert_eq!(pids[&1], Usage::Measured(50.0));
        assert_eq!(pids[&2], Usage::Measured(20.0));
        assert_eq!(
            engines,
            [
                ("3D".into(), Usage::Measured(60.0)),
                ("Copy".into(), Usage::Measured(25.0))
            ]
        );
    }
    #[test]
    fn invalid_counters_propagate_partial_coverage_without_inventing_zero() {
        let ids = [id(1, 1, 0, "3D"), id(1, 1, 1, "Copy"), id(2, 1, 0, "3D")];
        let (total, pids, _) = aggregate(ids.iter().zip([
            Usage::Measured(0.0),
            Usage::Unavailable,
            Usage::Measured(8.0),
        ]));
        assert_eq!(total, Usage::Partial(8.0));
        assert_eq!(pids[&1], Usage::Partial(0.0));
        assert_eq!(pids[&2], Usage::Measured(8.0));
        assert!(!pids.contains_key(&3));
        assert_eq!(Usage::Measured(0.0).label(), "0.00%");
        assert_eq!(Usage::Unavailable.label(), "-- %");
        assert!(Usage::Partial(8.0).exact().is_none());
    }
    #[test]
    fn unnamed_windows_engines_are_real_engines_not_parse_failures() {
        // Verbatim shape from an RTX 5070 Ti driver (pid 4, engines 7 to 13).
        let parsed =
            EngineInstance::parse("pid_4_luid_0x00000000_0x00011A4A_phys_0_eng_11_engtype_")
                .expect("empty engtype is still a real engine");
        assert_eq!(parsed.pid, 4);
        assert_eq!(parsed.kind, UNNAMED_ENGINE);
        assert_eq!(parsed.physical, "luid_0x00000000_0x00011A4A_phys_0_eng_11");
        assert_eq!(Usage::Partial(3.07).label(), "3.07%+");
    }
    #[test]
    fn parser_requires_real_adapter_and_engine_identity() {
        for value in [
            "pid_1_engtype_3D",
            "pid_x_luid_0x0_0x1_phys_0_eng_0_engtype_3D",
            "pid_1_luid_0x0_0x1_phys_0_eng_x_engtype_3D",
        ] {
            assert!(EngineInstance::parse(value).is_none());
        }
        assert_ne!(id(1, 1, 0, "3D").physical, id(1, 2, 0, "3D").physical);
    }
}
