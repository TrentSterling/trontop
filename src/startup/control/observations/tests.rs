use super::*;
use crate::startup::control::{Action, Approval, Control, Failure, RawValue, Registration};
use crate::startup::{Read, ReadState, Source};
fn row() -> StartupRow {
    StartupRow {
        key: "Owned".into(),
        name: "Fixture".into(),
        command: "fixture.exe".into(),
        source: Source::UserRun,
        control: Some(Control {
            registration: Some(Registration::Run(RawValue {
                kind: 1,
                bytes: vec![],
            })),
            approval: Approval::Missing,
        }),
    }
}
fn read(row: StartupRow, at: Instant) -> Snapshot {
    let mut snapshot = Snapshot::default();
    snapshot.apply(
        vec![Read {
            source: row.source,
            rows: vec![row],
            state: ReadState::Readable,
        }],
        at,
    );
    snapshot
}
fn done(target: StartupRow, at: Instant) -> Outcome {
    Outcome {
        request: Request::new(target.clone(), Action::Disable),
        result: Ok(Receipt {
            target,
            before: Approval::Missing,
            after: Approval::Missing.changed(false, 42).unwrap(),
            observed_at: at,
        }),
    }
}
#[test]
fn post_command_read_outranks_old_inventory_and_exact_undo_survives_matching_refresh() {
    let at = Instant::now();
    let snapshot = read(row(), at - Duration::from_secs(2));
    let mut history = Observations::default();
    history.apply(&done(row(), at));
    history.reconcile(&snapshot);
    let (source, entry) = snapshot.rows().next().unwrap();
    let (current, freshness, fresh) = history.view(source, entry, at);
    assert!(fresh);
    assert_eq!(history.observed_at(&current, entry.observed_at), at);
    assert_eq!(
        history.next_state_change(&snapshot, at),
        Some(at + Duration::from_secs(75))
    );
    assert_eq!(
        history.next_state_change(&snapshot, at + Duration::from_secs(76)),
        None
    );
    assert!(!history.view(source, entry, at + Duration::from_secs(76)).2);
    assert_eq!(freshness, "Command read");
    assert_eq!(
        current.control.as_ref().unwrap().approval.state(),
        super::super::State::Disabled
    );
    let undo = history.undo(&current).unwrap();
    let latest = read(current.into_owned(), at + Duration::from_millis(1));
    history.reconcile(&latest);
    assert!(history.undo(&latest.rows().next().unwrap().1.row).is_some());
    assert_eq!(undo.action, Action::Restore(Approval::Missing));
    history.reconcile(&read(row(), at + Duration::from_millis(2)));
    assert!(
        history.undo(&row()).is_none(),
        "external change invalidates Undo"
    );
}
#[test]
fn unknown_outcome_requires_a_read_started_after_failure_and_never_offers_old_undo() {
    let mut history = Observations::default();
    let old = read(row(), Instant::now() - Duration::from_secs(1));
    history.apply(&done(row(), Instant::now()));
    history.apply(&Outcome {
        request: Request::new(row(), Action::Enable),
        result: Err(Failure::uncertain("unknown")),
    });
    history.reconcile(&old);
    let (source, entry) = old.rows().next().unwrap();
    assert_eq!(
        history.view(source, entry, Instant::now()).1,
        "Refresh required"
    );
    assert!(!history.view(source, entry, Instant::now()).2);
    assert!(history.undo(&row()).is_none());
    history.reconcile(&read(row(), Instant::now() + Duration::from_millis(1)));
    assert!(history.view(source, entry, Instant::now()).2);
}
#[test]
fn changed_registration_cached_rows_removal_and_retention_limits_fail_closed() {
    let at = Instant::now();
    let mut history = Observations::default();
    history.apply(&done(row(), at));
    let mut changed = row();
    changed.command = "replacement.exe".into();
    let old = read(changed, at - Duration::from_secs(2));
    let (source, entry) = old.rows().next().unwrap();
    assert_eq!(history.view(source, entry, at).1, "Pre-command");
    assert!(!history.view(source, entry, at).2);
    let mut missing = Snapshot::default();
    missing.apply(
        vec![Read {
            source: Source::UserRun,
            rows: vec![],
            state: ReadState::Missing,
        }],
        at + Duration::from_secs(1),
    );
    history.reconcile(&missing);
    assert!(history.records.is_empty());
    for i in 0..=LIMIT {
        let mut target = row();
        target.key = format!("Owned{i}");
        history.apply(&done(target, at));
    }
    assert_eq!(history.records.len(), LIMIT);
    let evicted = read(row(), at - Duration::from_secs(1));
    let (source, entry) = evicted.rows().next().unwrap();
    assert!(!history.view(source, entry, at).2);
    let fresh = read(row(), at + Duration::from_secs(1));
    let (source, entry) = fresh.rows().next().unwrap();
    assert!(history.view(source, entry, at + Duration::from_secs(1)).2);
}
