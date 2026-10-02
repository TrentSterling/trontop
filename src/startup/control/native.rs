//! Windows registry state only; no program launch, elevation or file mutation.
use super::*;
use std::path::{Path, PathBuf};
use windows::Win32::Foundation::{
    CloseHandle, ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_PATH_NOT_FOUND, ERROR_SUCCESS, HANDLE,
};
use windows::Win32::System::Registry::*;
use windows::core::{PCWSTR, w};

const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APPROVED: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved";

struct Address {
    root: HKEY,
    path: String,
    view: REG_SAM_FLAGS,
}

fn run_address(source: Source) -> Option<Address> {
    let root = match source {
        Source::UserRun => HKEY_CURRENT_USER,
        Source::MachineRun | Source::MachineRun32 => HKEY_LOCAL_MACHINE,
        _ => return None,
    };
    Some(Address {
        root,
        path: RUN.into(),
        view: if source == Source::MachineRun32 {
            KEY_WOW64_32KEY
        } else {
            KEY_WOW64_64KEY
        },
    })
}

fn approval_address(source: Source) -> Address {
    Address {
        root: match source {
            Source::UserRun | Source::UserFolder => HKEY_CURRENT_USER,
            _ => HKEY_LOCAL_MACHINE,
        },
        path: format!(
            r"{}\{}",
            APPROVED,
            match source {
                Source::MachineRun32 => "Run32",
                Source::UserFolder | Source::MachineFolder => "StartupFolder",
                _ => "Run",
            }
        ),
        view: KEY_WOW64_64KEY,
    }
}

struct RegistryKey(HKEY);
impl Drop for RegistryKey {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
fn missing(status: windows::Win32::Foundation::WIN32_ERROR) -> bool {
    matches!(status, ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND)
}
fn win_error(verb: &str, status: windows::Win32::Foundation::WIN32_ERROR) -> String {
    format!(
        "Could not {verb} (Windows error {}). No elevation was attempted.",
        status.0
    )
}

impl RegistryKey {
    fn open(
        address: &Address,
        access: REG_SAM_FLAGS,
        tx: Option<&Transaction>,
        create: bool,
    ) -> Result<Option<Self>, String> {
        let path = wide(&address.path);
        let mut handle = HKEY::default();
        let rights = access | address.view;
        let status = unsafe {
            match (tx, create) {
                (Some(tx), true) => RegCreateKeyTransactedW(
                    address.root,
                    PCWSTR(path.as_ptr()),
                    None,
                    windows::core::PWSTR::null(),
                    REG_OPTION_NON_VOLATILE,
                    rights,
                    None,
                    &mut handle,
                    None,
                    tx.0,
                    None,
                ),
                (Some(tx), false) => RegOpenKeyTransactedW(
                    address.root,
                    PCWSTR(path.as_ptr()),
                    None,
                    rights,
                    &mut handle,
                    tx.0,
                    None,
                ),
                (None, false) => RegOpenKeyExW(
                    address.root,
                    PCWSTR(path.as_ptr()),
                    None,
                    rights,
                    &mut handle,
                ),
                (None, true) => {
                    return Err("Creating an approval key requires a transaction.".into());
                }
            }
        };
        if missing(status) {
            Ok(None)
        } else if status == ERROR_SUCCESS {
            Ok(Some(Self(handle)))
        } else {
            Err(win_error("open the startup registry key", status))
        }
    }

    fn read(&self, name: &str, limit: usize) -> Result<Option<RawValue>, String> {
        let name = wide(name);
        let mut bytes = vec![0u8; 256];
        for _ in 0..4 {
            let mut length = bytes.len() as u32;
            let mut kind = REG_VALUE_TYPE::default();
            let status = unsafe {
                RegQueryValueExW(
                    self.0,
                    PCWSTR(name.as_ptr()),
                    None,
                    Some(&mut kind),
                    Some(bytes.as_mut_ptr()),
                    Some(&mut length),
                )
            };
            if missing(status) {
                return Ok(None);
            }
            if length as usize > limit {
                return Err("The startup registry value exceeds the supported size.".into());
            }
            if status == ERROR_MORE_DATA && length as usize > bytes.len() {
                bytes.resize(length as usize, 0);
                continue;
            }
            if status != ERROR_SUCCESS || length as usize > bytes.len() {
                return Err(win_error("read the complete startup value", status));
            }
            bytes.truncate(length as usize);
            return Ok(Some(RawValue {
                kind: kind.0,
                bytes,
            }));
        }
        Err("The startup value kept changing during the read. Refresh before changing it.".into())
    }

    fn write_approval(&self, name: &str, approval: &Approval) -> Result<(), String> {
        let name = wide(name);
        let status = unsafe {
            match approval {
                Approval::Missing => RegDeleteValueW(self.0, PCWSTR(name.as_ptr())),
                Approval::Value(value) => RegSetValueExW(
                    self.0,
                    PCWSTR(name.as_ptr()),
                    None,
                    REG_VALUE_TYPE(value.kind),
                    Some(&value.bytes),
                ),
                Approval::Unreadable(_) => {
                    return Err("An unreadable approval record cannot be written.".into());
                }
            }
        };
        if status == ERROR_SUCCESS || (matches!(approval, Approval::Missing) && missing(status)) {
            Ok(())
        } else {
            Err(win_error("write the startup approval", status))
        }
    }
}

fn approval_at(address: &Address, name: &str) -> Approval {
    match RegistryKey::open(address, KEY_QUERY_VALUE, None, false)
        .and_then(|key| key.map_or(Ok(None), |key| key.read(name, APPROVAL_LIMIT)))
    {
        Ok(None) => Approval::Missing,
        Ok(Some(value)) => Approval::Value(value),
        Err(error) => Approval::Unreadable(error),
    }
}

pub fn observe_run(source: Source, name: &str, value: RawValue) -> Control {
    Control {
        registration: Some(Registration::Run(value)),
        approval: approval_at(&approval_address(source), name),
    }
}

pub fn folder_path(source: Source) -> Result<PathBuf, String> {
    use windows::Win32::UI::Shell::{
        FOLDERID_CommonStartup, FOLDERID_Startup, KF_FLAG_DEFAULT, SHGetKnownFolderPath,
    };
    let id = match source {
        Source::UserFolder => FOLDERID_Startup,
        Source::MachineFolder => FOLDERID_CommonStartup,
        _ => return Err("This source is not a Startup folder.".into()),
    };
    let path = unsafe { SHGetKnownFolderPath(&id, KF_FLAG_DEFAULT, None) }
        .map_err(|error| format!("Could not resolve the Windows Startup folder: {error}"))?;
    let decoded = unsafe { path.to_string() };
    unsafe {
        windows::Win32::System::Com::CoTaskMemFree(Some(path.0.cast()));
    }
    decoded
        .map(PathBuf::from)
        .map_err(|error| format!("Windows returned an invalid Startup folder path: {error}"))
}

fn file_identity(file: &std::fs::File) -> Result<FileIdentity, String> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_DIRECTORY, GetFileInformationByHandle,
    };
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    unsafe { GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &mut info) }
        .map_err(|error| format!("Could not verify the Startup file: {error}"))?;
    if info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY.0 != 0 {
        return Err("Startup directories cannot be controlled as files.".into());
    }
    let time = |value: windows::Win32::Foundation::FILETIME| {
        (u64::from(value.dwHighDateTime) << 32) | u64::from(value.dwLowDateTime)
    };
    Ok(FileIdentity {
        volume: info.dwVolumeSerialNumber,
        index: (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
        created: time(info.ftCreationTime),
        modified: time(info.ftLastWriteTime),
        length: (u64::from(info.nFileSizeHigh) << 32) | u64::from(info.nFileSizeLow),
    })
}

fn open_file(path: &Path, lock: bool) -> Result<std::fs::File, String> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows::Win32::Storage::FileSystem::{
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ,
        FILE_SHARE_WRITE,
    };
    std::fs::OpenOptions::new()
        .read(true)
        .access_mode(if lock {
            windows::Win32::Foundation::GENERIC_READ.0
        } else {
            FILE_READ_ATTRIBUTES.0
        })
        .share_mode(if lock {
            FILE_SHARE_READ.0
        } else {
            (FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE).0
        })
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0)
        .open(path)
        .map_err(|error| format!("Could not verify and retain the Startup file: {error}"))
}

pub fn observe_folder(source: Source, name: &str, path: &Path) -> Control {
    let registration = open_file(path, false)
        .and_then(|file| file_identity(&file))
        .ok()
        .map(Registration::File);
    Control {
        registration,
        approval: approval_at(&approval_address(source), name),
    }
}

struct Transaction(HANDLE);
impl Transaction {
    fn new() -> Result<Self, String> {
        unsafe {
            windows::Win32::Storage::FileSystem::CreateTransaction(
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
                0,
                0,
                5000,
                w!("Trontop confirmed startup approval"),
            )
        }
        .map(Self)
        .map_err(|error| {
            format!("Windows registry transactions are unavailable. No change was made: {error}")
        })
    }
    fn commit(&self) -> Result<(), String> {
        unsafe { windows::Win32::Storage::FileSystem::CommitTransaction(self.0) }.map_err(|error| format!("Windows could not confirm the startup transaction. Refresh to verify the current state: {error}"))
    }
}
impl Drop for Transaction {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Storage::FileSystem::RollbackTransaction(self.0);
            let _ = CloseHandle(self.0);
        }
    }
}

fn decode_run(raw: &RawValue) -> Option<String> {
    if !matches!(raw.kind, 1 | 2) || !raw.bytes.len().is_multiple_of(2) {
        return None;
    }
    let text = raw
        .bytes
        .chunks_exact(2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .take_while(|value| *value != 0)
        .collect::<Vec<_>>();
    String::from_utf16(&text).ok()
}

pub fn apply(request: &Request) -> Result<Receipt, Failure> {
    request.validate().map_err(Failure::unchanged)?;
    let row = &request.target;
    apply_at(
        request,
        &approval_address(row.source),
        run_address(row.source).as_ref(),
        if matches!(row.source, Source::UserFolder | Source::MachineFolder) {
            Some(folder_path(row.source).map_err(Failure::unchanged)?)
        } else {
            None
        },
    )
}

fn guard_run(
    run: &Address,
    tx: &Transaction,
    row: &StartupRow,
    control: &Control,
) -> Result<RegistryKey, Failure> {
    let key = RegistryKey::open(run, KEY_QUERY_VALUE | KEY_SET_VALUE, Some(tx), false)
        .map_err(Failure::unchanged)?
        .ok_or_else(|| Failure::unchanged("The original Run key is gone. No change was made."))?;
    let actual = key
        .read(&row.key, VALUE_LIMIT)
        .map_err(Failure::unchanged)?
        .ok_or_else(|| {
            Failure::unchanged("The startup registration was removed. No change was made.")
        })?;
    if control.registration != Some(Registration::Run(actual.clone()))
        || decode_run(&actual).as_deref() != Some(row.command.as_str())
    {
        return Err(Failure::unchanged(
            "The startup registration changed. Refresh and review it again. No change was made.",
        ));
    }
    // TxR read-only access does not conflict with non-transacted edits. Staging
    // identical bytes guards the registration and approval in one commit.
    key.write_approval(&row.key, &Approval::Value(actual.clone()))
        .map_err(Failure::unchanged)?;
    // A writer could have won before that guard was acquired. Check the
    // committed view after staging, while subsequent conflicting edits refuse.
    let committed = RegistryKey::open(run, KEY_QUERY_VALUE, None, false)
        .map_err(Failure::unchanged)?
        .ok_or_else(|| Failure::unchanged("The Run key disappeared while checking it."))?
        .read(&row.key, VALUE_LIMIT)
        .map_err(Failure::unchanged)?;
    if committed.as_ref() != Some(&actual) {
        return Err(Failure::unchanged(
            "The startup registration changed while acquiring its guard. No change was made.",
        ));
    }
    Ok(key)
}

fn apply_at(
    request: &Request,
    address: &Address,
    run: Option<&Address>,
    folder: Option<PathBuf>,
) -> Result<Receipt, Failure> {
    request.validate().map_err(Failure::unchanged)?;
    let row = &request.target;
    let control = row
        .control
        .as_ref()
        .ok_or_else(|| Failure::unchanged("Missing startup identity."))?;
    let tx = Transaction::new().map_err(Failure::unchanged)?;
    let _registration_guard = if let Some(run) = run {
        let key = guard_run(run, &tx, row, control)?;
        (Some(key), None)
    } else {
        let folder =
            folder.ok_or_else(|| Failure::unchanged("Missing Startup folder identity."))?;
        let path = folder.join(&row.key);
        if path.to_string_lossy().replace('/', "\\").to_lowercase()
            != row.command.replace('/', "\\").to_lowercase()
        {
            return Err(Failure::unchanged(
                "The file path no longer matches the Windows Startup folder. No change was made.",
            ));
        }
        let file = open_file(&path, true).map_err(Failure::unchanged)?;
        if control.registration
            != Some(Registration::File(
                file_identity(&file).map_err(Failure::unchanged)?,
            ))
        {
            return Err(Failure::unchanged(
                "The Startup file was replaced or changed. Refresh and review it again. No change was made.",
            ));
        }
        (None, Some(file))
    };
    let key = RegistryKey::open(address, KEY_QUERY_VALUE | KEY_SET_VALUE, Some(&tx), true)
        .map_err(Failure::unchanged)?
        .ok_or_else(|| Failure::unchanged("Windows returned no startup approval key."))?;
    let before = key
        .read(&row.key, APPROVAL_LIMIT)
        .map_err(Failure::unchanged)?
        .map_or(Approval::Missing, Approval::Value);
    if before != control.approval {
        return Err(Failure::unchanged(
            "The startup approval changed in another application. Refresh before changing it. No change was made.",
        ));
    }
    let after = match &request.action {
        Action::Restore(approval) => approval.clone(),
        Action::Enable | Action::Disable => {
            let time =
                unsafe { windows::Win32::System::SystemInformation::GetSystemTimeAsFileTime() };
            let at = (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime);
            before
                .changed(matches!(request.action, Action::Enable), at)
                .map_err(Failure::unchanged)?
        }
    };
    key.write_approval(&row.key, &after)
        .map_err(Failure::unchanged)?;
    let written = key
        .read(&row.key, APPROVAL_LIMIT)
        .map_err(Failure::unchanged)?
        .map_or(Approval::Missing, Approval::Value);
    if written != after {
        return Err(Failure::unchanged(
            "Startup approval readback did not match. The transaction was not committed.",
        ));
    }
    if approval_at(address, &row.key) != before {
        return Err(Failure::unchanged(
            "The startup approval changed while acquiring its guard. The transaction was not committed.",
        ));
    }
    request.validate().map_err(Failure::unchanged)?;
    tx.commit().map_err(Failure::uncertain)?;
    if approval_at(address, &row.key) != after {
        return Err(Failure::uncertain(
            "The approval changed after commit or could not be read back. Refresh to verify its current state.",
        ));
    }
    Ok(Receipt {
        target: row.clone(),
        before,
        after,
        observed_at: Instant::now(),
    })
}

#[cfg(test)]
mod tests;
