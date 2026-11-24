use super::*;
use crate::specs::native::wmi;
use std::os::windows::process::CommandExt;
use std::process::Command;
use windows::Win32::Storage::FileSystem::{GetDriveTypeW, GetVolumeInformationW};
use windows::Win32::System::Threading::{CREATE_NEW_CONSOLE, CREATE_NO_WINDOW};
use windows::Win32::System::WindowsProgramming::DRIVE_FIXED;
use windows::core::PCWSTR;
mod extensions;
pub(super) mod registry;
mod view;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

pub(super) fn standard_user() -> io::Result<()> {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Security::{GetTokenInformation, TOKEN_ELEVATION, TokenElevation};
    let mut token = TOKEN_ELEVATION::default();
    let mut len = 0;
    // GetCurrentProcessToken pseudo-handle. The process owns it for its lifetime.
    unsafe {
        GetTokenInformation(
            HANDLE(-4isize as _),
            TokenElevation,
            Some((&raw mut token).cast()),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        )?;
    }
    if token.TokenIsElevated != 0 {
        return Err(io::Error::other(
            "Open Trontop as a standard user for this action.",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy)]
pub(super) enum Destination {
    Apps,
    Schedule,
    Drivers,
    Store,
    Bin,
    Protection,
    Restore,
    Optimize,
    Disks,
    GoogleStorage,
    OneDriveStorage,
}
impl Destination {
    fn command(self) -> (&'static str, Option<&'static str>) {
        match self {
            Self::Apps => ("uri", Some("ms-settings:appsfeatures")),
            Self::Schedule => ("uri", Some("ms-settings:storagepolicies")),
            Self::Drivers => ("uri", Some("ms-settings:windowsupdate-optionalupdates")),
            Self::Store => ("uri", Some("ms-windows-store://downloadsandupdates")),
            Self::Bin => ("explorer.exe", Some("shell:RecycleBinFolder")),
            Self::Protection => ("SystemPropertiesProtection.exe", None),
            Self::Restore => ("rstrui.exe", None),
            Self::Optimize => ("dfrgui.exe", None),
            Self::Disks => ("mmc.exe", Some("diskmgmt.msc")),
            Self::GoogleStorage => ("uri", Some("https://one.google.com/storage/management")),
            Self::OneDriveStorage => ("uri", Some("https://onedrive.live.com/?v=managestorage")),
        }
    }

    fn open(self) -> io::Result<String> {
        let (program, arg) = self.command();
        if program == "uri" {
            use windows::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};
            let uri = wide(arg.unwrap());
            let status = unsafe {
                ShellExecuteW(
                    None,
                    windows::core::w!("open"),
                    PCWSTR(uri.as_ptr()),
                    PCWSTR::null(),
                    PCWSTR::null(),
                    SW_SHOWNORMAL,
                )
            };
            if status.0 as isize <= 32 {
                return Err(io::Error::other(format!(
                    "Windows could not open this page ({})",
                    status.0 as isize
                )));
            }
        } else {
            let sys = cleaner::system_directory()?;
            let base = if program == "explorer.exe" {
                sys.parent()
                    .ok_or_else(|| io::Error::other("Windows directory unavailable"))?
            } else {
                &sys
            };
            let mut command = Command::new(base.join(program));
            if let Some(a) = arg {
                if program == "mmc.exe" {
                    command.arg(sys.join(a));
                } else {
                    command.arg(a);
                }
            }
            command.spawn()?;
        }
        Ok("Opened in Windows.".into())
    }
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub(super) enum Section {
    #[default]
    Health,
    Updates,
    Browsers,
    Registry,
    Recovery,
    Storage,
    Schedule,
    Performance,
    Drivers,
    Cloud,
}
#[derive(Default)]
pub(super) struct ViewState {
    section: Section,
    pub health: Option<Health>,
    pub report: Option<(Section, String)>,
    pub registry: Option<Vec<registry::Entry>>,
    registry_review: Option<registry::Entry>,
    package_id: String,
    update_review: Option<String>,
    pub wipe: Option<Volume>,
    typed: String,
    pub points: Option<Vec<RestorePoint>>,
    point_review: Option<RestorePoint>,
    memory_first: bool,
    process_filter: String,
    sleep_review: Option<(crate::model::ProcessIdentity, String)>,
    pub inspect: Option<crate::model::ProcessIdentity>,
}
impl ViewState {
    pub fn clear_review(&mut self) {
        self.sleep_review = None;
        self.update_review = None;
        self.registry_review = None;
        self.point_review = None;
        self.wipe = None;
        self.typed.clear();
    }
}

#[derive(Default)]
pub(super) struct Health {
    bytes: u64,
    files: usize,
    excluded: usize,
    apps: usize,
    unreadable: usize,
    bin: Option<(u64, u64)>,
}
fn health(stop: &AtomicBool, count: &AtomicU64) -> io::Result<Health> {
    use windows::Win32::UI::Shell::{SHQUERYRBINFO, SHQueryRecycleBinW};
    let preview = cleaner::preview(stop, count)?;
    let apps = apps::inventory()?;
    let mut bin = SHQUERYRBINFO {
        cbSize: size_of::<SHQUERYRBINFO>() as u32,
        ..Default::default()
    };
    let bin = unsafe { SHQueryRecycleBinW(PCWSTR::null(), &mut bin) }
        .ok()
        .and_then(|_| {
            Some((
                bin.i64NumItems.try_into().ok()?,
                bin.i64Size.try_into().ok()?,
            ))
        });
    Ok(Health {
        bytes: preview.bytes,
        files: preview.files.len(),
        excluded: preview.skipped + preview.issues.len(),
        apps: apps.rows.len(),
        unreadable: apps.issues,
        bin,
    })
}

#[derive(Clone, Copy)]
enum Browser {
    Chrome,
    Edge,
    Brave,
    Firefox,
    Vivaldi,
}
impl Browser {
    fn name(self) -> &'static str {
        match self {
            Self::Chrome => "Chrome",
            Self::Edge => "Edge",
            Self::Brave => "Brave",
            Self::Firefox => "Firefox",
            Self::Vivaldi => "Vivaldi",
        }
    }
    fn open(self, extensions: bool) -> io::Result<String> {
        standard_user()?;
        let (relative, page) = match self {
            Self::Vivaldi => (
                r"Vivaldi\Application\vivaldi.exe",
                if extensions {
                    "vivaldi://extensions"
                } else {
                    "vivaldi://settings/clearBrowserData"
                },
            ),
            Self::Chrome => (
                r"Google\Chrome\Application\chrome.exe",
                if extensions {
                    "chrome://extensions"
                } else {
                    "chrome://settings/clearBrowserData"
                },
            ),
            Self::Edge => (
                r"Microsoft\Edge\Application\msedge.exe",
                if extensions {
                    "edge://extensions"
                } else {
                    "edge://settings/clearBrowserData"
                },
            ),
            Self::Brave => (
                r"BraveSoftware\Brave-Browser\Application\brave.exe",
                if extensions {
                    "brave://extensions"
                } else {
                    "brave://settings/clearBrowserData"
                },
            ),
            Self::Firefox => (
                r"Mozilla Firefox\firefox.exe",
                if extensions {
                    "about:addons"
                } else {
                    "-preferences"
                },
            ),
        };
        let path = [
            "ProgramW6432",
            "ProgramFiles",
            "ProgramFiles(x86)",
            "LOCALAPPDATA",
        ]
        .iter()
        .filter_map(std::env::var_os)
        .map(|base| PathBuf::from(base).join(relative))
        .find(|p| p.is_absolute() && p.is_file())
        .ok_or_else(|| {
            io::Error::other(format!(
                "{} was not found in its standard install locations.",
                self.name()
            ))
        })?;
        Command::new(path).arg(page).spawn()?;
        Ok(format!("Opened {} settings.", self.name()))
    }
}

fn winget() -> io::Result<Command> {
    standard_user()?;
    let alias = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .ok_or_else(|| io::Error::other("LocalAppData unavailable"))?
        .join(r"Microsoft\WindowsApps\Microsoft.DesktopAppInstaller_8wekyb3d8bbwe\winget.exe");
    Ok(Command::new(alias))
}
fn available_updates() -> io::Result<String> {
    let output = winget()?
        .args(["upgrade", "--source", "winget", "--disable-interactivity"])
        .creation_flags(CREATE_NO_WINDOW.0)
        .output()?;
    let raw = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let body = raw
        .lines()
        .filter(|s| !s.trim_matches([' ', '\\', '|', '-', '/', '\r']).is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    Ok(format!("WinGet exit {} \n{}", output.status, body.trim()))
}
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 256
        && id.as_bytes()[0].is_ascii_alphanumeric()
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b".-_+".contains(&c))
}
fn upgrade(id: &str) -> io::Result<String> {
    if !valid_id(id) {
        return Err(io::Error::other("Invalid WinGet package ID."));
    }
    let status = winget()?
        .args([
            "upgrade",
            "--id",
            id,
            "--exact",
            "--source",
            "winget",
            "--interactive",
        ])
        .creation_flags(CREATE_NEW_CONSOLE.0)
        .status()?;
    if !status.success() {
        return Err(io::Error::other(format!(
            "WinGet returned {status} for {id}"
        )));
    }
    Ok(format!(
        "WinGet finished updating {id}. Check for updates to refresh the list."
    ))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RestorePoint {
    number: u32,
    time: String,
    description: String,
}
fn restore_points() -> io::Result<Vec<RestorePoint>> {
    let rows = wmi::query_once(
        r"ROOT\DEFAULT",
        "SELECT SequenceNumber, CreationTime, Description FROM SystemRestore",
        Duration::from_secs(10),
    )
    .map_err(io::Error::other)?;
    let mut points = Vec::new();
    for row in rows {
        let number = row
            .u64("SequenceNumber")
            .and_then(|n| u32::try_from(n).ok())
            .ok_or_else(|| io::Error::other("Missing restore point number"))?;
        points.push(RestorePoint {
            number,
            time: row.text("CreationTime").unwrap_or_default(),
            description: row.text("Description").unwrap_or_default(),
        });
    }
    points.sort_by_key(|p| std::cmp::Reverse(p.number));
    Ok(points)
}
fn remove_point(point: &RestorePoint) -> io::Result<Vec<RestorePoint>> {
    let points = restore_points()?;
    if !points.iter().skip(1).any(|p| p == point) {
        return Err(io::Error::other(
            "Point changed or is the most recent restore point.",
        ));
    }
    let code = unsafe { windows::Win32::System::Restore::SRRemoveRestorePoint(point.number) };
    if code != 0 {
        return Err(io::Error::from_raw_os_error(code as i32));
    }
    restore_points()
}

fn drive_root(s: &str) -> Option<String> {
    let b = s.as_bytes();
    (b.len() == 3 && b[0].is_ascii_alphabetic() && b[1..] == *b":\\")
        .then(|| s.to_ascii_uppercase())
}
fn fixed_disk(root: &str) -> bool {
    unsafe { GetDriveTypeW(PCWSTR(wide(root).as_ptr())) == DRIVE_FIXED }
}
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Volume {
    root: String,
    serial: u32,
    label: String,
    guid: String,
    fs: String,
    entire: bool,
}
fn volume(mount: &str, entire: bool) -> io::Result<Volume> {
    use windows::Win32::Storage::FileSystem::GetVolumeNameForVolumeMountPointW;
    use windows::Win32::System::WindowsProgramming::DRIVE_REMOVABLE;
    let root = drive_root(mount).ok_or_else(|| io::Error::other("Select a drive letter."))?;
    let drive_type = unsafe { GetDriveTypeW(PCWSTR(wide(&root).as_ptr())) };
    if !matches!(drive_type, DRIVE_FIXED | DRIVE_REMOVABLE) {
        return Err(io::Error::other("Select a mounted local data volume."));
    }
    let (mut label, mut fs, mut serial) = ([0u16; 261], [0u16; 64], 0);
    unsafe {
        GetVolumeInformationW(
            PCWSTR(wide(&root).as_ptr()),
            Some(&mut label),
            Some(&mut serial),
            None,
            None,
            Some(&mut fs),
        )?;
    }
    let decode = |v: &[u16]| {
        String::from_utf16_lossy(&v[..v.iter().position(|x| *x == 0).unwrap_or(v.len())])
    };
    let fs = decode(&fs).to_ascii_uppercase();
    if !entire && fs != "NTFS" {
        return Err(io::Error::other("Free-space wiping requires NTFS."));
    }
    let mut guid = [0u16; 64];
    unsafe {
        GetVolumeNameForVolumeMountPointW(PCWSTR(wide(&root).as_ptr()), &mut guid)?;
    }
    let guid = decode(&guid);
    if entire {
        if !matches!(fs.as_str(), "NTFS" | "EXFAT" | "FAT32") {
            return Err(io::Error::other(
                "Choose an NTFS, exFAT or FAT32 data volume.",
            ));
        }
        let paths = [
            Some(cleaner::system_directory()?),
            Some(std::env::current_exe()?),
            std::env::var_os("USERPROFILE").map(PathBuf::from),
        ];
        for path in paths.into_iter().flatten() {
            let path = std::fs::canonicalize(path)?;
            if display(&path).to_ascii_uppercase().starts_with(&root) {
                return Err(io::Error::other(
                    "This volume contains Windows, Trontop or your active profile.",
                ));
            }
        }
        let rows = wmi::query_once(
            r"ROOT\CIMV2",
            "SELECT DeviceID, BootVolume, SystemVolume, PageFilePresent FROM Win32_Volume",
            Duration::from_secs(10),
        )
        .map_err(io::Error::other)?;
        if !rows.iter().any(|r| {
            r.text("DeviceID")
                .is_some_and(|id| id.eq_ignore_ascii_case(&guid))
                && ["BootVolume", "SystemVolume", "PageFilePresent"]
                    .iter()
                    .all(|k| r.bool(k) == Some(false))
        }) {
            return Err(io::Error::other(
                "Windows could not confirm this is a data volume without a boot, system or paging role.",
            ));
        }
    }
    Ok(Volume {
        root,
        serial,
        label: decode(&label),
        guid,
        fs,
        entire,
    })
}
fn wipe(volume_before: &Volume) -> io::Result<String> {
    if &volume(&volume_before.root, volume_before.entire)? != volume_before {
        return Err(io::Error::other("Volume changed. Review it again."));
    }
    let system = cleaner::system_directory()?;
    let mut command = if volume_before.entire {
        let mut cmd = Command::new(system.join("format.com"));
        // Bind FORMAT to the reviewed volume GUID even if a drive letter is reassigned.
        // Keep Windows' interactive prompts. A full zero pass covers this logical volume.
        cmd.arg(&volume_before.guid)
            .arg(format!("/FS:{}", volume_before.fs))
            .arg("/P:0");
        cmd
    } else {
        let mut cmd = Command::new(system.join("cipher.exe"));
        cmd.arg(format!("/w:{}", volume_before.root));
        cmd
    };
    let status = command.creation_flags(CREATE_NEW_CONSOLE.0).status()?;
    if !status.success() {
        return Err(io::Error::other(format!("Windows returned {status}.")));
    }
    Ok(format!(
        "Windows returned success for the {} operation on {}.",
        if volume_before.entire {
            "format"
        } else {
            "free-space"
        },
        volume_before.root
    ))
}

pub(super) fn powershell_path() -> io::Result<PathBuf> {
    Ok(cleaner::system_directory()?.join(r"WindowsPowerShell\v1.0\powershell.exe"))
}
pub(super) fn powershell(script: &str, environment: Option<(&str, &str)>) -> io::Result<String> {
    let mut cmd = Command::new(powershell_path()?);
    cmd.args(["-NoLogo","-NoProfile","-NonInteractive","-Command"])
        .arg(format!("$ErrorActionPreference='Stop'; [Console]::OutputEncoding=[Text.UTF8Encoding]::new($false); {script}"))
        .creation_flags(CREATE_NO_WINDOW.0);
    if let Some((key, value)) = environment {
        cmd.env(key, value);
    }
    let out = cmd.output()?;
    if !out.status.success() {
        return Err(io::Error::other(
            String::from_utf8_lossy(&out.stderr).trim().to_owned(),
        ));
    }
    String::from_utf8(out.stdout).map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::super::tests::Fixture;
    use super::*;
    #[test]
    fn reviews_accept_only_drive_roots_and_exact_package_ids() {
        for mount in [r"C:", r"C:\folder", r"\\server\drive", r"..\"] {
            assert!(drive_root(mount).is_none());
        }
        assert_eq!(drive_root(r"d:\"), Some(r"D:\".into()));
        for id in ["--all", "", "A B", "A;B", "A\"B", "A/B", "Microsoft.X*"] {
            assert!(!valid_id(id));
        }
        assert!(valid_id("Microsoft.PowerToys"));
    }
    #[test]
    fn extension_inventory_handles_chromium_locale_and_firefox_active_state() {
        let f = Fixture::new();
        let profile =
            "Google/Chrome/User Data/Profile 3/Extensions/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/1_0";
        let manifest = f.file(&format!("{profile}/manifest.json"), 0);
        std::fs::write(
            manifest,
            r#"{"name":"__MSG_title__","default_locale":"en","version":"1.2"}"#,
        )
        .unwrap();
        let locale = f.file(&format!("{profile}/_locales/en/messages.json"), 0);
        std::fs::write(locale, r#"{"TITLE":{"message":"Example extension"}}"#).unwrap();
        let ff = f.file("Mozilla/Firefox/Profiles/sample/extensions.json", 0);
        std::fs::write(
            ff,
            r#"{"addons":[{"type":"extension","id":"example","active":false,"version":"2.0"}]}"#,
        )
        .unwrap();
        let report = extensions::scan_profiles(&f.0, &f.0).unwrap();
        assert!(
            report.contains("2 extension records")
                && report.contains("Example extension")
                && report.contains("disabled")
        );
    }
    #[test]
    #[ignore = "read-only local provider checks, without starting any maintenance action"]
    fn live_maintenance_inventory_counts_only() {
        let h = health(&AtomicBool::default(), &AtomicU64::default()).unwrap();
        assert!(h.apps > 0);
        println!(
            "Health: {} desktop apps, {} cleaner items / {} logical bytes, Recycle Bin totals available: {}",
            h.apps,
            h.files,
            h.bytes,
            h.bin.is_some()
        );
        println!(
            "Extensions: {}",
            extensions::inventory().unwrap().lines().next().unwrap()
        );
        println!(
            "App Paths: {} stale or archived",
            registry::scan().unwrap().len()
        );
        match restore_points() {
            Ok(rows) => println!("Restore points available: {}", rows.len()),
            Err(e) => println!("System Restore provider unavailable: {e}"),
        }
        let dir = cleaner::system_directory().unwrap();
        assert!(
            dir.join("cipher.exe").is_file()
                && dir.join("schtasks.exe").is_file()
                && powershell_path().unwrap().is_file()
        );
        let mount = format!("{}\\", &dir.to_string_lossy()[..2]);
        assert!(volume(&mount, false).is_ok());
        assert!(volume(&mount, true).is_err());
        println!("Verified system volume identity and native tool paths (read only).");
    }
}
