use super::*;
use std::sync::atomic::AtomicU64;

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::current_dir()
            .unwrap()
            .join("target/failure-tests");
        fs::create_dir_all(&root).unwrap();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = root.join(format!(
            "{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn log(&self) -> PathBuf {
        self.0.join(LOG_NAME)
    }

    fn records(&self) -> Vec<serde_json::Value> {
        let bytes = fs::read_to_string(self.log()).unwrap();
        assert!(bytes.len() <= MAX_FILE_BYTES);
        for line in bytes.lines() {
            assert!(serde_json::from_str::<serde_json::Value>(line).is_ok());
        }
        let body = bytes.strip_prefix(HEADER).unwrap();
        assert!(body.ends_with('\n'));
        let records = body
            .lines()
            .map(|line| {
                assert!(line.len() < MAX_RECORD_BYTES);
                serde_json::from_str::<serde_json::Value>(line).unwrap()
            })
            .collect::<Vec<_>>();
        assert!(records.len() <= MAX_RECORDS);
        records
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let root = std::env::current_dir()
            .unwrap()
            .join("target/failure-tests");
        assert!(self.0.starts_with(&root) && self.0 != root);
        let _ = fs::remove_dir_all(&self.0); // Only this exclusively created fixture.
    }
}

fn record(at: u64) -> Vec<u8> {
    encode(
        Kind::RustPanic,
        Some(("src/app.rs", 123, 4)),
        Some("main"),
        Some(at),
    )
    .unwrap()
}

#[test]
fn gpu_failure_events_are_closed_categories_without_driver_messages() {
    for (kind, label) in [
        (Kind::GpuDeviceLost, "gpu_device_lost"),
        (Kind::GpuUploadFailed, "gpu_upload_failed"),
        (Kind::GpuRecoveryStarted, "gpu_recovery_started"),
        (Kind::GpuRecovered, "gpu_recovered"),
        (Kind::GpuRecoveryFailed, "gpu_recovery_failed"),
    ] {
        let bytes = encode(kind, None, Some("trontop-gpu-recovery"), Some(123)).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["kind"], label);
        assert_eq!(value["thread_role"], "gpu_recovery");
        assert!(value["source_file"].is_null());
        assert!(value.get("message").is_none());
        assert!(value.get("payload").is_none());
    }
}

#[test]
fn failure_record_has_fixed_fields_and_excludes_paths_thread_payloads_and_unknown_time() {
    let bytes = encode(
        Kind::RustPanic,
        Some(("C:\\PRIVATE-USER\\PRIVATE-FOLDER\\worker\nfile.rs", 42, 9)),
        Some("PRIVATE-PROCESS-COMMAND"),
        None,
    )
    .unwrap();
    let text = std::str::from_utf8(&bytes).unwrap();
    assert!(!text.contains("PRIVATE"));
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let keys = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    assert_eq!(
        keys,
        [
            "build",
            "kind",
            "profile",
            "schema",
            "source_column",
            "source_file",
            "source_line",
            "target",
            "thread_role",
            "utc_unix_ms",
            "version"
        ]
    );
    assert_eq!(value["source_file"], "worker_file.rs");
    assert_eq!(value["source_line"], 42);
    assert_eq!(value["thread_role"], "other");
    assert!(value["utc_unix_ms"].is_null());
    assert!(
        encode(
            Kind::Graphics,
            Some((&"a".repeat(10_000), 1, 2)),
            None,
            None
        )
        .unwrap()
        .len()
            < MAX_RECORD_BYTES
    );
}

#[test]
fn rolling_log_retains_latest_32_records_without_touching_neighboring_files() {
    let fixture = Fixture::new();
    let sentinel = fixture.0.join("my-notes.txt");
    fs::write(&sentinel, "KEEP THIS").unwrap();
    for at in 0..100 {
        append(&fixture.log(), &record(at)).unwrap();
    }
    let records = fixture.records();
    assert_eq!(records.len(), MAX_RECORDS);
    assert_eq!(records[0]["utc_unix_ms"], 68);
    assert_eq!(records[31]["utc_unix_ms"], 99);
    assert_eq!(fs::read_to_string(sentinel).unwrap(), "KEEP THIS");
    assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 2);
}

#[test]
fn unknown_oversized_or_corrupt_logs_and_invalid_destinations_are_preserved() {
    let fixture = Fixture::new();
    for content in [
        b"unrelated user content".to_vec(),
        vec![b'a'; MAX_FILE_BYTES + 1],
        format!("{HEADER}not valid json\n").into_bytes(),
        format!("{HEADER}{{\"schema\":\"different-app\"}}\n").into_bytes(),
    ] {
        fs::write(fixture.log(), &content).unwrap();
        assert!(append(&fixture.log(), &record(1)).is_err());
        assert_eq!(fs::read(fixture.log()).unwrap(), content);
    }
    assert!(append(Path::new("relative-failure.log"), &record(1)).is_err());
    assert!(append(&fixture.0, &record(1)).is_err());
    #[cfg(windows)]
    {
        let original = format!("{HEADER}{}", std::str::from_utf8(&record(1)).unwrap());
        fs::write(fixture.log(), &original).unwrap();
        let original_permissions = fs::metadata(fixture.log()).unwrap().permissions();
        let mut readonly = original_permissions.clone();
        readonly.set_readonly(true);
        fs::set_permissions(fixture.log(), readonly).unwrap();
        let refused = append(&fixture.log(), &record(2));
        fs::set_permissions(fixture.log(), original_permissions).unwrap();
        assert!(refused.is_err());
        assert_eq!(fs::read_to_string(fixture.log()).unwrap(), original);
        assert!(!local_absolute(Path::new("\\\\server\\share\\failure.log")));
        assert!(!local_absolute(Path::new(
            "\\\\?\\UNC\\server\\share\\failure.log"
        )));
        assert!(local_absolute(Path::new("C:\\Local\\failure.log")));
        assert!(local_absolute(Path::new("\\\\?\\C:\\Local\\failure.log")));
    }
}

#[test]
fn interrupted_final_line_recovers_complete_records_only() {
    let fixture = Fixture::new();
    append(&fixture.log(), &record(1)).unwrap();
    let mut file = OpenOptions::new().append(true).open(fixture.log()).unwrap();
    file.write_all(b"{\"schema\":\"trontop-failure-v1\",\"incomplete")
        .unwrap();
    drop(file);
    append(&fixture.log(), &record(2)).unwrap();
    let records = fixture.records();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0]["utc_unix_ms"], 1);
    assert_eq!(records[1]["utc_unix_ms"], 2);
}

#[test]
fn file_lock_contention_skips_immediately_and_recovers_without_truncation() {
    let fixture = Fixture::new();
    append(&fixture.log(), &record(1)).unwrap();
    let before = fs::read(fixture.log()).unwrap();
    let held = open_log(&fixture.log()).unwrap();
    held.try_lock().unwrap();
    let start = std::time::Instant::now();
    assert!(append(&fixture.log(), &record(2)).is_err());
    assert!(start.elapsed() < std::time::Duration::from_secs(1));
    drop(held);
    assert_eq!(fs::read(fixture.log()).unwrap(), before);
    append(&fixture.log(), &record(3)).unwrap();
    assert_eq!(fixture.records().len(), 2);
}

#[test]
fn concurrent_recorders_leave_a_bounded_parseable_log_and_release_locks() {
    let fixture = Fixture::new();
    append(&fixture.log(), &record(0)).unwrap();
    let barrier = Arc::new(std::sync::Barrier::new(8));
    let threads = (0..8)
        .map(|_| {
            let path = fixture.log();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let recorder = Recorder::new(Some(path));
                barrier.wait();
                for _ in 0..30 {
                    recorder.record(Kind::EventLoop, None);
                }
            })
        })
        .collect::<Vec<_>>();
    for thread in threads {
        thread.join().unwrap();
    }
    append(&fixture.log(), &record(999)).unwrap();
    assert_eq!(fixture.records().last().unwrap()["utc_unix_ms"], 999);
}

#[test]
fn healthy_disabled_busy_and_runner_error_paths_do_not_change_original_results() {
    let fixture = Fixture::new();
    let reporter = Recorder::new(Some(fixture.log()));
    crate::record_run_failure(&reporter, &Ok(()));
    assert!(!fixture.log().exists());
    Recorder::new(None).record(Kind::RustPanic, None);
    reporter.writing.store(true, Ordering::Release);
    reporter.record(Kind::RustPanic, None);
    assert!(!fixture.log().exists());
    reporter.writing.store(false, Ordering::Release);
    let result = Err(eframe::Error::AppCreation(Box::new(io::Error::other(
        "PRIVATE-ERROR-PAYLOAD",
    ))));
    crate::record_run_failure(&reporter, &result);
    assert!(
        matches!(result, Err(ref error) if error.to_string().contains("PRIVATE-ERROR-PAYLOAD"))
    );
    let text = fs::read_to_string(fixture.log()).unwrap();
    assert!(!text.contains("PRIVATE"));
    let records = fixture.records();
    assert_eq!(records[0]["kind"], "app_creation");
    assert!(records[0]["source_file"].is_null());
}

#[test]
fn background_records_bound_queue_and_never_wait_for_a_blocked_writer() {
    use std::sync::mpsc;
    use std::time::{Duration, Instant};
    let (entered, waiting) = mpsc::channel();
    let (release, blocked) = mpsc::channel();
    let (written, records) = mpsc::channel();
    let mut first = Some(blocked);
    let background = BackgroundRecorder::with_writer(move |record| {
        if let Some(blocked) = first.take() {
            entered.send(()).unwrap();
            let _ = blocked.recv();
        }
        written.send(record).unwrap();
    });
    assert!(background.record(Kind::GpuDeviceLost));
    waiting.recv_timeout(Duration::from_secs(3)).unwrap();
    for _ in 0..MAX_PENDING {
        assert!(background.record(Kind::GpuRecoveryStarted));
    }
    let start = Instant::now();
    for _ in 0..1000 {
        assert!(!background.record(Kind::GpuRecoveryFailed));
    }
    // The actual renderer callback must update UI recovery state even when its
    // diagnostic queue is saturated and the writer remains blocked.
    let signal = AtomicBool::new(true);
    crate::handle_renderer_event(
        &signal,
        &background,
        eframe::egui_wgpu::RendererEvent::Recovered,
    );
    assert!(!signal.load(Ordering::Acquire));
    crate::handle_renderer_event(
        &signal,
        &background,
        eframe::egui_wgpu::RendererEvent::DeviceLost,
    );
    assert!(signal.load(Ordering::Acquire));
    let enqueue_time = start.elapsed();
    let start = Instant::now();
    drop(background);
    let drop_time = start.elapsed();
    release.send(()).unwrap();
    for index in 0..=MAX_PENDING {
        let bytes = records.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(bytes.len() <= MAX_RECORD_BYTES);
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            value["kind"],
            if index == 0 {
                "gpu_device_lost"
            } else {
                "gpu_recovery_started"
            }
        );
    }
    // The owned worker is finished, rather than abandoned in the suite.
    assert!(matches!(
        records.recv_timeout(Duration::from_secs(3)),
        Err(mpsc::RecvTimeoutError::Disconnected)
    ));
    assert!(enqueue_time < Duration::from_secs(1), "{enqueue_time:?}");
    assert!(drop_time < Duration::from_millis(200), "{drop_time:?}");
    println!(
        "Background failure log: 1000 saturated attempts {enqueue_time:?}, drop {drop_time:?}; {} buffered records drained",
        MAX_PENDING + 1
    );
}

#[test]
fn background_records_keep_caller_role_time_and_existing_private_log_schema() {
    use std::time::Duration;
    let fixture = Fixture::new();
    let recorder = Arc::new(Recorder::new(Some(fixture.log())));
    let (written, records) = std::sync::mpsc::channel();
    let background = BackgroundRecorder::with_writer(move |record| {
        recorder.append_record(&record);
        written.send(record).unwrap();
    });
    let caller = std::thread::Builder::new()
        .name("trontop-gpu-recovery".into())
        .spawn(move || {
            let before = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis();
            assert!(background.record(Kind::GpuRecovered));
            let after = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis();
            (before, after)
        })
        .unwrap();
    let (before, after) = caller.join().unwrap();
    let bytes = records.recv_timeout(Duration::from_secs(3)).unwrap();
    assert!(matches!(
        records.recv_timeout(Duration::from_secs(3)),
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected)
    ));
    let record: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(record["thread_role"], "gpu_recovery");
    assert_eq!(record["kind"], "gpu_recovered");
    assert!((before..=after).contains(&(record["utc_unix_ms"].as_u64().unwrap() as u128)));
    for key in ["source_file", "source_line", "source_column"] {
        assert!(record[key].is_null());
    }
    assert_eq!(fixture.records(), vec![record]);
}

#[test]
fn background_disabled_disconnected_and_healthy_paths_do_not_create_files() {
    let fixture = Fixture::new();
    let background = BackgroundRecorder::new(Arc::new(Recorder::new(Some(fixture.log()))));
    assert!(!fixture.log().exists());
    drop(background); // Healthy path has no disk operation.
    assert!(!BackgroundRecorder::new(Arc::new(Recorder::new(None))).record(Kind::GpuRecovered));
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    drop(receiver);
    assert!(
        !BackgroundRecorder {
            sender: Some(sender)
        }
        .record(Kind::GpuDeviceLost)
    );
    assert!(!fixture.log().exists());
}

// This exact test is also the hidden child entry point. Never creates a native
// app, sampler, tray or window and never installs a hook into the parent test suite.
#[test]
fn panic_hook_child() {
    let Some(path) = std::env::var_os("TRONTOP_FAILURE_TEST_CHILD") else {
        return;
    };
    let path = PathBuf::from(path);
    let root = std::env::current_dir()
        .unwrap()
        .join("target/failure-tests");
    assert!(path.starts_with(&root) && path.file_name().unwrap() == LOG_NAME);
    if std::env::var_os("TRONTOP_FAILURE_TEST_ABORT").is_some() {
        #[cfg(windows)]
        unsafe {
            #[link(name = "kernel32")]
            unsafe extern "system" {
                fn SetErrorMode(mode: u32) -> u32;
            }
            SetErrorMode(0x0001 | 0x0002); // Only this disposable child's error UI.
        }
        std::panic::set_hook(Box::new(|_| std::process::abort()));
    }
    Arc::new(Recorder::new(Some(path))).install();
    panic!("PRIVATE-PANIC-PAYLOAD C:/PRIVATE-ACCOUNT/PRIVATE-FILE");
}

#[test]
fn real_panic_hook_writes_before_unwind_and_abort_in_owned_hidden_children() {
    for abort in [false, true] {
        let fixture = Fixture::new();
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", "failure::tests::panic_hook_child", "--nocapture"])
            .env("TRONTOP_FAILURE_TEST_CHILD", fixture.log())
            .env_remove("TRONTOP_FAILURE_TEST_ABORT")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        if abort {
            command.env("TRONTOP_FAILURE_TEST_ABORT", "1");
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let mut child = command.spawn().unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if std::time::Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("Owned failure probe did not exit within ten seconds");
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        assert!(!status.success());
        let records = fixture.records();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["kind"], "rust_panic");
        assert_eq!(records[0]["source_file"], "tests.rs");
        assert!(records[0]["source_line"].as_u64().unwrap() > 0);
        let text = fs::read_to_string(fixture.log()).unwrap();
        assert!(!text.contains("PRIVATE"));
    }
}
