//! Read-only registry access. Keys are opened with query/enumerate rights and
//! the 64-bit view only; nothing is created, written or deleted.
#[cfg(windows)]
use super::NativeError;

/// Largest value or name list this helper will copy.
const MAX_VALUE_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hive {
    LocalMachine,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegValue {
    /// REG_SZ, or REG_EXPAND_SZ left unexpanded.
    Text(String),
    MultiText(Vec<String>),
    U32(u32),
    U64(u64),
    Binary(Vec<u8>),
}

impl RegValue {
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Self::U32(value) => Some(u64::from(*value)),
            Self::U64(value) => Some(*value),
            _ => None,
        }
    }
}

const REG_SZ: u32 = 1;
const REG_EXPAND_SZ: u32 = 2;
const REG_BINARY: u32 = 3;
const REG_DWORD: u32 = 4;
const REG_DWORD_BIG_ENDIAN: u32 = 5;
const REG_MULTI_SZ: u32 = 7;
const REG_QWORD: u32 = 11;

/// Decodes raw registry data. None for unsupported kinds or invalid sizes.
pub fn decode(kind: u32, bytes: &[u8]) -> Option<RegValue> {
    match kind {
        REG_SZ | REG_EXPAND_SZ => super::utf16_bytes_until_nul(bytes).map(RegValue::Text),
        REG_MULTI_SZ => {
            if !bytes.len().is_multiple_of(2) {
                return None;
            }
            let words = bytes
                .chunks_exact(2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .collect::<Vec<_>>();
            Some(RegValue::MultiText(
                words
                    .split(|w| *w == 0)
                    .filter(|part| !part.is_empty())
                    .map(String::from_utf16_lossy)
                    .collect(),
            ))
        }
        REG_DWORD => bytes
            .try_into()
            .ok()
            .map(|b: [u8; 4]| RegValue::U32(u32::from_le_bytes(b))),
        REG_DWORD_BIG_ENDIAN => bytes
            .try_into()
            .ok()
            .map(|b: [u8; 4]| RegValue::U32(u32::from_be_bytes(b))),
        REG_QWORD => bytes
            .try_into()
            .ok()
            .map(|b: [u8; 8]| RegValue::U64(u64::from_le_bytes(b))),
        REG_BINARY => Some(RegValue::Binary(bytes.to_vec())),
        _ => None,
    }
}

#[cfg(windows)]
mod native {
    use super::*;
    #[cfg(test)]
    use windows::Win32::Foundation::ERROR_NO_MORE_ITEMS;
    use windows::Win32::Foundation::{
        ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_PATH_NOT_FOUND, ERROR_SUCCESS, WIN32_ERROR,
    };
    #[cfg(test)]
    use windows::Win32::System::Registry::RegEnumKeyExW;
    use windows::Win32::System::Registry::{
        HKEY, HKEY_LOCAL_MACHINE, KEY_ENUMERATE_SUB_KEYS, KEY_QUERY_VALUE, KEY_WOW64_64KEY,
        REG_VALUE_TYPE, RegCloseKey, RegOpenKeyExW, RegQueryValueExW,
    };
    use windows::core::PCWSTR;
    #[cfg(test)]
    use windows::core::PWSTR;

    struct Key(HKEY);
    impl Drop for Key {
        fn drop(&mut self) {
            // SAFETY: owns exactly one successfully opened key.
            unsafe {
                let _ = RegCloseKey(self.0);
            }
        }
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(Some(0)).collect()
    }

    fn missing(code: WIN32_ERROR) -> bool {
        matches!(code, ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND)
    }

    /// Ok(None) when the key does not exist.
    fn open(hive: Hive, path: &str) -> Result<Option<Key>, NativeError> {
        let root = match hive {
            Hive::LocalMachine => HKEY_LOCAL_MACHINE,
        };
        let path = wide(path);
        let mut key = HKEY::default();
        // SAFETY: terminated UTF-16 path and writable output; query rights only.
        let status = unsafe {
            RegOpenKeyExW(
                root,
                PCWSTR(path.as_ptr()),
                None,
                KEY_QUERY_VALUE | KEY_ENUMERATE_SUB_KEYS | KEY_WOW64_64KEY,
                &mut key,
            )
        };
        if missing(status) {
            Ok(None)
        } else if status != ERROR_SUCCESS {
            Err(NativeError::win32("RegOpenKeyExW", status.0))
        } else {
            Ok(Some(Key(key)))
        }
    }

    /// One value; `name` "" reads the key's default value. Ok(None) when the
    /// key or value is absent, or the value has an unsupported type.
    pub fn read(hive: Hive, path: &str, name: &str) -> Result<Option<RegValue>, NativeError> {
        let Some(key) = open(hive, path)? else {
            return Ok(None);
        };
        let name = wide(name);
        let mut buffer = vec![0u8; 512];
        for _ in 0..4 {
            let mut kind = REG_VALUE_TYPE::default();
            let mut length = buffer.len() as u32;
            // SAFETY: buffer capacity matches `length`; no pointers retained.
            let status = unsafe {
                RegQueryValueExW(
                    key.0,
                    PCWSTR(name.as_ptr()),
                    None,
                    Some(&mut kind),
                    Some(buffer.as_mut_ptr()),
                    Some(&mut length),
                )
            };
            if missing(status) {
                return Ok(None);
            }
            if status == ERROR_MORE_DATA {
                if length as usize > MAX_VALUE_BYTES {
                    return Err(NativeError::Malformed("registry value is too large"));
                }
                buffer.resize(length as usize, 0);
                continue;
            }
            if status != ERROR_SUCCESS {
                return Err(NativeError::win32("RegQueryValueExW", status.0));
            }
            let data = buffer
                .get(..length as usize)
                .ok_or(NativeError::Malformed("registry value length"))?;
            return Ok(decode(kind.0, data));
        }
        Err(NativeError::Malformed("registry value kept growing"))
    }

    /// Up to `limit` subkey names. Ok(empty) when the key is absent.
    #[cfg(test)]
    pub fn subkeys(hive: Hive, path: &str, limit: usize) -> Result<Vec<String>, NativeError> {
        let Some(key) = open(hive, path)? else {
            return Ok(Vec::new());
        };
        let mut names = Vec::new();
        let mut name = vec![0u16; 256];
        for index in 0..limit.min(u32::MAX as usize) as u32 {
            let mut length = name.len() as u32;
            // SAFETY: name capacity matches `length`; key class/time not requested.
            let status = unsafe {
                RegEnumKeyExW(
                    key.0,
                    index,
                    Some(PWSTR(name.as_mut_ptr())),
                    &mut length,
                    None,
                    None,
                    None,
                    None,
                )
            };
            if status == ERROR_NO_MORE_ITEMS {
                break;
            }
            if status != ERROR_SUCCESS {
                return Err(NativeError::win32("RegEnumKeyExW", status.0));
            }
            names.push(String::from_utf16_lossy(
                name.get(..length as usize)
                    .ok_or(NativeError::Malformed("registry key name length"))?,
            ));
        }
        Ok(names)
    }
}

#[cfg(windows)]
pub use native::read;
#[cfg(all(windows, test))]
pub use native::subkeys;

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16(text: &str) -> Vec<u8> {
        text.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    #[test]
    fn decode_covers_every_supported_kind_and_rejects_bad_sizes() {
        assert_eq!(
            decode(REG_SZ, &utf16("Windows 11 Home\0junk")),
            Some(RegValue::Text("Windows 11 Home".into()))
        );
        assert_eq!(decode(REG_SZ, &[0x41]), None);
        assert_eq!(
            decode(REG_MULTI_SZ, &utf16("a\0bc\0\0")),
            Some(RegValue::MultiText(vec!["a".into(), "bc".into()]))
        );
        assert_eq!(
            decode(REG_DWORD, &26100u32.to_le_bytes()).and_then(|v| v.as_u64()),
            Some(26100)
        );
        assert_eq!(decode(REG_DWORD, &[1, 2, 3]), None);
        assert_eq!(
            decode(REG_DWORD_BIG_ENDIAN, &[0, 0, 1, 0]),
            Some(RegValue::U32(256))
        );
        assert_eq!(
            decode(REG_QWORD, &7u64.to_le_bytes()),
            Some(RegValue::U64(7))
        );
        assert_eq!(decode(REG_BINARY, &[9]), Some(RegValue::Binary(vec![9])));
        assert_eq!(decode(0, &[]), None);
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "Read-only registry query of the Windows version key; no writes"]
    fn native_specs_registry_read_only_probe() {
        let path = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
        for name in ["ProductName", "DisplayVersion", "CurrentBuild", "UBR"] {
            println!("{name} = {:?}", read(Hive::LocalMachine, path, name));
        }
        println!(
            "missing = {:?}",
            read(Hive::LocalMachine, r"SOFTWARE\Trontop\NoSuchKey", "x")
        );
        println!(
            "subkeys(CentralProcessor) = {:?}",
            subkeys(
                Hive::LocalMachine,
                r"HARDWARE\DESCRIPTION\System\CentralProcessor",
                4
            )
            .map(|k| k.len())
        );
    }
}
