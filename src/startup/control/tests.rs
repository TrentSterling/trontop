use super::*;

fn approval(state: u32, at: u64) -> Approval {
    Approval::Value(RawValue {
        kind: 3,
        bytes: state
            .to_le_bytes()
            .into_iter()
            .chain(at.to_le_bytes())
            .collect(),
    })
}

#[test]
fn approval_decodes_only_known_complete_records_and_preserves_variant_when_toggled() {
    assert_eq!(Approval::Missing.state(), State::Enabled);
    assert_eq!(
        Approval::Unreadable("access denied".into()).state(),
        State::Unavailable
    );
    for (code, enabled) in [(2, true), (3, false), (6, true), (7, false)] {
        let record = approval(code, 123);
        assert_eq!(
            record.state(),
            if enabled {
                State::Enabled
            } else {
                State::Disabled
            }
        );
        let changed = record.changed(!enabled, 456).unwrap();
        assert_eq!(changed, approval(code ^ 1, if enabled { 456 } else { 0 }));
    }
    for record in [
        approval(8, 0),
        approval(0x102, 0),
        Approval::Value(RawValue {
            kind: 1,
            bytes: vec![0; 12],
        }),
        Approval::Value(RawValue {
            kind: 3,
            bytes: vec![2],
        }),
    ] {
        assert_eq!(record.state(), State::Unsupported);
        assert!(record.changed(true, 123).is_err());
    }
    assert_eq!(
        Approval::Missing.changed(false, 456).unwrap(),
        approval(3, 456)
    );
}

#[test]
fn malformed_names_registration_kinds_expired_requests_and_unknown_undo_fail_closed() {
    let row = StartupRow {
        key: "Fixture".into(),
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
    };
    assert!(
        Request::new(row.clone(), Action::Disable)
            .validate()
            .is_ok()
    );
    for key in ["", "bad\0key"] {
        let mut bad = row.clone();
        bad.key = key.into();
        assert!(Request::new(bad, Action::Enable).validate().is_err());
    }
    let mut bad = row.clone();
    bad.control = None;
    assert!(Request::new(bad, Action::Enable).validate().is_err());
    let mut bad = row.clone();
    bad.source = Source::UserFolder;
    assert!(Request::new(bad, Action::Enable).validate().is_err());
    let mut expired = Request::new(row.clone(), Action::Disable);
    expired.confirmed_at = Instant::now() - Duration::from_secs(31);
    assert!(expired.validate().unwrap_err().contains("expired"));
    assert!(
        Request::new(row, Action::Restore(approval(9, 0)))
            .validate()
            .is_err()
    );
}

#[test]
fn undo_restores_exact_prior_bytes_or_absence_and_retains_the_registration_identity() {
    let before = approval(7, 987);
    let after = before.changed(true, 123).unwrap();
    let target = StartupRow {
        key: "FiXtUrE".into(),
        name: "Fixture".into(),
        command: "fixture.exe".into(),
        source: Source::UserRun,
        control: Some(Control {
            registration: Some(Registration::Run(RawValue {
                kind: 2,
                bytes: vec![42, 0],
            })),
            approval: before.clone(),
        }),
    };
    let receipt = Receipt {
        target: target.clone(),
        before: before.clone(),
        after: after.clone(),
        observed_at: Instant::now(),
    };
    let undo = receipt.undo();
    assert_eq!(undo.action, Action::Restore(before));
    assert_eq!(undo.target.control.as_ref().unwrap().approval, after);
    assert_eq!(
        undo.target.control.as_ref().unwrap().registration,
        target.control.as_ref().unwrap().registration
    );
    assert_eq!(Key::of(&target).name, "fixture");
}
