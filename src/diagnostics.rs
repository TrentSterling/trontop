//! Bounded provider health and allowlisted support reports. No native/UI calls.
use std::fmt::Write;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Provider {
    System,
    GpuActivity,
    GpuSensors,
    Services,
    Startup,
    ProcessControls,
    StorageSensors,
    DiskActivity,
}

impl Provider {
    pub const ALL: [Self; 8] = [
        Self::System,
        Self::GpuActivity,
        Self::GpuSensors,
        Self::Services,
        Self::Startup,
        Self::ProcessControls,
        Self::StorageSensors,
        Self::DiskActivity,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::System => "System telemetry",
            Self::GpuActivity => "GPU activity / Windows PDH",
            Self::GpuSensors => "GPU sensors / NVIDIA NVML",
            Self::Services => "Windows services",
            Self::Startup => "Startup inventory",
            Self::ProcessControls => "Process inspection",
            Self::StorageSensors => "Drive sensors / Windows storage",
            Self::DiskActivity => "Physical disks / Windows PDH",
        }
    }

    pub fn cadence(self) -> Duration {
        Duration::from_secs(match self {
            Self::Services | Self::Startup => 30,
            Self::ProcessControls | Self::StorageSensors => 5,
            _ => 1,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum State {
    #[default]
    Starting,
    Live,
    Partial,
    Stale,
    Unavailable,
}

impl State {
    pub fn label(self) -> &'static str {
        match self {
            Self::Starting => "Starting",
            Self::Live => "Live",
            Self::Partial => "Partial",
            Self::Stale => "Stale",
            Self::Unavailable => "Unavailable",
        }
    }
}

// Closed vocabulary: raw native messages can contain paths or identifiers. They
// must never enter the default copyable support report.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Issue {
    MissingSystemData,
    GpuCounters,
    GpuDriver,
    PartialSensors,
    ServiceQuery,
    StartupSources,
    InventoryTimeout,
    InventoryWorker,
    ProcessAccess,
    StorageSensors,
    DiskCounters,
}

impl Issue {
    pub fn description(self) -> &'static str {
        match self {
            Self::MissingSystemData => "CPU or memory information is unavailable.",
            Self::GpuCounters => "GPU counters are unavailable or have no valid readings.",
            Self::GpuDriver => "No supported GPU sensor data; driver missing or query failed.",
            Self::PartialSensors => "Some sensor fields are unsupported or unavailable.",
            Self::ServiceQuery => "Service inventory could not refresh; cached rows may remain.",
            Self::StartupSources => "Some startup sources could not be read completely.",
            Self::InventoryTimeout => {
                "Inventory read exceeded five seconds; prior fields remain cached. No duplicate worker was started."
            }
            Self::InventoryWorker => "Inventory worker is unavailable; prior fields remain cached.",
            Self::ProcessAccess => "Some process identities or scheduler fields are inaccessible.",
            Self::StorageSensors => {
                "Some drive temperatures are unavailable, cached, timed out or beyond the worker limit."
            }
            Self::DiskCounters => {
                "Physical disk counters are unavailable, partial or stale; retained values are not live readings."
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Health {
    pub last_attempt: Option<Instant>,
    pub last_success: Option<Instant>,
    pub query_millis: Option<f64>,
    pub coverage: Option<(usize, usize)>,
    pub issue: Option<Issue>,
    state: State,
}

impl Health {
    pub fn record(
        &mut self,
        at: Instant,
        elapsed: Duration,
        state: State,
        coverage: Option<(usize, usize)>,
        issue: Option<Issue>,
    ) {
        self.last_attempt = Some(at);
        self.query_millis = Some(elapsed.as_secs_f64() * 1000.0);
        self.coverage = coverage;
        self.issue = issue;
        self.state = state;
        if matches!(state, State::Live | State::Partial) {
            self.last_success = Some(at);
        } else if state == State::Unavailable && self.last_success.is_some() {
            self.state = State::Stale;
        }
    }

    pub fn state(&self, provider: Provider, now: Instant) -> State {
        let max_age = provider.cadence().mul_f32(2.5).max(Duration::from_secs(3));
        if matches!(self.state, State::Live | State::Partial | State::Starting)
            && self
                .last_attempt
                .is_some_and(|at| now.saturating_duration_since(at) > max_age)
        {
            if self.last_success.is_some() {
                State::Stale
            } else {
                State::Unavailable
            }
        } else {
            self.state
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Diagnostics {
    entries: [Health; 8],
}

impl Diagnostics {
    pub fn get(&self, provider: Provider) -> &Health {
        &self.entries[provider as usize]
    }
    pub fn get_mut(&mut self, provider: Provider) -> &mut Health {
        &mut self.entries[provider as usize]
    }
}

pub fn age(at: Option<Instant>, now: Instant) -> String {
    at.map(|at| {
        format!(
            "{:.1}s ago",
            now.saturating_duration_since(at).as_secs_f64()
        )
    })
    .unwrap_or_else(|| "Never".into())
}

pub fn support_report(diagnostics: &Diagnostics, now: Instant) -> String {
    // Intentionally accepts ONLY diagnostic metadata, never SystemSnapshot or
    // arbitrary provider error strings. Report fields are explicitly allowlisted.
    let mut report = format!(
        "Trontop support report v1\nVersion: {}\nBuild: {}\nTarget: {}\nProfile: {}\n",
        env!("CARGO_PKG_VERSION"),
        env!("TRONTOP_BUILD_ID"),
        env!("TRONTOP_BUILD_TARGET"),
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "optimized release"
        }
    );
    report.push_str("Local-only diagnostics; no automatic upload.\n\n");
    let _ = writeln!(
        report,
        "Local failure log (when available): {}",
        crate::failure::LOCATION_HINT
    );
    report.push_str("Rust panic/native-runner categories only; no panic/error payloads or dumps. Up to 32 local records; not included in this report.\n\n");
    for provider in Provider::ALL {
        let health = diagnostics.get(provider);
        let _ = writeln!(
            report,
            "{}: {}",
            provider.name(),
            health.state(provider, now).label()
        );
        let _ = writeln!(
            report,
            "  Attempt: {}; last usable data: {}",
            age(health.last_attempt, now),
            age(health.last_success, now)
        );
        if let Some(ms) = health.query_millis {
            let _ = writeln!(report, "  Query: {ms:.3} ms");
        }
        if let Some((readable, total)) = health.coverage {
            let _ = writeln!(report, "  Coverage: {readable}/{total}");
        }
        if let Some(issue) = health.issue {
            let _ = writeln!(report, "  {}", issue.description());
        }
    }
    report.push_str("\nExcluded: process details, paths, commands, account/host names, GPU UUIDs and network addresses.\n");
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_tracks_failure_recovery_partial_and_independent_freshness() {
        let now = Instant::now();
        let mut health = Health::default();
        assert_eq!(health.state(Provider::Services, now), State::Starting);
        health.record(
            now,
            Duration::ZERO,
            State::Unavailable,
            None,
            Some(Issue::ServiceQuery),
        );
        assert_eq!(health.state(Provider::Services, now), State::Unavailable);
        health.record(now, Duration::from_millis(3), State::Live, None, None);
        let later = now + Duration::from_secs(30);
        health.record(
            later,
            Duration::from_millis(1),
            State::Unavailable,
            None,
            Some(Issue::ServiceQuery),
        );
        assert_eq!(health.last_success, Some(now));
        assert_eq!(health.state(Provider::Services, later), State::Stale);
        health.record(
            later,
            Duration::ZERO,
            State::Partial,
            Some((3, 5)),
            Some(Issue::StartupSources),
        );
        assert_eq!(health.state(Provider::Startup, later), State::Partial);
        assert_eq!(
            health.state(Provider::Startup, later + Duration::from_secs(76)),
            State::Stale
        );
        health.record(later, Duration::ZERO, State::Live, Some((5, 5)), None);
        assert_eq!(health.state(Provider::Startup, later), State::Live);
        assert!(health.issue.is_none());
    }

    #[test]
    fn report_is_allowlisted_and_missing_timing_is_not_zero() {
        let report = support_report(&Diagnostics::default(), Instant::now());
        assert!(report.contains("Version:"));
        assert!(report.contains("Attempt: Never; last usable data: Never"));
        assert!(!report.contains("Query: 0"));
        assert!(!report.contains("C:\\"));
        assert!(!report.contains("C:/"));
        assert!(!report.contains("HEADLESS-FIXTURE"));
    }
}
