use super::*;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;

pub(crate) struct Fixture(pub PathBuf);
impl Fixture {
    pub(crate) fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::current_dir()
            .unwrap()
            .join("target/preferences-tests");
        fs::create_dir_all(&root).unwrap();
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
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
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let root = std::env::current_dir()
            .unwrap()
            .join("target/preferences-tests");
        assert!(self.0.is_absolute() && self.0.starts_with(&root) && self.0 != root);
        let _ = fs::remove_dir_all(&self.0); // Only the exclusively owned fixture.
    }
}

fn snapshot(seed: u8) -> Snapshot {
    let theme = crate::theme::ThemeSettings {
        accent: [seed, 192, 130],
        ..Default::default()
    }
    .encode();
    let library =
        serde_json::json!({"version":1,"themes":[{"name":"Fixture palette", "theme": theme}]})
            .to_string();
    let mut memory = egui::Memory::default();
    memory.options.zoom_factor = 1.25;
    memory
        .data
        .insert_persisted(egui::Id::new("fixture-position"), 42.5_f32);
    Snapshot {
        theme,
        library,
        memory,
    }
}

#[test]
fn preference_files_migrate_without_changing_legacy_and_restore_theme_library_memory() {
    let fixture = Fixture::new();
    let mut store = file::Store::new(fixture.0.clone());
    let original = snapshot(24);
    let values = BTreeMap::from([
        (
            crate::theme::LEGACY_STORAGE_KEY.to_owned(),
            "1;160,80,220;30,180,210;1;45;0.25;0.8;6".to_owned(),
        ),
        (
            crate::theme_studio::LIBRARY_KEY.to_owned(),
            original.library.clone(),
        ),
        ("egui".to_owned(), ron::to_string(&original.memory).unwrap()),
        ("unknown-preserved-key".to_owned(), "keep me".to_owned()),
    ]);
    let legacy = ron::to_string(&values).unwrap();
    let path = fixture.0.join(LEGACY_NAME);
    fs::write(&path, &legacy).unwrap();
    let loaded = store.load().unwrap();
    assert_eq!(loaded.theme.accent, [160, 80, 220]);
    assert_eq!(loaded.library.as_deref(), Some(original.library.as_str()));
    assert_eq!(loaded.memory.unwrap().options.zoom_factor, 1.25);
    assert!(!fixture.0.join(FILE_NAME).exists()); // Reading never writes migration.
    store
        .save(original.clone(), &AtomicBool::new(false))
        .unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), legacy);
    let mut fresh = file::Store::new(fixture.0.clone());
    let loaded = fresh.load().unwrap();
    assert_eq!(loaded.theme.encode(), original.theme);
    assert_eq!(loaded.library.as_deref(), Some(original.library.as_str()));
    let mut memory = loaded.memory.unwrap();
    assert_eq!(memory.options.zoom_factor, 1.25);
    assert_eq!(
        memory
            .data
            .get_persisted::<f32>(egui::Id::new("fixture-position")),
        Some(42.5)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture.0.join(FILE_NAME)).unwrap()).unwrap();
    assert_eq!(json["values"]["unknown-preserved-key"], "keep me");
    assert!(!fs::read_dir(&fixture.0).unwrap().any(|entry| {
        entry
            .unwrap()
            .path()
            .extension()
            .is_some_and(|ext| ext == "tmp")
    }));
}

#[test]
fn preference_files_preserve_invalid_oversized_newer_readonly_and_conflicting_data() {
    let fixture = Fixture::new();
    let target = fixture.0.join(FILE_NAME);
    for bytes in [
        b"broken json".to_vec(),
        br#"{"format":"trontop-settings","version":999,"values":{}}"#.to_vec(),
        vec![b' '; 2 * 1024 * 1024 + 1],
        br#"{"format":"trontop-settings","version":1,"values":{"trontop.theme-library.v1":"bad"}}"#
            .to_vec(),
    ] {
        fs::write(&target, &bytes).unwrap();
        let mut store = file::Store::new(fixture.0.clone());
        assert!(store.load().is_err());
        assert!(store.save(snapshot(40), &AtomicBool::new(false)).is_err());
        assert_eq!(fs::read(&target).unwrap(), bytes);
    }
    fs::remove_file(&target).unwrap();
    let mut first = file::Store::new(fixture.0.clone());
    let mut second = file::Store::new(fixture.0.clone());
    first.load().unwrap();
    second.load().unwrap();
    first.save(snapshot(10), &AtomicBool::new(false)).unwrap();
    let saved = fs::read(&target).unwrap();
    assert!(
        second
            .save(snapshot(20), &AtomicBool::new(false))
            .unwrap_err()
            .contains("another instance")
    );
    assert_eq!(fs::read(&target).unwrap(), saved);
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(fixture.0.join("settings-v3.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    assert!(
        first
            .save(snapshot(30), &AtomicBool::new(false))
            .unwrap_err()
            .contains("Another Trontop")
    );
    drop(lock);
    assert_eq!(fs::read(&target).unwrap(), saved);
    assert!(first.save(snapshot(30), &AtomicBool::new(true)).is_err());
    assert_eq!(fs::read(&target).unwrap(), saved);
    #[cfg(windows)]
    {
        let original = fs::metadata(&target).unwrap().permissions();
        let mut readonly = original.clone();
        readonly.set_readonly(true);
        fs::set_permissions(&target, readonly).unwrap();
        let result = first.save(snapshot(31), &AtomicBool::new(false));
        fs::set_permissions(&target, original).unwrap();
        assert!(result.is_err());
        assert_eq!(fs::read(&target).unwrap(), saved);
    }
    first.save(snapshot(32), &AtomicBool::new(false)).unwrap();
    assert_eq!(
        file::Store::new(fixture.0.clone())
            .load()
            .unwrap()
            .theme
            .accent[0],
        32
    );
    assert!(!fs::read_dir(&fixture.0).unwrap().any(|entry| {
        entry
            .unwrap()
            .path()
            .extension()
            .is_some_and(|ext| ext == "tmp")
    }));
}

#[test]
fn preference_files_reject_unreadable_output_and_preserve_existing_data() {
    let fixture = Fixture::new();
    let path = fixture.0.join(FILE_NAME);
    let mut store = file::Store::new(fixture.0.clone());
    store.load().unwrap();
    store.save(snapshot(1), &AtomicBool::new(false)).unwrap();
    let original = fs::read(&path).unwrap();
    for scale in [0.0, -1.0, f32::INFINITY, f32::NAN, 11.0] {
        let mut invalid = snapshot(2);
        invalid.memory.options.zoom_factor = scale;
        assert!(store.save(invalid, &AtomicBool::new(false)).is_err());
        assert_eq!(fs::read(&path).unwrap(), original);
    }
    let values: BTreeMap<_, _> = (0..64).map(|i| (format!("preserve-{i}"), "yes")).collect();
    let bytes = serde_json::to_vec(
        &serde_json::json!({"format":"trontop-settings", "version":1,"values":values}),
    )
    .unwrap();
    fs::write(&path, &bytes).unwrap();
    let mut store = file::Store::new(fixture.0.clone());
    store.load().unwrap();
    assert!(store.save(snapshot(2), &AtomicBool::new(false)).is_err());
    assert_eq!(fs::read(&path).unwrap(), bytes);
}

struct Fake {
    load_release: Option<Receiver<()>>,
    save_release: Option<Receiver<()>>,
    saved: SyncSender<String>,
    finished: Option<SyncSender<()>>,
    fail: bool,
}
impl Backend for Fake {
    fn load(&mut self) -> Result<Loaded, String> {
        if let Some(release) = self.load_release.take() {
            let _ = release.recv();
        }
        Ok(Loaded::default())
    }
    fn save(&mut self, snapshot: Snapshot, _: &AtomicBool) -> Result<(), String> {
        self.saved.send(snapshot.theme).unwrap();
        if let Some(release) = self.save_release.take() {
            let _ = release.recv();
        }
        if std::mem::take(&mut self.fail) {
            Err("Injected write failure".into())
        } else {
            Ok(())
        }
    }
}
impl Drop for Fake {
    fn drop(&mut self) {
        if let Some(done) = self.finished.take() {
            let _ = done.send(());
        }
    }
}

fn until(mut condition: impl FnMut() -> bool) {
    let until = Instant::now() + Duration::from_secs(3);
    while !condition() {
        assert!(Instant::now() < until, "owned settings worker timed out");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn preferences_blocked_worker_coalesces_latest_changes_and_retains_failure_until_retry() {
    let (release, blocked) = mpsc::channel();
    let (saved, calls) = mpsc::sync_channel(4);
    let (finished, done) = mpsc::sync_channel(1);
    let mut controller = Controller::with_backend(
        Fake {
            load_release: None,
            save_release: Some(blocked),
            saved,
            finished: Some(finished),
            fail: true,
        },
        egui::Context::default(),
    );
    until(|| controller.poll().is_some());
    controller.update(snapshot(1));
    controller.dispatch(true);
    assert_eq!(
        calls.recv_timeout(Duration::from_secs(3)).unwrap(),
        snapshot(1).theme
    );
    let start = Instant::now();
    for seed in 2..=100 {
        controller.update(snapshot(seed));
        controller.poll();
        controller.dispatch(true);
    }
    assert!(calls.try_recv().is_err()); // No queued stale saves or worker respawn.
    assert!(controller.pending());
    let elapsed = start.elapsed();
    release.send(()).unwrap();
    until(|| {
        controller.poll();
        controller.error().is_some()
    });
    for _ in 0..100 {
        controller.dispatch(true);
    }
    assert!(calls.try_recv().is_err()); // No automatic failed-save loop.
    assert!(controller.pending());
    controller.retry();
    assert_eq!(
        calls.recv_timeout(Duration::from_secs(3)).unwrap(),
        snapshot(100).theme
    );
    until(|| {
        controller.poll();
        !controller.pending()
    });
    assert_eq!(controller.status(), "Settings saved");
    drop(controller);
    done.recv_timeout(Duration::from_secs(3)).unwrap();
    assert!(elapsed < Duration::from_secs(1), "{elapsed:?}");
    println!(
        "99 settings updates/polls while writer blocked: {elapsed:?}; only first/latest snapshots dispatched"
    );
}

#[test]
fn preferences_pending_read_and_write_never_join_on_controller_drop() {
    for block_read in [true, false] {
        let (release, blocked) = mpsc::channel();
        let (saved, calls) = mpsc::sync_channel(1);
        let (finished, done) = mpsc::sync_channel(1);
        let (load_release, save_release) = if block_read {
            (Some(blocked), None)
        } else {
            (None, Some(blocked))
        };
        let mut controller = Controller::with_backend(
            Fake {
                load_release,
                save_release,
                saved,
                finished: Some(finished),
                fail: false,
            },
            egui::Context::default(),
        );
        if block_read {
            assert!(!controller.can_edit());
            for _ in 0..1000 {
                assert!(controller.poll().is_none());
            }
        } else {
            until(|| controller.poll().is_some());
            controller.update(snapshot(1));
            controller.dispatch(true);
            calls.recv_timeout(Duration::from_secs(3)).unwrap();
        }
        // Test drop on its own thread so a regression cannot hang the test runner.
        let (dropped, drop_done) = mpsc::sync_channel(1);
        let dropper = std::thread::spawn(move || {
            drop(controller);
            dropped.send(()).unwrap();
        });
        let early = drop_done.recv_timeout(Duration::from_millis(200));
        drop(release); // Always unblock owned backend before assertion/cleanup.
        if early.is_err() {
            drop_done.recv_timeout(Duration::from_secs(3)).unwrap();
        }
        dropper.join().unwrap();
        done.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(early.is_ok(), "controller joined blocked read/write");
    }
}

#[test]
fn preferences_disabled_mode_has_no_worker_no_poll_timer_and_no_pending_save() {
    let mut controller = Controller::default();
    controller.update(snapshot(1));
    controller.dispatch(true);
    controller.retry();
    assert!(controller.can_edit());
    assert!(!controller.enabled());
    assert!(!controller.pending());
    assert!(controller.poll().is_none());
    assert!(controller.commands.is_none());
}
