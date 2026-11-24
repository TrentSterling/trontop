use crate::model::ProcessIdentity;
use std::{io, path::Path};

pub(super) struct FileUser {
    pub identity: ProcessIdentity,
    pub name: String,
}

#[cfg(windows)]
pub(super) fn query(path: &Path) -> io::Result<Vec<FileUser>> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Foundation::{ERROR_MORE_DATA, ERROR_SUCCESS};
    use windows::Win32::System::RestartManager::*;
    use windows::core::{PCWSTR, PWSTR};

    struct Session(u32);
    impl Drop for Session {
        fn drop(&mut self) {
            unsafe {
                let _ = RmEndSession(self.0);
            }
        }
    }

    if !path.is_file() {
        return Err(io::Error::other("Choose an existing file."));
    }
    let path = std::path::absolute(path)?;
    let wide: Vec<_> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut id = 0;
    let mut key = [0u16; CCH_RM_SESSION_KEY as usize + 1];
    unsafe {
        RmStartSession(&mut id, None, PWSTR(key.as_mut_ptr())).ok()?;
    }
    let session = Session(id);
    unsafe {
        RmRegisterResources(session.0, Some(&[PCWSTR(wide.as_ptr())]), None, None).ok()?;
    }
    let mut rows = Vec::<RM_PROCESS_INFO>::new();
    for _ in 0..5 {
        let (mut needed, mut count, mut reasons) = (0, rows.len() as u32, 0);
        let buffer = (!rows.is_empty()).then_some(rows.as_mut_ptr());
        let status = unsafe { RmGetList(session.0, &mut needed, &mut count, buffer, &mut reasons) };
        if status == ERROR_SUCCESS {
            rows.truncate(count as usize);
            return Ok(rows
                .into_iter()
                .map(|r| {
                    let ft = r.Process.ProcessStartTime;
                    let end = r
                        .strAppName
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(r.strAppName.len());
                    FileUser {
                        identity: ProcessIdentity {
                            pid: r.Process.dwProcessId,
                            created_at_100ns: u64::from(ft.dwHighDateTime) << 32
                                | u64::from(ft.dwLowDateTime),
                        },
                        name: String::from_utf16_lossy(&r.strAppName[..end]),
                    }
                })
                .collect());
        }
        if status != ERROR_MORE_DATA {
            return Err(io::Error::from_raw_os_error(status.0 as i32));
        }
        rows.resize(needed as usize, Default::default());
    }
    Err(io::Error::other(
        "The process list kept changing. Run the lookup again.",
    ))
}

#[cfg(not(windows))]
pub(super) fn query(_: &Path) -> io::Result<Vec<FileUser>> {
    Err(io::ErrorKind::Unsupported.into())
}
