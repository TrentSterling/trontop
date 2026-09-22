use super::*;
use crate::{
    diagnostics::{Provider, State},
    gpu_activity::Usage,
    model::ProcessRow,
};
use std::io::Write;
use std::time::Duration;

fn capture(private_details: bool, format: Format) -> Capture {
    let mut snapshot = SystemSnapshot {
        sequence: 41,
        host_name: "PRIVATE-HOST".into(),
        ..Default::default()
    };
    snapshot.processes = [
        Usage::Measured(0.0),
        Usage::Partial(2.5),
        Usage::Unavailable,
        Usage::Warming,
        Usage::Unreported,
        Usage::Measured(f32::NAN),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, gpu_percent)| ProcessRow {
        pid: index as u32 + 900_000,
        name: "测试,\"App\"\r\n.exe".into(),
        user: "PRIVATE-ACCOUNT".into(),
        command: "PRIVATE-COMMAND --token=secret".into(),
        executable: Some("C:/PRIVATE-PATH/app.exe".into()),
        cwd: Some("C:/PRIVATE-CWD".into()),
        gpu_percent,
        memory_bytes: u64::MAX,
        ..Default::default()
    })
    .collect();
    let at = Instant::now() - Duration::from_secs(20);
    snapshot.diagnostics.get_mut(Provider::System).record(
        at,
        Duration::ZERO,
        State::Live,
        None,
        None,
    );
    snapshot.diagnostics.get_mut(Provider::GpuActivity).record(
        at,
        Duration::ZERO,
        State::Live,
        None,
        None,
    );
    Capture::new(
        snapshot,
        Options {
            format,
            private_details,
        },
    )
}
fn encoded(capture: &Capture) -> Vec<u8> {
    let mut bytes = Vec::new();
    encode::write(&mut bytes, capture, &AtomicBool::new(false)).unwrap();
    bytes
}

#[test]
fn adapter_export_keeps_source_scope_missing_and_retained_memory() {
    let mut c = capture(false, Format::Json);
    let mut adapter = crate::gpu_adapters::Adapter::default();
    adapter.memory[0].record(Some(9_123_456_789), c.at - Duration::from_secs(1));
    adapter.memory[0].record(None, c.at);
    adapter.memory[1].record(Some(0), c.at);
    c.snapshot.gpu.adapters.push(adapter);
    let data: serde_json::Value = serde_json::from_slice(&encoded(&c)).unwrap();
    let a = &data["gpu_adapters"][0];
    assert_eq!(a["memory"][0]["bytes"], 9_123_456_789_u64);
    assert_eq!(a["memory"][0]["state"], "Cached");
    assert_eq!(a["memory"][1]["bytes"], 0);
    assert!(a["memory"][2]["bytes"].is_null());
    assert!(a["dedicated_video_capacity_bytes"].is_null());
    assert!(
        a["memory_source"]
            .as_str()
            .unwrap()
            .contains("whole physical adapter")
    );
}

#[test]
fn cpu_clock_export_keeps_nulls_interval_group_identity_and_source() {
    let mut c = capture(false, Format::Json);
    c.snapshot.cpu.frequency_mhz = 3700;
    let missing: serde_json::Value = serde_json::from_slice(&encoded(&c)).unwrap();
    assert!(missing["system"]["cpu"]["clocks"]["average_mhz"].is_null());
    c.snapshot.cpu.clocks = Some(crate::cpu_clock::Values {
        average_mhz: 5125.0,
        fastest_mhz: 5400.0,
        slowest_mhz: 4300.0,
        interval_seconds: 1.2,
        processors: vec![crate::cpu_clock::Processor {
            group: 1,
            number: 2,
            nominal_mhz: 3200,
            mhz: None,
        }],
    });
    let h = c.snapshot.diagnostics.get_mut(Provider::CpuClock);
    h.record(
        c.at - Duration::from_secs(1),
        Duration::ZERO,
        State::Live,
        None,
        None,
    );
    h.record(c.at, Duration::ZERO, State::Unavailable, None, None);
    let doc: serde_json::Value = serde_json::from_slice(&encoded(&c)).unwrap();
    let v = &doc["system"]["cpu"]["clocks"];
    assert_eq!(v["average_mhz"], 5125.0);
    assert_eq!(v["interval_seconds"], 1.2);
    assert_eq!(v["state"], "Stale");
    assert_eq!(v["last_usable_age_seconds"], 1.0);
    assert_eq!(v["processors"][0]["group"], 1);
    assert_eq!(v["processors"][0]["number"], 2);
    assert!(v["processors"][0]["interval_mhz"].is_null());
    assert_eq!(v["source"], crate::cpu_clock::SOURCE);
    assert!(
        doc["system"]["cpu"]["frequency_source"]
            .as_str()
            .unwrap()
            .contains("legacy power clock")
    );
}

#[test]
fn empty_json_is_valid_and_has_no_invented_system_sample() {
    let capture = Capture::new(SystemSnapshot::default(), Options::default());
    let mut bytes = Vec::new();
    encode::write(&mut bytes, &capture, &AtomicBool::new(false)).unwrap();
    let doc: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(doc["system"].is_null());
    assert_eq!(doc["metadata"]["has_sample"], false);
    assert_eq!(
        doc["providers"].as_array().unwrap().len(),
        Provider::ALL.len()
    );
    assert_eq!(doc["processes"].as_array().unwrap().len(), 0);
}

#[test]
fn memory_counters_export_has_exact_bytes_nulls_and_cached_provenance() {
    let mut c = capture(false, Format::Json);
    let missing: serde_json::Value = serde_json::from_slice(&encoded(&c)).unwrap();
    assert!(missing["system"]["memory_counters"]["commit_bytes"].is_null());
    assert!(missing["system"]["swap_used_bytes"].is_null());
    c.snapshot.memory_details = Some(crate::memory_metrics::Values {
        commit_bytes: 24 << 30,
        commit_limit_bytes: 80 << 30,
        physical_total_bytes: 64 << 30,
        ..Default::default()
    });
    let h = c.snapshot.diagnostics.get_mut(Provider::MemoryCounters);
    h.record(
        c.at - Duration::from_secs(2),
        Duration::ZERO,
        State::Live,
        None,
        None,
    );
    h.record(c.at, Duration::ZERO, State::Unavailable, None, None);
    let doc: serde_json::Value = serde_json::from_slice(&encoded(&c)).unwrap();
    let m = &doc["system"]["memory_counters"];
    assert_eq!(m["commit_bytes"].as_u64(), Some(24 << 30));
    assert_eq!(m["commit_limit_bytes"].as_u64(), Some(80 << 30));
    assert_eq!(m["state"], "Stale");
    assert_eq!(m["last_usable_age_seconds"], 2.0);
    assert_eq!(doc["system"]["swap_used_bytes"], 0);
    assert!(
        doc["system"]["swap_semantics"]
            .as_str()
            .unwrap()
            .contains("not page-file occupancy")
    );
}

#[test]
fn physical_disk_export_preserves_cached_missing_zero_and_private_instances() {
    use crate::disk_activity::{Device, Reading, Snapshot};
    let mut c = capture(false, Format::Json);
    let at = c.at;
    c.snapshot.physical_disks = Arc::new(Snapshot {
        at: Some(at),
        generation: 3,
        devices: vec![Device {
            number: 7,
            instance: "7 PRIVATE-MOUNT".into(),
            readings: [
                Reading {
                    value: Some(0.0),
                    at: Some(at),
                },
                Reading {
                    value: Some(4.5),
                    at: Some(at - Duration::from_secs(10)),
                },
                Reading::default(),
                Reading::default(),
                Reading::default(),
            ],
        }],
        ..Default::default()
    });
    let bytes = encoded(&c);
    let doc: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let disk = &doc["physical_disks"][0];
    assert_eq!(disk["disk_number"], 7);
    assert!(disk["instance"].is_null());
    assert_eq!(disk["metrics"]["active_percent"]["value"], 0.0);
    assert_eq!(disk["metrics"]["active_percent"]["state"], "Live");
    assert_eq!(disk["metrics"]["response_millis"]["value"], 4.5);
    assert_eq!(disk["metrics"]["response_millis"]["state"], "Cached");
    assert!(disk["metrics"]["outstanding_requests"]["value"].is_null());
    assert!(!String::from_utf8(bytes).unwrap().contains("PRIVATE-MOUNT"));
    c.options.private_details = true;
    assert!(
        String::from_utf8(encoded(&c))
            .unwrap()
            .contains("PRIVATE-MOUNT")
    );
}

#[test]
fn json_roundtrips_unicode_integers_privacy_and_missing_vs_zero() {
    let mut c = capture(false, Format::Json);
    let bytes = encoded(&c);
    let doc: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(doc["processes"][0]["name"], c.snapshot.processes[0].name);
    assert_eq!(doc["processes"][0]["memory_bytes"].as_u64(), Some(u64::MAX));
    assert_eq!(doc["processes"][0]["gpu"]["percent"], 0.0);
    assert_eq!(doc["processes"][1]["gpu"]["lower_bound_percent"], 2.5);
    assert!(doc["processes"][1]["gpu"]["percent"].is_null());
    for (i, state) in [
        (2, "unavailable"),
        (3, "warming"),
        (4, "unreported"),
        (5, "invalid"),
    ] {
        assert_eq!(doc["processes"][i]["gpu"]["state"], state);
        assert!(doc["processes"][i]["gpu"]["percent"].is_null());
    }
    assert_eq!(doc["providers"][0]["state"], "Stale");
    assert!(doc["system"]["cpu"]["temperature_c"].is_null());
    assert!(!String::from_utf8(bytes).unwrap().contains("PRIVATE-"));
    c.options.private_details = true;
    let text = String::from_utf8(encoded(&c)).unwrap();
    for field in [
        "PRIVATE-HOST",
        "PRIVATE-ACCOUNT",
        "PRIVATE-COMMAND",
        "PRIVATE-PATH",
        "PRIVATE-CWD",
    ] {
        assert!(text.contains(field));
    }
}

#[test]
fn json_preserves_sensor_inventory_states_without_leaking_private_fields_or_raw_errors() {
    use crate::{gpu_sensors::AdapterSensors, model::*, startup, storage_sensors as storage};
    let mut c = capture(false, Format::Json);
    c.snapshot.disks.push(DiskRow {
        name: "Fixture SSD".into(),
        mount: "PRIVATE-MOUNT".into(),
        ..Default::default()
    });
    c.snapshot.networks.push(NetworkRow {
        name: "PRIVATE-ADAPTER".into(),
        ..Default::default()
    });
    c.snapshot.users.push(UserSummary {
        name: "PRIVATE-USER".into(),
        ..Default::default()
    });
    c.snapshot.gpu_sensors.using_cached = true;
    c.snapshot.gpu_sensors.last_success = Some(c.at - Duration::from_secs(60));
    c.snapshot.gpu_sensors.error = Some("RAW-NATIVE-ERROR".into());
    c.snapshot.gpu_sensors.adapters.push(AdapterSensors {
        name: "Fixture GPU".into(),
        uuid: Some("PRIVATE-UUID".into()),
        temperature_c: Some(42),
        power_w: None,
        fan_percent: Some(0),
        error: Some("RAW-NATIVE-ERROR".into()),
        ..Default::default()
    });
    Arc::make_mut(&mut c.snapshot.storage_sensors)
        .drives
        .push(storage::DriveReading {
            device: storage::Device {
                id: "PRIVATE-DRIVE-ID".into(),
                name: "Fixture SSD".into(),
            },
            temperatures: storage::Temperatures {
                warning: Some(70),
                critical: Some(90),
                sensors: vec![
                    storage::Temperature {
                        index: 0,
                        celsius: Some(0),
                        over_threshold: Some(70),
                        under_threshold: None,
                        event: false,
                    },
                    storage::Temperature {
                        index: 1,
                        celsius: None,
                        over_threshold: None,
                        under_threshold: None,
                        event: false,
                    },
                ],
            },
            last_attempt: Some(c.at),
            last_success: Some(c.at - Duration::from_secs(60)),
            query_millis: Some(5000.0),
            error: Some(storage::Error::Timeout),
            present: true,
        });
    let startup = Arc::make_mut(&mut c.snapshot.startup);
    startup.apply(
        vec![startup::Read {
            source: startup::Source::UserRun,
            state: startup::ReadState::Readable,
            rows: vec![StartupRow {
                name: "Fixture startup".into(),
                key: "PRIVATE-KEY".into(),
                command: "PRIVATE-STARTUP-COMMAND".into(),
                source: startup::Source::UserRun,
            }],
        }],
        c.at - Duration::from_secs(60),
    );
    startup.apply(vec![], c.at); // Failed read retains the row, explicitly cached.
    Arc::make_mut(&mut c.snapshot.services).push(ServiceRow {
        name: "FixtureService".into(),
        display_name: "Fixture service".into(),
        ..Default::default()
    });
    let bytes = encoded(&c);
    let text = std::str::from_utf8(&bytes).unwrap();
    assert!(!text.contains("PRIVATE-"));
    assert!(!text.contains("RAW-NATIVE-ERROR"));
    let doc: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(doc["gpu_sensors"][0]["cached"], true);
    assert_eq!(doc["gpu_sensors"][0]["temperature_c"], 42);
    assert!(doc["gpu_sensors"][0]["power_w"].is_null());
    assert_eq!(doc["gpu_sensors"][0]["fan_target_percent"], 0);
    assert_eq!(doc["drive_sensors"][0]["state"], "Cached");
    assert_eq!(doc["drive_sensors"][0]["sensors"][0]["celsius"], 0);
    assert!(doc["drive_sensors"][0]["sensors"][1]["celsius"].is_null());
    assert_eq!(doc["startup"][0]["state"], "Cached");
    assert!(
        doc["services"][0]["source"]
            .as_str()
            .unwrap()
            .contains("command observations not included")
    );
    c.options.private_details = true;
    let text = String::from_utf8(encoded(&c)).unwrap();
    for field in [
        "PRIVATE-MOUNT",
        "PRIVATE-ADAPTER",
        "PRIVATE-USER",
        "PRIVATE-UUID",
        "PRIVATE-DRIVE-ID",
        "PRIVATE-KEY",
        "PRIVATE-STARTUP-COMMAND",
    ] {
        assert!(
            text.contains(field),
            "explicit private export must include {field}"
        );
    }
    assert!(!text.contains("RAW-NATIVE-ERROR"));
}

// Independent strict parser for the encoder's quoted-field CSV contract.
fn csv(bytes: &[u8]) -> Vec<Vec<String>> {
    let text = std::str::from_utf8(bytes)
        .unwrap()
        .strip_prefix('\u{feff}')
        .unwrap();
    let mut chars = text.chars().peekable();
    let mut rows = Vec::new();
    while chars.peek().is_some() {
        let mut row = Vec::new();
        loop {
            assert_eq!(chars.next(), Some('"'));
            let mut field = String::new();
            loop {
                match chars.next().unwrap() {
                    '"' if chars.peek() == Some(&'"') => {
                        chars.next();
                        field.push('"');
                    }
                    '"' => break,
                    c => field.push(c),
                }
            }
            row.push(field);
            match chars.next() {
                Some(',') => {}
                Some('\r') => {
                    assert_eq!(chars.next(), Some('\n'));
                    break;
                }
                other => panic!("invalid CSV separator: {other:?}"),
            }
        }
        rows.push(row);
    }
    rows
}

#[test]
fn csv_has_fixed_columns_unicode_escape_and_spreadsheet_prefix_defense() {
    let mut c = capture(false, Format::Csv);
    let rows = csv(&encoded(&c));
    let column = |name: &str| rows[0].iter().position(|h| h == name).unwrap();
    assert_eq!(rows.len(), 7);
    assert!(rows.iter().all(|row| row.len() == rows[0].len()));
    assert_eq!(rows[1][column("name")], c.snapshot.processes[0].name);
    assert_eq!(rows[1][column("gpu_percent")], "0");
    assert_eq!(rows[2][column("gpu_percent")], "");
    assert_eq!(rows[2][column("gpu_lower_bound_percent")], "2.5");
    assert_eq!(rows[1][column("gpu_provider_state")], "Stale");
    assert!(!rows[0].iter().any(|c| c == "command"));
    assert!(!String::from_utf8(encoded(&c)).unwrap().contains("PRIVATE-"));
    c.options.private_details = true;
    for danger in [
        "=1+2",
        "+cmd",
        "-cmd",
        "@SUM(A1)",
        "  =cmd",
        "\tcmd",
        "\r\n=cmd",
        "\u{feff}=cmd",
        "＝cmd",
    ] {
        c.snapshot.processes[0].command = danger.into();
        let rows = csv(&encoded(&c));
        let index = rows[0].iter().position(|h| h == "command").unwrap();
        assert_eq!(rows[1][index], format!("'{danger}"));
    }
}

#[test]
fn cancelled_and_oversized_encodes_return_errors_instead_of_truncating_successfully() {
    let c = capture(false, Format::Json);
    let mut bytes = Vec::new();
    assert!(encode::write(&mut bytes, &c, &AtomicBool::new(true)).is_err());
    assert!(bytes.is_empty());
    let mut limited = file::Limited {
        inner: Vec::new(),
        written: 0,
        limit: 128,
    };
    assert!(encode::write(&mut limited, &c, &AtomicBool::new(false)).is_err());
    assert!(limited.written <= 128);
}

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!(
                "export-test-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        std::fs::create_dir(&path).unwrap(); // Exclusive ownership of this new fixture directory.
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        for entry in std::fs::read_dir(&self.0).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                std::fs::remove_dir(entry.path()).unwrap();
            } else {
                std::fs::remove_file(entry.path()).unwrap();
            }
        }
        std::fs::remove_dir(&self.0).unwrap();
    }
}

#[test]
fn disk_save_replaces_only_on_success_and_cleans_its_own_temporary_files() {
    let dir = Directory::new();
    let destination = dir.0.join("结果.json");
    let mut original = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&destination)
        .unwrap();
    original.write_all(b"fixture original").unwrap();
    drop(original);
    let c = capture(false, Format::Json);
    assert!(file::save(&destination, &c, &AtomicBool::new(true)).is_err());
    assert_eq!(std::fs::read(&destination).unwrap(), b"fixture original");
    assert!(file::save_limited(&destination, &c, &AtomicBool::new(false), 128).is_err());
    assert_eq!(std::fs::read(&destination).unwrap(), b"fixture original");
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        let locked = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&destination)
            .unwrap();
        assert!(file::save(&destination, &c, &AtomicBool::new(false)).is_err());
        assert_eq!(std::fs::read(&destination).unwrap(), b"fixture original");
        drop(locked);
    }
    let bytes = file::save(&destination, &c, &AtomicBool::new(false)).unwrap();
    assert_eq!(std::fs::read(&destination).unwrap(), encoded(&c));
    assert_eq!(std::fs::metadata(&destination).unwrap().len(), bytes);
    let directory_target = dir.0.join("directory.json");
    std::fs::create_dir(&directory_target).unwrap();
    assert!(file::save(&directory_target, &c, &AtomicBool::new(false)).is_err());
    assert!(directory_target.is_dir());
    assert!(file::save(&dir.0.join("wrong.exe"), &c, &AtomicBool::new(false)).is_err());
    assert!(file::save(&PathBuf::from("relative.json"), &c, &AtomicBool::new(false)).is_err());
    assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 2);
}

#[test]
fn worker_is_single_flight_retains_captured_data_and_drop_does_not_wait() {
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let release_rx = std::sync::Mutex::new(release_rx);
    let (stopped_tx, stopped_rx) = mpsc::channel();
    let mut exporter = Exporter::with_backend(move |capture, stop| {
        entered_tx.send(capture.snapshot.sequence).unwrap();
        release_rx.lock().unwrap().recv().unwrap();
        stopped_tx.send(stop.load(Ordering::Acquire)).unwrap();
        Outcome::Cancelled
    });
    let ctx = eframe::egui::Context::default();
    exporter
        .submit(capture(false, Format::Json), ctx.clone())
        .unwrap();
    assert_eq!(entered_rx.recv_timeout(Duration::from_secs(3)).unwrap(), 41);
    assert!(exporter.submit(capture(false, Format::Csv), ctx).is_err());
    assert!(exporter.poll().is_none());
    let at = Instant::now();
    drop(exporter);
    assert!(at.elapsed() < Duration::from_millis(200));
    release_tx.send(()).unwrap();
    assert!(stopped_rx.recv_timeout(Duration::from_secs(3)).unwrap());
}

#[test]
fn worker_returns_cancel_failure_success_and_allows_subsequent_jobs() {
    let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = Arc::clone(&count);
    let mut exporter =
        Exporter::with_backend(move |c, _| match counter.fetch_add(1, Ordering::Relaxed) {
            0 => Outcome::Cancelled,
            1 => Outcome::Failed("Fixture failure".into()),
            _ => Outcome::Saved {
                path: "fixture.json".into(),
                bytes: 123,
                sequence: c.snapshot.sequence,
            },
        });
    for index in 0..3 {
        exporter
            .submit(
                capture(false, Format::Json),
                eframe::egui::Context::default(),
            )
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        let result = loop {
            if let Some(result) = exporter.poll() {
                break result;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(2));
        };
        assert!(!exporter.busy());
        match (index, result) {
            (0, Outcome::Cancelled)
            | (1, Outcome::Failed(_))
            | (2, Outcome::Saved { sequence: 41, .. }) => {}
            other => panic!("unexpected result: {other:?}"),
        }
    }
    assert_eq!(count.load(Ordering::Relaxed), 3);
}

#[test]
fn system_specs_text_and_json_exclude_private_values_and_drive_paths() {
    use crate::specs::fixtures;
    let specs = |format, private_details| {
        Capture::specs(
            SystemSnapshot::default(),
            fixtures::snapshot(),
            Options {
                format,
                private_details,
            },
        )
    };
    let text = encoded(&specs(Format::SpecsText, false));
    assert!(text.starts_with(b"\xEF\xBB\xBF"), "UTF-8 BOM for Notepad");
    let text = String::from_utf8(text[3..].to_vec()).unwrap();
    assert!(text.contains("\r\n") && !text.replace("\r\n", "").contains('\n'));
    assert!(text.contains("Private values: hidden"));
    assert!(text.contains("Serial number: [hidden]"));
    assert!(text.contains("State: Complete"));
    assert!(!text.contains(fixtures::PRIVATE_SERIAL));
    assert!(!text.contains(fixtures::DRIVE_INTERFACE));
    let json = encoded(&specs(Format::SpecsJson, false));
    let document: serde_json::Value = serde_json::from_slice(&json).unwrap();
    assert_eq!(document["kind"], "trontop_system_specs");
    assert_eq!(document["private_values"], "excluded");
    let raw = String::from_utf8(json).unwrap();
    assert!(!raw.contains(fixtures::PRIVATE_SERIAL));
    assert!(!raw.contains(fixtures::DRIVE_INTERFACE));
    assert!(raw.contains("\"excluded\": true"));
    assert!(raw.contains("\"key\": \"DriveTemperature\""));
    let private = String::from_utf8(encoded(&specs(Format::SpecsJson, true))).unwrap();
    assert!(private.contains(fixtures::PRIVATE_SERIAL));
    assert!(
        !private.contains(fixtures::DRIVE_INTERFACE),
        "interface paths never leave the app"
    );
    let missing = Capture::new(
        SystemSnapshot::default(),
        Options {
            format: Format::SpecsText,
            private_details: false,
        },
    );
    let mut sink = Vec::new();
    assert!(encode::write(&mut sink, &missing, &AtomicBool::new(false)).is_err());
}
