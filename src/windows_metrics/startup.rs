//! Read-only startup inventory; no shell process and no localized output parsing.
use crate::model::StartupRow;
use crate::startup::{ENTRY_LIMIT, Read, ReadState as SourceState, Source, TEXT_LIMIT};

#[derive(Default)]
pub struct StartupInventory {
    pub sources: Vec<Read>,
}

impl StartupInventory {
    fn add(&mut self, source: Source, rows: Vec<StartupRow>, state: SourceState) {
        self.sources.push(Read {
            source,
            rows,
            state,
        });
    }

    #[cfg(test)]
    pub fn coverage(&self) -> (usize, usize) {
        (
            self.sources
                .iter()
                .filter(|read| read.state != SourceState::Failed)
                .count(),
            self.sources.len(),
        )
    }
}

pub fn enumerate_startup() -> StartupInventory {
    let mut inventory = StartupInventory::default();
    #[cfg(windows)]
    {
        use windows::Win32::System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
        for (root, key, name) in [
            (
                HKEY_CURRENT_USER,
                r"Software\Microsoft\Windows\CurrentVersion\Run",
                Source::UserRun,
            ),
            (
                HKEY_LOCAL_MACHINE,
                r"Software\Microsoft\Windows\CurrentVersion\Run",
                Source::MachineRun,
            ),
            (
                HKEY_LOCAL_MACHINE,
                r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Run",
                Source::MachineRun32,
            ),
        ] {
            let (rows, state) = read_run_key(root, key, name);
            inventory.add(name, rows, state);
        }
        for (variable, name) in [
            ("APPDATA", Source::UserFolder),
            ("PROGRAMDATA", Source::MachineFolder),
        ] {
            let Some(root) = std::env::var_os(variable) else {
                inventory.add(name, Vec::new(), SourceState::Failed);
                continue;
            };
            let path =
                std::path::Path::new(&root).join("Microsoft/Windows/Start Menu/Programs/Startup");
            let (rows, state) = read_folder(&path, name);
            inventory.add(name, rows, state);
        }
    }
    #[cfg(not(windows))]
    for source in Source::ALL {
        inventory.add(source, Vec::new(), SourceState::Failed);
    }
    // Keep source attribution even when the same command appears in multiple places.
    inventory
}

fn read_folder(path: &std::path::Path, source: Source) -> (Vec<StartupRow>, SourceState) {
    let entries = match std::fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) => {
            return (
                Vec::new(),
                if error.kind() == std::io::ErrorKind::NotFound {
                    SourceState::Missing
                } else {
                    SourceState::Failed
                },
            );
        }
    };
    let mut rows = Vec::new();
    let mut text_bytes = 0;
    let mut state = SourceState::Readable;
    for (index, entry) in entries.enumerate() {
        if index >= ENTRY_LIMIT {
            state = SourceState::Failed;
            break;
        }
        match entry {
            Ok(entry) => {
                let path = entry.path();
                let file_name = path.file_name().unwrap_or_default().to_string_lossy();
                // Windows never launches desktop.ini; it is folder-view
                // metadata Explorer writes into every Startup folder, not a
                // startup entry. Case-insensitive: Windows file names are.
                if file_name.eq_ignore_ascii_case("desktop.ini") {
                    continue;
                }
                let row = StartupRow {
                    key: file_name.into_owned(),
                    name: path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    command: path.display().to_string(),
                    source,
                };
                text_bytes += row.text_bytes();
                if text_bytes > TEXT_LIMIT {
                    state = SourceState::Failed;
                    break;
                }
                rows.push(row);
            }
            Err(_) => state = SourceState::Failed,
        }
    }
    (rows, state)
}

fn decode_string(bytes: &[u8]) -> Option<String> {
    if !bytes.len().is_multiple_of(2) {
        return None;
    }
    let utf16 = bytes
        .chunks_exact(2)
        .map(|v| u16::from_le_bytes([v[0], v[1]]))
        .take_while(|v| *v != 0)
        .collect::<Vec<_>>();
    String::from_utf16(&utf16).ok()
}

#[cfg(windows)]
fn read_run_key(
    root: windows::Win32::System::Registry::HKEY,
    path: &str,
    source: Source,
) -> (Vec<StartupRow>, SourceState) {
    use windows::Win32::Foundation::{
        ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_NO_MORE_ITEMS, ERROR_PATH_NOT_FOUND,
        ERROR_SUCCESS,
    };
    use windows::Win32::System::Registry::{
        HKEY, KEY_QUERY_VALUE, REG_EXPAND_SZ, REG_SZ, RegCloseKey, RegEnumValueW, RegOpenKeyExW,
    };
    use windows::core::{PCWSTR, PWSTR};
    struct Key(HKEY);
    impl Drop for Key {
        fn drop(&mut self) {
            unsafe {
                let _ = RegCloseKey(self.0);
            }
        }
    }
    let wide = path.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let mut key = HKEY::default();
    // SAFETY: terminated UTF-16 and writable key; query-only, never creates a key.
    let result =
        unsafe { RegOpenKeyExW(root, PCWSTR(wide.as_ptr()), None, KEY_QUERY_VALUE, &mut key) };
    if result != ERROR_SUCCESS {
        return (
            Vec::new(),
            if matches!(result, ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND) {
                SourceState::Missing
            } else {
                SourceState::Failed
            },
        );
    }
    let key = Key(key);
    let mut name = vec![0_u16; 32768];
    let mut data = vec![0_u8; 16384];
    let mut rows = Vec::new();
    let mut text_bytes = 0;
    let mut state = SourceState::Readable;
    for index in 0..ENTRY_LIMIT as u32 {
        let mut read = None;
        for _ in 0..4 {
            let mut name_len = name.len() as u32;
            let mut data_len = data.len() as u32;
            let mut kind = 0_u32;
            // SAFETY: buffers match supplied capacities; no pointers retained.
            let status = unsafe {
                RegEnumValueW(
                    key.0,
                    index,
                    Some(PWSTR(name.as_mut_ptr())),
                    &mut name_len,
                    None,
                    Some(&mut kind),
                    Some(data.as_mut_ptr()),
                    Some(&mut data_len),
                )
            };
            if status == ERROR_NO_MORE_ITEMS {
                return (rows, state);
            }
            if status == ERROR_MORE_DATA && data_len as usize > data.len() && data_len <= 1_048_576
            {
                data.resize(data_len as usize, 0);
                continue;
            }
            if status != ERROR_SUCCESS
                || name_len as usize > name.len()
                || data_len as usize > data.len()
            {
                return (rows, SourceState::Failed);
            }
            read = Some((name_len as usize, data_len as usize, kind));
            break;
        }
        let Some((name_len, data_len, kind)) = read else {
            return (rows, SourceState::Failed);
        };
        if kind != REG_SZ.0 && kind != REG_EXPAND_SZ.0 {
            continue;
        }
        let Some(command) = decode_string(&data[..data_len]) else {
            state = SourceState::Failed;
            continue;
        };
        let name = String::from_utf16_lossy(&name[..name_len]);
        if !name.is_empty() && !command.is_empty() {
            let row = StartupRow {
                key: name.clone(),
                name,
                command,
                source,
            };
            text_bytes += row.text_bytes();
            if text_bytes > TEXT_LIMIT {
                return (rows, SourceState::Failed);
            }
            rows.push(row);
        }
    }
    (rows, SourceState::Failed) // bounded enumeration reached its cap
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registry_text_handles_unicode_no_terminator_and_invalid_lengths() {
        let bytes = "C:\\測試\\app.exe --value=REG_SZ"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        assert_eq!(
            decode_string(&bytes).as_deref(),
            Some("C:\\測試\\app.exe --value=REG_SZ")
        );
        assert!(decode_string(&[1]).is_none());
        assert_eq!(decode_string(&[65, 0, 0, 0, 66, 0]).as_deref(), Some("A"));
    }
    #[test]
    fn absent_sources_are_distinct_from_unreadable_sources() {
        let mut inventory = StartupInventory::default();
        inventory.add(Source::UserRun, vec![], SourceState::Readable);
        inventory.add(Source::MachineRun, vec![], SourceState::Missing);
        inventory.add(Source::MachineRun32, vec![], SourceState::Failed);
        assert_eq!(inventory.coverage(), (2, 3));
        assert!(inventory.sources.iter().all(|read| read.rows.is_empty()));
    }
    #[test]
    fn read_folder_skips_desktop_ini_case_insensitively() {
        // Windows never launches desktop.ini; it is folder-view metadata
        // Explorer writes into every Startup folder, not a startup entry
        // (P17). A real temp directory, not a mock, so this exercises the
        // same std::fs::read_dir path the live scan uses.
        let dir = std::env::temp_dir().join(format!(
            "trontop-startup-test-{}-{}",
            std::process::id(),
            line!()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("DeskTop.INI"), b"[.ShellClassInfo]").unwrap();
        std::fs::write(dir.join("MyApp.lnk"), b"").unwrap();
        let (rows, state) = read_folder(&dir, Source::UserFolder);
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(state, SourceState::Readable);
        assert_eq!(rows.len(), 1, "desktop.ini must not become an entry");
        assert_eq!(rows[0].name, "MyApp");
    }
    #[cfg(windows)]
    #[test]
    fn native_startup_inventory_is_read_only_and_accounts_for_every_source() {
        let inventory = enumerate_startup();
        assert_eq!(inventory.sources.len(), 5);
        assert_eq!(inventory.coverage().1, 5);
        assert!(inventory.sources.iter().all(|read| {
            read.rows
                .iter()
                .all(|row| row.source == read.source && !row.key.is_empty())
        }));
    }
}
