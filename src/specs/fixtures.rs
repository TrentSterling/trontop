//! Synthetic TEST DATA for System page tests. Never a runtime fallback: this
//! module only exists under cfg(test). Names match the ui_smoke sampler fixture
//! so live keys resolve against it.
use super::*;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

pub const PRIVATE_SERIAL: &str = "FIXTURE-PRIVATE-SERIAL-0042";
pub const GPU_NAME: &str = "Fixture NVIDIA GPU (test data)";
pub const DRIVE_INTERFACE: &str = "PRIVATE-FIXTURE-STORAGE-INTERFACE";

fn health(state: SectionState, at: Instant) -> SectionHealth {
    SectionHealth {
        state,
        collected_at: Some(at),
        collected_wall: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(1_750_000_000)),
        duration: Some(Duration::from_micros(4_200)),
        collecting_since: None,
        issues: Vec::new(),
    }
}

fn cpu() -> Section {
    let cores = (0..4).map(|number| {
        Group::new(format!("Fixture core {number}"))
            .collapsed()
            .row(Row::live(
                "Clock",
                LiveKey::CpuCoreClock { group: 0, number },
            ))
            .row(Row::known(
                "Core type",
                if number < 2 {
                    "Performance"
                } else {
                    "Efficient"
                },
            ))
    });
    let mut cores_group = Group::new("Cores");
    for core in cores {
        cores_group.push_group(core);
    }
    Section::new(SectionId::Cpu)
        .summary_line(
            SummaryLine::known("Fixture processor with a long descriptive model name")
                .live(LiveKey::CpuPackageTemperature),
        )
        .group(
            Group::new("Fixture processor with a long descriptive model name")
                .live(LiveKey::CpuPackageTemperature)
                .row(Row::known("Cores", "24 (8 Performance + 16 Efficient)"))
                .row(Row::known("Threads", "24"))
                .row(Row::known("L3 cache", "36").unit("MB"))
                .row(Row::live("Average clock", LiveKey::CpuClockAverage))
                .row(Row::unavailable(
                    "Package power",
                    "not exposed by Windows without a kernel driver",
                ))
                .row(Row::live(
                    "Package temperature",
                    LiveKey::CpuPackageTemperature,
                ))
                .group(cores_group),
        )
}

fn board() -> Section {
    Section::new(SectionId::Motherboard)
        .summary_line(SummaryLine::known("Fixture Board Co. FX-900 (test data)"))
        .group(
            Group::new("Baseboard")
                .row(Row::known("Manufacturer", "Fixture Board Co."))
                .row(Row::known("Model", "FX-900"))
                .row(Row::known("Serial number", PRIVATE_SERIAL).private())
                .row(Row::unavailable("Version", "not set by the manufacturer")),
        )
        .group(
            Group::new("BIOS")
                .row(Row::known("Version", "1.23"))
                .row(Row::known("Date", "2026-01-15")),
        )
}

fn graphics() -> Section {
    let gpu = |metric| LiveKey::Gpu {
        adapter: GpuRef {
            name: GPU_NAME.into(),
            ordinal: 0,
        },
        metric,
    };
    Section::new(SectionId::Graphics)
        .summary_line(SummaryLine::known(GPU_NAME).live(gpu(GpuMetric::Temperature)))
        .group(
            Group::new(GPU_NAME)
                .live(gpu(GpuMetric::Temperature))
                .row(Row::known("Dedicated memory", "16").unit("GB"))
                .row(Row::live("Core clock", gpu(GpuMetric::CoreClock)))
                .row(Row::live("Board power", gpu(GpuMetric::Power))),
        )
        .issue("Fixture monitor EDID could not be read")
}

fn storage() -> Section {
    let temperature = LiveKey::DriveTemperature {
        interface: DRIVE_INTERFACE.into(),
    };
    Section::new(SectionId::Storage)
        .summary_line(
            SummaryLine::known("Fixture NVMe drive (test data), 2 TB").live(temperature.clone()),
        )
        .group(
            Group::new("Fixture NVMe drive (test data)")
                .live(temperature.clone())
                .row(Row::known("Interface", "NVMe"))
                .row(Row::known("Serial number", PRIVATE_SERIAL).private())
                .row(Row::live("Temperature", temperature)),
        )
}

pub fn snapshot() -> Snapshot {
    // Fixtures do not age into stale state while a slow suite renders.
    let at = Instant::now() + Duration::from_secs(3600);
    let mut snapshot = Snapshot::default();
    for entry in &mut snapshot.entries {
        let (section, state) = match entry.id {
            SectionId::Cpu => (cpu(), SectionState::Complete),
            SectionId::Motherboard => (board(), SectionState::Complete),
            SectionId::Graphics => (graphics(), SectionState::Partial),
            SectionId::Storage => (storage(), SectionState::Complete),
            SectionId::Network => {
                // A slow second read keeps the retained section.
                entry.health.state = SectionState::Collecting;
                continue;
            }
            id => (Section::not_implemented(id), SectionState::Unavailable),
        };
        entry.section = Some(Arc::new(section));
        entry.health = health(state, at);
    }
    snapshot.bridge = Arc::new(BridgeReadings {
        readings: vec![BridgeReading {
            key: LiveKey::CpuPackageTemperature,
            label: "Fixture CPU Package".into(),
            value: 88.0,
            unit: LiveUnit::Celsius,
            source: "Fixture sensor provider (test data)".into(),
        }],
        status: Value::known("Fixture sensor provider (test data)"),
        collected_at: Some(at),
        retry_after: None,
    });
    snapshot
}
