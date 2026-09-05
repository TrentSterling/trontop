use super::*;

fn row(source: Source, key: &str, command: &str) -> StartupRow {
    StartupRow {
        key: key.into(),
        name: key.into(),
        command: command.into(),
        source,
    }
}
fn read(source: Source, state: ReadState, rows: Vec<StartupRow>) -> Read {
    Read {
        source,
        state,
        rows,
    }
}
fn source(snapshot: &Snapshot, id: Source) -> &SourceSnapshot {
    snapshot.sources.iter().find(|s| s.source == id).unwrap()
}

#[test]
fn failed_sources_preserve_entries_while_other_sources_update_or_disappear() {
    let mut snapshot = Snapshot::default();
    let at = Instant::now();
    let user = Source::UserRun;
    let machine = Source::MachineRun;
    snapshot.apply(
        vec![
            read(
                user,
                ReadState::Readable,
                vec![row(user, "Player", "old.exe")],
            ),
            read(
                machine,
                ReadState::Readable,
                vec![row(machine, "Helper", "helper.exe")],
            ),
        ],
        at,
    );
    snapshot.apply(
        vec![
            read(user, ReadState::Failed, vec![]),
            read(
                machine,
                ReadState::Readable,
                vec![row(machine, "New", "new.exe")],
            ),
        ],
        at + Duration::from_secs(30),
    );
    let retained = source(&snapshot, user);
    assert_eq!(retained.entries[0].row.command, "old.exe");
    assert_eq!(retained.entries[0].observed_at, at);
    assert_eq!(retained.last_complete, Some(at));
    assert_eq!(retained.state(at + Duration::from_secs(30)), State::Cached);
    assert_eq!(source(&snapshot, machine).entries[0].row.name, "New");
    snapshot.apply(
        vec![
            read(user, ReadState::Missing, vec![]),
            read(machine, ReadState::Readable, vec![]),
        ],
        at + Duration::from_secs(60),
    );
    assert_eq!(
        source(&snapshot, user).state(at + Duration::from_secs(60)),
        State::Absent
    );
    assert_eq!(
        source(&snapshot, machine).state(at + Duration::from_secs(60)),
        State::Empty
    );
    assert_eq!(snapshot.rows().count(), 0);
    assert_eq!(snapshot.coverage(), (2, 5));
}

#[test]
fn partial_reads_merge_observed_entries_without_fabricating_removals_or_freshness() {
    let mut snapshot = Snapshot::default();
    let at = Instant::now();
    let id = Source::UserRun;
    snapshot.apply(
        vec![read(
            id,
            ReadState::Readable,
            vec![row(id, "A", "old.exe"), row(id, "B", "keep.exe")],
        )],
        at,
    );
    let later = at + Duration::from_secs(30);
    snapshot.apply(
        vec![read(
            id,
            ReadState::Failed,
            vec![row(id, "A", "changed.exe"), row(id, "C", "new.exe")],
        )],
        later,
    );
    let cached = source(&snapshot, id);
    assert_eq!(cached.entries.len(), 3);
    assert_eq!(cached.last_complete, Some(at));
    assert_eq!(cached.state(later), State::Partial);
    assert_eq!(cached.row_state(&cached.entries[0], later), "Observed");
    assert_eq!(cached.entries[0].row.command, "changed.exe");
    assert_eq!(cached.row_state(&cached.entries[1], later), "Cached");
    assert_eq!(cached.entries[1].observed_at, at);
    assert_eq!(cached.row_state(&cached.entries[2], later), "Observed");
    // A failed source with no earlier complete snapshot still keeps real partial data.
    let other = Source::UserFolder;
    snapshot.apply(
        vec![read(
            other,
            ReadState::Failed,
            vec![row(other, "First.lnk", "first.lnk")],
        )],
        later,
    );
    snapshot.apply(
        vec![read(other, ReadState::Failed, vec![])],
        later + Duration::from_secs(1),
    );
    assert_eq!(source(&snapshot, other).entries.len(), 1);
    assert_eq!(source(&snapshot, other).last_complete, None);
    assert_eq!(
        source(&snapshot, other).state(later + Duration::from_secs(1)),
        State::Cached
    );
    snapshot.apply(
        vec![read(
            id,
            ReadState::Readable,
            vec![row(id, "C", "recovered.exe")],
        )],
        later + Duration::from_secs(30),
    );
    assert_eq!(source(&snapshot, id).entries.len(), 1);
    assert_eq!(
        source(&snapshot, id).state(later + Duration::from_secs(30)),
        State::Live
    );
}

#[test]
fn starting_missing_unreadable_and_aged_empty_results_are_distinct() {
    let mut snapshot = Snapshot::default();
    let at = Instant::now();
    assert_eq!(snapshot.sources.len(), 5);
    assert!(
        snapshot
            .sources
            .iter()
            .all(|s| s.state(at) == State::Starting)
    );
    snapshot.apply(vec![read(Source::UserRun, ReadState::Readable, vec![])], at);
    assert_eq!(
        source(&snapshot, Source::MachineRun).state(at),
        State::Unavailable
    );
    assert_eq!(source(&snapshot, Source::UserRun).state(at), State::Empty);
    assert_eq!(
        source(&snapshot, Source::UserRun).state(at + STALE_AFTER + Duration::from_secs(1)),
        State::Cached
    );
    assert!(source(&snapshot, Source::UserRun).entries.is_empty());
}

#[test]
fn entry_identity_keeps_same_stems_and_same_names_across_sources_separate() {
    let at = Instant::now();
    let mut snapshot = Snapshot::default();
    let id = Source::UserFolder;
    let mut first = row(id, "Player.lnk", "one.lnk");
    first.name = "Player".into();
    let mut second = row(id, "Player.cmd", "two.cmd");
    second.name = "Player".into();
    snapshot.apply(
        vec![
            read(id, ReadState::Readable, vec![first, second]),
            read(
                Source::UserRun,
                ReadState::Readable,
                vec![row(Source::UserRun, "Player", "exe")],
            ),
        ],
        at,
    );
    assert_eq!(snapshot.rows().count(), 3);
    snapshot.apply(
        vec![read(
            id,
            ReadState::Failed,
            vec![row(id, "Player.lnk", "changed.lnk")],
        )],
        at + Duration::from_secs(1),
    );
    assert_eq!(source(&snapshot, id).entries.len(), 2);
    assert_eq!(source(&snapshot, Source::UserRun).entries.len(), 1);
}

#[test]
fn repeated_incomplete_reads_have_bounded_retention_and_complete_reads_reset_it() {
    let at = Instant::now();
    let mut snapshot = Snapshot::default();
    let id = Source::UserRun;
    snapshot.apply(
        vec![read(
            id,
            ReadState::Failed,
            (0..ENTRY_LIMIT)
                .map(|i| row(id, &i.to_string(), "fixture"))
                .collect(),
        )],
        at,
    );
    snapshot.apply(
        vec![read(
            id,
            ReadState::Failed,
            vec![row(id, "extra", "fixture")],
        )],
        at + Duration::from_secs(1),
    );
    assert_eq!(source(&snapshot, id).entries.len(), ENTRY_LIMIT);
    assert!(source(&snapshot, id).retention_limited);
    snapshot.apply(
        vec![read(
            id,
            ReadState::Readable,
            vec![row(id, "final", "fixture")],
        )],
        at + Duration::from_secs(2),
    );
    assert_eq!(source(&snapshot, id).entries.len(), 1);
    assert!(!source(&snapshot, id).retention_limited);
}

#[test]
fn oversized_reads_cannot_clear_good_rows_or_exceed_the_text_budget() {
    let at = Instant::now();
    let mut snapshot = Snapshot::default();
    let id = Source::UserRun;
    snapshot.apply(
        vec![read(
            id,
            ReadState::Readable,
            vec![row(id, "Keep", "good.exe")],
        )],
        at,
    );
    snapshot.apply(
        vec![read(
            id,
            ReadState::Readable,
            vec![row(id, "Oversized", &"x".repeat(TEXT_LIMIT + 1))],
        )],
        at,
    );
    let kept = source(&snapshot, id);
    assert_eq!(kept.state(at), State::Cached);
    assert_eq!(kept.entries.len(), 1);
    assert_eq!(kept.entries[0].row.command, "good.exe");
    assert!(kept.retention_limited);
    assert!(
        kept.entries
            .iter()
            .map(|entry| entry.row.text_bytes())
            .sum::<usize>()
            <= TEXT_LIMIT
    );
}

#[test]
fn failed_read_at_the_same_timestamp_cannot_mark_old_entries_as_observed() {
    let at = Instant::now();
    let mut snapshot = Snapshot::default();
    let id = Source::UserRun;
    snapshot.apply(
        vec![read(
            id,
            ReadState::Readable,
            vec![row(id, "Same", "app.exe")],
        )],
        at,
    );
    snapshot.apply(vec![], at);
    let kept = source(&snapshot, id);
    assert_eq!(kept.state(at), State::Cached);
    assert_eq!(kept.row_state(&kept.entries[0], at), "Cached");
}

#[test]
fn flattened_order_is_stable_and_rebuilt_after_authoritative_removals() {
    let at = Instant::now();
    let mut snapshot = Snapshot::default();
    snapshot.apply(
        vec![
            read(
                Source::UserRun,
                ReadState::Readable,
                vec![
                    row(Source::UserRun, "Z", "z"),
                    row(Source::UserRun, "A", "a"),
                ],
            ),
            read(
                Source::MachineRun,
                ReadState::Readable,
                vec![
                    row(Source::MachineRun, "B", "b"),
                    row(Source::MachineRun, "A", "a"),
                ],
            ),
        ],
        at,
    );
    assert_eq!(
        snapshot
            .rows()
            .map(|(s, e)| (s.source, e.row.name.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (Source::UserRun, "A"),
            (Source::MachineRun, "A"),
            (Source::MachineRun, "B"),
            (Source::UserRun, "Z")
        ]
    );
    snapshot.apply(
        vec![read(Source::UserRun, ReadState::Missing, vec![])],
        at + Duration::from_secs(1),
    );
    assert_eq!(
        snapshot
            .rows()
            .map(|(_, e)| e.row.name.as_str())
            .collect::<Vec<_>>(),
        vec!["A", "B"]
    );
}
