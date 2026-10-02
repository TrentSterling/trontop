//! Cross-consumer regressions: resolver output must survive Copy/JSON without
//! losing freshness, units, device identity or the explicit private-value gate.
use super::*;
use crate::diagnostics::{Issue, Provider as HealthProvider, State};
use crate::model::SystemSnapshot;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn report_for(key: &LiveKey) -> Snapshot {
    let mut specs = Snapshot::default();
    specs.entries[0].section = Some(Arc::new(
        Section::new(specs.entries[0].id)
            .summary_line(SummaryLine::known("Fixture device").live(key.clone()))
            .group(Group::new("Fixture device").row(Row::live("Reading", key.clone()))),
    ));
    specs
}

fn assert_report_value(
    key: &LiveKey,
    snapshot: &SystemSnapshot,
    bridge: &BridgeReadings,
    now: Instant,
    expected: &str,
) {
    let specs = report_for(key);
    let source = Some((snapshot, bridge, now));
    let resolved = resolve(key, snapshot, bridge, now);
    assert_eq!(resolved.value.text(), Some(expected), "{key:?}");
    let plain = text(&specs, source, false);
    assert!(plain.contains(&format!("Reading: {expected}")), "{plain}");
    let json = json(&specs, source, false);
    let section = &json["sections"][0];
    assert_eq!(section["summary"][0]["live"]["value"], expected);
    assert_eq!(section["groups"][0]["items"][0]["live"]["value"], expected);
    assert_eq!(
        section["summary"][0]["live"],
        section["groups"][0]["items"][0]["live"]
    );
}

#[test]
fn private_summary_gates_live_key_value_source_and_unavailable_reason_in_every_report() {
    let now = Instant::now();
    let snapshot = SystemSnapshot::default();
    let key = LiveKey::Sensor {
        id: "FIXTURE-PRIVATE-ID".into(),
    };
    let mut specs = report_for(&key);
    let section = Arc::make_mut(specs.entries[0].section.as_mut().unwrap());
    section.summary[0].text = Value::known("FIXTURE-PRIVATE-HEADLINE");
    section.summary[0].private = true;
    let Item::Row(row) = &mut section.groups[0].items[0] else {
        unreachable!()
    };
    row.private = true;
    let bridge = BridgeReadings {
        status: Value::known("Fixture provider"),
        collected_at: Some(now),
        retry_after: None,
        readings: vec![BridgeReading {
            key,
            value: 987.25,
            unit: LiveUnit::Watts,
            label: "FIXTURE-PRIVATE-LABEL".into(),
            source: "FIXTURE-PRIVATE-SOURCE".into(),
        }],
    };
    for live in [None, Some((&snapshot, &bridge, now))] {
        let plain = text(&specs, live, false);
        let encoded = json(&specs, live, false);
        assert!(plain.contains("[hidden]"));
        for secret in ["FIXTURE-PRIVATE", "987.2"] {
            assert!(
                !plain.contains(secret),
                "masked text leaked {secret}: {plain}"
            );
            assert!(
                !encoded.to_string().contains(secret),
                "masked JSON leaked {secret}: {encoded}"
            );
        }
        assert_eq!(encoded["sections"][0]["summary"][0]["excluded"], true);
        assert!(encoded["sections"][0]["summary"][0].get("live").is_none());
        assert_eq!(
            encoded["sections"][0]["groups"][0]["items"][0]["live"]["excluded"],
            true
        );
        assert!(text(&specs, live, true).contains("FIXTURE-PRIVATE-HEADLINE"));
        let revealed = json(&specs, live, true);
        assert_eq!(
            revealed["sections"][0]["summary"][0]["live"]["key"],
            "Sensor(FIXTURE-PRIVATE-ID)"
        );
    }
    let mut unavailable = bridge.clone();
    unavailable.status = Value::unavailable("FIXTURE-PRIVATE-FAILURE");
    let live = Some((&snapshot, &unavailable, now));
    assert!(!text(&specs, live, false).contains("FIXTURE-PRIVATE-FAILURE"));
    // bridge_status is public provider health. A private summary must still omit
    // its live payload even when resolving the key would return that reason.
    assert!(
        json(&specs, live, false)["sections"][0]["summary"][0]
            .get("live")
            .is_none()
    );
    assert!(text(&specs, live, true).contains("Unavailable (FIXTURE-PRIVATE-FAILURE)"));
}

#[test]
fn gpu_sensor_age_applies_to_all_metrics_without_fresh_temperature_coloring() {
    let now = Instant::now();
    let mut snapshot = SystemSnapshot::default();
    snapshot.gpu_sensors.attempted_at = Some(now);
    snapshot.gpu_sensors.last_success = Some(now);
    snapshot.gpu_sensors.adapters = vec![crate::gpu_sensors::AdapterSensors {
        name: "Fixture GPU".into(),
        temperature_c: Some(86),
        power_w: Some(120.5),
        graphics_clock_mhz: Some(2400),
        memory_clock_mhz: Some(9000),
        fan_percent: Some(37),
        memory: Some((1 << 30, 8 << 30)),
        ..Default::default()
    }];
    let bridge = BridgeReadings::default();
    for (metric, expected) in [
        (GpuMetric::Temperature, "86 °C"),
        (GpuMetric::Power, "120.5 W"),
        (GpuMetric::CoreClock, "2400 MHz"),
        (GpuMetric::MemoryClock, "9000 MHz"),
        (GpuMetric::FanTarget, "37.0%"),
        (GpuMetric::MemoryUsed, "1.00 GiB of 8.00 GiB"),
    ] {
        let key = LiveKey::Gpu {
            adapter: GpuRef {
                name: " fixture GPU ".into(),
                ordinal: 0,
            },
            metric,
        };
        let boundary = now + Duration::from_secs(3);
        assert_report_value(&key, &snapshot, &bridge, boundary, expected);
        if metric == GpuMetric::Temperature {
            assert_eq!(
                resolve(&key, &snapshot, &bridge, boundary).celsius,
                Some(86.0)
            );
        }
        let expired = boundary + Duration::from_millis(1);
        assert_report_value(
            &key,
            &snapshot,
            &bridge,
            expired,
            &format!("{expected} (cached)"),
        );
        assert_eq!(resolve(&key, &snapshot, &bridge, expired).celsius, None);
        let specs = report_for(&key);
        assert_eq!(
            json(&specs, Some((&snapshot, &bridge, expired)), false)["sections"][0]["summary"][0]["live"]
                ["cached"],
            true
        );
        snapshot.gpu_sensors.using_cached = true;
        assert_report_value(
            &key,
            &snapshot,
            &bridge,
            now,
            &format!("{expected} (cached)"),
        );
        snapshot.gpu_sensors.using_cached = false;
        snapshot.gpu_sensors.last_success = None;
        assert_report_value(
            &key,
            &snapshot,
            &bridge,
            now,
            &format!("{expected} (cached)"),
        );
        snapshot.gpu_sensors.last_success = Some(now);
        assert_report_value(&key, &snapshot, &bridge, now, expected);
    }
}

#[test]
fn retained_cpu_clock_failures_and_expiry_stay_cached_until_recovery() {
    retained_counter_failures_and_expiry(HealthProvider::CpuClock);
}

#[test]
fn retained_commit_failures_and_expiry_stay_cached_until_recovery() {
    retained_counter_failures_and_expiry(HealthProvider::MemoryCounters);
}

fn retained_counter_failures_and_expiry(only: HealthProvider) {
    let now = Instant::now();
    let mut snapshot = SystemSnapshot {
        sequence: 1,
        ..Default::default()
    };
    snapshot.cpu.clocks = Some(crate::cpu_clock::Values {
        average_mhz: 2800.0,
        fastest_mhz: 4200.0,
        slowest_mhz: 1400.0,
        interval_seconds: 1.0,
        processors: vec![
            crate::cpu_clock::Processor {
                group: 0,
                number: 2,
                nominal_mhz: 3000,
                mhz: Some(1400.0),
            },
            crate::cpu_clock::Processor {
                group: 1,
                number: 2,
                nominal_mhz: 3000,
                mhz: Some(4200.0),
            },
        ],
    });
    snapshot.memory_details = Some(crate::memory_metrics::Values {
        commit_bytes: 2 << 30,
        commit_limit_bytes: 8 << 30,
        ..Default::default()
    });
    let bridge = BridgeReadings::default();
    for (provider, issue, keys) in [
        (
            HealthProvider::CpuClock,
            Issue::CpuClock,
            vec![
                (LiveKey::CpuClockAverage, "2800 MHz"),
                (LiveKey::CpuClockFastest, "4200 MHz"),
                (
                    LiveKey::CpuCoreClock {
                        group: 0,
                        number: 2,
                    },
                    "1400 MHz",
                ),
                (
                    LiveKey::CpuCoreClock {
                        group: 1,
                        number: 2,
                    },
                    "4200 MHz",
                ),
            ],
        ),
        (
            HealthProvider::MemoryCounters,
            Issue::MemoryCounters,
            vec![(LiveKey::MemoryCommit, "2.00 GiB of 8.00 GiB")],
        ),
    ]
    .into_iter()
    .filter(|(provider, _, _)| *provider == only)
    {
        for (key, expected) in keys {
            snapshot.diagnostics.get_mut(provider).record(
                now,
                Duration::ZERO,
                State::Live,
                None,
                None,
            );
            assert_report_value(
                &key,
                &snapshot,
                &bridge,
                now + Duration::from_secs(3),
                expected,
            );
            assert_report_value(
                &key,
                &snapshot,
                &bridge,
                now + Duration::from_millis(3001),
                &format!("{expected} (cached)"),
            );
            snapshot.diagnostics.get_mut(provider).record(
                now + Duration::from_secs(1),
                Duration::ZERO,
                State::Unavailable,
                None,
                Some(issue),
            );
            assert_report_value(
                &key,
                &snapshot,
                &bridge,
                now + Duration::from_secs(1),
                &format!("{expected} (cached)"),
            );
            snapshot.diagnostics.get_mut(provider).record(
                now + Duration::from_secs(2),
                Duration::ZERO,
                State::Live,
                None,
                None,
            );
            assert_report_value(
                &key,
                &snapshot,
                &bridge,
                now + Duration::from_secs(2),
                expected,
            );
        }
    }
    let missing_core = LiveKey::CpuCoreClock {
        group: 2,
        number: 2,
    };
    assert_eq!(
        resolve(&missing_core, &snapshot, &bridge, now)
            .value
            .reason(),
        Some("no clock reading for this logical processor")
    );
    assert!(
        !resolve(&missing_core, &SystemSnapshot::default(), &bridge, now)
            .value
            .is_known()
    );
    snapshot.cpu.clocks = None;
    snapshot.memory_details = Some(Default::default());
    for (key, reason) in [
        (
            LiveKey::CpuClockFastest,
            "CPU clock counters are unavailable",
        ),
        (
            LiveKey::MemoryCommit,
            "Windows commit counters are unavailable",
        ),
    ] {
        assert_eq!(
            resolve(&key, &snapshot, &bridge, now).value.reason(),
            Some(reason)
        );
        assert!(
            !resolve(&key, &SystemSnapshot::default(), &bridge, now)
                .value
                .is_known()
        );
    }
}

#[test]
fn bridge_units_keep_stable_ids_sources_and_non_temperature_values_in_reports() {
    let now = Instant::now();
    let snapshot = SystemSnapshot::default();
    let inputs = [
        (LiveUnit::Rpm, 1234.0, "1234 RPM"),
        (LiveUnit::Volts, 1.234, "1.234 V"),
        (LiveUnit::Watts, 42.25, "42.2 W"),
        (LiveUnit::Megahertz, 3200.0, "3200 MHz"),
        (LiveUnit::Percent, 12.6, "13%"),
    ];
    let bridge = BridgeReadings {
        status: Value::known("Fixture bridge"),
        collected_at: Some(now),
        retry_after: None,
        readings: inputs
            .iter()
            .enumerate()
            .map(|(index, (unit, value, _))| BridgeReading {
                key: LiveKey::Sensor {
                    id: format!("fixture-sensor-{index}"),
                },
                label: "Shared label".into(),
                source: "Fixture bridge".into(),
                value: *value,
                unit: *unit,
            })
            .collect(),
    };
    for (index, (_, _, expected)) in inputs.iter().enumerate() {
        let key = LiveKey::Sensor {
            id: format!("fixture-sensor-{index}"),
        };
        assert_report_value(&key, &snapshot, &bridge, now, expected);
        let specs = report_for(&key);
        let encoded = json(&specs, Some((&snapshot, &bridge, now)), false);
        let payload = &encoded["sections"][0]["summary"][0]["live"];
        assert_eq!(payload["source"], "Fixture bridge: Shared label");
        assert!(payload.get("celsius").is_none());
        let probe = json(&specs, None, false);
        assert_eq!(
            probe["sections"][0]["summary"][0]["live"]["key"],
            format!("Sensor(fixture-sensor-{index})")
        );
        assert!(
            probe["sections"][0]["summary"][0]["live"]
                .get("value")
                .is_none()
        );
    }
}

#[test]
fn drive_composite_preference_fallback_and_retention_preserve_private_device_paths() {
    use crate::storage_sensors::{Device, DriveReading, Error, Temperature, Temperatures};
    let now = Instant::now();
    let interface = "PRIVATE-FIXTURE-DRIVE-PATH";
    let key = LiveKey::DriveTemperature {
        interface: interface.to_lowercase(),
    };
    let mut storage = crate::storage_sensors::Snapshot {
        inventory_at: Some(now),
        inventory_error: None,
        drives: vec![DriveReading {
            device: Device {
                id: interface.into(),
                name: "Fixture SSD".into(),
            },
            temperatures: Temperatures {
                sensors: vec![
                    Temperature {
                        index: 7,
                        celsius: Some(70),
                        over_threshold: None,
                        under_threshold: None,
                        event: false,
                    },
                    Temperature {
                        index: 0,
                        celsius: Some(45),
                        over_threshold: None,
                        under_threshold: None,
                        event: false,
                    },
                ],
                ..Default::default()
            },
            last_attempt: Some(now),
            last_success: Some(now),
            query_millis: None,
            error: None,
            present: true,
        }],
    };
    let bridge = BridgeReadings::default();
    let mut snapshot = SystemSnapshot::default();
    for (stage, expected, celsius) in [
        (0, "45 °C", Some(45.0)),
        (1, "70 °C", Some(70.0)),
        (2, "70 °C (cached)", None),
        (3, "70 °C", Some(70.0)),
    ] {
        if stage == 1 {
            storage.drives[0].temperatures.sensors[1].celsius = None;
        }
        if stage == 2 {
            storage.drives[0].error = Some(Error::Timeout);
            storage.drives[0].present = false;
        }
        if stage == 3 {
            storage.drives[0].error = None;
            storage.drives[0].present = true;
            storage.drives[0].last_success = Some(now);
        }
        snapshot.storage_sensors = Arc::new(storage.clone());
        assert_report_value(&key, &snapshot, &bridge, now, expected);
        assert_eq!(resolve(&key, &snapshot, &bridge, now).celsius, celsius);
        let specs = report_for(&key);
        for live in [None, Some((&snapshot, &bridge, now))] {
            assert!(
                !text(&specs, live, true)
                    .to_ascii_uppercase()
                    .contains(interface)
            );
            assert!(
                !json(&specs, live, true)
                    .to_string()
                    .to_ascii_uppercase()
                    .contains(interface)
            );
        }
    }
    storage.drives[0].last_success = None;
    storage.drives[0].error = Some(Error::Windows(5));
    snapshot.storage_sensors = Arc::new(storage.clone());
    assert!(
        resolve(&key, &snapshot, &bridge, now)
            .value
            .reason()
            .unwrap()
            .contains("access denied")
    );
    storage.drives[0].temperatures.sensors.clear();
    storage.drives[0].error = None;
    snapshot.storage_sensors = Arc::new(storage.clone());
    assert_eq!(
        resolve(&key, &snapshot, &bridge, now).value.reason(),
        Some("no temperature reading yet")
    );
    storage.drives.clear();
    snapshot.storage_sensors = Arc::new(storage);
    assert_eq!(
        resolve(&key, &snapshot, &bridge, now).value.reason(),
        Some("Windows reports no temperature interface for this drive")
    );
}

#[test]
fn network_alias_lookup_and_missing_counters_do_not_borrow_another_interface() {
    let now = Instant::now();
    let snapshot = SystemSnapshot {
        sequence: 1,
        networks: vec![crate::model::NetworkRow {
            name: "Fixture Ethernet".into(),
            received_bytes_per_sec: 2048.0,
            transmitted_bytes_per_sec: 1024.0,
            total_received_bytes: 100_000,
            total_transmitted_bytes: 50_000,
        }],
        ..Default::default()
    };
    let bridge = BridgeReadings::default();
    let key = LiveKey::NetworkThroughput {
        interface: "Fixture Ethernet".into(),
    };
    assert_report_value(&key, &snapshot, &bridge, now, "Rx 2.00 KB/s / Tx 1.00 KB/s");
    let wrong_alias = LiveKey::NetworkThroughput {
        interface: "fixture ethernet".into(),
    };
    assert_eq!(
        resolve(&wrong_alias, &snapshot, &bridge, now)
            .value
            .reason(),
        Some("interface not present in network counters")
    );
    let encoded = json(
        &report_for(&wrong_alias),
        Some((&snapshot, &bridge, now)),
        false,
    );
    assert!(encoded["sections"][0]["summary"][0]["live"]["value"].is_null());
    assert_eq!(
        encoded["sections"][0]["summary"][0]["live"]["unavailable"],
        "interface not present in network counters"
    );
}
