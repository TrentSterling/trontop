use super::*;

struct Fixture {
    path: String,
    run: Address,
    approval: Address,
}
impl Fixture {
    fn new() -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = format!(
            r"Software\Trontop\Tests\Startup\{}-{unique}",
            std::process::id()
        );
        let address = Address {
            root: HKEY_CURRENT_USER,
            path: path.clone(),
            view: KEY_WOW64_64KEY,
        };
        let name = wide(&path);
        let mut key = HKEY::default();
        let mut disposition = REG_CREATE_KEY_DISPOSITION::default();
        let status = unsafe {
            RegCreateKeyExW(
                address.root,
                PCWSTR(name.as_ptr()),
                None,
                windows::core::PWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_READ | KEY_WRITE | address.view,
                None,
                &mut key,
                Some(&mut disposition),
            )
        };
        assert_eq!(
            status, ERROR_SUCCESS,
            "Creating an isolated owned test key failed"
        );
        let key = RegistryKey(key);
        assert_eq!(
            disposition, REG_CREATED_NEW_KEY,
            "Fixture collision; existing key is never reused"
        );
        drop(key);
        let fixture = Self {
            run: Address {
                root: HKEY_CURRENT_USER,
                path: format!(r"{path}\Run"),
                view: KEY_WOW64_64KEY,
            },
            approval: Address {
                root: HKEY_CURRENT_USER,
                path: format!(r"{path}\Approved"),
                view: KEY_WOW64_64KEY,
            },
            path,
        };
        let value = RawValue {
            kind: REG_EXPAND_SZ.0,
            bytes: r"%TEMP%\fixture.exe --测试"
                .encode_utf16()
                .chain(Some(0))
                .flat_map(u16::to_le_bytes)
                .collect(),
        };
        let tx = Transaction::new().unwrap();
        let run = RegistryKey::open(
            &fixture.run,
            KEY_QUERY_VALUE | KEY_SET_VALUE,
            Some(&tx),
            true,
        )
        .unwrap()
        .unwrap();
        run.write_approval("OwnedFixture", &Approval::Value(value))
            .unwrap();
        tx.commit().unwrap();
        fixture
    }
    fn request(&self, action: Action) -> Request {
        let key = RegistryKey::open(&self.run, KEY_QUERY_VALUE, None, false)
            .unwrap()
            .unwrap();
        let raw = key.read("OwnedFixture", VALUE_LIMIT).unwrap().unwrap();
        Request::new(
            StartupRow {
                key: "OwnedFixture".into(),
                name: "Owned isolated Startup fixture".into(),
                command: decode_run(&raw).unwrap(),
                source: Source::UserRun,
                control: Some(Control {
                    registration: Some(Registration::Run(raw)),
                    approval: approval_at(&self.approval, "OwnedFixture"),
                }),
            },
            action,
        )
    }
    fn apply(&self, request: &Request) -> Result<Receipt, Failure> {
        apply_at(request, &self.approval, Some(&self.run), None)
    }
    fn registration(&self) -> RawValue {
        RegistryKey::open(&self.run, KEY_QUERY_VALUE, None, false)
            .unwrap()
            .unwrap()
            .read("OwnedFixture", VALUE_LIMIT)
            .unwrap()
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // Fixed children of the exclusively created leaf; no recursive deletion.
        assert!(self.path.starts_with(r"Software\Trontop\Tests\Startup\"));
        for path in [&self.approval.path, &self.run.path, &self.path] {
            let path = wide(path);
            unsafe {
                let _ = RegDeleteKeyExW(
                    HKEY_CURRENT_USER,
                    PCWSTR(path.as_ptr()),
                    KEY_WOW64_64KEY.0,
                    None,
                );
            }
        }
    }
}

#[test]
fn owned_registry_disable_enable_and_exact_undo_preserve_the_original_command() {
    let fixture = Fixture::new();
    let original = fixture.registration();
    let receipt = fixture.apply(&fixture.request(Action::Disable)).unwrap();
    assert_eq!(receipt.before, Approval::Missing);
    assert_eq!(receipt.after.state(), State::Disabled);
    assert_eq!(
        approval_at(&fixture.approval, "OwnedFixture"),
        receipt.after
    );
    assert_eq!(fixture.registration(), original);
    fixture.apply(&receipt.undo()).unwrap();
    assert_eq!(
        approval_at(&fixture.approval, "OwnedFixture"),
        Approval::Missing
    );
    let disabled = fixture.apply(&fixture.request(Action::Disable)).unwrap();
    let enabled = fixture.apply(&fixture.request(Action::Enable)).unwrap();
    assert_eq!(enabled.after.state(), State::Enabled);
    fixture.apply(&enabled.undo()).unwrap();
    assert_eq!(
        approval_at(&fixture.approval, "OwnedFixture"),
        disabled.after
    );
    assert_eq!(fixture.registration(), original);
    println!(
        "OWNED_STARTUP: atomic disable/enable, exact byte/absence undo, command preserved PASS"
    );
}

#[test]
fn stale_approval_and_changed_command_abort_without_creating_or_overwriting_approval() {
    let fixture = Fixture::new();
    let stale = fixture.request(Action::Disable);
    let changed = fixture.apply(&fixture.request(Action::Disable)).unwrap();
    assert!(
        fixture
            .apply(&stale)
            .unwrap_err()
            .message
            .contains("another application")
    );
    assert_eq!(
        approval_at(&fixture.approval, "OwnedFixture"),
        changed.after
    );
    fixture.apply(&changed.undo()).unwrap();
    let mut stale = fixture.request(Action::Disable);
    stale.target.command = "a different program".into();
    assert!(
        fixture
            .apply(&stale)
            .unwrap_err()
            .message
            .contains("registration changed")
    );
    assert_eq!(
        approval_at(&fixture.approval, "OwnedFixture"),
        Approval::Missing
    );
}

#[test]
fn transaction_drop_discards_uncommitted_writes_and_new_keys() {
    let fixture = Fixture::new();
    {
        let tx = Transaction::new().unwrap();
        let key = RegistryKey::open(
            &fixture.approval,
            KEY_QUERY_VALUE | KEY_SET_VALUE,
            Some(&tx),
            true,
        )
        .unwrap()
        .unwrap();
        key.write_approval(
            "OwnedFixture",
            &Approval::Missing.changed(false, 123).unwrap(),
        )
        .unwrap();
    }
    assert_eq!(
        approval_at(&fixture.approval, "OwnedFixture"),
        Approval::Missing
    );
    assert!(
        RegistryKey::open(&fixture.approval, KEY_QUERY_VALUE, None, false)
            .unwrap()
            .is_none()
    );
}

struct OwnedFolder {
    path: PathBuf,
    file: PathBuf,
}
impl OwnedFolder {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let parent = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/test-startup-fixtures");
        std::fs::create_dir_all(&parent).unwrap();
        let path = parent.join(format!("{}-{nonce}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        let file = path.join("OwnedFixture.lnk");
        use std::io::Write;
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&file)
            .unwrap()
            .write_all(b"owned fixture, never executed")
            .unwrap();
        Self { path, file }
    }
    fn request(&self, fixture: &Fixture) -> Request {
        Request::new(
            StartupRow {
                key: "OwnedFixture.lnk".into(),
                name: "Owned folder fixture".into(),
                command: self.file.display().to_string(),
                source: Source::UserFolder,
                control: Some(Control {
                    registration: Some(Registration::File(
                        file_identity(&open_file(&self.file, false).unwrap()).unwrap(),
                    )),
                    approval: approval_at(&fixture.approval, "OwnedFixture.lnk"),
                }),
            },
            Action::Disable,
        )
    }
}
impl Drop for OwnedFolder {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.file);
        let _ = std::fs::remove_dir(&self.path);
    }
}
#[test]
fn owned_folder_disable_undo_preserves_file_and_rejects_replacement_missing_and_wrong_path() {
    let fixture = Fixture::new();
    let folder = OwnedFolder::new();
    let original = std::fs::read(&folder.file).unwrap();
    let request = folder.request(&fixture);
    let receipt = apply_at(&request, &fixture.approval, None, Some(folder.path.clone())).unwrap();
    assert_eq!(receipt.after.state(), State::Disabled);
    apply_at(
        &receipt.undo(),
        &fixture.approval,
        None,
        Some(folder.path.clone()),
    )
    .unwrap();
    assert_eq!(
        approval_at(&fixture.approval, "OwnedFixture.lnk"),
        Approval::Missing
    );
    assert_eq!(std::fs::read(&folder.file).unwrap(), original);
    let mut wrong = folder.request(&fixture);
    wrong.target.command = folder.path.join("another.lnk").display().to_string();
    assert!(
        !apply_at(&wrong, &fixture.approval, None, Some(folder.path.clone()))
            .unwrap_err()
            .uncertain
    );
    std::fs::write(&folder.file, b"different owned fixture").unwrap();
    assert!(
        apply_at(&request, &fixture.approval, None, Some(folder.path.clone()))
            .unwrap_err()
            .message
            .contains("replaced or changed")
    );
    std::fs::remove_file(&folder.file).unwrap();
    assert!(apply_at(&request, &fixture.approval, None, Some(folder.path.clone())).is_err());
    assert_eq!(
        approval_at(&fixture.approval, "OwnedFixture.lnk"),
        Approval::Missing
    );
    println!(
        "OWNED_STARTUP_FOLDER: disable, exact undo, byte preservation, replacement/missing/path refusal PASS"
    );
}
#[test]
fn retained_file_handle_prevents_concurrent_write_and_replacement_and_directories_refuse() {
    let folder = OwnedFolder::new();
    let guard = open_file(&folder.file, true).unwrap();
    assert!(std::fs::write(&folder.file, b"cannot change held fixture").is_err());
    assert!(std::fs::remove_file(&folder.file).is_err());
    drop(guard);
    assert!(
        open_file(&folder.path, false)
            .and_then(|file| file_identity(&file))
            .is_err()
    );
    std::fs::write(&folder.file, b"guard released").unwrap();
}
#[test]
fn malformed_and_oversized_values_changed_registration_and_unknown_approval_refuse() {
    assert_eq!(
        decode_run(&RawValue {
            kind: 3,
            bytes: vec![0, 0]
        }),
        None
    );
    assert_eq!(
        decode_run(&RawValue {
            kind: 1,
            bytes: vec![1]
        }),
        None
    );
    assert_eq!(
        decode_run(&RawValue {
            kind: 1,
            bytes: vec![0, 0xd8]
        }),
        None
    );
    let fixture = Fixture::new();
    let request = fixture.request(Action::Disable);
    let key = RegistryKey::open(&fixture.run, KEY_QUERY_VALUE | KEY_SET_VALUE, None, false)
        .unwrap()
        .unwrap();
    let replacement = RawValue {
        kind: 1,
        bytes: "replacement.exe"
            .encode_utf16()
            .chain(Some(0))
            .flat_map(u16::to_le_bytes)
            .collect(),
    };
    key.write_approval("OwnedFixture", &Approval::Value(replacement))
        .unwrap();
    assert!(
        fixture
            .apply(&request)
            .unwrap_err()
            .message
            .contains("registration changed")
    );
    assert_eq!(
        approval_at(&fixture.approval, "OwnedFixture"),
        Approval::Missing
    );
    key.write_approval(
        "Oversized",
        &Approval::Value(RawValue {
            kind: 3,
            bytes: vec![0; APPROVAL_LIMIT + 1],
        }),
    )
    .unwrap();
    assert!(key.read("Oversized", APPROVAL_LIMIT).is_err());
    let mut invalid = fixture.request(Action::Disable);
    invalid.target.control.as_mut().unwrap().approval = Approval::Value(RawValue {
        kind: 3,
        bytes: vec![9; 12],
    });
    assert!(fixture.apply(&invalid).is_err());
}

#[test]
fn concurrent_registration_edit_is_refused_or_aborts_the_pending_approval_transaction() {
    let fixture = Fixture::new();
    let tx = Transaction::new().unwrap();
    let original = {
        let key = RegistryKey::open(
            &fixture.run,
            KEY_QUERY_VALUE | KEY_SET_VALUE,
            Some(&tx),
            false,
        )
        .unwrap()
        .unwrap();
        let original = key.read("OwnedFixture", VALUE_LIMIT).unwrap().unwrap();
        key.write_approval("OwnedFixture", &Approval::Value(original.clone()))
            .unwrap();
        original
    };
    let approval = RegistryKey::open(
        &fixture.approval,
        KEY_QUERY_VALUE | KEY_SET_VALUE,
        Some(&tx),
        true,
    )
    .unwrap()
    .unwrap();
    approval
        .write_approval(
            "OwnedFixture",
            &Approval::Missing.changed(false, 123).unwrap(),
        )
        .unwrap();
    let key = RegistryKey::open(&fixture.run, KEY_QUERY_VALUE | KEY_SET_VALUE, None, false)
        .unwrap()
        .unwrap();
    let edit = key.write_approval(
        "OwnedFixture",
        &Approval::Value(RawValue {
            kind: 1,
            bytes: "concurrent.exe"
                .encode_utf16()
                .chain(Some(0))
                .flat_map(u16::to_le_bytes)
                .collect(),
        }),
    );
    let commit = tx.commit();
    assert!(
        edit.is_err() || commit.is_err(),
        "A changed registration must never accompany a committed stale approval"
    );
    if edit.is_err() {
        assert_eq!(fixture.registration(), original);
    }
    if commit.is_err() {
        drop(approval);
        drop(tx);
        assert_eq!(
            approval_at(&fixture.approval, "OwnedFixture"),
            Approval::Missing
        );
    }
}

#[test]
fn registration_edit_before_write_guard_cannot_overwrite_newer_committed_command() {
    let fixture = Fixture::new();
    let request = fixture.request(Action::Disable);
    let tx = Transaction::new().unwrap();
    let reading = RegistryKey::open(&fixture.run, KEY_QUERY_VALUE, Some(&tx), false)
        .unwrap()
        .unwrap();
    assert_eq!(
        reading.read("OwnedFixture", VALUE_LIMIT).unwrap().unwrap(),
        fixture.registration()
    );
    let replacement = RawValue {
        kind: 1,
        bytes: "newer.exe"
            .encode_utf16()
            .chain(Some(0))
            .flat_map(u16::to_le_bytes)
            .collect(),
    };
    RegistryKey::open(&fixture.run, KEY_SET_VALUE, None, false)
        .unwrap()
        .unwrap()
        .write_approval("OwnedFixture", &Approval::Value(replacement.clone()))
        .unwrap();
    assert!(
        guard_run(
            &fixture.run,
            &tx,
            &request.target,
            request.target.control.as_ref().unwrap()
        )
        .is_err()
    );
    drop(reading);
    drop(tx);
    assert_eq!(fixture.registration(), replacement);
    assert_eq!(
        approval_at(&fixture.approval, "OwnedFixture"),
        Approval::Missing
    );
}
