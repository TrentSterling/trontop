use super::*;
use crate::diagnostics::Provider;
use std::sync::atomic::AtomicUsize;

fn wait_until(mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !ready() {
        assert!(Instant::now() < deadline, "fixture worker did not finish");
        thread::sleep(Duration::from_millis(2));
    }
}

fn unchanged<T>(cache: &Arc<T>, _: Instant) -> Arc<T> {
    Arc::clone(cache)
}

#[test]
fn service_failure_retains_cache_and_last_success_until_recovery() {
    let mut cache = Arc::new(Vec::new());
    let mut health = Health::default();
    let at = Instant::now();
    apply_services(&mut cache, &mut health, Ok(vec![41_u32]), at);
    apply_services(
        &mut cache,
        &mut health,
        Err("Fixture private path".into()),
        at,
    );
    assert_eq!(cache.as_slice(), &[41]);
    assert_eq!(health.last_success, Some(at));
    assert_eq!(health.state(Provider::Services, at), State::Stale);
    assert_eq!(health.issue, Some(Issue::ServiceQuery));
    apply_services(&mut cache, &mut health, Ok(vec![42]), at);
    assert_eq!(cache.as_slice(), &[42]);
    assert_eq!(health.state(Provider::Services, at), State::Live);
}

#[test]
fn stuck_inventory_keeps_other_worker_updates_and_drop_nonblocking_without_respawn() {
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let calls = Arc::new(AtomicUsize::new(0));
    let worker_calls = Arc::clone(&calls);
    let mut slow = Worker::spawn(
        "fixture-stuck-inventory",
        Vec::<u32>::new(),
        unchanged,
        move |cache, health, at| {
            worker_calls.fetch_add(1, Ordering::Relaxed);
            entered_tx.send(at).unwrap();
            release_rx.recv().unwrap();
            apply_services(cache, health, Ok(vec![123]), at);
        },
    );
    let mut fast = Worker::spawn(
        "fixture-fast-inventory",
        Vec::<u32>::new(),
        unchanged,
        |cache, health, at| {
            let value = cache.first().copied().unwrap_or(0) + 1;
            apply_services(cache, health, Ok(vec![value]), at);
        },
    );
    let at = entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    wait_until(|| fast.latest(Instant::now()).0.as_slice() == [1]);
    for _ in 0..100 {
        slow.request();
    }
    let started = Instant::now();
    for _ in 0..1000 {
        let (_, health) = slow.latest(at + SLOW_AFTER);
        assert_eq!(health.issue, Some(Issue::InventoryTimeout));
        assert_eq!(
            health.state(Provider::Startup, at + SLOW_AFTER),
            State::Unavailable
        );
        assert!(health.query_millis.is_none());
    }
    assert!(started.elapsed() < Duration::from_secs(1));
    fast.request();
    wait_until(|| fast.latest(Instant::now()).0.as_slice() == [2]);
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    let started = Instant::now();
    drop(slow);
    // The read stays blocked until after this check; 2 s absorbs a loaded run.
    assert!(started.elapsed() < Duration::from_secs(2));
    release_tx.send(()).unwrap();
    // Dropping cancels the queued refresh, even after the blocking read returns.
    assert!(entered_rx.recv_timeout(Duration::from_secs(3)).is_err());
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}

#[test]
fn timed_out_startup_preserves_rows_reuses_failed_view_and_recovers() {
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let mut count = 0;
    let mut worker = Worker::spawn(
        "fixture-startup-retention",
        startup::Snapshot::default(),
        invalidate_startup,
        move |cache, health, at| {
            count += 1;
            if count == 2 {
                entered_tx.send(at).unwrap();
                release_rx.recv().unwrap();
            }
            let reads = startup::Source::ALL
                .into_iter()
                .map(|source| startup::Read {
                    source,
                    state: startup::ReadState::Readable,
                    rows: vec![crate::model::StartupRow {
                        key: "fixture".into(),
                        name: format!("value{count}"),
                        source,
                        command: "not executed".into(),
                    }],
                })
                .collect();
            apply_startup(cache, health, reads, at);
        },
    );
    wait_until(|| worker.latest(Instant::now()).0.rows().count() == 5);
    let old_success = worker.latest(Instant::now()).1.last_success;
    worker.request();
    let at = entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let (stale, health) = worker.latest(at + SLOW_AFTER);
    assert_eq!(health.last_success, old_success);
    assert_eq!(health.coverage, Some((0, 5)));
    assert_eq!(stale.rows().count(), 5);
    assert!(
        stale
            .sources
            .iter()
            .all(|s| s.state(at + SLOW_AFTER) == startup::State::Cached)
    );
    assert!(Arc::ptr_eq(&stale, &worker.latest(at + SLOW_AFTER).0));
    release_tx.send(()).unwrap();
    wait_until(|| {
        worker
            .latest(Instant::now())
            .0
            .rows()
            .all(|(_, e)| e.row.name == "value2")
    });
    let (recovered, health) = worker.latest(Instant::now());
    assert!(health.issue.is_none());
    assert_eq!(health.last_success, Some(at));
    assert!(
        recovered
            .sources
            .iter()
            .all(|s| s.state(Instant::now()) == startup::State::Live)
    );
}

#[test]
fn refresh_during_read_is_coalesced_and_next_read_timestamp_is_new() {
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let mut count = 0;
    let mut worker = Worker::spawn(
        "fixture-coalescing",
        Vec::new(),
        unchanged,
        move |cache, health, at| {
            count += 1;
            if count == 1 {
                entered_tx.send(at).unwrap();
                release_rx.recv().unwrap();
            }
            apply_services(cache, health, Ok(vec![count]), at);
            if count == 2 {
                done_tx.send(at).unwrap();
            }
        },
    );
    let first = entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let request_at = Instant::now();
    for _ in 0..100 {
        worker.request();
    }
    release_tx.send(()).unwrap();
    let second = done_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    assert!(second > first && second > request_at);
    wait_until(|| worker.latest(Instant::now()).0.as_slice() == [2]);
    assert!(done_rx.recv_timeout(Duration::from_millis(30)).is_err());
}

#[test]
fn publication_contention_uses_cached_snapshot_and_worker_failure_is_explicit() {
    let mut worker = Worker::spawn(
        "fixture-publication",
        vec![7],
        unchanged,
        |cache, health, at| apply_services(cache, health, Ok(vec![8]), at),
    );
    wait_until(|| worker.latest(Instant::now()).0.as_slice() == [8]);
    let shared = Arc::clone(&worker.shared);
    let _held = shared.lock().unwrap();
    let started = Instant::now();
    assert_eq!(worker.latest(Instant::now()).0.as_slice(), &[8]);
    assert!(started.elapsed() < Duration::from_secs(2));
    drop(_held);
    // Simulate a failed spawn/finished worker without invoking any OS inventory.
    worker.stop.store(true, Ordering::Release);
    worker.requests.take();
    wait_until(|| worker.thread.as_ref().unwrap().is_finished());
    let (rows, health) = worker.latest(Instant::now());
    assert_eq!(rows.as_slice(), &[8]);
    assert_eq!(health.issue, Some(Issue::InventoryWorker));
    assert_eq!(
        health.state(Provider::Services, Instant::now()),
        State::Stale
    );
    assert!(health.query_millis.is_none());
}

#[test]
#[ignore = "Read-only native Startup/SCM worker lifecycle; no service commands, GUI or OS input"]
fn native_inventory_workers_publish_read_only_snapshots() {
    let started = Instant::now();
    let mut inventories = Inventories::spawn();
    let deadline = started + Duration::from_secs(15);
    let (startup, services) = loop {
        let startup = inventories.startup();
        let services = inventories.services();
        if startup.1.query_millis.is_some() && services.1.query_millis.is_some() {
            break (startup, services);
        }
        assert!(
            Instant::now() < deadline,
            "read-only inventory probe did not complete in 15 seconds"
        );
        thread::sleep(Duration::from_millis(10));
    };
    assert!(services.1.last_success.is_some());
    assert_eq!(startup.0.sources.len(), 5);
    let poll_started = Instant::now();
    for _ in 0..1000 {
        inventories.startup();
        inventories.services();
    }
    eprintln!(
        "Read-only worker probe: {} startup entries, {} services; startup {:?} ms, services {:?} ms; 1000 snapshot pairs {:?}; ready in {:?}",
        startup.0.rows().count(),
        services.0.len(),
        startup.1.query_millis,
        services.1.query_millis,
        poll_started.elapsed(),
        started.elapsed()
    );
}
