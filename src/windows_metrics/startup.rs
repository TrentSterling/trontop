//! Read-only startup inventory; no shell process and no localized output parsing.
use crate::model::StartupRow;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceState {
    Readable,
    Missing,
    Failed,
}

#[derive(Default)]
pub struct StartupInventory {
    pub rows: Vec<StartupRow>,
    pub sources: Vec<(&'static str, SourceState)>,
}

impl StartupInventory {
    fn add(&mut self, name: &'static str, rows: Vec<StartupRow>, state: SourceState) {
        self.rows.extend(rows);
        self.sources.push((name, state));
    }

    pub fn coverage(&self) -> (usize, usize) {
        (
            self.sources
                .iter()
                .filter(|(_, state)| *state != SourceState::Failed)
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
                "Current user Run key",
            ),
            (
                HKEY_LOCAL_MACHINE,
                r"Software\Microsoft\Windows\CurrentVersion\Run",
                "Machine Run key",
            ),
            (
                HKEY_LOCAL_MACHINE,
                r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Run",
                "32-bit machine Run key",
            ),
        ] {
            let (rows, state) = read_run_key(root, key, name);
            inventory.add(name, rows, state);
        }
        for (variable, name) in [
            ("APPDATA", "Current user Startup folder"),
            ("PROGRAMDATA", "Machine Startup folder"),
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
    inventory.add("Windows startup sources", Vec::new(), SourceState::Failed);
    inventory
        .rows
        .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    // Keep source attribution even when the same command appears in multiple places.
    inventory
}

fn read_folder(path: &std::path::Path, source: &str) -> (Vec<StartupRow>, SourceState) {
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
    let mut state = SourceState::Readable;
    for (index, entry) in entries.enumerate() {
        if index >= 4096 {
            state = SourceState::Failed;
            break;
        }
        match entry {
            Ok(entry) => {
                let path = entry.path();
                rows.push(StartupRow {
                    name: path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    command: path.display().to_string(),
                    source: source.into(),
                });
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
    source: &str,
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
    let mut state = SourceState::Readable;
    for index in 0..4096 {
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
            rows.push(StartupRow {
                name,
                command,
                source: source.into(),
            });
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
        inventory.add("Empty", vec![], SourceState::Readable);
        inventory.add("Absent", vec![], SourceState::Missing);
        inventory.add("Denied", vec![], SourceState::Failed);
        assert_eq!(inventory.coverage(), (2, 3));
        assert!(inventory.rows.is_empty());
    }
    #[cfg(windows)]
    #[test]
    fn native_startup_inventory_is_read_only_and_accounts_for_every_source() {
        let inventory = enumerate_startup();
        assert_eq!(inventory.sources.len(), 5);
        assert_eq!(inventory.coverage().1, 5);
        assert!(inventory.rows.iter().all(|row| !row.source.is_empty()));
    }
}
