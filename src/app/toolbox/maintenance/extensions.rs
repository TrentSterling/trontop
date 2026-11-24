use super::*;
use serde_json::Value;
use std::fs;
use std::io::Read;

const LIMIT: u64 = 8 * 1024 * 1024;
fn directories(path: &Path, skipped: &mut usize) -> Vec<PathBuf> {
    match fs::canonicalize(path) {
        Ok(real) if real == path => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Vec::new(),
        _ => {
            *skipped += 1;
            return Vec::new();
        }
    }
    let Ok(entries) = fs::read_dir(path) else {
        *skipped += 1;
        return Vec::new();
    };
    entries
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            let meta = fs::symlink_metadata(&path).ok()?;
            (meta.is_dir() && !scan::linked(&meta)).then_some(path)
        })
        .collect()
}
fn json(path: &Path) -> io::Result<Value> {
    if fs::canonicalize(path)? != path {
        return Err(io::Error::other("Skipped redirected file"));
    }
    let file = fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take(LIMIT + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > LIMIT {
        return Err(io::Error::other("Manifest is too large"));
    }
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}
fn name(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
pub(super) fn scan_profiles(local: &Path, roaming: &Path) -> io::Result<String> {
    let local = fs::canonicalize(local)?;
    let roaming = fs::canonicalize(roaming)?;
    let mut rows = Vec::new();
    let mut skipped = 0;
    for (browser, relative) in [
        ("Chrome", "Google/Chrome/User Data"),
        ("Edge", "Microsoft/Edge/User Data"),
        ("Brave", "BraveSoftware/Brave-Browser/User Data"),
        ("Vivaldi", "Vivaldi/User Data"),
    ] {
        for profile in directories(&local.join(relative), &mut skipped) {
            let profile_name = filename(&profile);
            if profile_name != "Default"
                && !profile_name
                    .strip_prefix("Profile ")
                    .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            {
                continue;
            }
            for ext in directories(&profile.join("Extensions"), &mut skipped) {
                let id = filename(&ext);
                if id.len() != 32 || !id.bytes().all(|b| (b'a'..=b'p').contains(&b)) {
                    continue;
                }
                for dir in directories(&ext, &mut skipped) {
                    let Ok(m) = json(&dir.join("manifest.json")) else {
                        skipped += 1;
                        continue;
                    };
                    let mut title = name(&m["name"]).to_owned();
                    if let Some(token) = title
                        .strip_prefix("__MSG_")
                        .and_then(|s| s.strip_suffix("__"))
                    {
                        let locale = name(&m["default_locale"]);
                        if !locale.is_empty()
                            && locale
                                .bytes()
                                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
                            && let Ok(messages) =
                                json(&dir.join("_locales").join(locale).join("messages.json"))
                            && let Some(message) = messages.as_object().and_then(|map| {
                                map.iter().find(|(key, _)| key.eq_ignore_ascii_case(token))
                            })
                            && let Some(label) = message.1["message"].as_str()
                        {
                            title = label.to_owned();
                        }
                    }
                    rows.push(format!(
                        "{browser}  /  {profile_name}  /  {title}  {}  ·  {id}",
                        name(&m["version"])
                    ));
                }
            }
        }
    }
    for profile in directories(&roaming.join("Mozilla/Firefox/Profiles"), &mut skipped) {
        let data = match json(&profile.join("extensions.json")) {
            Ok(v) => v,
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        if let Some(addons) = data["addons"].as_array() {
            for a in addons.iter().filter(|a| a["type"] == "extension") {
                let title = a["defaultLocale"]["name"]
                    .as_str()
                    .unwrap_or_else(|| name(&a["id"]));
                rows.push(format!(
                    "Firefox  /  {}  /  {title}  {}  ·  {}",
                    filename(&profile),
                    name(&a["version"]),
                    match a["active"].as_bool() {
                        Some(true) => "enabled",
                        Some(false) => "disabled",
                        None => "unknown",
                    }
                ));
            }
        }
    }
    rows.sort();
    Ok(format!(
        "{} extension records on disk · {} locations skipped\n{}",
        rows.len(),
        skipped,
        rows.join("\n")
    ))
}
pub(super) fn inventory() -> io::Result<String> {
    let dir = |s| {
        std::env::var_os(s)
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .ok_or_else(|| io::Error::other("Browser profile locations unavailable"))
    };
    scan_profiles(&dir("LOCALAPPDATA")?, &dir("APPDATA")?)
}
