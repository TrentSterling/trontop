use super::*;
use std::fs;
use std::sync::atomic::AtomicUsize;

pub(super) struct Fixture(pub(super) PathBuf);
impl Fixture {
    pub(super) fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!(
                "toolbox-fixture-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    pub(super) fn file(&self, name: &str, bytes: usize) -> PathBuf {
        let p = self.0.join(name);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, vec![7u8; bytes]).unwrap();
        p
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[cfg(windows)]
#[test]
fn duplicate_removal_pins_the_survivor_and_compares_full_contents_before_deletion() {
    use std::os::windows::fs::OpenOptionsExt;
    let f = Fixture::new();
    let keep = fs::canonicalize(f.file("keep.bin", 65537)).unwrap();
    let remove = fs::canonicalize(f.file("remove.bin", 65537)).unwrap();
    let review = || cleaner::CopyReview::capture(keep.clone(), remove.clone(), 65537).unwrap();
    let plan = review();
    let locked = fs::File::options()
        .read(true)
        .share_mode(0)
        .open(&keep)
        .unwrap();
    assert!(plan.apply().is_err());
    drop(locked);
    let mut changed = vec![7u8; 65537];
    changed[65536] = 8;
    fs::write(&remove, &changed).unwrap();
    assert!(review().apply().is_err() && remove.exists());
    fs::copy(&keep, &remove).unwrap();
    let stale = review();
    let old = f.0.join("renamed.bin");
    fs::rename(&remove, &old).unwrap();
    fs::copy(&keep, &remove).unwrap();
    assert!(stale.apply().is_err() && remove.exists());
    review().apply().unwrap();
    assert!(keep.exists() && old.exists() && !remove.exists());
}

#[test]
fn duplicate_compare_checks_contents_past_the_header_and_excludes_hard_links() {
    use std::io::{Seek, Write};
    let f = Fixture::new();
    let bytes = 1024 * 1024;
    f.file("one.bin", bytes);
    f.file("two.bin", bytes);
    let different = f.file("same-prefix.bin", bytes);
    let mut writer = fs::File::options().write(true).open(different).unwrap();
    writer.seek(std::io::SeekFrom::End(-1)).unwrap();
    writer.write_all(&[99]).unwrap();
    drop(writer);
    let linked = f.file("linked.bin", bytes);
    fs::hard_link(linked, f.0.join("other-link.bin")).unwrap();
    let stop = AtomicBool::default();
    let count = AtomicU64::default();
    let scan = scan::run(&f.0, &stop, &count).unwrap();
    assert_eq!(scan.types[0], ("bin".into(), (5 * bytes as u64, 5)));
    let report = duplicates::find(&scan, &stop, &count).unwrap();
    assert_eq!(report.groups.len(), 1);
    #[cfg(windows)]
    {
        let mut paths: Vec<_> = report.groups[0].1.iter().map(|p| filename(p)).collect();
        paths.sort();
        assert_eq!(paths, ["one.bin", "two.bin"]);
        assert_eq!(report.skipped, 2);
    }
}

#[test]
fn scan_measures_project_outputs_and_compares_two_completed_scans() {
    let f = Fixture::new();
    f.file("game/ProjectSettings/ProjectVersion.txt", 1);
    f.file("game/Library/import.bin", 8192);
    f.file("other/Library/source", 120);
    f.file("crate/Cargo.toml", 2);
    f.file("crate/target/build.exe", 2048);
    f.file("site/package.json", 2);
    f.file("site/node_modules/package/index.js", 32);
    let deleted = f.file("removed", 64);
    let run = || scan::run(&f.0, &AtomicBool::default(), &AtomicU64::default()).unwrap();
    let a = run();
    assert_eq!((a.files, a.bytes, a.skipped), (8, 10461, 0));
    assert_eq!(
        a.projects
            .iter()
            .map(|p| (p.kind, p.bytes))
            .collect::<Vec<_>>(),
        [("Unity", 8192), ("Rust", 2048), ("Node", 32)]
    );
    assert_eq!(a.largest[0].0, 8192);
    fs::remove_file(deleted).unwrap();
    f.file("game/Library/new", 999);
    let b = run();
    let delta = scan::changes(&a, &b);
    assert_eq!(
        delta
            .iter()
            .map(|(p, n)| (filename(p), *n))
            .collect::<Vec<_>>(),
        [("game".into(), 999), ("removed".into(), -64)]
    );
    assert_eq!(
        scan::run(&f.0, &AtomicBool::new(true), &AtomicU64::default())
            .unwrap_err()
            .kind(),
        io::ErrorKind::Interrupted
    );
}

#[test]
#[cfg(windows)]
fn cleanup_deletes_only_unchanged_selected_files_from_preview() {
    use std::os::windows::fs::OpenOptionsExt;
    let f = Fixture::new();
    let old = SystemTime::now() - Duration::from_secs(35 * 86400);
    for name in [
        "delete.tmp",
        "keep.tmp",
        "changed.tmp",
        "busy.tmp",
        "replaced.tmp",
        "hardlink.tmp",
        "keep.txt",
        "nested/nested.tmp",
        "recent.tmp",
        "crashes/game.dmp",
    ] {
        let p = f.file(name, 17);
        if name != "recent.tmp" {
            fs::OpenOptions::new()
                .write(true)
                .open(p)
                .unwrap()
                .set_modified(old)
                .unwrap();
        }
    }
    fs::hard_link(f.0.join("hardlink.tmp"), f.0.join("alias.txt")).unwrap();
    let mut preview = cleaner::inspect(&[
        (f.0.clone(), cleaner::Rule::Temp),
        (f.0.join("crashes"), cleaner::Rule::Dump),
    ]);
    assert_eq!(preview.files.len(), 6);
    preview.files.retain(|c| filename(&c.path) != "keep.tmp");
    fs::write(f.0.join("changed.tmp"), b"new content").unwrap();
    fs::rename(f.0.join("replaced.tmp"), f.0.join("old-name")).unwrap();
    let replaced = f.file("replaced.tmp", 17);
    fs::OpenOptions::new()
        .write(true)
        .open(replaced)
        .unwrap()
        .set_modified(old)
        .unwrap();
    let _held = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(f.0.join("busy.tmp"))
        .unwrap();
    let receipt = cleaner::apply(preview.files);
    assert_eq!(
        (receipt.deleted, receipt.bytes, receipt.failures.len()),
        (2, 34, 3),
        "{:?}",
        receipt.failures
    );
    assert!(!f.0.join("delete.tmp").exists());
    assert!(!f.0.join("crashes/game.dmp").exists());
    for name in [
        "keep.tmp",
        "changed.tmp",
        "busy.tmp",
        "replaced.tmp",
        "hardlink.tmp",
        "keep.txt",
        "nested/nested.tmp",
        "recent.tmp",
    ] {
        assert!(f.0.join(name).is_file());
    }
}

#[test]
fn drive_watch_detects_mount_changes_and_ignores_free_space_and_order() {
    let a = DiskRow {
        mount: "C:\\".into(),
        name: "System".into(),
        ..Default::default()
    };
    let b = DiskRow {
        mount: "F:\\".into(),
        name: "External".into(),
        ..Default::default()
    };
    let mut toolbox = Toolbox::default();
    toolbox.observe_volumes(std::slice::from_ref(&a), &[a.clone(), b.clone()]);
    toolbox.observe_volumes(
        &[b.clone(), a.clone()],
        &[
            DiskRow {
                available_bytes: 100,
                ..a.clone()
            },
            b.clone(),
        ],
    );
    toolbox.observe_volumes(&[a.clone(), b], &[a]);
    assert_eq!(toolbox.events.len(), 2);
    assert!(!toolbox.events[0].added && toolbox.events[1].added);
}

#[cfg(windows)]
#[test]
fn cleanup_catalog_limits_profiles_payload_names_and_ages_and_keeps_other_data() {
    use cleaner::Rule;
    let f = Fixture::new();
    let local = f.0.join("local");
    let system = f.0.join("windows");
    let payload = "0123456789abcdef_0";
    let fx_hash = "0123456789ABCDEF0123456789ABCDEF01234567";
    let old = SystemTime::now() - Duration::from_secs(40 * 86400);
    let eligible = [
        ("local/Temp/old.tmp".to_owned(), Rule::Temp),
        ("windows/Temp/old.TMP".into(), Rule::WindowsTemp),
        ("local/CrashDumps/app.dmp".into(), Rule::Dump),
        (
            format!("local/Google/Chrome/User Data/Profile 3/Code Cache/js/{payload}"),
            Rule::Chrome,
        ),
        (
            format!("local/Microsoft/Edge/User Data/Default/Cache/Cache_Data/{payload}"),
            Rule::Edge,
        ),
        (
            format!(
                "local/BraveSoftware/Brave-Browser/User Data/Default/Code Cache/wasm/{payload}"
            ),
            Rule::Brave,
        ),
        (
            format!("local/Mozilla/Firefox/Profiles/fixture.default/cache2/entries/{fx_hash}"),
            Rule::Firefox,
        ),
        (
            "local/NuGet/v3-cache/feed-a/nupkg_fixture.dat".into(),
            Rule::NuGet,
        ),
    ];
    let protected = [
        "local/Google/Chrome/User Data/Profile 3/Network/Cookies".into(),
        "local/Google/Chrome/User Data/Profile 3/Login Data".into(),
        "local/Google/Chrome/User Data/Profile 3/History".into(),
        "local/Google/Chrome/User Data/Profile 3/Code Cache/js/index".into(),
        format!("local/Google/Chrome/User Data/MyFiles/Code Cache/js/{payload}"),
        format!("local/Mozilla/Firefox/Profiles/fixture.default/storage/default/{fx_hash}"),
        "local/Microsoft/Edge/User Data/Default/Cache/Cache_Data/data_0".into(),
        "local/NuGet/v3-cache/feed-a/local.nupkg".into(),
        "local/NuGet/packages/durable.dat".into(),
        "windows/Temp/nested/other.tmp".into(),
    ];
    for name in eligible.iter().map(|(p, _)| p).chain(&protected) {
        let p = f.file(name, 23);
        fs::File::options()
            .write(true)
            .open(p)
            .unwrap()
            .set_modified(old)
            .unwrap();
    }
    let recent = f.file(
        "local/Google/Chrome/User Data/Profile 3/Code Cache/js/1123456789abcdef_0",
        17,
    );
    let count = AtomicU64::default();
    let preview =
        cleaner::analyze(&local, None, Some(&system), &AtomicBool::default(), &count).unwrap();
    assert_eq!(
        (preview.files.len(), preview.bytes, preview.groups.len()),
        (8, 184, 8),
        "{:?}",
        preview.issues
    );
    for (name, rule) in &eligible {
        let group = preview.groups.iter().find(|g| g.rule == *rule).unwrap();
        assert_eq!(group.bytes, 23);
        assert_eq!(
            preview.files[group.range.start].path,
            fs::canonicalize(f.0.join(name)).unwrap()
        );
    }
    let receipt =
        cleaner::apply_with_processes(preview.files, &HashSet::from(["chrome.exe".into()]));
    assert_eq!(
        (receipt.deleted, receipt.bytes, receipt.failures.len()),
        (7, 161, 1)
    );
    assert!(f.0.join(&eligible[3].0).exists() && recent.exists());
    assert!(protected.iter().all(|p| f.0.join(p).exists()));
    assert!(
        matches!(cleaner::analyze(&local,None,None,&AtomicBool::new(true),&count),Err(e) if e.kind()==io::ErrorKind::Interrupted)
    );
}

#[cfg(windows)]
#[test]
fn cleaner_skips_cache_directory_redirected_through_an_ancestor_junction() {
    let f = Fixture::new();
    let outside = f.file("other/Cache_Data/1123456789abcdef_0", 20);
    fs::File::options()
        .write(true)
        .open(&outside)
        .unwrap()
        .set_modified(SystemTime::now() - Duration::from_secs(10 * 86400))
        .unwrap();
    let profile = f.0.join("Google/Chrome/User Data/Default");
    fs::create_dir_all(&profile).unwrap();
    assert!(
        std::process::Command::new("cmd.exe")
            .args(["/d", "/c", "mklink", "/J"])
            .arg(profile.join("Cache"))
            .arg(f.0.join("other"))
            .output()
            .unwrap()
            .status
            .success()
    );
    let preview = cleaner::analyze(
        &f.0,
        None,
        None,
        &AtomicBool::default(),
        &AtomicU64::default(),
    )
    .unwrap();
    assert!(preview.files.is_empty() && !preview.issues.is_empty() && outside.exists());
}

#[cfg(windows)]
#[test]
fn desktop_and_developer_cleanup_obey_paths_filenames_age_and_running_owner() {
    let f = Fixture::new();
    let payload = "0123456789abcdef_0";
    let wheel = format!(
        "local/pip/cache/wheels/ab/cd/ef/{}/package-1.0-py3-none-any.whl",
        "a".repeat(50)
    );
    let archive = format!(
        "local/electron/Cache/{}/electron-v44.5.0-win32-x64.zip",
        "a".repeat(64)
    );
    let eligible = [
        format!("roaming/Code/Code Cache/js/{payload}"),
        format!("roaming/discord/Cache/Cache_Data/{payload}"),
        format!("local/Steam/htmlcache/Default/Cache/Cache_Data/{payload}"),
        "local/npm-cache/_logs/2026-05-02T09_10_11_123Z-debug-0.log".into(),
        "roaming/obs-studio/logs/2026-05-02 09-10-11.txt".into(),
        wheel.clone(),
        archive.clone(),
    ];
    let protected = [
        format!("roaming/Code/User/workspaceStorage/{payload}"),
        format!("roaming/discord/Local Storage/{payload}"),
        "roaming/Code/Cache/Cache_Data/index".into(),
        "local/npm-cache/_logs/private.log".into(),
        "roaming/obs-studio/logs/notes.txt".into(),
        wheel.replace("ab/cd/ef", "user/project/vendor"),
        archive.replace("electron-v44.5.0-win32-x64.zip", "project.zip"),
        "roaming/Code/User/settings.json".into(),
    ];
    for path in eligible.iter().chain(&protected) {
        let path = f.file(path, 11);
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(SystemTime::now() - Duration::from_secs(40 * 86400))
            .unwrap();
    }
    let fresh = f.file("roaming/obs-studio/logs/2026-05-03 09-10-11.txt", 17);
    let preview = cleaner::analyze(
        &f.0.join("local"),
        Some(&f.0.join("roaming")),
        None,
        &AtomicBool::default(),
        &AtomicU64::default(),
    )
    .unwrap();
    assert_eq!(
        (preview.files.len(), preview.groups.len(), preview.bytes),
        (7, 7, 77)
    );
    let receipt =
        cleaner::apply_with_processes(preview.files, &HashSet::from(["discord.exe".into()]));
    assert_eq!((receipt.deleted, receipt.failures.len()), (6, 1));
    assert!(f.0.join(&eligible[1]).exists() && fresh.exists());
    assert!(protected.iter().all(|p| f.0.join(p).is_file()));
}

#[cfg(windows)]
#[test]
#[ignore = "read-only preview of the current Windows user's cache categories"]
fn live_cleaner_preview_reports_only_aggregate_counts() {
    let preview = cleaner::preview(&AtomicBool::default(), &AtomicU64::default()).unwrap();
    for group in preview.groups {
        println!(
            "{}: {} files, {} bytes",
            group.rule.label(),
            group.range.len(),
            group.bytes
        );
    }
    println!(
        "{} eligible files, {} logical bytes, {} skipped files, {} directory issues",
        preview.files.len(),
        preview.bytes,
        preview.skipped,
        preview.issues.len()
    );
    assert!(cleaner::disk_cleanup_program().unwrap().is_file());
}

#[cfg(windows)]
#[test]
fn cleaner_review_cancel_and_confirmation_use_the_visible_selection() {
    use crate::app::ui_smoke::{app, click_local_text, frame};
    let fixture = Fixture::new();
    let path = fixture.file("owned.tmp", 32);
    let keep = fixture.file("CrashDumps/owned.dmp", 20);
    fs::File::options()
        .write(true)
        .open(&keep)
        .unwrap()
        .set_modified(SystemTime::now() - Duration::from_secs(35 * 86400))
        .unwrap();
    fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(SystemTime::now() - Duration::from_secs(10 * 86400))
        .unwrap();
    let settings = ThemeSettings::default();
    let ctx = egui::Context::default();
    theme::install(&ctx, settings);
    let mut app = app(settings, false);
    app.page = Page::Toolbox;
    app.toolbox.tool = Tool::Cleaner;
    app.toolbox.preview = Some(cleaner::inspect(&[
        (fixture.0.clone(), cleaner::Rule::Temp),
        (fixture.0.join("CrashDumps"), cleaner::Rule::Dump),
    ]));
    let size = Vec2::new(1000.0, 580.0);
    click_local_text(&ctx, &mut app, size, "Select all");
    assert_eq!(app.toolbox.selected.len(), 2);
    click_local_text(&ctx, &mut app, size, "Clear");
    click_local_text(&ctx, &mut app, size, "All categories");
    click_local_text(&ctx, &mut app, size, "User temporary files   1 file   32 B");
    click_local_text(&ctx, &mut app, size, "Select category");
    click_local_text(&ctx, &mut app, size, "Review 1 file (32 B)");
    assert!(app.toolbox.review.is_some() && path.exists());
    click_local_text(&ctx, &mut app, size, "Keep files");
    assert!(app.toolbox.review.is_none() && path.exists());
    click_local_text(&ctx, &mut app, size, "Review 1 file (32 B)");
    click_local_text(&ctx, &mut app, size, "Permanently delete selected");
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while app.toolbox.receipt.is_none() {
        frame(&ctx, &mut app, size, vec![]);
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(app.toolbox.receipt.unwrap().deleted, 1);
    assert!(!path.exists());
    assert!(keep.exists());
}

#[test]
#[cfg(windows)]
#[ignore = "uses a real Windows Restart Manager session and a disposable held file"]
fn restart_manager_finds_this_process_holding_a_fixture() {
    let f = Fixture::new();
    let path = f.file("lock.bin", 4);
    let _held = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    for input in [&path, &fs::canonicalize(&path).unwrap()] {
        let rows = file_users::query(input).unwrap();
        assert!(
            rows.iter()
                .any(|r| r.identity.pid == std::process::id() && r.identity.created_at_100ns > 0),
            "No holder for {}",
            input.display()
        );
    }
}
