use super::*;
use std::cmp::Reverse;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(in crate::app::toolbox) struct AppCache {
    label: &'static str,
    profile: &'static str,
    owner: &'static str,
}

const fn cache(label: &'static str, profile: &'static str, owner: &'static str) -> AppCache {
    AppCache {
        label,
        profile,
        owner,
    }
}
const APPS: &[AppCache] = &[
    cache("VS Code cache", "Code", "code.exe"),
    cache(
        "VS Code Insiders cache",
        "Code - Insiders",
        "code - insiders.exe",
    ),
    cache("VSCodium cache", "VSCodium", "vscodium.exe"),
    cache("Cursor cache", "Cursor", "cursor.exe"),
    cache("Discord cache", "discord", "discord.exe"),
    cache("Discord PTB cache", "discordptb", "discordptb.exe"),
    cache("Discord Canary cache", "discordcanary", "discordcanary.exe"),
    cache("Slack cache", "Slack", "slack.exe"),
    cache(
        "GitHub Desktop cache",
        "GitHub Desktop",
        "githubdesktop.exe",
    ),
    cache("Unity Hub cache", "UnityHub", "unity hub.exe"),
    cache("Obsidian cache", "obsidian", "obsidian.exe"),
    cache("Signal cache", "Signal", "signal.exe"),
    cache("Notion cache", "Notion", "notion.exe"),
    cache("Postman cache", "Postman", "postman.exe"),
];
const CHROMIUM_LEAVES: &[&str] = &["Cache/Cache_Data", "Code Cache/js", "Code Cache/wasm"];

fn electron_zip(name: &str) -> bool {
    let Some((version, arch)) = name
        .strip_prefix("electron-v")
        .and_then(|n| n.strip_suffix(".zip"))
        .and_then(|n| n.rsplit_once("-win32-"))
    else {
        return false;
    };
    let mut parts = version.split('.');
    matches!(arch, "x64" | "ia32" | "arm64")
        && (0..3).all(|_| {
            parts
                .next()
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        })
        && parts.next().is_none()
}

fn timestamp_name(text: &str, pattern: &str) -> bool {
    text.len() == pattern.len()
        && text.bytes().zip(pattern.bytes()).all(|(c, p)| {
            if p == b'#' {
                c.is_ascii_digit()
            } else {
                c == p
            }
        })
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(in crate::app::toolbox) enum Rule {
    Temp,
    WindowsTemp,
    Dump,
    Chrome,
    Edge,
    Brave,
    Vivaldi,
    Firefox,
    NuGet,
    App(&'static AppCache),
    NpmLogs,
    ObsLogs,
    Steam,
    PipWheels,
    ElectronDownloads,
}

impl Rule {
    pub fn label(self) -> &'static str {
        match self {
            Self::Temp => "User temporary files",
            Self::WindowsTemp => "Windows temporary files",
            Self::Dump => "Application crash dumps",
            Self::Chrome => "Chrome cache",
            Self::Edge => "Edge cache",
            Self::Brave => "Brave cache",
            Self::Vivaldi => "Vivaldi cache",
            Self::Firefox => "Firefox cache",
            Self::NuGet => "NuGet HTTP cache",
            Self::App(app) => app.label,
            Self::NpmLogs => "npm diagnostic logs",
            Self::ObsLogs => "OBS Studio logs",
            Self::Steam => "Steam web cache",
            Self::PipWheels => "pip built wheels",
            Self::ElectronDownloads => "Electron runtime downloads",
        }
    }

    pub(in crate::app::toolbox) fn owner_exe(self) -> Option<&'static str> {
        match self {
            Self::Chrome => Some("chrome.exe"),
            Self::Edge => Some("msedge.exe"),
            Self::Brave => Some("brave.exe"),
            Self::Vivaldi => Some("vivaldi.exe"),
            Self::Firefox => Some("firefox.exe"),
            Self::App(app) => Some(app.owner),
            Self::Steam => Some("steamwebhelper.exe"),
            _ => None,
        }
    }

    fn accepts_name(self, path: &Path) -> bool {
        let extension = |ext: &str| {
            path.extension()
                .is_some_and(|e| e.eq_ignore_ascii_case(ext))
        };
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            return false;
        };
        let hex = |s: &str, len| s.len() == len && s.bytes().all(|c| c.is_ascii_hexdigit());
        match self {
            Self::Temp | Self::WindowsTemp => extension("tmp"),
            Self::Dump => extension("dmp"),
            Self::NuGet => extension("dat"),
            Self::Firefox => hex(name, 40),
            Self::PipWheels => name.strip_suffix(".whl").is_some_and(|stem| {
                stem.split('-').count() >= 5
                    && stem
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b".-_+!".contains(&c))
            }),
            Self::ElectronDownloads => electron_zip(name),
            Self::NpmLogs => name.split_once("-debug-").is_some_and(|(date, n)| {
                timestamp_name(date, "####-##-##T##_##_##_###Z")
                    && n.strip_suffix(".log")
                        .is_some_and(|n| !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit()))
            }),
            Self::ObsLogs => timestamp_name(name, "####-##-## ##-##-##.txt"),
            Self::Chrome
            | Self::Edge
            | Self::Brave
            | Self::Vivaldi
            | Self::Steam
            | Self::App(_) => name
                .split_once('_')
                .is_some_and(|(hash, stream)| hex(hash, 16) && matches!(stream, "0" | "1" | "s")),
        }
    }

    pub(super) fn matches(self, path: &Path, stamp: &Stamp, now: SystemTime) -> bool {
        let days = if matches!(
            self,
            Self::Dump | Self::NpmLogs | Self::ObsLogs | Self::PipWheels | Self::ElectronDownloads
        ) {
            30
        } else {
            7
        };
        self.accepts_name(path)
            && now
                .duration_since(stamp.modified)
                .is_ok_and(|age| age >= Duration::from_secs(days * 86_400))
    }
}

struct Analyzer<'a> {
    result: Preview,
    now: SystemTime,
    cancel: &'a AtomicBool,
    checked: &'a AtomicU64,
}

impl<'a> Analyzer<'a> {
    fn new(cancel: &'a AtomicBool, checked: &'a AtomicU64) -> Self {
        Self {
            result: Preview::default(),
            now: SystemTime::now(),
            cancel,
            checked,
        }
    }

    // Every expected path is built underneath an already canonical base.
    // A parent junction must never turn a cache rule into a scan elsewhere.
    fn contents(&mut self, path: &Path) -> Option<fs::ReadDir> {
        let entries = (|| {
            let meta = fs::symlink_metadata(path)?;
            if !meta.is_dir() || linked(&meta) || fs::canonicalize(path)? != path {
                return Err(io::Error::other("Skipped linked or invalid directory"));
            }
            fs::read_dir(path)
        })();
        match entries {
            Ok(dir) => Some(dir),
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => {
                self.result.issues.push(format!("{}: {e}", path.display()));
                None
            }
        }
    }

    fn check_cancel(&self) -> io::Result<()> {
        if self.cancel.load(Ordering::Relaxed) {
            Err(io::ErrorKind::Interrupted.into())
        } else {
            Ok(())
        }
    }

    fn files(&mut self, root: &Path, rule: Rule) -> io::Result<()> {
        self.check_cancel()?;
        let Some(entries) = self.contents(root) else {
            return Ok(());
        };
        for entry in entries {
            self.check_cancel()?;
            let Ok(entry) = entry else {
                self.result.skipped += 1;
                continue;
            };
            let path = entry.path();
            if !rule.accepts_name(&path) {
                continue;
            }
            self.checked.fetch_add(1, Ordering::Relaxed);
            match native::open(&path, false) {
                Ok((_, stamp)) if rule.matches(&path, &stamp, self.now) => {
                    self.result.files.push(Candidate { path, rule, stamp })
                }
                Ok(_) => {}
                Err(_) => self.result.skipped += 1,
            }
        }
        Ok(())
    }

    fn hashed_files(&mut self, root: &Path, rule: Rule, shards: &[usize]) -> io::Result<()> {
        let Some((&width, rest)) = shards.split_first() else {
            return self.files(root, rule);
        };
        self.check_cancel()?;
        if let Some(entries) = self.contents(root) {
            for entry in entries {
                self.check_cancel()?;
                let Ok(entry) = entry else {
                    self.result.skipped += 1;
                    continue;
                };
                let name = entry.file_name();
                if name
                    .to_str()
                    .is_some_and(|s| s.len() == width && s.bytes().all(|b| b.is_ascii_hexdigit()))
                {
                    self.hashed_files(&entry.path(), rule, rest)?;
                }
            }
        }
        Ok(())
    }

    fn profiles(&mut self, parent: &Path, rule: Rule, leaves: &[&str]) -> io::Result<()> {
        self.check_cancel()?;
        let Some(entries) = self.contents(parent) else {
            return Ok(());
        };
        for entry in entries {
            self.check_cancel()?;
            let Ok(entry) = entry else {
                self.result.skipped += 1;
                continue;
            };
            let Ok(meta) = entry.metadata() else {
                self.result.skipped += 1;
                continue;
            };
            if !meta.is_dir() || linked(&meta) {
                continue;
            }
            if matches!(
                rule,
                Rule::Chrome | Rule::Edge | Rule::Brave | Rule::Vivaldi | Rule::Steam
            ) {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name != "Default"
                    && !name
                        .strip_prefix("Profile ")
                        .is_some_and(|n| !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit()))
                {
                    continue;
                }
            }
            for leaf in leaves {
                self.files(&entry.path().join(leaf), rule)?;
            }
        }
        Ok(())
    }

    fn finish(mut self) -> Preview {
        let result = &mut self.result;
        result.files.sort_unstable_by(|a, b| {
            (a.rule, Reverse(a.bytes()), &a.path).cmp(&(b.rule, Reverse(b.bytes()), &b.path))
        });
        let mut start = 0;
        for group in result.files.chunk_by(|a, b| a.rule == b.rule) {
            let end = start + group.len();
            let bytes = group.iter().map(Candidate::bytes).sum();
            result.groups.push(Group {
                rule: group[0].rule,
                range: start..end,
                bytes,
            });
            result.bytes += bytes;
            start = end;
        }
        self.result
    }
}

pub(in crate::app::toolbox) fn preview(
    cancel: &AtomicBool,
    checked: &AtomicU64,
) -> io::Result<Preview> {
    let local = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .ok_or_else(|| io::Error::other("LocalAppData is unavailable."))?;
    let windows = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute());
    let roaming = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute());
    analyze(
        &local,
        roaming.as_deref(),
        windows.as_deref(),
        cancel,
        checked,
    )
}

pub(in crate::app::toolbox) fn analyze(
    local: &Path,
    roaming: Option<&Path>,
    windows: Option<&Path>,
    cancel: &AtomicBool,
    checked: &AtomicU64,
) -> io::Result<Preview> {
    let local = fs::canonicalize(local)?;
    let mut analyzer = Analyzer::new(cancel, checked);
    analyzer.files(&local.join("Temp"), Rule::Temp)?;
    analyzer.files(&local.join("CrashDumps"), Rule::Dump)?;
    if let Some(windows) = windows {
        match fs::canonicalize(windows) {
            Ok(root) if root != local => analyzer.files(&root.join("Temp"), Rule::WindowsTemp)?,
            Ok(_) => {}
            Err(e) => analyzer.result.issues.push(format!("Windows Temp: {e}")),
        }
    }
    for (rule, profiles) in [
        (Rule::Chrome, "Google/Chrome/User Data"),
        (Rule::Edge, "Microsoft/Edge/User Data"),
        (Rule::Vivaldi, "Vivaldi/User Data"),
        (Rule::Brave, "BraveSoftware/Brave-Browser/User Data"),
    ] {
        analyzer.profiles(&local.join(profiles), rule, CHROMIUM_LEAVES)?;
    }
    analyzer.profiles(
        &local.join("Mozilla/Firefox/Profiles"),
        Rule::Firefox,
        &["cache2/entries"],
    )?;
    analyzer.profiles(&local.join("NuGet/v3-cache"), Rule::NuGet, &[""])?;
    analyzer.profiles(&local.join("Steam/htmlcache"), Rule::Steam, CHROMIUM_LEAVES)?;
    analyzer.hashed_files(
        &local.join("pip/cache/wheels"),
        Rule::PipWheels,
        &[2, 2, 2, 50],
    )?;
    analyzer.hashed_files(
        &local.join("electron/Cache"),
        Rule::ElectronDownloads,
        &[64],
    )?;
    analyzer.files(&local.join("npm-cache/_logs"), Rule::NpmLogs)?;
    if let Some(roaming) = roaming {
        match fs::canonicalize(roaming) {
            Ok(root) => {
                for app in APPS {
                    for leaf in CHROMIUM_LEAVES {
                        analyzer.files(&root.join(app.profile).join(leaf), Rule::App(app))?;
                    }
                }
                analyzer.files(&root.join("obs-studio/logs"), Rule::ObsLogs)?;
            }
            Err(e) => analyzer.result.issues.push(format!("Roaming AppData: {e}")),
        }
    }
    Ok(analyzer.finish())
}

#[cfg(test)]
pub(in crate::app::toolbox) fn inspect(roots: &[(PathBuf, Rule)]) -> Preview {
    let (cancel, count) = (AtomicBool::default(), AtomicU64::default());
    let mut analyzer = Analyzer::new(&cancel, &count);
    for (root, rule) in roots {
        analyzer
            .files(&fs::canonicalize(root).unwrap(), *rule)
            .unwrap();
    }
    analyzer.finish()
}
