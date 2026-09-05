use super::*;
use std::sync::mpsc;

fn batch() -> Batch {
    [10.0, 0.005, 256.0, 1_000_000_000.0, 0.0].map(|v| Ok(vec![("0 C: D:".into(), Some(v))]))
}

#[test]
fn native_units_zero_and_non_percentage_values_are_preserved() {
    let at = Instant::now();
    let mut snapshot = Snapshot::default();
    snapshot.apply(batch(), at, Duration::from_micros(700));
    assert_eq!(
        snapshot.devices[0].readings.map(|r| r.live(&snapshot, at)),
        [
            Some(90.0),
            Some(5.0),
            Some(256.0),
            Some(1_000_000_000.0),
            Some(0.0)
        ]
    );
    assert_eq!(snapshot.state(at), State::Live);
    for raw in [f64::NAN, f64::INFINITY, -1.0] {
        for metric in Metric::ALL {
            assert_eq!(metric.convert(raw), None);
        }
    }
    assert_eq!(Metric::Response.convert(f64::MAX), None);
    assert_eq!(Metric::Active.convert(101.0), Some(0.0));
    assert_eq!(Metric::Active.convert(0.0), Some(100.0));
}

#[test]
fn startup_stall_is_unavailable_in_fields_and_provider_health() {
    let at = Instant::now();
    let snapshot = Snapshot {
        at: Some(at),
        ..Default::default()
    };
    assert_eq!(snapshot.state(at), State::Starting);
    assert_eq!(Reading::default().state(&snapshot, at), "Warming");
    let later = at + STALE_AFTER + Duration::from_secs(1);
    assert_eq!(snapshot.state(later), State::Unavailable);
    assert_eq!(Reading::default().state(&snapshot, later), "Unavailable");
    assert_eq!(snapshot.health(later).issue, Some(Issue::DiskCounters));
}

#[test]
fn partial_failed_stalled_and_recovered_samples_never_fake_freshness() {
    let at = Instant::now();
    let mut s = Snapshot::default();
    s.apply(batch(), at, Duration::ZERO);
    let later = at + Duration::from_secs(1);
    let mut partial = batch();
    partial[1] = Err("Fixture failure");
    s.apply(partial, later, Duration::ZERO);
    assert_eq!(s.state(later), State::Partial);
    assert_eq!(s.devices[0].readings[1].state(&s, later), "Cached");
    assert_eq!(s.devices[0].readings[1].value, Some(5.0));
    assert_eq!(s.devices[0].readings[1].at, Some(at));
    assert_eq!(s.devices[0].readings[1].live(&s, later), None);
    assert_eq!(
        s.state(later + STALE_AFTER + Duration::from_millis(1)),
        State::Stale
    );
    let later = later + Duration::from_secs(1);
    s.apply(
        std::array::from_fn(|_| Err("Fixture failure")),
        later,
        Duration::ZERO,
    );
    assert_eq!(s.state(later), State::Stale);
    assert_eq!(s.devices.len(), 1);
    s.apply(batch(), later + Duration::from_secs(1), Duration::ZERO);
    assert_eq!(s.state(later + Duration::from_secs(1)), State::Live);
    let h = s.health(later + Duration::from_secs(1));
    assert_eq!(h.coverage, Some((5, 5)));
    assert!(h.issue.is_none());
}

#[test]
fn complete_inventory_removes_missing_disks_partial_retains_and_bounds_them() {
    let at = Instant::now();
    let mut s = Snapshot::default();
    s.apply(batch(), at, Duration::ZERO);
    let mut missing: Batch = std::array::from_fn(|_| Ok(vec![]));
    missing[4] = Err("Fixture");
    s.apply(missing, at + Duration::from_secs(1), Duration::ZERO);
    assert_eq!(s.devices.len(), 1);
    s.apply(
        std::array::from_fn(|_| Ok(vec![])),
        at + Duration::from_secs(2),
        Duration::ZERO,
    );
    assert!(s.devices.is_empty());
    assert_eq!(s.state(at + Duration::from_secs(2)), State::Unavailable);
    let large = std::array::from_fn(|_| {
        Ok((0..200)
            .map(|n| (format!("{n} C:"), Some(0.0)))
            .chain([
                ("_Total".into(), Some(9.0)),
                ("Not a disk".into(), Some(0.0)),
            ])
            .collect())
    });
    s.apply(large, at + Duration::from_secs(3), Duration::ZERO);
    assert_eq!(s.devices.len(), MAX_DEVICES);
    assert!(s.error.is_some());
    assert_eq!(s.state(at + Duration::from_secs(3)), State::Partial);
    assert!(
        s.devices
            .windows(2)
            .all(|pair| pair[0].number <= pair[1].number)
    );
}

#[test]
fn duplicate_or_invalid_instance_values_are_not_summed_or_used() {
    let mut s = Snapshot::default();
    let at = Instant::now();
    let mut b = batch();
    b[0] = Ok(vec![
        ("0 C: D:".into(), Some(30.0)),
        ("0 C: D:".into(), Some(40.0)),
    ]);
    b[1] = Ok(vec![("0 C: D:".into(), Some(f64::NAN))]);
    s.apply(b, at, Duration::ZERO);
    assert_eq!(s.devices[0].readings[0].value, None);
    assert_eq!(s.devices[0].readings[1].value, None);
    assert_eq!(s.state(at), State::Partial);
    assert_eq!(disk_number("2"), Some(2));
    assert_eq!(disk_number("12 C: D:"), Some(12));
    for bad in ["_Total", "0x1", "-1 C:", "0\nC:", "4294967296 C:"] {
        assert_eq!(disk_number(bad), None);
    }
    assert_eq!(disk_number(&format!("0 {}", "x".repeat(600))), None);
}

#[test]
fn worker_publishes_once_without_native_calls_and_drop_does_not_wait_for_stall() {
    let mut m = Monitor::spawn_with(|| Box::new(batch));
    let deadline = Instant::now() + Duration::from_secs(2);
    while m.snapshot().generation == 0 && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(m.snapshot().devices.len(), 1);
    let first = m.snapshot();
    assert!(Arc::ptr_eq(&first, &m.snapshot()));
    drop(m);
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let worker_calls = calls.clone();
    let mut blocked = Monitor::spawn_with(move || {
        Box::new(move || {
            worker_calls.fetch_add(1, Ordering::Relaxed);
            entered_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            batch()
        })
    });
    entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let begun = Instant::now();
    for _ in 0..1000 {
        assert_eq!(blocked.snapshot().generation, 0);
    }
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    drop(blocked);
    let elapsed = begun.elapsed();
    release_tx.send(()).unwrap();
    assert!(
        elapsed < Duration::from_millis(250),
        "Read/drop waited on provider: {elapsed:?}"
    );
}

#[test]
fn mailbox_takes_newest_snapshot_and_never_waits_for_publication_lock() {
    let mut m = Monitor::spawn_with(|| Box::new(batch));
    let deadline = Instant::now() + Duration::from_secs(2);
    while m.snapshot().generation == 0 && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(5));
    }
    let mut new = (*m.snapshot()).clone();
    new.generation = 99;
    let pending = m.pending.clone();
    let mut locked = pending.lock().unwrap();
    *locked = Some(Arc::new(new));
    assert_ne!(m.snapshot().generation, 99); // locked publication, retained local cache
    drop(locked);
    assert_eq!(m.snapshot().generation, 99);
    assert!(pending.lock().unwrap().is_none());
}
