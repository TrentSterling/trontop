use super::*;

fn pixels() -> Option<Pixels> {
    Some(Pixels(vec![255; PIXEL_BYTES]))
}
fn path(index: usize) -> PathBuf {
    PathBuf::from(format!(r"C:\Fixture\app-{index}.exe"))
}
fn settle(cache: &mut Cache, ctx: &egui::Context, now: Instant) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while cache.outstanding > 0 && Instant::now() < deadline {
        cache.poll(ctx, now);
        std::thread::yield_now();
    }
    assert_eq!(cache.outstanding, 0, "fixture worker did not complete");
}

#[test]
fn path_gate_rejects_remote_device_relative_ads_and_unbounded_inputs() {
    for bad in [
        r"\\server\share\x.exe",
        r"\\?\UNC\host\share\x.exe",
        r"\\.\C:\x.exe",
        r"C:x.exe",
        "x.exe",
        r"C:\a\..\x.exe",
        r"C:\a\x.exe:stream.exe",
        r"C:\x.dll",
        "C:\\x\0.exe",
        r"C:\a.\x.exe",
    ] {
        assert!(
            local_executable(Path::new(bad)).is_none(),
            "accepted {bad:?}"
        );
    }
    assert!(local_executable(Path::new(&format!("C:\\{}.exe", "x".repeat(5000)))).is_none());
    assert_eq!(
        local_executable(Path::new(r"\\?\C:\Fixture\app.exe")),
        Some(PathBuf::from(r"C:\Fixture\app.exe"))
    );
    assert_eq!(
        local_executable(Path::new("C:/Fixture/日本語.EXE")),
        Some(PathBuf::from("C:\\Fixture\\日本語.EXE"))
    );
}

#[test]
fn requests_deduplicate_failures_back_off_and_refresh_keeps_old_texture() {
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = calls.clone();
    let ctx = egui::Context::default();
    let mut cache = Cache::with_loader(ctx.clone(), move |_| {
        if observed.fetch_add(1, Ordering::Relaxed) == 0 {
            pixels()
        } else {
            None
        }
    });
    let now = Instant::now();
    for _ in 0..20 {
        assert!(cache.texture(&path(1), now).is_none());
    }
    assert_eq!(cache.outstanding, 1);
    settle(&mut cache, &ctx, now);
    let original = cache.texture(&path(1), now).unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    assert_eq!(cache.texture(&path(1), now + REFRESH), Some(original));
    settle(&mut cache, &ctx, now + REFRESH);
    assert_eq!(
        cache.texture(&path(1), now + REFRESH + RETRY / 2),
        Some(original)
    );
    assert_eq!(calls.load(Ordering::Relaxed), 2);
    cache.texture(&path(1), now + REFRESH + RETRY);
    settle(&mut cache, &ctx, now + REFRESH + RETRY);
    assert_eq!(calls.load(Ordering::Relaxed), 3);
}

#[test]
fn invalid_or_missing_pixels_are_negatively_cached_without_uploading() {
    for invalid in [false, true] {
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let observed = calls.clone();
        let caller = std::thread::current().id();
        let ctx = egui::Context::default();
        let mut cache = Cache::with_loader(ctx.clone(), move |_| {
            assert_ne!(std::thread::current().id(), caller);
            observed.fetch_add(1, Ordering::Relaxed);
            invalid.then(|| Pixels(vec![0; 16]))
        });
        let now = Instant::now();
        cache.texture(&path(1), now);
        settle(&mut cache, &ctx, now);
        for _ in 0..20 {
            assert!(cache.texture(&path(1), now + RETRY / 2).is_none());
        }
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert!(cache.entries[&path(1)].texture.is_none());
        cache.texture(&path(1), now + RETRY);
        settle(&mut cache, &ctx, now + RETRY);
        assert_eq!(calls.load(Ordering::Relaxed), 2);
    }
}

#[test]
fn stuck_loader_bounds_queue_and_drop_without_starting_replacement_workers() {
    let (release, blocked) = mpsc::channel();
    let (started, running) = mpsc::channel();
    let (done, finished) = mpsc::channel();
    let ctx = egui::Context::default();
    let mut cache = Cache::with_loader(ctx, move |_| {
        started.send(()).unwrap();
        blocked.recv().unwrap();
        done.send(()).unwrap();
        pixels()
    });
    let now = Instant::now();
    cache.texture(&path(0), now);
    running.recv_timeout(Duration::from_secs(2)).unwrap();
    for index in 1..2000 {
        cache.texture(&path(index), now);
    }
    assert_eq!(cache.outstanding, OUTSTANDING);
    assert_eq!(cache.entries.len(), CAPACITY);
    assert!(running.try_recv().is_err());
    let started = Instant::now();
    drop(cache);
    assert!(started.elapsed() < Duration::from_millis(250));
    release.send(()).unwrap();
    finished.recv_timeout(Duration::from_secs(2)).unwrap();
}

#[test]
fn textures_and_negative_cache_stay_bounded_while_old_paths_are_evicted() {
    let ctx = egui::Context::default();
    let mut cache = Cache::with_loader(ctx.clone(), |_| pixels());
    let now = Instant::now();
    for index in 0..CAPACITY + 40 {
        cache.poll(&ctx, now);
        cache.texture(&path(index), now);
        settle(&mut cache, &ctx, now);
        assert!(cache.entries.len() <= CAPACITY);
    }
    assert!(!cache.entries.contains_key(&path(0)));
    assert!(cache.entries.contains_key(&path(CAPACITY + 39)));
    assert_eq!(
        cache
            .entries
            .values()
            .filter(|e| e.texture.is_some())
            .count(),
        CAPACITY
    );
}

#[test]
fn disconnected_worker_becomes_disabled_without_ui_panics_or_respawn() {
    let ctx = egui::Context::default();
    let (requests, receiver) = mpsc::sync_channel(OUTSTANDING);
    let (sender, results) = mpsc::sync_channel(OUTSTANDING);
    drop(receiver);
    drop(sender);
    let mut cache = Cache {
        worker: Some(Worker {
            requests,
            results,
            stop: Arc::new(AtomicBool::new(false)),
        }),
        ..Default::default()
    };
    cache.poll(&ctx, Instant::now());
    assert!(cache.worker.is_none());
    assert!(cache.texture(&path(2), Instant::now()).is_none());
    assert_eq!(cache.outstanding, 0);
}

#[test]
fn ready_icons_respect_the_per_frame_upload_budget() {
    let ctx = egui::Context::default();
    let (requests, receiver) = mpsc::sync_channel(OUTSTANDING);
    let (sender, results) = mpsc::sync_channel(OUTSTANDING);
    let mut cache = Cache {
        worker: Some(Worker {
            requests,
            results,
            stop: Arc::new(AtomicBool::new(false)),
        }),
        ..Default::default()
    };
    let now = Instant::now();
    let total = UPLOADS_PER_FRAME * 3;
    for index in 0..total {
        cache.texture(&path(index), now);
        let request = receiver.try_recv().unwrap();
        sender
            .try_send(Loaded {
                request,
                pixels: pixels(),
            })
            .unwrap();
    }
    for frame in 1..=3 {
        cache.poll(&ctx, now);
        assert_eq!(
            cache
                .entries
                .values()
                .filter(|e| e.texture.is_some())
                .count(),
            UPLOADS_PER_FRAME * frame
        );
        assert_eq!(cache.outstanding, total - UPLOADS_PER_FRAME * frame);
    }
}
