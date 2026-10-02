//! WMI contract fixtures passed through the production join and report paths.
//! No COM connection, disk handle, private user data or filesystem mutation.
use super::*;
use crate::specs::native::wmi::WmiValue;

fn row(values: &[(&str, WmiValue)]) -> WmiRow {
    WmiRow {
        properties: values
            .iter()
            .map(|(k, v)| ((*k).into(), v.clone()))
            .collect(),
    }
}
fn text(value: &str) -> WmiValue {
    WmiValue::Text(value.into())
}
fn number(value: u64) -> WmiValue {
    WmiValue::UInt(value)
}

#[derive(Default)]
struct Provider {
    physical: Vec<WmiRow>,
    disks: Vec<WmiRow>,
    volumes: Vec<WmiRow>,
    partitions: Vec<WmiRow>,
    fail: Option<&'static str>,
}

impl Provider {
    fn query(&self, wql: &str) -> Result<Vec<WmiRow>, String> {
        assert!(
            wql.starts_with("SELECT "),
            "only fixture read queries allowed"
        );
        let class = wql.rsplit(" FROM ").next().unwrap();
        if self.fail == Some(class) {
            return Err("fixture access denied".into());
        }
        Ok(match class {
            "MSFT_PhysicalDisk" => self.physical.clone(),
            "MSFT_Disk" => self.disks.clone(),
            "MSFT_Volume" => self.volumes.clone(),
            "MSFT_Partition" => self.partitions.clone(),
            "MSFT_StorageReliabilityCounter" => Vec::new(),
            _ => panic!("unexpected query: {wql}"),
        })
    }
}

fn disk(number: u32) -> Disk {
    Disk {
        number: Some(number),
        name: Some(format!("Fixture disk {number}")),
        ..Default::default()
    }
}
fn disk_row(n: u64, uid: &str, bytes: u64) -> WmiRow {
    row(&[
        ("Number", number(n)),
        ("UniqueId", text(uid)),
        ("UniqueIdFormat", number(2)),
        ("Size", number(bytes)),
        ("PartitionStyle", number(2)),
    ])
}
fn physical_row(id: &str, uid: &str, bytes: u64, health: u64) -> WmiRow {
    row(&[
        ("DeviceId", text(id)),
        ("UniqueId", text(uid)),
        ("UniqueIdFormat", number(2)),
        ("Size", number(bytes)),
        ("HealthStatus", number(health)),
        ("MediaType", number(4)),
        ("SpindleSpeed", number(0)),
    ])
}
fn partition(n: u64, p: u64, letter: u64, size: u64, path: &str) -> WmiRow {
    row(&[
        ("DiskNumber", number(n)),
        ("PartitionNumber", number(p)),
        ("DriveLetter", number(letter)),
        ("Size", number(size)),
        ("AccessPaths", WmiValue::Array(vec![text(path)])),
        ("GptType", text("ebd0a0a2-b9e5-4433-87c0-68b6b72699c7")),
    ])
}
fn volume(path: &str, size: u64, free: u64, label: &str) -> WmiRow {
    row(&[
        ("Path", text(path)),
        ("Size", number(size)),
        ("SizeRemaining", number(free)),
        ("FileSystem", text("NTFS")),
        ("FileSystemLabel", text(label)),
    ])
}
fn run(disks: &mut [Disk], provider: &Provider) -> (Vec<String>, Value) {
    let mut issues = Vec::new();
    let reliability = collect(disks, &mut issues, |wql| provider.query(wql));
    (issues, reliability)
}
fn report(disks: Vec<Disk>, issues: Vec<String>, reliability: Value) -> String {
    crate::specs::probe_text(&[build(Ok(disks), reliability, issues)])
}

#[test]
fn overflow_disk_numbers_never_attach_metadata_or_partitions_to_another_disk() {
    let mut disks = vec![disk(0), disk(1)];
    let provider = Provider {
        disks: vec![disk_row(1u64 << 32, "wrong", 1 << 40)],
        partitions: vec![partition((1u64 << 32) + 1, 2, b'Z' as u64, 1 << 30, "bad")],
        ..Default::default()
    };
    run(&mut disks, &provider);
    assert!(
        disks
            .iter()
            .all(|d| d.partition_style.is_none() && d.partitions.is_empty())
    );
}

#[test]
fn physical_device_ids_are_not_os_disk_numbers_and_unique_ids_join_the_right_disk() {
    let mut disks = vec![disk(0), disk(1)];
    let provider = Provider {
        disks: vec![disk_row(0, "uid-A", 1 << 40), disk_row(1, "uid-B", 1 << 39)],
        physical: vec![
            physical_row("0", "uid-B", 1 << 39, 2),
            physical_row("opaque id", "uid-A", 1 << 40, 0),
        ],
        ..Default::default()
    };
    let (issues, _) = run(&mut disks, &provider);
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!((disks[0].bytes, disks[0].health), (Some(1 << 40), Some(0)));
    assert_eq!((disks[1].bytes, disks[1].health), (Some(1 << 39), Some(2)));
}

#[test]
fn duplicate_physical_identity_keeps_os_capacity_but_never_chooses_first_health() {
    let mut disks = vec![disk(0)];
    let provider = Provider {
        disks: vec![disk_row(0, "same-private-id", 1 << 40)],
        physical: vec![
            physical_row("0", "same-private-id", 1 << 40, 0),
            physical_row("99", "same-private-id", 1 << 40, 2),
        ],
        ..Default::default()
    };
    let (issues, _) = run(&mut disks, &provider);
    assert_eq!(disks[0].bytes, Some(1 << 40));
    assert!(disks[0].health.is_none() && disks[0].media.is_none());
    assert!(issues.iter().any(|s| s.contains("ambiguous")), "{issues:?}");
    assert!(!issues.join(" ").contains("same-private-id"));
}

#[test]
fn mismatched_or_missing_identifier_formats_do_not_join_physical_facts() {
    for format in [WmiValue::Null, number(3), number(u64::MAX)] {
        let mut disks = vec![disk(0)];
        let mut physical = physical_row("0", "same-id", 1 << 40, 2);
        physical.properties.push(("unused".into(), WmiValue::Null));
        *physical
            .properties
            .iter_mut()
            .find(|(k, _)| k == "UniqueIdFormat")
            .unwrap() = ("UniqueIdFormat".into(), format);
        let provider = Provider {
            disks: vec![disk_row(0, "same-id", 1 << 40)],
            physical: vec![physical],
            ..Default::default()
        };
        run(&mut disks, &provider);
        assert!(disks[0].health.is_none());
        assert_eq!(disks[0].bytes, Some(1 << 40));
    }
}

#[test]
fn unknown_partition_size_and_number_preserve_readable_facts_without_zero_claims() {
    let mut disks = vec![disk(0)];
    let provider = Provider {
        partitions: vec![row(&[
            ("DiskNumber", number(0)),
            ("PartitionNumber", WmiValue::Null),
            ("Size", WmiValue::Null),
            ("DriveLetter", number(b'C' as u64)),
            ("MbrType", number(7)),
        ])],
        ..Default::default()
    };
    let (issues, reliability) = run(&mut disks, &provider);
    let text = report(disks, issues, reliability);
    assert!(
        text.contains("Partition (number unavailable) (C:)") && text.contains("Size unavailable"),
        "{text}"
    );
    assert!(text.contains("MBR type 07h"));
    assert!(!text.contains("Partition 0") && !text.contains("0 B"));
}

#[test]
fn partition_numbers_and_letters_reject_truncation_while_real_lowercase_letters_normalize() {
    let mut disks = vec![disk(0)];
    let provider = Provider {
        partitions: vec![
            partition(
                0,
                (1u64 << 32) + 7,
                (1u64 << 32) + b'Z' as u64,
                1 << 20,
                "none",
            ),
            partition(0, 2, b'c' as u64, 1 << 30, "none"),
            partition(0, 1, 0, 1 << 30, "none"),
        ],
        ..Default::default()
    };
    let (issues, reliability) = run(&mut disks, &provider);
    let text = report(disks, issues, reliability);
    assert!(text.contains("Partition (number unavailable)"));
    assert!(text.contains("Partition 2 (C:)"));
    assert!(!text.contains("(Z:)") && !text.contains("Partition 7"));
    assert!(text.find("Partition 1:").unwrap() < text.find("Partition 2").unwrap());
}

#[test]
fn volume_query_failure_is_reported_while_readable_partition_metadata_survives() {
    let mut disks = vec![disk(0)];
    let provider = Provider {
        partitions: vec![partition(0, 1, b'C' as u64, 1 << 30, "path")],
        fail: Some("MSFT_Volume"),
        ..Default::default()
    };
    let (issues, reliability) = run(&mut disks, &provider);
    assert!(
        issues
            .iter()
            .any(|s| s.contains("MSFT_Volume") && s.contains("fixture access denied")),
        "{issues:?}"
    );
    let text = report(disks, issues, reliability);
    assert!(text.contains("Partition 1 (C:)") && text.contains("Basic data"));
}

#[test]
fn case_insensitive_volume_paths_join_and_valid_zero_free_space_is_retained() {
    let mut disks = vec![disk(0)];
    let provider = Provider {
        partitions: vec![partition(
            0,
            1,
            b'D' as u64,
            2 << 30,
            r"\\?\Volume{fixture}\",
        )],
        volumes: vec![volume(
            r"\\?\VOLUME{FIXTURE}\",
            1 << 30,
            0,
            "Fixture full volume",
        )],
        ..Default::default()
    };
    let (issues, _) = run(&mut disks, &provider);
    assert!(issues.is_empty());
    let p = &disks[0].partitions[0];
    assert_eq!(p.file_system.as_deref(), Some("NTFS"));
    assert_eq!(p.label.as_deref(), Some("Fixture full volume"));
    assert_eq!(p.free, Some(0));
}

#[test]
fn ambiguous_volume_paths_do_not_mix_first_volume_fields_into_partition() {
    let mut disks = vec![disk(0)];
    let provider = Provider {
        partitions: vec![partition(0, 1, 0, 1 << 30, "private-volume-path")],
        volumes: vec![
            volume("private-volume-path", 1 << 30, 10, "First"),
            volume("PRIVATE-VOLUME-PATH", 1 << 30, 20, "Second"),
        ],
        ..Default::default()
    };
    let (issues, _) = run(&mut disks, &provider);
    let p = &disks[0].partitions[0];
    assert!(p.file_system.is_none() && p.label.is_none() && p.free.is_none());
    assert!(issues.iter().any(|s| s.contains("ambiguous")));
    assert!(!issues.join(" ").contains("private-volume-path"));
}

#[test]
fn impossible_volume_free_space_is_unavailable_but_other_facts_stay_readable() {
    let mut disks = vec![disk(0)];
    let provider = Provider {
        partitions: vec![partition(0, 1, 0, 1 << 30, "path")],
        volumes: vec![volume("path", 100, 101, "Readable label")],
        ..Default::default()
    };
    let (issues, _) = run(&mut disks, &provider);
    let p = &disks[0].partitions[0];
    assert!(p.free.is_none());
    assert_eq!(p.label.as_deref(), Some("Readable label"));
    assert!(issues.iter().any(|s| s.contains("free space")));
}

#[test]
fn physical_query_failure_does_not_erase_os_size_partition_style_or_nvme_health() {
    let mut d = disk(0);
    d.nvme = Some(NvmeHealth {
        used: 4,
        ..Default::default()
    });
    let mut disks = vec![d];
    let provider = Provider {
        disks: vec![disk_row(0, "uid", 1 << 40)],
        fail: Some("MSFT_PhysicalDisk"),
        ..Default::default()
    };
    let (issues, _) = run(&mut disks, &provider);
    assert_eq!(disks[0].bytes, Some(1 << 40));
    assert_eq!(disks[0].partition_style, Some(2));
    assert_eq!(disks[0].nvme.as_ref().unwrap().used, 4);
    assert!(issues.iter().any(|s| s.contains("MSFT_PhysicalDisk")));
}

#[test]
fn each_provider_failure_is_independent_and_reliability_query_never_claims_ata_attributes() {
    for fail in [
        "MSFT_Disk",
        "MSFT_Partition",
        "MSFT_StorageReliabilityCounter",
    ] {
        let mut disks = vec![disk(0)];
        let provider = Provider {
            fail: Some(fail),
            ..Default::default()
        };
        let (issues, reliability) = run(&mut disks, &provider);
        if fail == "MSFT_StorageReliabilityCounter" {
            assert!(
                matches!(reliability, Value::Unavailable(ref reason) if reason.contains("fixture access denied"))
            );
        } else {
            assert!(issues.iter().any(|s| s.contains(fail)));
        }
    }
    let (_, reliability) = run(&mut [], &Provider::default());
    assert!(
        matches!(reliability, Value::Unavailable(ref reason) if reason.contains("does not send"))
    );
}
