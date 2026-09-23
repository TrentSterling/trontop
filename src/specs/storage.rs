//! Storage and Optical Drives sections (lane: storage).
//!
//! Scope: physical disks with model, capacity, bus/interface (NVMe, SATA, USB;
//! never "Unknown" when STORAGE_ADAPTER_DESCRIPTOR/bus type says otherwise),
//! media type (SSD/HDD by seek penalty), firmware, partitions and volumes,
//! SMART/health where Windows exposes it without pass-through, and a live
//! LiveKey::DriveTemperature { interface } using the GUID_DEVINTERFACE_DISK
//! path (serials private). Optical: drives, media capabilities, loaded media.
use super::{Context, Group, LiveKey, Row, Section, SectionId, SummaryLine, Value};
use crate::format;

#[cfg(windows)]
mod native;

const NOT_REPORTED_DRIVE: &str = "not reported by the drive or Windows";

/// STORAGE_DEVICE_DESCRIPTOR fields, parsed by offset.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Descriptor {
    vendor: Option<String>,
    product: Option<String>,
    revision: Option<String>,
    serial: Option<String>,
    bus: u32,
    removable: bool,
}

fn descriptor_text(bytes: &[u8], offset: usize) -> Option<String> {
    if offset == 0 || offset >= bytes.len() {
        return None;
    }
    let raw = &bytes[offset..];
    let end = raw.iter().position(|b| *b == 0).unwrap_or(raw.len());
    let text = String::from_utf8_lossy(&raw[..end]).trim().to_string();
    (!text.is_empty()).then_some(text)
}

fn parse_descriptor(bytes: &[u8]) -> Option<Descriptor> {
    let dword = |at: usize| {
        bytes
            .get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    if bytes.len() < 32 {
        return None;
    }
    let size = (dword(4)? as usize).min(bytes.len());
    let bytes = &bytes[..size];
    Some(Descriptor {
        removable: bytes.get(10).is_some_and(|b| *b != 0),
        vendor: descriptor_text(bytes, dword(12)? as usize),
        product: descriptor_text(bytes, dword(16)? as usize),
        revision: descriptor_text(bytes, dword(20)? as usize),
        serial: descriptor_text(bytes, dword(24)? as usize),
        bus: dword(28)?,
    })
}

/// STORAGE_BUS_TYPE names (ntddstor.h).
fn bus_name(bus: u32) -> Option<&'static str> {
    Some(match bus {
        0x01 => "SCSI",
        0x02 => "ATAPI",
        0x03 => "ATA",
        0x04 => "IEEE 1394",
        0x06 => "Fibre Channel",
        0x07 => "USB",
        0x08 => "RAID",
        0x09 => "iSCSI",
        0x0A => "SAS",
        0x0B => "SATA",
        0x0C => "SD",
        0x0D => "MMC",
        0x0E => "Virtual",
        0x0F => "File-backed virtual",
        0x10 => "Storage Spaces",
        0x11 => "NVMe",
        0x12 => "Storage class memory",
        0x13 => "UFS",
        0x14 => "NVMe over Fabrics",
        _ => return None,
    })
}

/// NVMe SMART / Health Information log (log page 02h), NVMe base spec 5.16.1.3.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct NvmeHealth {
    critical_warning: u8,
    composite_kelvin: u16,
    spare: u8,
    spare_threshold: u8,
    used: u8,
    units_read: u128,
    units_written: u128,
    power_cycles: u128,
    power_on_hours: u128,
    unsafe_shutdowns: u128,
    media_errors: u128,
    error_entries: u128,
}

fn parse_nvme_health(log: &[u8]) -> Option<NvmeHealth> {
    if log.len() < 192 {
        return None;
    }
    let u128_at = |at: usize| u128::from_le_bytes(log[at..at + 16].try_into().unwrap_or([0; 16]));
    Some(NvmeHealth {
        critical_warning: log[0],
        composite_kelvin: u16::from_le_bytes([log[1], log[2]]),
        spare: log[3],
        spare_threshold: log[4],
        used: log[5],
        units_read: u128_at(32),
        units_written: u128_at(48),
        power_cycles: u128_at(112),
        power_on_hours: u128_at(128),
        unsafe_shutdowns: u128_at(144),
        media_errors: u128_at(160),
        error_entries: u128_at(176),
    })
}

fn critical_warning(bits: u8) -> String {
    if bits == 0 {
        return "None".into();
    }
    let names = [
        "available spare below threshold",
        "temperature threshold exceeded",
        "reliability degraded",
        "media read-only",
        "volatile memory backup failed",
        "persistent memory region read-only",
    ];
    names
        .iter()
        .enumerate()
        .filter(|(bit, _)| bits & (1 << bit) != 0)
        .map(|(_, name)| *name)
        .collect::<Vec<_>>()
        .join(", ")
}

/// NVMe data units are 1000 x 512 bytes.
fn data_units(units: u128) -> String {
    let bytes = units.saturating_mul(512_000);
    format::bytes(u64::try_from(bytes).unwrap_or(u64::MAX))
}

fn health_group(health: &NvmeHealth) -> Group {
    let mut group = Group::new("NVMe health log");
    group.push_row(Row::known(
        "Critical warnings",
        critical_warning(health.critical_warning),
    ));
    group.push_row(
        Row::known("Percentage used", format!("{}%", health.used))
            .note("Vendor estimate of rated endurance consumed; can exceed 100%"),
    );
    group.push_row(Row::known(
        "Available spare",
        format!("{}% (threshold {}%)", health.spare, health.spare_threshold),
    ));
    group.push_row(Row::known("Data read", data_units(health.units_read)));
    group.push_row(Row::known("Data written", data_units(health.units_written)));
    group.push_row(Row::known(
        "Power-on hours",
        health.power_on_hours.to_string(),
    ));
    group.push_row(Row::known("Power cycles", health.power_cycles.to_string()));
    group.push_row(Row::known(
        "Unsafe shutdowns",
        health.unsafe_shutdowns.to_string(),
    ));
    group.push_row(Row::known("Media errors", health.media_errors.to_string()));
    group.push_row(Row::known(
        "Error log entries",
        health.error_entries.to_string(),
    ));
    if health.composite_kelvin > 0 {
        group.push_row(
            Row::known(
                "Temperature at read time",
                format!("{} \u{b0}C", i32::from(health.composite_kelvin) - 273),
            )
            .note("Composite temperature when this section was read; the live value is on the drive title"),
        );
    }
    group
}

/// Well-known GPT partition type GUIDs.
fn gpt_type(guid: &str) -> Option<&'static str> {
    Some(
        match guid.trim_matches(['{', '}']).to_ascii_lowercase().as_str() {
            "c12a7328-f81f-11d2-ba4b-00a0c93ec93b" => "EFI system",
            "e3c9e316-0b5c-4db8-817d-f92df00215ae" => "Microsoft reserved",
            "ebd0a0a2-b9e5-4433-87c0-68b6b72699c7" => "Basic data",
            "de94bba4-06d1-4d40-a16a-bfd50179d6ac" => "Windows recovery",
            "5808c8aa-7e8f-42e0-85d2-e1e90434cfb3" => "LDM metadata",
            "af9b60a0-1431-4f62-bc68-3311714a69ad" => "LDM data",
            "e75caf8f-f680-4cee-afa3-b001e56efc2d" => "Storage Spaces",
            "0fc63daf-8483-4772-8e79-3d69d8477de4" => "Linux filesystem",
            "0657fd6d-a4ab-43c4-84e5-0933c84b4f4f" => "Linux swap",
            _ => return None,
        },
    )
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Partition {
    number: u32,
    letter: Option<char>,
    bytes: u64,
    kind: Option<String>,
    file_system: Option<String>,
    label: Option<String>,
    free: Option<u64>,
}

/// Everything collected for one GUID_DEVINTERFACE_DISK device.
#[derive(Clone, Debug, Default, PartialEq)]
struct Disk {
    /// Interface path; private, used only as the live temperature key.
    interface: String,
    name: Option<String>,
    number: Option<u32>,
    descriptor: Option<Descriptor>,
    /// IncursSeekPenalty: true for rotating media.
    seek_penalty: Option<bool>,
    trim: Option<bool>,
    sectors: Option<(u32, u32)>,
    bytes: Option<u64>,
    /// MSFT_PhysicalDisk fields.
    media: Option<u64>,
    spindle: Option<u64>,
    health: Option<u64>,
    partition_style: Option<u64>,
    controller: Option<String>,
    link: Option<String>,
    service: Option<String>,
    nvme: Option<NvmeHealth>,
    /// Why the NVMe health log is missing, when it is.
    nvme_error: Option<String>,
    partitions: Vec<Partition>,
}

fn media_text(disk: &Disk) -> Option<&'static str> {
    match (disk.media, disk.seek_penalty) {
        (Some(3), _) => Some("Hard disk (HDD)"),
        (Some(4), _) => Some("Solid state (SSD)"),
        (Some(5), _) => Some("Storage class memory"),
        (_, Some(true)) => Some("Hard disk (HDD)"),
        (_, Some(false)) => Some("Solid state (SSD)"),
        _ => None,
    }
}

fn disk_title(disk: &Disk) -> String {
    disk.name
        .clone()
        .or_else(|| {
            let d = disk.descriptor.as_ref()?;
            let text = [d.vendor.as_deref(), d.product.as_deref()]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" ");
            (!text.is_empty()).then_some(text)
        })
        .unwrap_or_else(|| "Disk".into())
}

fn disk_group(disk: &Disk, reliability: &Value) -> Group {
    let key = LiveKey::DriveTemperature {
        interface: disk.interface.clone(),
    };
    let descriptor = disk.descriptor.clone().unwrap_or_default();
    let mut group = Group::new(disk_title(disk)).live(key.clone());
    group.push_row(Row::new(
        "Capacity",
        Value::from_option(disk.bytes.map(format::bytes), NOT_REPORTED_DRIVE),
    ));
    let bus = bus_name(descriptor.bus);
    group.push_row(
        Row::new(
            "Interface",
            Value::from_option(
                bus.map(|bus| match &disk.link {
                    Some(link) if bus == "NVMe" => format!("NVMe ({link})"),
                    _ => bus.to_string(),
                }),
                format!("Windows reports bus type {}", descriptor.bus),
            ),
        )
        .note("STORAGE_DEVICE_DESCRIPTOR bus type; PCIe link from the NVMe controller"),
    );
    group.push_row(Row::new(
        "Media",
        Value::from_option(media_text(disk), NOT_REPORTED_DRIVE),
    ));
    if let Some(rpm) = disk.spindle.filter(|r| *r > 0 && *r != u64::from(u32::MAX)) {
        group.push_row(Row::known("Rotation speed", format!("{rpm} RPM")));
    }
    group.push_row(Row::new(
        "Firmware",
        Value::from_option(descriptor.revision.clone(), NOT_REPORTED_DRIVE),
    ));
    group.push_row(
        Row::new(
            "Serial number",
            Value::from_option(descriptor.serial.clone(), NOT_REPORTED_DRIVE),
        )
        .private(),
    );
    group.push_row(Row::new(
        "Controller",
        Value::from_option(disk.controller.clone(), NOT_REPORTED_DRIVE),
    ));
    group.push_row(
        Row::new(
            "Driver",
            Value::from_option(disk.service.clone(), NOT_REPORTED_DRIVE),
        )
        .note("Windows service of the storage controller (or of the disk when no controller is known)"),
    );
    group.push_row(Row::new(
        "Removable",
        Value::known(if descriptor.removable { "Yes" } else { "No" }),
    ));
    group.push_row(
        Row::new(
            "TRIM",
            Value::from_option(
                disk.trim.map(|t| {
                    if t {
                        "Supported"
                    } else {
                        "Not supported by the drive"
                    }
                }),
                NOT_REPORTED_DRIVE,
            ),
        )
        .note(
            "DEVICE_TRIM_DESCRIPTOR: drive capability, not the fsutil DisableDeleteNotify setting",
        ),
    );
    group.push_row(Row::new(
        "Sector size",
        Value::from_option(
            disk.sectors.map(|(logical, physical)| {
                format!("{logical} bytes logical, {physical} bytes physical")
            }),
            NOT_REPORTED_DRIVE,
        ),
    ));
    group.push_row(Row::new(
        "Partition style",
        Value::from_option(
            disk.partition_style.and_then(|s| match s {
                1 => Some("MBR"),
                2 => Some("GPT"),
                0 => Some("Not initialized"),
                _ => None,
            }),
            NOT_REPORTED_DRIVE,
        ),
    ));
    group.push_row(
        Row::new(
            "Windows health status",
            Value::from_option(
                disk.health.and_then(|h| match h {
                    0 => Some("Healthy"),
                    1 => Some("Warning"),
                    2 => Some("Unhealthy"),
                    _ => None,
                }),
                NOT_REPORTED_DRIVE,
            ),
        )
        .note("MSFT_PhysicalDisk HealthStatus (Windows Storage Management)"),
    );
    group.push_row(Row::live("Temperature", key));
    if bus == Some("NVMe") {
        match &disk.nvme {
            Some(health) => group.push_group(health_group(health)),
            None => group.push_row(Row::unavailable(
                "NVMe health log",
                disk.nvme_error.clone().unwrap_or_else(|| "not read".into()),
            )),
        }
    } else {
        group.push_row(Row::new("SMART attributes", reliability.clone()).note(
            "ATA SMART needs pass-through or the reliability counters, which Windows restricts",
        ));
    }
    if !disk.partitions.is_empty() {
        let mut partitions = Group::new(format!("Partitions ({})", disk.partitions.len()));
        for part in &disk.partitions {
            let mut title = format!("Partition {}", part.number);
            if let Some(letter) = part.letter {
                title.push_str(&format!(" ({letter}:)"));
            }
            let mut text = format::bytes(part.bytes);
            if let Some(kind) = &part.kind {
                text.push_str(&format!(", {kind}"));
            }
            if let Some(fs) = &part.file_system {
                text.push_str(&format!(", {fs}"));
            }
            if let Some(free) = part.free {
                text.push_str(&format!(", {} free", format::bytes(free)));
            }
            if let Some(label) = &part.label {
                text.push_str(&format!(", \"{label}\""));
            }
            partitions.push_row(Row::known(title, text));
        }
        group.push_group(partitions);
    }
    // The interface path embeds the drive serial; it stays a live-value key
    // only and is never displayed, copied or saved.
    group
}

fn build(disks: Result<Vec<Disk>, String>, reliability: Value, issues: Vec<String>) -> Section {
    let disks = match disks {
        Ok(disks) if !disks.is_empty() => disks,
        Ok(_) => {
            return Section::unavailable(SectionId::Storage, "Windows reports no disk devices");
        }
        Err(reason) => return Section::unavailable(SectionId::Storage, reason),
    };
    let mut section = Section::new(SectionId::Storage);
    for issue in issues {
        section.push_issue(issue);
    }
    for disk in &disks {
        let mut line = disk_title(disk);
        let bus = disk.descriptor.as_ref().and_then(|d| bus_name(d.bus));
        let parts = [
            disk.bytes.map(format::bytes),
            bus.map(str::to_string),
            media_text(disk).map(|m| {
                if m.contains("SSD") {
                    "SSD".to_string()
                } else if m.contains("HDD") {
                    "HDD".to_string()
                } else {
                    m.to_string()
                }
            }),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
        if !parts.is_empty() {
            line.push_str(&format!(" ({})", parts.join(" ")));
        }
        section.push_summary(SummaryLine::known(line).live(LiveKey::DriveTemperature {
            interface: disk.interface.clone(),
        }));
        section.push_group(disk_group(disk, &reliability));
    }
    section
}

fn build_optical(drives: Result<Vec<(String, Vec<Row>)>, String>) -> Section {
    match drives {
        Err(reason) => Section::unavailable(SectionId::OpticalDrives, reason),
        Ok(drives) if drives.is_empty() => Section::new(SectionId::OpticalDrives)
            .summary_line(SummaryLine::known("No optical disk drives"))
            .group(Group::new("Optical drives").row(Row::known("Drives present", "None"))),
        Ok(drives) => {
            let mut section = Section::new(SectionId::OpticalDrives);
            for (name, rows) in drives {
                section.push_summary(SummaryLine::known(name.clone()));
                section.push_group(Group::new(name).rows(rows));
            }
            section
        }
    }
}

pub fn collect(ctx: &Context) -> Section {
    #[cfg(windows)]
    {
        native::collect(ctx)
    }
    #[cfg(not(windows))]
    {
        let _ = ctx;
        Section::unavailable(SectionId::Storage, "read on Windows only")
    }
}

pub fn collect_optical(ctx: &Context) -> Section {
    #[cfg(windows)]
    {
        build_optical(native::optical(ctx))
    }
    #[cfg(not(windows))]
    {
        let _ = ctx;
        build_optical(Err("read on Windows only".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor_bytes(bus: u32) -> Vec<u8> {
        let mut bytes = vec![0u8; 36];
        let strings = b"TEAM\0TM8FP6002T\0SN26510\0FIXTURE-DRIVE-SERIAL\0";
        let base = bytes.len() as u32;
        bytes.extend_from_slice(strings);
        let size = bytes.len() as u32;
        bytes[0..4].copy_from_slice(&36u32.to_le_bytes());
        bytes[4..8].copy_from_slice(&size.to_le_bytes());
        bytes[12..16].copy_from_slice(&base.to_le_bytes());
        bytes[16..20].copy_from_slice(&(base + 5).to_le_bytes());
        bytes[20..24].copy_from_slice(&(base + 16).to_le_bytes());
        bytes[24..28].copy_from_slice(&(base + 24).to_le_bytes());
        bytes[28..32].copy_from_slice(&bus.to_le_bytes());
        bytes
    }

    #[test]
    fn descriptors_parse_by_offset_and_survive_corruption() {
        let bytes = descriptor_bytes(0x11);
        let parsed = parse_descriptor(&bytes).unwrap();
        assert_eq!(parsed.product.as_deref(), Some("TM8FP6002T"));
        assert_eq!(parsed.revision.as_deref(), Some("SN26510"));
        assert_eq!(bus_name(parsed.bus), Some("NVMe"));
        for cut in 0..bytes.len() {
            let _ = parse_descriptor(&bytes[..cut]);
        }
        for index in 0..bytes.len() {
            let mut bad = bytes.clone();
            bad[index] = 0xFF;
            let _ = parse_descriptor(&bad);
        }
    }

    #[test]
    fn nvme_health_log_decodes_the_documented_offsets() {
        let mut log = vec![0u8; 512];
        log[0] = 0b0000_0100;
        log[1..3].copy_from_slice(&310u16.to_le_bytes());
        log[3] = 100;
        log[4] = 10;
        log[5] = 3;
        log[32] = 200;
        log[128] = 0x10;
        log[129] = 0x27;
        let health = parse_nvme_health(&log).unwrap();
        assert_eq!(health.composite_kelvin, 310);
        assert_eq!(health.power_on_hours, 10_000);
        assert_eq!(
            critical_warning(health.critical_warning),
            "reliability degraded"
        );
        assert_eq!(data_units(health.units_read), format::bytes(102_400_000));
        assert!(parse_nvme_health(&log[..100]).is_none());
    }

    #[test]
    fn nvme_interface_is_never_unknown_and_serials_stay_private() {
        let disk = Disk {
            interface: "PRIVATE-FIXTURE-INTERFACE".into(),
            name: Some("TEAM TM8FP6002T".into()),
            descriptor: parse_descriptor(&descriptor_bytes(0x11)),
            bytes: Some(2_048_408_248_320),
            media: Some(4),
            link: Some("PCIe 4.0 x4".into()),
            nvme: Some(NvmeHealth::default()),
            trim: Some(true),
            partitions: vec![Partition {
                number: 3,
                letter: Some('C'),
                bytes: 2_027_299_012_608,
                kind: gpt_type("{ebd0a0a2-b9e5-4433-87c0-68b6b72699c7}").map(str::to_string),
                file_system: Some("NTFS".into()),
                free: Some(1 << 40),
                ..Default::default()
            }],
            ..Default::default()
        };
        let section = build(
            Ok(vec![disk]),
            Value::unavailable("requires administrator"),
            Vec::new(),
        );
        let text = crate::specs::probe_text(std::slice::from_ref(&section));
        assert!(text.contains("Interface: NVMe (PCIe 4.0 x4)"), "{text}");
        assert!(
            text.contains("  TEAM TM8FP6002T (1.86 TB NVMe SSD)"),
            "{text}"
        );
        assert!(
            text.contains("Partition 3 (C:): 1.84 TB, Basic data, NTFS, 1.00 TB free"),
            "{text}"
        );
        assert!(text.contains("NVMe health log"), "{text}");
        assert!(text.contains("TRIM: Supported"), "{text}");
        let hdd = Disk {
            trim: Some(false),
            ..Default::default()
        };
        let hdd = crate::specs::probe_text(&[build(Ok(vec![hdd]), Value::known("-"), Vec::new())]);
        assert!(hdd.contains("TRIM: Not supported by the drive"), "{hdd}");
        assert!(!text.contains("FIXTURE-DRIVE-SERIAL") && !text.contains("PRIVATE-FIXTURE"));
        let mut revealed = String::new();
        crate::specs::section_text(&mut revealed, &section, None, true);
        assert!(revealed.contains("FIXTURE-DRIVE-SERIAL"));
        assert!(
            !revealed.contains("PRIVATE-FIXTURE-INTERFACE"),
            "interface paths are never shown, even revealed"
        );
        assert_eq!(
            section.summary[0].live,
            Some(LiveKey::DriveTemperature {
                interface: "PRIVATE-FIXTURE-INTERFACE".into()
            })
        );
        let optical = build_optical(Ok(Vec::new()));
        assert_eq!(optical.completeness(), crate::specs::Completeness::Complete);
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "Read-only Storage and Optical Drives specs probe; no driver, elevation, window or input"]
    fn native_specs_storage_read_only_probe() {
        let started = std::time::Instant::now();
        let sections = [
            collect(&Context::probe()),
            collect_optical(&Context::probe()),
        ];
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        println!("{}", crate::specs::probe_text(&sections));
        println!("collected in {elapsed:.3} ms (private values masked)");
    }
}
