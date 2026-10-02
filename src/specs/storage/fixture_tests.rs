//! Independent descriptor/health/section fixtures; no device opens or writes.
use super::*;

fn report(disks: Vec<Disk>) -> String {
    crate::specs::probe_text(&[build(
        Ok(disks),
        Value::unavailable("fixture SMART unavailable"),
        Vec::new(),
    )])
}

#[test]
fn missing_descriptor_does_not_claim_fixed_media() {
    let text = report(vec![Disk::default()]);
    assert!(text.contains("Removable: Unavailable"), "{text}");
    assert!(!text.contains("Removable: No"));
    for (removable, expected) in [(false, "No"), (true, "Yes")] {
        let text = report(vec![Disk {
            descriptor: Some(Descriptor {
                removable,
                ..Default::default()
            }),
            ..Default::default()
        }]);
        assert!(text.contains(&format!("Removable: {expected}")), "{text}");
    }
}

#[test]
fn invalid_descriptor_headers_cannot_supply_bus_or_media_facts() {
    let mut bytes = super::tests::descriptor_bytes(0x11);
    for (version, size) in [(0_u32, 64_u32), (36, 0), (36, 31), (64, 36)] {
        bytes[0..4].copy_from_slice(&version.to_le_bytes());
        bytes[4..8].copy_from_slice(&size.to_le_bytes());
        assert!(
            parse_descriptor(&bytes).is_none(),
            "invalid header {version}/{size}"
        );
    }
}

#[test]
fn descriptor_strings_require_termination_inside_the_reported_body() {
    let mut bytes = super::tests::descriptor_bytes(0x11);
    bytes[12..16].copy_from_slice(&28_u32.to_le_bytes());
    let parsed = parse_descriptor(&bytes).unwrap();
    assert!(parsed.vendor.is_none(), "header bytes are not vendor text");
    let mut bytes = super::tests::descriptor_bytes(0x11);
    let at = bytes.len() as u32;
    bytes.extend_from_slice(b"UNTERMINATED");
    bytes[24..28].copy_from_slice(&at.to_le_bytes());
    let length = bytes.len() as u32;
    bytes[4..8].copy_from_slice(&length.to_le_bytes());
    assert!(parse_descriptor(&bytes).unwrap().serial.is_none());
    bytes.push(0);
    let length = bytes.len() as u32;
    bytes[4..8].copy_from_slice(&length.to_le_bytes());
    assert_eq!(
        parse_descriptor(&bytes).unwrap().serial.as_deref(),
        Some("UNTERMINATED")
    );
}

#[test]
fn wide_nvme_data_counts_preserve_the_original_quantity() {
    assert_eq!(data_units(200), format::bytes(102_400_000));
    let units = u128::MAX;
    let text = data_units(units);
    assert!(text.contains(&units.to_string()), "{text}");
    assert!(
        text.contains("512,000"),
        "the exact conversion factor remains visible"
    );
    assert_ne!(text, format::bytes(u64::MAX));
}

#[test]
fn reserved_nvme_warning_bits_remain_visible_instead_of_becoming_blank() {
    for bits in [0x40, 0x80, 0xc0, 0x84] {
        let text = critical_warning(bits);
        assert!(text.contains("unrecognized"), "{bits:02X}: {text}");
        if bits & 4 != 0 {
            assert!(text.contains("reliability degraded"));
        }
    }
}

fn protocol_reply() -> Vec<u8> {
    let mut reply = vec![0_u8; 560];
    for (at, value) in [
        (0, 48_u32),
        (4, 48),
        (8, 3),
        (12, 2),
        (16, 2),
        (24, 40),
        (28, 512),
    ] {
        reply[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    reply[48 + 1..48 + 3].copy_from_slice(&310_u16.to_le_bytes());
    reply[48 + 3] = 100;
    reply
}

#[test]
fn malformed_protocol_headers_do_not_turn_header_bytes_into_health_readings() {
    for (at, value) in [
        (0, 0_u32),
        (0, 64),
        (4, 0),
        (4, u32::MAX),
        (8, 1),
        (12, 1),
        (24, 0),
        (24, u32::MAX),
        (28, 511),
        (28, u32::MAX),
    ] {
        let mut reply = protocol_reply();
        reply[at..at + 4].copy_from_slice(&value.to_le_bytes());
        assert!(
            parse_nvme_reply(&reply).is_none(),
            "invalid field {at}: {value}"
        );
    }
    let parsed = parse_nvme_reply(&protocol_reply()).unwrap();
    assert_eq!(parsed.composite_kelvin, 310);
    assert_eq!(parsed.spare, 100);
    let reply = protocol_reply();
    for cut in 0..reply.len() {
        assert!(
            parse_nvme_reply(&reply[..cut]).is_none(),
            "short reply: {cut}"
        );
    }
    let mut extended = vec![0_u8; 576];
    extended[..48].copy_from_slice(&reply[..48]);
    extended[0..4].copy_from_slice(&64_u32.to_le_bytes());
    extended[4..8].copy_from_slice(&64_u32.to_le_bytes());
    extended[24..28].copy_from_slice(&56_u32.to_le_bytes());
    extended[64..].copy_from_slice(&reply[48..]);
    assert_eq!(parse_nvme_reply(&extended).unwrap().composite_kelvin, 310);
}

#[test]
fn disk_failure_preserves_collection_issues_even_when_no_devices_are_readable() {
    for disks in [
        Ok(Vec::new()),
        Err("Fixture device enumeration failure".into()),
    ] {
        let section = build(
            disks,
            Value::unavailable("Fixture SMART"),
            vec!["Fixture read budget".into()],
        );
        assert!(section.issues.iter().any(|s| s == "Fixture read budget"));
    }
}

#[test]
fn native_bus_names_survive_media_and_title_fallbacks_without_private_paths() {
    let buses = [
        (1, "SCSI"),
        (2, "ATAPI"),
        (3, "ATA"),
        (4, "IEEE 1394"),
        (5, "SSA"),
        (6, "Fibre Channel"),
        (7, "USB"),
        (8, "RAID"),
        (9, "iSCSI"),
        (10, "SAS"),
        (11, "SATA"),
        (12, "SD"),
        (13, "MMC"),
        (14, "Virtual"),
        (15, "File-backed virtual"),
        (16, "Storage Spaces"),
        (17, "NVMe"),
        (18, "Storage class memory"),
        (19, "UFS"),
        (20, "NVMe over Fabrics"),
    ];
    for (bus, name) in buses {
        let text = report(vec![Disk {
            interface: "FIXTURE-PRIVATE-DISK-INTERFACE".into(),
            descriptor: Some(Descriptor {
                vendor: Some("Fixture".into()),
                product: Some("Drive".into()),
                bus,
                ..Default::default()
            }),
            media: Some(5),
            ..Default::default()
        }]);
        assert!(
            text.contains(&format!("Interface: {name}")) && text.contains("Fixture Drive"),
            "{text}"
        );
        assert!(
            text.contains("Media: Storage class memory")
                && !text.contains("FIXTURE-PRIVATE-DISK-INTERFACE")
        );
    }
    for (seek_penalty, name) in [(true, "Hard disk (HDD)"), (false, "Solid state (SSD)")] {
        let text = report(vec![Disk {
            seek_penalty: Some(seek_penalty),
            ..Default::default()
        }]);
        assert!(text.contains(&format!("Media: {name}")), "{text}");
    }
    let text = report(vec![Disk {
        descriptor: Some(Descriptor {
            bus: 0xffff,
            ..Default::default()
        }),
        ..Default::default()
    }]);
    assert!(text.contains("Interface: Unavailable (Windows reports bus type 65535)"));
    assert!(text.contains("  Disk") && text.contains("Media: Unavailable"));
}

#[test]
fn disk_health_layout_and_partition_types_survive_partial_reports() {
    for (style, style_name, health, health_name) in [
        (0, "Not initialized", 0, "Healthy"),
        (1, "MBR", 1, "Warning"),
        (2, "GPT", 2, "Unhealthy"),
        (9, "Unavailable", 9, "Unavailable"),
    ] {
        let text = report(vec![Disk {
            partition_style: Some(style),
            health: Some(health),
            spindle: Some(7200),
            sectors: Some((512, 4096)),
            ..Default::default()
        }]);
        assert!(
            text.contains(&format!("Partition style: {style_name}"))
                && text.contains(&format!("Windows health status: {health_name}")),
            "{text}"
        );
        assert!(
            text.contains("Rotation speed: 7200 RPM")
                && text.contains("Sector size: 512 bytes logical, 4096 bytes physical")
        );
    }
    let guids = [
        ("c12a7328-f81f-11d2-ba4b-00a0c93ec93b", "EFI system"),
        ("e3c9e316-0b5c-4db8-817d-f92df00215ae", "Microsoft reserved"),
        ("ebd0a0a2-b9e5-4433-87c0-68b6b72699c7", "Basic data"),
        ("de94bba4-06d1-4d40-a16a-bfd50179d6ac", "Windows recovery"),
        ("5808c8aa-7e8f-42e0-85d2-e1e90434cfb3", "LDM metadata"),
        ("af9b60a0-1431-4f62-bc68-3311714a69ad", "LDM data"),
        ("e75caf8f-f680-4cee-afa3-b001e56efc2d", "Storage Spaces"),
        ("0fc63daf-8483-4772-8e79-3d69d8477de4", "Linux filesystem"),
        ("0657fd6d-a4ab-43c4-84e5-0933c84b4f4f", "Linux swap"),
    ];
    for (guid, name) in guids {
        let input = format!("{{{}}}", guid.to_uppercase());
        assert_eq!(gpt_type(&input), Some(name));
        let text = report(vec![Disk {
            partitions: vec![Partition {
                number: Some(2),
                bytes: Some(1 << 30),
                kind: gpt_type(&input).map(str::to_string),
                file_system: Some("NTFS".into()),
                label: Some("Fixture volume".into()),
                ..Default::default()
            }],
            ..Default::default()
        }]);
        assert!(
            text.contains(&format!(
                "Partition 2: 1.00 GiB, {name}, NTFS, \"Fixture volume\""
            )),
            "{text}"
        );
    }
    assert_eq!(gpt_type("fixture-unknown-type"), None);
    let text = report(vec![Disk {
        spindle: Some(u64::from(u32::MAX)),
        descriptor: Some(Descriptor {
            bus: 17,
            ..Default::default()
        }),
        nvme_error: Some("Fixture permission failure".into()),
        ..Default::default()
    }]);
    assert!(
        !text.contains("Rotation speed:")
            && text.contains("NVMe health log: Unavailable (Fixture permission failure)")
    );
}

#[test]
fn nvme_health_and_optical_sections_keep_readings_reasons_and_privacy() {
    let mut log = vec![0_u8; 512];
    log[0] = 0x3f;
    log[3] = 95;
    log[4] = 10;
    log[5] = 255;
    for at in [32, 48, 112, 128, 144, 160, 176] {
        log[at..at + 16].copy_from_slice(&u128::MAX.to_le_bytes());
    }
    let health = parse_nvme_health(&log).unwrap();
    let text = crate::specs::group_text(&health_group(&health), None, false);
    for phrase in [
        "available spare below threshold",
        "temperature threshold exceeded",
        "reliability degraded",
        "media read-only",
        "volatile memory backup failed",
        "persistent memory region read-only or unreliable",
        "Percentage used: 255%",
        "Available spare: 95% (threshold 10%)",
    ] {
        assert!(text.contains(phrase), "{text}");
    }
    assert!(text.contains(&format!("Power-on hours: {}", u128::MAX)));
    assert!(text.contains(&format!("Data written: {} data units", u128::MAX)));
    assert!(!text.contains("Temperature at read time:"));
    assert_eq!(critical_warning(0), "None");
    let sections = [
        build_optical(Ok(vec![(
            "Fixture optical drive".into(),
            vec![
                Row::known("Manufacturer", "Fixture optical maker"),
                Row::known("Device instance", "FIXTURE-OPTICAL-PRIVATE").private(),
            ],
        )])),
        build_optical(Err("Fixture optical enumeration failure".into())),
    ];
    let text = crate::specs::probe_text(&sections);
    assert!(
        text.contains("Fixture optical maker")
            && text.contains("Fixture optical enumeration failure")
    );
    assert!(!text.contains("FIXTURE-OPTICAL-PRIVATE"));
    let mut revealed = String::new();
    crate::specs::section_text(&mut revealed, &sections[0], None, true);
    assert!(revealed.contains("FIXTURE-OPTICAL-PRIVATE"));
}
