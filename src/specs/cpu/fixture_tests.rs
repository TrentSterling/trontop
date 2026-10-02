//! Independently supplied hardware facts, resolved through the production
//! section builder and reports. No native reads or invented live temperatures.
use super::*;

fn report(facts: Facts) -> String {
    crate::specs::probe_text(&[build(facts)])
}

fn clocks(values: &[u32]) -> Result<NominalClocks, String> {
    Ok(values
        .iter()
        .enumerate()
        .map(|(number, mhz)| ((0, number as u32), *mhz))
        .collect())
}

#[test]
fn absent_cpuid_preserves_windows_flags_without_claiming_amd_or_no_hypervisor() {
    let section = build(Facts {
        firmware_flag: Some(true),
        slat_flag: Some(false),
        ..Default::default()
    });
    let group = section
        .groups
        .iter()
        .find(|g| g.title == "Virtualization")
        .unwrap();
    for label in ["Hardware virtualization", "Hypervisor"] {
        let row = group
            .items
            .iter()
            .filter_map(|i| match i {
                crate::specs::Item::Row(row) => Some(row),
                _ => None,
            })
            .find(|r| r.label == label)
            .unwrap();
        assert!(
            row.value.reason().is_some_and(|r| r.contains("CPUID")),
            "{label}: {:?}",
            row.value
        );
    }
    let text = crate::specs::probe_text(&[section]);
    assert!(text.contains("Enabled in firmware: Yes"));
    assert!(text.contains("Second level address translation: No"));
    assert!(!text.contains("AMD-V") && !text.contains("no hypervisor is running"));
    assert!(text.contains("Name: Unavailable") && text.contains("Cores: Unavailable"));
}

#[test]
fn unknown_x86_vendor_is_not_labeled_amd_by_virtualization_or_instruction_sets() {
    let text = report(Facts {
        cpuid: Some(Cpuid {
            vendor: "FixtureVendor".into(),
            extended1: (0, 1 << 29),
            leaf1: (1 << 5, 0),
            ..Default::default()
        }),
        ..Default::default()
    });
    assert!(text.contains("Vendor: FixtureVendor"));
    assert!(text.contains("Instruction sets: x86-64"));
    assert!(text.contains("Hardware virtualization: VMX/SVM: supported"));
    assert!(!text.contains("AMD64") && !text.contains("AMD-V"));
    assert!(text.contains("Codename: Unavailable") && text.contains("Socket: Unavailable"));
}

#[test]
fn missing_leaf16_bus_uses_smbios_even_when_rated_frequencies_exist() {
    for (frequency, expected) in [
        (Some((3600, 5200, 0)), "100 MHz"),
        (Some((3600, 5200, 125)), "125 MHz"),
        (None, "100 MHz"),
    ] {
        let text = report(Facts {
            cpuid: Some(Cpuid {
                vendor: "GenuineIntel".into(),
                frequency,
                ..Default::default()
            }),
            socket: Some((Some("Fixture socket".into()), Some(100))),
            ..Default::default()
        });
        assert!(
            text.contains(&format!("Bus (reference) clock: {expected}")),
            "{text}"
        );
        if frequency.is_some() {
            assert!(
                text.contains("Maximum (rated): 5200 MHz")
                    && text.contains("Base (rated): 3600 MHz")
            );
        }
    }
}

#[test]
fn zero_nominal_values_are_missing_per_core_type_instead_of_zero_mhz() {
    let topology = Topology {
        packages: 1,
        cores: vec![
            Core {
                efficiency: 2,
                smt: false,
                processors: vec![(0, 0)],
            },
            Core {
                efficiency: 0,
                smt: false,
                processors: vec![(0, 1)],
            },
        ],
        caches: Vec::new(),
    };
    let text = report(Facts {
        topology: Ok(topology),
        nominal_mhz: clocks(&[0, 3000]),
        ..Default::default()
    });
    assert!(
        text.contains("Base clock (Performance): Unavailable"),
        "{text}"
    );
    assert!(text.contains("Base clock (Efficient): 3000 MHz"), "{text}");
    assert!(!text.contains(": 0 MHz") && !text.contains("0 to 3000"));
    let text = report(Facts {
        nominal_mhz: clocks(&[0, 0]),
        ..Default::default()
    });
    assert!(text.contains("Base clock: Unavailable"), "{text}");
}

#[test]
fn nonhybrid_smt_multi_group_and_middle_efficiency_cache_facts_survive_building() {
    let homogeneous = Topology {
        packages: 2,
        cores: vec![
            Core {
                efficiency: 0,
                smt: true,
                processors: vec![(0, 0), (0, 1)],
            },
            Core {
                efficiency: 0,
                smt: true,
                processors: vec![(1, 0), (1, 1)],
            },
        ],
        caches: vec![
            Cache {
                level: 1,
                kind: 1,
                bytes: 64 * 1024,
                line: 64,
                ways: 0xff,
                processors: vec![(0, 0), (0, 1)],
            },
            Cache {
                level: 1,
                kind: 3,
                bytes: 32 * 1024,
                line: 64,
                ways: 0,
                processors: vec![(1, 0), (1, 1)],
            },
        ],
    };
    let text = report(Facts {
        topology: Ok(homogeneous),
        nominal_mhz: clocks(&[2500, 2700]),
        ..Default::default()
    });
    for expected in [
        "Packages: 2",
        "Cores: 2",
        "Threads: 4 (simultaneous multithreading on)",
        "Hybrid architecture: No",
        "Base clock: 2700 MHz",
        "L1 instruction: 64 KB",
        "L1 trace: 32 KB",
        "[live CpuCoreClock(1:0)]",
        "[live CpuCoreClock(1:1)]",
    ] {
        assert!(text.contains(expected), "{expected}: {text}");
    }
    let topology = Topology {
        packages: 1,
        cores: vec![
            Core {
                efficiency: 9,
                processors: vec![(0, 0)],
                ..Default::default()
            },
            Core {
                efficiency: 4,
                processors: vec![(0, 1)],
                ..Default::default()
            },
            Core {
                efficiency: 0,
                processors: vec![(0, 2)],
                ..Default::default()
            },
        ],
        caches: vec![Cache {
            level: 2,
            kind: 0,
            bytes: 2 * 1024 * 1024,
            line: 128,
            ways: 4,
            processors: vec![(0, 1)],
        }],
    };
    let text = report(Facts {
        topology: Ok(topology),
        nominal_mhz: clocks(&[3500, 2500, 1500]),
        ..Default::default()
    });
    assert!(text.contains("3 (1 Performance + 1 Efficiency class 4 + 1 Efficient)"));
    assert!(text.contains("Base clock (Efficiency class 4): 2500 MHz"));
    assert!(text.contains("L2: 2 MB (Efficiency class 4)"));
}

#[test]
fn unknown_model_suffix_does_not_infer_a_desktop_socket_or_mobile_codename() {
    for suffix in ["XYZ", "G0", "G8", "G9"] {
        let text = report(Facts {
            cpuid: Some(Cpuid {
                vendor: "GenuineIntel".into(),
                brand: Some(format!("Intel Core Ultra 9 285{suffix}")),
                signature: 0x000c_0662,
                ..Default::default()
            }),
            ..Default::default()
        });
        assert!(text.contains("Codename: Arrow Lake\n"), "{text}");
        assert!(text.contains("Socket: Unavailable"), "{text}");
    }
}

#[test]
fn nominal_reader_deduplicates_identities_preserves_partial_groups_and_rejects_zero() {
    let mut calls = Vec::new();
    let (values, issues) = collect_nominal_mhz(
        [(1, 0), (0, 0), (1, 0), (1, 1), (0, 1)],
        || false,
        |group, number| {
            calls.push((group, number));
            match (group, number) {
                (1, 0) => Ok(4200),
                (0, 0) => Ok(1800),
                (0, 1) => Ok(0),
                _ => Err("Fixture failure".into()),
            }
        },
    );
    assert_eq!(calls, [(1, 0), (0, 0), (1, 1), (0, 1)]);
    assert_eq!(
        values.as_ref().unwrap(),
        &[((1, 0), 4200), ((0, 0), 1800)].into_iter().collect()
    );
    assert_eq!(
        issues,
        ["Nominal CPU clocks unavailable for 2 logical processors."]
    );
    let topology = Topology {
        packages: 1,
        cores: vec![
            Core {
                efficiency: 2,
                processors: vec![(1, 0), (1, 1)],
                smt: true,
            },
            Core {
                efficiency: 0,
                processors: vec![(0, 0), (0, 1)],
                smt: true,
            },
        ],
        caches: Vec::new(),
    };
    let text = report(Facts {
        topology: Ok(topology),
        nominal_mhz: values,
        issues,
        ..Default::default()
    });
    for expected in [
        "Base clock (Performance): 4200 MHz (partial)",
        "Base clock (Efficient): 1800 MHz (partial)",
        "[live CpuCoreClock(1:0)]",
        "[live CpuCoreClock(0:0)]",
        "Nominal CPU clocks unavailable for 2",
    ] {
        assert!(text.contains(expected), "{text}");
    }
}

#[test]
fn nominal_reader_budget_expiry_does_not_read_more_processors_or_discard_success() {
    let calls = std::cell::Cell::new(0);
    let (values, issues) = collect_nominal_mhz(
        [(0, 0), (1, 0), (1, 1)],
        || calls.get() == 1,
        |_, _| {
            calls.set(calls.get() + 1);
            Ok(3000)
        },
    );
    assert_eq!(values.unwrap(), [((0, 0), 3000)].into_iter().collect());
    assert!(issues.iter().any(|s| s.contains("budget exhausted")));
    let (values, issues) = collect_nominal_mhz(
        [(0, 0)],
        || true,
        |_, _| panic!("stopped reader queried native frequency"),
    );
    assert!(values.is_err());
    assert!(issues.iter().any(|s| s.contains("budget exhausted")));
    let (values, issues) = collect_nominal_mhz(
        [(0, 0), (1, 0), (2, 0)],
        || false,
        |group, _| {
            if group == 0 {
                Ok(0)
            } else if group == 1 {
                Ok(100_001)
            } else {
                Err("Fixture failure".into())
            }
        },
    );
    assert!(values.is_err());
    assert_eq!(
        issues,
        ["Nominal CPU clocks unavailable for 3 logical processors."]
    );
}

#[test]
fn missing_package_records_do_not_invent_one_socket_when_cores_are_readable() {
    let topology = Topology {
        packages: 0,
        cores: vec![Core {
            processors: vec![(1, 3)],
            ..Default::default()
        }],
        caches: Vec::new(),
    };
    let text = report(Facts {
        topology: Ok(topology),
        ..Default::default()
    });
    assert!(text.contains("Packages: Unavailable (Windows did not report processor packages)"));
    assert!(text.contains("Cores: 1") && text.contains("Threads: 1 (one per core)"));
    assert!(text.contains("[live CpuCoreClock(1:3)]"));
}
