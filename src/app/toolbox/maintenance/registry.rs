use super::*;
use crate::specs::native::registry::{self, Hive};

const BASE: &str = r"Software\Microsoft\Windows\CurrentVersion\App Paths";
const BACKUP: &str = ".TrontopDisabled";

#[derive(Clone, PartialEq, Eq)]
pub(in crate::app::toolbox) struct Entry {
    pub key: String,
    pub target: String,
    pub archived: bool,
}

fn inspect(base: &str, key: &str) -> io::Result<Option<Entry>> {
    let archived = key.ends_with(BACKUP);
    let original = key.strip_suffix(BACKUP).unwrap_or(key);
    if !original.to_ascii_lowercase().ends_with(".exe") || key.contains(['\\', '/']) {
        return Ok(None);
    }
    let value = registry::read(Hive::CurrentUser, &format!(r"{base}\{key}"), "")
        .map_err(io::Error::other)?;
    let Some(target) = value.as_ref().and_then(|v| v.text()) else {
        return Ok(None);
    };
    let path = target.trim().trim_matches('"');
    if !archived {
        let Some(root) = path.get(..3).filter(|s| drive_root(s).is_some()) else {
            return Ok(None);
        };
        if !path.to_ascii_lowercase().ends_with(".exe")
            || path.contains('%')
            || !fixed_disk(root)
            || !matches!(std::fs::metadata(path), Err(e) if e.kind()==io::ErrorKind::NotFound)
        {
            return Ok(None);
        }
    }
    Ok(Some(Entry {
        key: key.into(),
        target: target.into(),
        archived,
    }))
}

pub(super) fn scan() -> io::Result<Vec<Entry>> {
    scan_at(BASE)
}
fn scan_at(base: &str) -> io::Result<Vec<Entry>> {
    let keys = registry::subkeys(Hive::CurrentUser, base, usize::MAX).map_err(io::Error::other)?;
    let mut rows = Vec::new();
    for key in keys {
        if let Some(entry) = inspect(base, &key)? {
            rows.push(entry);
        }
    }
    Ok(rows)
}

pub(super) fn toggle(entry: &Entry) -> io::Result<Vec<Entry>> {
    toggle_at(BASE, entry)
}
fn toggle_at(base: &str, entry: &Entry) -> io::Result<Vec<Entry>> {
    use windows::Win32::System::Registry::*;
    if inspect(base, &entry.key)?.as_ref() != Some(entry) {
        return Err(io::Error::other("Registration changed. Scan again."));
    }
    let new = if entry.archived {
        entry.key.strip_suffix(BACKUP).unwrap().to_owned()
    } else {
        format!("{}{BACKUP}", entry.key)
    };
    let mut parent = HKEY::default();
    // Renaming the whole key retains every value and subkey. An existing destination causes failure.
    unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(wide(base).as_ptr()),
            None,
            KEY_READ | KEY_WRITE | KEY_WOW64_64KEY,
            &mut parent,
        )
        .ok()?;
        let status = RegRenameKey(
            parent,
            PCWSTR(wide(&entry.key).as_ptr()),
            PCWSTR(wide(&new).as_ptr()),
        );
        let _ = RegCloseKey(parent);
        status.ok()?;
    }
    scan_at(base)
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::System::Registry::*;
    use windows::core::PWSTR;
    #[test]
    fn owned_app_path_can_be_archived_and_restored_with_extra_values_preserved() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = format!(
            r"Software\Trontop\Tests\AppPaths\{}-{unique}",
            std::process::id()
        );
        let root = wide(&base);
        struct Owned(Vec<u16>);
        impl Drop for Owned {
            fn drop(&mut self) {
                unsafe {
                    let _ = RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(self.0.as_ptr()));
                }
            }
        }
        let create = |parent: HKEY, path: &[u16]| {
            let mut key = HKEY::default();
            let mut disposition = REG_CREATE_KEY_DISPOSITION::default();
            unsafe {
                RegCreateKeyExW(
                    parent,
                    PCWSTR(path.as_ptr()),
                    None,
                    PWSTR::null(),
                    REG_OPTION_VOLATILE,
                    KEY_READ | KEY_WRITE | KEY_WOW64_64KEY,
                    None,
                    &mut key,
                    Some(&mut disposition),
                )
                .ok()
                .unwrap();
            }
            assert_eq!(disposition, REG_CREATED_NEW_KEY);
            key
        };
        let parent = create(HKEY_CURRENT_USER, &root);
        let _owned = Owned(root);
        let child = create(parent, &wide("Missing.exe"));
        let target = cleaner::system_directory()
            .unwrap()
            .join(format!("Trontop-absent-{unique}.exe"));
        assert!(!target.exists());
        let bytes = |s: &str| {
            wide(s)
                .into_iter()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>()
        };
        unsafe {
            RegSetValueExW(
                child,
                PCWSTR::null(),
                None,
                REG_SZ,
                Some(&bytes(&target.to_string_lossy())),
            )
            .ok()
            .unwrap();
            RegSetValueExW(
                child,
                PCWSTR(wide("Extra").as_ptr()),
                None,
                REG_SZ,
                Some(&bytes("retained")),
            )
            .ok()
            .unwrap();
            RegCloseKey(child).ok().unwrap();
            RegCloseKey(parent).ok().unwrap();
        }
        let found = scan_at(&base).unwrap();
        assert_eq!(found.len(), 1);
        let archived = toggle_at(&base, &found[0]).unwrap();
        assert!(archived[0].archived && toggle_at(&base, &found[0]).is_err());
        let restored = toggle_at(&base, &archived[0]).unwrap();
        assert!(!restored[0].archived);
        assert_eq!(
            registry::read(Hive::CurrentUser, &format!(r"{base}\Missing.exe"), "Extra")
                .unwrap()
                .unwrap()
                .text(),
            Some("retained")
        );
    }
}
