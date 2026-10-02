//! Firmware byte fixtures and independent PCI facts pass through the actual
//! section builder. Expectations use DMTF DSP0134 3.8.0 (2024-08-05).
use super::*;
use crate::specs::native::smbios::tests::structure;

fn bios_table(major: u8, minor: u8, rom: u16, date: &str) -> Table {
    let mut body = vec![0_u8; 0x1a - 4];
    body[0x08 - 4] = 1;
    body[0x09 - 4] = 0xff;
    body[0x14 - 4] = major;
    body[0x15 - 4] = minor;
    body[0x18 - 4..].copy_from_slice(&rom.to_le_bytes());
    Table::from_structures(3, 8, structure(0, 0, &body, &[date]))
}

#[test]
fn missing_smbios_preserves_independent_chipset_rows_and_all_issues() {
    let section = build(
        None,
        Chipset {
            bridge: Some(("Fixture LPC".into(), None)),
            host: Some(("Fixture host".into(), Some("9.1".into()))),
            error: None,
        },
        vec![
            "SMBIOS fixture failure".into(),
            "Second fixture issue".into(),
        ],
    );
    let text = crate::specs::probe_text(std::slice::from_ref(&section));
    assert!(
        text.contains("Chipset bridge (LPC/eSPI): Fixture LPC"),
        "{text}"
    );
    assert!(text.contains("Host bridge: Fixture host"), "{text}");
    assert!(section.issues.iter().any(|s| s == "Second fixture issue"));
    assert!(section.summary[0].text.reason().is_some());
    assert!(!text.contains("SMBIOS has no BIOS record"));
}

#[test]
fn bios_partial_unknown_release_never_becomes_revision_255() {
    for (major, minor) in [(4, 0xff), (0xff, 3), (0xff, 0xff)] {
        let table = bios_table(major, minor, 16, "01/02/2026");
        let text = crate::specs::probe_text(&[build(Some(&table), Chipset::default(), Vec::new())]);
        assert!(text.contains("System BIOS release: Unavailable"), "{text}");
        assert!(!text.contains("4.255") && !text.contains("255.3"));
    }
}

#[test]
fn firmware_rom_units_match_dmtf_and_reserved_units_stay_unavailable() {
    for (word, expected) in [
        (16, "16 MiB"),
        (0x4030, "48 GiB"),
        (0x8001, "Unavailable"),
        (0xc001, "Unavailable"),
    ] {
        let table = bios_table(3, 2, word, "01/02/2026");
        let text = crate::specs::probe_text(&[build(Some(&table), Chipset::default(), Vec::new())]);
        assert!(text.contains(&format!("ROM size: {expected}")), "{text}");
    }
    let table = Table::from_structures(2, 0, structure(0, 0, &[0, 0, 0, 0, 0, 7], &[]));
    let text = crate::specs::probe_text(&[build(Some(&table), Chipset::default(), Vec::new())]);
    assert!(text.contains("ROM size: 512 KiB"), "{text}");
}

#[test]
fn invalid_bios_dates_stay_raw_in_reports_and_never_normalize_to_invalid_calendar_dates() {
    for date in [
        "02/29/2023",
        "02/30/2024",
        "04/31/2026",
        "07/15/abcd",
        "07/15/2026/trailer",
        "01/01/0000",
        "+2/+3/2026",
    ] {
        assert_eq!(bios_date(date), None, "{date}");
        let table = bios_table(3, 2, 16, date);
        let text = crate::specs::probe_text(&[build(Some(&table), Chipset::default(), Vec::new())]);
        assert!(
            text.contains(&format!("Release date: {date}")),
            "original firmware text remains available"
        );
    }
    for (date, iso) in [
        ("02/29/2024", "2024-02-29"),
        ("02/29/2000", "2000-02-29"),
        ("12/31/99", "1999-12-31"),
    ] {
        assert_eq!(bios_date(date).as_deref(), Some(iso));
    }
    assert_eq!(bios_date("02/29/1900"), None);
}

#[test]
fn locked_chassis_types_and_slot_width_usage_are_read_from_firmware_bytes() {
    let chassis = [
        (4, "Low profile desktop"),
        (5, "Pizza box"),
        (6, "Mini tower"),
        (7, "Tower"),
        (8, "Portable"),
        (9, "Laptop"),
        (10, "Notebook"),
        (11, "Handheld"),
        (12, "Docking station"),
        (13, "All in one"),
        (14, "Sub notebook"),
        (15, "Space-saving"),
        (16, "Lunch box"),
        (17, "Main server chassis"),
        (23, "Rack mount chassis"),
        (24, "Sealed-case PC"),
        (28, "Blade"),
        (30, "Tablet"),
        (31, "Convertible"),
        (32, "Detachable"),
        (33, "IoT gateway"),
        (34, "Embedded PC"),
        (35, "Mini PC"),
        (36, "Stick PC"),
    ];
    for (code, expected) in chassis {
        let table = Table::from_structures(
            3,
            8,
            structure(
                3,
                3,
                &[1, code | 0x80, 0, 2],
                &["Fixture case maker", "FIXTURE-PRIVATE-CHASSIS"],
            ),
        );
        let text = crate::specs::probe_text(&[build(Some(&table), Chipset::default(), Vec::new())]);
        assert!(
            text.contains(&format!("Chassis type: {expected}")),
            "{text}"
        );
        assert!(!text.contains("FIXTURE-PRIVATE-CHASSIS"));
    }
    let mut bytes = Vec::new();
    for (index, (kind, width, usage)) in [
        (0x1f, 8, 4),
        (0x20, 9, 5),
        (0x21, 10, 3),
        (0xa7, 11, 4),
        (0xad, 12, 5),
        (0xb3, 14, 0),
        (0xc4, 0, 0),
        (0xff, 0, 0),
    ]
    .into_iter()
    .enumerate()
    {
        bytes.extend(structure(9, index as u16, &[0, kind, width, usage], &[]));
    }
    let table = Table::from_structures(3, 8, bytes);
    let text = crate::specs::probe_text(&[build(
        Some(&table),
        Chipset {
            error: Some("Fixture PCI failure".into()),
            ..Default::default()
        },
        Vec::new(),
    )]);
    for expected in [
        "M.2 Socket 1-DP (key A), x1 electrical, Available",
        "M.2 Socket 1-SD (key E), x2 electrical, Unavailable",
        "M.2 Socket 2 (key B), x4 electrical, In use",
        "PCI Express x2, x8 electrical, Available",
        "PCI Express 2.0 x2, x12 electrical, Unavailable",
        "PCI Express 3.0 x2, x32 electrical",
        "PCI Express 6.0 or later",
        "Slot 0007h: Unavailable",
        "Fixture PCI failure",
    ] {
        assert!(text.contains(expected), "{expected}: {text}");
    }
    let missing = build(None, Chipset::default(), Vec::new());
    assert_eq!(missing.summary[0].text.reason(), Some("SMBIOS unavailable"));
    let mut bytes = structure(2, 2, &[0, 1], &["Fixture model without maker"]);
    bytes.extend_from_slice(&[0, 1, 2]);
    let table = Table::from_structures(3, 8, bytes);
    let section = build(Some(&table), Chipset::default(), Vec::new());
    assert_eq!(
        section.summary[0].text.text(),
        Some("Fixture model without maker")
    );
    assert!(section.issues.iter().any(|s| s.contains("ended early")));
}

#[cfg(windows)]
#[test]
fn pci_chipset_facts_keep_class_identity_vendor_fallback_and_private_paths_out_of_reports() {
    use crate::specs::native::setupapi::DeviceInfo;
    let device = |name: &str, hardware: &str, class: &str| DeviceInfo {
        instance_id: "FIXTURE-PRIVATE-PCI-INSTANCE".into(),
        friendly_name: (!name.is_empty()).then(|| name.into()),
        hardware_ids: (!hardware.is_empty())
            .then(|| hardware.into())
            .into_iter()
            .collect(),
        compatible_ids: vec![class.into()],
        ..Default::default()
    };
    let mut host = device("Fixture host", "PCI\\VEN_1022&DEV_14B5", "PCI\\CC_060000");
    host.driver_version = Some("2.4".into());
    let chipset = chipset_from_devices(&[
        device("Fixture GPU", "PCI\\VEN_10DE&DEV_2C05", "PCI\\CC_030000"),
        device("Fixture LPC", "pci\\ven_8086&dev_7a86", "pci\\cc_060100"),
        device("", "PCI\\VEN_FFFF&DEV_1234", "PCI\\CC_060101"),
        device("Fixture unnamed vendor bridge", "", "PCI\\CC_060102"),
        host,
    ]);
    let text = crate::specs::probe_text(&[build(
        None,
        chipset,
        vec!["Fixture SMBIOS unavailable".into()],
    )]);
    for expected in [
        "Intel Fixture LPC (PCI 8086:7A86)",
        "Unknown vendor PCI device (PCI FFFF:1234)",
        "Fixture unnamed vendor bridge",
        "AMD Fixture host (PCI 1022:14B5)",
    ] {
        assert!(text.contains(expected), "{text}");
    }
    assert!(!text.contains("Fixture GPU") && !text.contains("FIXTURE-PRIVATE-PCI-INSTANCE"));
    let section = build(
        None,
        Chipset {
            error: Some("Fixture PCI failure".into()),
            ..Default::default()
        },
        vec!["Fixture SMBIOS unavailable".into()],
    );
    assert!(
        section
            .issues
            .iter()
            .any(|s| s == "PCI inventory: Fixture PCI failure")
    );
}
