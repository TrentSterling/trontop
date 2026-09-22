//! Sensor bridge (lane: bridge): read-only connections to sensor providers the
//! user already runs (for example LibreHardwareMonitor's WMI namespace or
//! HWiNFO shared memory). Never install, start or elevate anything.
//!
//! `collect` lists the providers checked and what each exposes (Sensor Sources
//! section). `read_live` runs on its own fast worker and maps readings to
//! LiveKey values: CpuPackageTemperature, CpuCoreTemperature, MotherboardTemperature
//! and Sensor { id } for everything else.
use super::{
    BridgeReading, BridgeReadings, Context, Group, LiveKey, LiveUnit, Row, Section, SectionId,
    SummaryLine, Value,
};
use std::collections::BTreeMap;
use std::time::Duration;

#[cfg(windows)]
mod native;

/// Poll hint while a provider answers, and while none does.
const ACTIVE_POLL: Duration = Duration::from_secs(2);
const IDLE_POLL: Duration = Duration::from_secs(10);

pub const LIBRE: &str = "LibreHardwareMonitor (WMI)";
pub const OPEN: &str = "OpenHardwareMonitor (WMI)";
pub const HWINFO: &str = "HWiNFO (shared memory)";

/// One reading as a provider reports it, before key mapping.
#[derive(Clone, Debug, PartialEq)]
struct Raw {
    /// Stable provider-side identifier (LHM identifier or HWiNFO ids).
    id: String,
    /// Hardware or sensor-chip name, e.g. "Intel Core Ultra 9 285K".
    hardware: String,
    /// True when the hardware is the CPU package.
    cpu: bool,
    /// True when the hardware is the motherboard's Super I/O chip.
    board: bool,
    name: String,
    unit: LiveUnit,
    value: f64,
}

/// The LiveKeys a reading feeds. Every reading gets its own Sensor key; the
/// well-known ones also feed the dedicated CPU and board keys.
fn keys(raw: &Raw, core_index: &mut BTreeMap<String, u32>) -> Vec<LiveKey> {
    let mut keys = vec![LiveKey::Sensor { id: raw.id.clone() }];
    let name = raw.name.to_ascii_lowercase();
    match raw.unit {
        LiveUnit::Celsius if raw.cpu => {
            if name == "cpu package" || name.starts_with("core (tctl") || name == "package" {
                keys.push(LiveKey::CpuPackageTemperature);
            } else if name.starts_with("cpu core #")
                || name.starts_with("p-core")
                || name.starts_with("e-core")
                || (name.starts_with("core ") && name[5..].bytes().all(|b| b.is_ascii_digit()))
            {
                let next = core_index.len() as u32;
                let index = *core_index.entry(name).or_insert(next);
                keys.push(LiveKey::CpuCoreTemperature { index });
            }
        }
        LiveUnit::Celsius if raw.board && (name == "motherboard" || name == "system") => {
            keys.push(LiveKey::MotherboardTemperature);
        }
        LiveUnit::Watts if raw.cpu && (name == "cpu package" || name == "cpu package power") => {
            keys.push(LiveKey::Sensor {
                id: super::cpu::PACKAGE_POWER.into(),
            });
        }
        LiveUnit::Volts
            if (raw.cpu || raw.board)
                && (name == "cpu core"
                    || name == "vcore"
                    || name == "core voltage"
                    || name == "core vids") =>
        {
            keys.push(LiveKey::Sensor {
                id: super::cpu::CORE_VOLTAGE.into(),
            });
        }
        _ => {}
    }
    keys
}

/// Readings for every key, first provider wins for the shared keys.
fn readings(provider: &str, raws: &[Raw], out: &mut Vec<BridgeReading>) {
    let mut cores = BTreeMap::new();
    for raw in raws.iter().filter(|r| r.value.is_finite()) {
        for key in keys(raw, &mut cores) {
            if out.iter().any(|r| r.key == key) {
                continue;
            }
            out.push(BridgeReading {
                key,
                label: format!("{} / {}", raw.hardware, raw.name),
                value: raw.value,
                unit: raw.unit,
                source: provider.to_string(),
            });
        }
    }
}

/// HWiNFO shared memory v2 layout (HWiNFO SDK "hwisenssm2.h", packed).
#[derive(Clone, Debug, PartialEq)]
struct HwinfoSensor {
    id: u32,
    instance: u32,
    name: String,
}

fn ansi(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).trim().to_string()
}

fn parse_hwinfo(bytes: &[u8]) -> Result<Vec<Raw>, String> {
    let dword = |at: usize| {
        bytes
            .get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
    };
    let malformed = || "HWiNFO shared memory layout is not recognised".to_string();
    match dword(0).ok_or_else(malformed)? {
        0x5369_5748 => {}
        0x4441_4544 => return Err("HWiNFO shared memory is inactive (HWiNFO closed or its free-version time limit expired)".into()),
        _ => return Err(malformed()),
    }
    let (sensor_at, sensor_size, sensor_count) = (
        dword(20).ok_or_else(malformed)?,
        dword(24).ok_or_else(malformed)?,
        dword(28).ok_or_else(malformed)?,
    );
    let (reading_at, reading_size, reading_count) = (
        dword(32).ok_or_else(malformed)?,
        dword(36).ok_or_else(malformed)?,
        dword(40).ok_or_else(malformed)?,
    );
    if sensor_size < 264 || reading_size < 316 || sensor_count > 1024 || reading_count > 16384 {
        return Err(malformed());
    }
    let sensors = (0..sensor_count)
        .map(|i| {
            let at = sensor_at + i * sensor_size;
            let element = bytes.get(at..at + 264)?;
            let user = ansi(&element[136..264]);
            Some(HwinfoSensor {
                id: u32::from_le_bytes(element[0..4].try_into().ok()?),
                instance: u32::from_le_bytes(element[4..8].try_into().ok()?),
                name: if user.is_empty() {
                    ansi(&element[8..136])
                } else {
                    user
                },
            })
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(malformed)?;
    let mut raws = Vec::new();
    for i in 0..reading_count {
        let at = reading_at + i * reading_size;
        let element = bytes.get(at..at + 316).ok_or_else(malformed)?;
        let kind = u32::from_le_bytes(element[0..4].try_into().unwrap_or([0; 4]));
        let unit = match kind {
            1 => LiveUnit::Celsius,
            2 => LiveUnit::Volts,
            3 => LiveUnit::Rpm,
            5 => LiveUnit::Watts,
            6 => LiveUnit::Megahertz,
            7 => LiveUnit::Percent,
            _ => continue,
        };
        let sensor_index = u32::from_le_bytes(element[4..8].try_into().unwrap_or([0; 4])) as usize;
        let reading_id = u32::from_le_bytes(element[8..12].try_into().unwrap_or([0; 4]));
        let Some(sensor) = sensors.get(sensor_index) else {
            continue;
        };
        let user = ansi(&element[140..268]);
        let name = if user.is_empty() {
            ansi(&element[12..140])
        } else {
            user
        };
        let value = f64::from_le_bytes(element[284..292].try_into().unwrap_or([0; 8]));
        let lower = sensor.name.to_ascii_lowercase();
        raws.push(Raw {
            id: format!(
                "hwinfo/{:x}/{:x}/{reading_id:x}",
                sensor.id, sensor.instance
            ),
            hardware: sensor.name.clone(),
            cpu: lower.starts_with("cpu [")
                && !lower.contains("enhanced")
                && !lower.contains("c-state"),
            board: !lower.starts_with("cpu")
                && !lower.starts_with("gpu")
                && !lower.starts_with("s.m.a.r.t")
                && !lower.starts_with("drive"),
            name,
            unit,
            value,
        });
    }
    Ok(raws)
}

/// LibreHardwareMonitor / OpenHardwareMonitor Sensor rows: identifier,
/// hardware name, sensor type and value.
fn from_wmi(rows: &[(String, String, String, String, f64)]) -> Vec<Raw> {
    rows.iter()
        .filter_map(|(identifier, hardware, kind, name, value)| {
            let unit = match kind.as_str() {
                "Temperature" => LiveUnit::Celsius,
                "Voltage" => LiveUnit::Volts,
                "Fan" => LiveUnit::Rpm,
                "Power" => LiveUnit::Watts,
                "Clock" => LiveUnit::Megahertz,
                "Load" => LiveUnit::Percent,
                _ => return None,
            };
            let id = identifier.to_ascii_lowercase();
            Some(Raw {
                id: format!("wmi{identifier}"),
                hardware: hardware.clone(),
                cpu: id.starts_with("/intelcpu") || id.starts_with("/amdcpu"),
                board: id.starts_with("/lpc/") || id.starts_with("/motherboard"),
                name: name.clone(),
                unit,
                value: *value,
            })
        })
        .collect()
}

/// What each provider returned in one poll.
#[derive(Clone, Debug)]
struct Poll {
    providers: Vec<(&'static str, Result<Vec<Raw>, String>)>,
}

impl Poll {
    fn readings(&self) -> BridgeReadings {
        let mut out = Vec::new();
        let mut live = Vec::new();
        for (name, result) in &self.providers {
            if let Ok(raws) = result {
                live.push(*name);
                readings(name, raws, &mut out);
            }
        }
        BridgeReadings {
            readings: out,
            status: if live.is_empty() {
                Value::unavailable(format!(
                    "no sensor provider is running ({}); Windows exposes CPU and board temperatures only through a kernel driver, which Trontop never installs",
                    [LIBRE, OPEN, HWINFO].join(", ")
                ))
            } else {
                Value::known(live.join(", "))
            },
            collected_at: None,
            retry_after: Some(if live.is_empty() {
                IDLE_POLL
            } else {
                ACTIVE_POLL
            }),
        }
    }

    fn section(&self) -> Section {
        let mut section = Section::new(SectionId::SensorBridge);
        let mut providers = Group::new("Providers checked");
        let mut any = false;
        for (name, result) in &self.providers {
            match result {
                Ok(raws) => {
                    any = true;
                    providers.push_row(Row::known(
                        *name,
                        format!("Running: {} readings", raws.len()),
                    ));
                }
                // Whether a provider runs is itself a reading: show the reason.
                Err(reason) => {
                    let mut text = reason.clone();
                    if let Some(first) = text.get(..1) {
                        text.replace_range(..1, &first.to_ascii_uppercase());
                    }
                    providers.push_row(Row::known(*name, text));
                }
            }
        }
        providers.push_row(
            Row::known(
                "NVIDIA NVML",
                "Built in: GPU temperature, clocks, power and fan (see Graphics)",
            )
            .note("Read by the sampler when the NVIDIA driver is installed"),
        );
        providers.push_row(Row::known(
            "Windows storage temperature",
            "Built in: drive temperatures where the drive reports them (see Storage)",
        ));
        section.push_summary(SummaryLine::new(if any {
            Value::known(
                self.providers
                    .iter()
                    .filter(|(_, r)| r.is_ok())
                    .map(|(n, _)| *n)
                    .collect::<Vec<_>>()
                    .join(", "),
            )
        } else {
            Value::unavailable("no external sensor provider is running")
        }));
        section.push_group(providers);
        section.push_group(
            Group::new("Readings Trontop maps")
                .row(Row::live(
                    "CPU package temperature",
                    LiveKey::CpuPackageTemperature,
                ))
                .row(Row::live(
                    "Motherboard temperature",
                    LiveKey::MotherboardTemperature,
                ))
                .row(Row::live(
                    "CPU package power",
                    LiveKey::Sensor {
                        id: super::cpu::PACKAGE_POWER.into(),
                    },
                ))
                .row(Row::live(
                    "CPU core voltage",
                    LiveKey::Sensor {
                        id: super::cpu::CORE_VOLTAGE.into(),
                    },
                )),
        );
        for (name, result) in &self.providers {
            let Ok(raws) = result else {
                continue;
            };
            let mut by_hardware = BTreeMap::<&str, Vec<&Raw>>::new();
            for raw in raws {
                by_hardware
                    .entry(raw.hardware.as_str())
                    .or_default()
                    .push(raw);
            }
            for (hardware, raws) in by_hardware {
                let mut group = Group::new(format!("{hardware} ({name})")).collapsed();
                for raw in raws.iter().take(256) {
                    group.push_row(Row::live(
                        raw.name.clone(),
                        LiveKey::Sensor { id: raw.id.clone() },
                    ));
                }
                section.push_group(group);
            }
        }
        section.push_group(
            Group::new("How to add CPU and board temperatures")
                .collapsed()
                .row(Row::known(
                    "Option",
                    "Run LibreHardwareMonitor with its WMI provider, or HWiNFO with Shared Memory Support enabled",
                ))
                .row(Row::known(
                    "What Trontop does",
                    "Reads their published values only; it never installs, starts or elevates anything",
                )),
        );
        section
    }
}

fn poll(ctx: &Context) -> Poll {
    #[cfg(windows)]
    {
        native::poll(ctx)
    }
    #[cfg(not(windows))]
    {
        let _ = ctx;
        Poll {
            providers: vec![
                (LIBRE, Err("read on Windows only".into())),
                (OPEN, Err("read on Windows only".into())),
                (HWINFO, Err("read on Windows only".into())),
            ],
        }
    }
}

pub fn collect(ctx: &Context) -> Section {
    poll(ctx).section()
}

/// One fast poll of every running provider. The worker stamps `collected_at`.
pub fn read_live(ctx: &Context) -> BridgeReadings {
    poll(ctx).readings()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hwinfo_fixture() -> Vec<u8> {
        let mut bytes = vec![0u8; 48];
        bytes[0..4].copy_from_slice(&0x5369_5748u32.to_le_bytes());
        let sensor_at = 48usize;
        let reading_at = sensor_at + 2 * 264;
        for (index, value) in [
            (20, sensor_at),
            (24, 264),
            (28, 2),
            (32, reading_at),
            (36, 316),
            (40, 3),
        ] {
            bytes[index..index + 4].copy_from_slice(&(value as u32).to_le_bytes());
        }
        let mut sensor = |id: u32, name: &str| {
            let mut element = vec![0u8; 264];
            element[0..4].copy_from_slice(&id.to_le_bytes());
            element[8..8 + name.len()].copy_from_slice(name.as_bytes());
            bytes.extend(element);
        };
        sensor(0xF000_0300, "CPU [#0]: Fixture CPU");
        sensor(0xE000_0001, "Fixture Board (Nuvoton NCT0000)");
        let mut reading = |kind: u32, sensor: u32, id: u32, label: &str, value: f64| {
            let mut element = vec![0u8; 316];
            element[0..4].copy_from_slice(&kind.to_le_bytes());
            element[4..8].copy_from_slice(&sensor.to_le_bytes());
            element[8..12].copy_from_slice(&id.to_le_bytes());
            element[12..12 + label.len()].copy_from_slice(label.as_bytes());
            element[284..292].copy_from_slice(&value.to_le_bytes());
            bytes.extend(element);
        };
        reading(1, 0, 1, "CPU Package", 64.5);
        reading(5, 0, 2, "CPU Package Power", 88.0);
        reading(1, 1, 3, "Motherboard", 36.0);
        bytes
    }

    #[test]
    fn hwinfo_layout_maps_package_power_and_board_keys() {
        let raws = parse_hwinfo(&hwinfo_fixture()).unwrap();
        assert_eq!(raws.len(), 3);
        let poll = Poll {
            providers: vec![
                (LIBRE, Err("WMI namespace not present".into())),
                (HWINFO, Ok(raws)),
            ],
        };
        let live = poll.readings();
        assert_eq!(live.status, Value::known(HWINFO));
        let find = |key: &LiveKey| {
            live.readings
                .iter()
                .find(|r| &r.key == key)
                .map(|r| r.value)
        };
        assert_eq!(find(&LiveKey::CpuPackageTemperature), Some(64.5));
        assert_eq!(find(&LiveKey::MotherboardTemperature), Some(36.0));
        assert_eq!(
            find(&LiveKey::Sensor {
                id: crate::specs::cpu::PACKAGE_POWER.into()
            }),
            Some(88.0)
        );
        assert!(live.readings.iter().all(|r| r.source == HWINFO));
        let section = poll.section();
        let text = crate::specs::probe_text(std::slice::from_ref(&section));
        assert!(
            text.contains("HWiNFO (shared memory): Running: 3 readings"),
            "{text}"
        );
        assert!(
            text.contains("CPU [#0]: Fixture CPU (HWiNFO (shared memory))"),
            "{text}"
        );
    }

    #[test]
    fn corrupt_or_dead_shared_memory_is_an_error_not_a_panic() {
        let good = hwinfo_fixture();
        for cut in 0..good.len() {
            let _ = parse_hwinfo(&good[..cut]);
        }
        let mut dead = good.clone();
        dead[0..4].copy_from_slice(&0x4441_4544u32.to_le_bytes());
        assert!(parse_hwinfo(&dead).unwrap_err().contains("inactive"));
    }

    #[test]
    fn libre_identifiers_map_cores_in_order_and_no_provider_is_explained() {
        let rows = vec![
            (
                "/intelcpu/0/temperature/0".to_string(),
                "Fixture CPU".to_string(),
                "Temperature".to_string(),
                "CPU Core #1".to_string(),
                50.0,
            ),
            (
                "/intelcpu/0/temperature/1".into(),
                "Fixture CPU".into(),
                "Temperature".into(),
                "CPU Core #2".into(),
                52.0,
            ),
            (
                "/intelcpu/0/temperature/9".into(),
                "Fixture CPU".into(),
                "Temperature".into(),
                "CPU Package".into(),
                58.0,
            ),
            (
                "/intelcpu/0/data/0".into(),
                "Fixture CPU".into(),
                "Data".into(),
                "Ignored".into(),
                1.0,
            ),
        ];
        let raws = from_wmi(&rows);
        assert_eq!(raws.len(), 3);
        let mut out = Vec::new();
        readings(LIBRE, &raws, &mut out);
        assert!(
            out.iter()
                .any(|r| r.key == LiveKey::CpuCoreTemperature { index: 1 } && r.value == 52.0)
        );
        assert!(out.iter().any(|r| r.key == LiveKey::CpuPackageTemperature));
        let none = Poll {
            providers: vec![(LIBRE, Err("x".into())), (HWINFO, Err("y".into()))],
        }
        .readings();
        assert!(none.status.reason().unwrap().contains("never installs"));
        assert_eq!(none.retry_after, Some(IDLE_POLL));
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "Read-only Sensor Sources specs probe; no driver, elevation, window or input"]
    fn native_specs_bridge_read_only_probe() {
        let started = std::time::Instant::now();
        let sections = [collect(&Context::probe())];
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        println!("{}", crate::specs::probe_text(&sections));
        let live = read_live(&Context::probe());
        println!(
            "live status {:?}; {} readings",
            live.status,
            live.readings.len()
        );
        println!("collected in {elapsed:.3} ms (private values masked)");
    }
}
