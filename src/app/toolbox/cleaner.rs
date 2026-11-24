use super::scan::linked;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

mod catalog;
pub(super) use catalog::{Rule, preview};
#[cfg(test)]
pub(super) use catalog::{analyze, inspect};

#[cfg(windows)]
pub(super) fn disk_cleanup_program() -> io::Result<PathBuf> {
    Ok(system_directory()?.join("cleanmgr.exe"))
}

#[cfg(windows)]
pub(super) fn system_directory() -> io::Result<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    use windows::Win32::System::SystemInformation::GetSystemDirectoryW;
    let mut buf = [0; 32768];
    let len = unsafe { GetSystemDirectoryW(Some(&mut buf)) } as usize;
    if len == 0 {
        return Err(io::Error::last_os_error());
    }
    if len >= buf.len() {
        return Err(io::Error::other(
            "System directory exceeds the Windows path limit",
        ));
    }
    Ok(PathBuf::from(std::ffi::OsString::from_wide(&buf[..len])))
}

#[derive(Clone, Debug, PartialEq)]
struct Stamp {
    id: (u32, u32, u32),
    created: SystemTime,
    modified: SystemTime,
    bytes: u64,
}

#[derive(Clone, Debug)]
pub(super) struct Candidate {
    pub path: PathBuf,
    pub rule: Rule,
    stamp: Stamp,
}

impl Candidate {
    pub fn bytes(&self) -> u64 {
        self.stamp.bytes
    }
}

#[derive(Default)]
pub(super) struct Preview {
    pub files: Vec<Candidate>,
    pub issues: Vec<String>,
    pub skipped: usize,
    pub groups: Vec<Group>,
    pub bytes: u64,
}

pub(super) struct Group {
    pub rule: Rule,
    pub range: std::ops::Range<usize>,
    pub bytes: u64,
}

#[derive(Default)]
pub(super) struct Receipt {
    pub deleted: usize,
    pub bytes: u64,
    pub failures: Vec<String>,
}

pub(super) fn apply(files: Vec<Candidate>) -> Receipt {
    let running = if files.iter().any(|c| c.rule.owner_exe().is_some()) {
        use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
        let mut system = System::new();
        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing(),
        );
        system
            .processes()
            .values()
            .map(|p| p.name().to_string_lossy().to_ascii_lowercase())
            .collect()
    } else {
        Default::default()
    };
    apply_with_processes(files, &running)
}

pub(super) fn apply_with_processes(
    files: Vec<Candidate>,
    running: &std::collections::HashSet<String>,
) -> Receipt {
    let mut receipt = Receipt::default();
    for candidate in files {
        if let Some(exe) = candidate.rule.owner_exe()
            && (running.is_empty() || running.contains(exe))
        {
            receipt.failures.push(format!(
                "{}: close {exe} and preview again",
                candidate.path.display()
            ));
            continue;
        }
        let result = native::open(&candidate.path, true).and_then(|(file, stamp)| {
            if stamp != candidate.stamp
                || !candidate
                    .rule
                    .matches(&candidate.path, &stamp, SystemTime::now())
            {
                return Err(io::Error::other("File changed since preview"));
            }
            native::delete(&file)
        });
        match result {
            Ok(()) => {
                receipt.deleted += 1;
                receipt.bytes += candidate.bytes();
            }
            Err(e) => receipt
                .failures
                .push(format!("{}: {e}", candidate.path.display())),
        }
    }
    receipt
}

#[cfg(windows)]
mod native {
    use super::*;
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Storage::FileSystem::*;

    pub(super) fn open(path: &Path, deleting: bool) -> io::Result<(fs::File, Stamp)> {
        open_checked(path, deleting, false)
    }

    pub(super) fn open_checked(
        path: &Path,
        deleting: bool,
        compare: bool,
    ) -> io::Result<(fs::File, Stamp)> {
        let mut options = fs::OpenOptions::new();
        options
            .access_mode(
                FILE_READ_ATTRIBUTES.0
                    | if compare { FILE_READ_DATA.0 } else { 0 }
                    | if deleting { DELETE.0 } else { 0 },
            )
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0);
        if deleting {
            options.share_mode(0);
        } else if compare {
            options.share_mode(FILE_SHARE_READ.0);
        }
        let file = options.open(path)?;
        let meta = file.metadata()?;
        if !meta.is_file() || linked(&meta) || meta.permissions().readonly() {
            return Err(io::Error::other(
                "Skipped protected, linked or non-regular file",
            ));
        }
        let handle = HANDLE(file.as_raw_handle());
        let mut info = BY_HANDLE_FILE_INFORMATION::default();
        // Both the fingerprint and deletion refer to this same open file.
        unsafe {
            GetFileInformationByHandle(handle, &mut info)?;
        }
        if info.nNumberOfLinks != 1 || info.nFileIndexHigh == 0 && info.nFileIndexLow == 0 {
            return Err(io::Error::other(
                "File has multiple links or lacks a stable identity",
            ));
        }
        let mut name = vec![0u16; 512];
        loop {
            let len = unsafe { GetFinalPathNameByHandleW(handle, &mut name, FILE_NAME_NORMALIZED) }
                as usize;
            if len == 0 {
                return Err(io::Error::last_os_error());
            }
            if len < name.len() {
                name.truncate(len);
                break;
            }
            name.resize(len + 1, 0);
        }
        if Path::new(&OsString::from_wide(&name)) != path {
            return Err(io::Error::other("File resolved outside the previewed path"));
        }
        let stamp = Stamp {
            id: (
                info.dwVolumeSerialNumber,
                info.nFileIndexHigh,
                info.nFileIndexLow,
            ),
            created: meta.created()?,
            modified: meta.modified()?,
            bytes: meta.len(),
        };
        Ok((file, stamp))
    }

    pub(super) fn delete(file: &fs::File) -> io::Result<()> {
        let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
        unsafe {
            SetFileInformationByHandle(
                HANDLE(file.as_raw_handle()),
                FileDispositionInfo,
                (&raw const disposition).cast(),
                size_of::<FILE_DISPOSITION_INFO>() as u32,
            )?;
        }
        Ok(())
    }
}

#[cfg(windows)]
#[derive(Clone, Debug)]
pub(super) struct CopyReview {
    pub keep: PathBuf,
    pub remove: PathBuf,
    keep_stamp: Stamp,
    remove_stamp: Stamp,
}

#[cfg(windows)]
impl CopyReview {
    pub fn capture(keep: PathBuf, remove: PathBuf, expected: u64) -> io::Result<Self> {
        let (_, keep_stamp) = native::open(&keep, false)?;
        let (_, remove_stamp) = native::open(&remove, false)?;
        if keep_stamp.id == remove_stamp.id
            || keep_stamp.bytes != expected
            || remove_stamp.bytes != expected
        {
            return Err(io::Error::other("Duplicate group changed. Scan again."));
        }
        Ok(Self {
            keep,
            remove,
            keep_stamp,
            remove_stamp,
        })
    }
    pub fn apply(&self) -> io::Result<()> {
        use std::io::Read;
        let (mut keep, ks) = native::open_checked(&self.keep, false, true)?;
        let (mut remove, rs) = native::open_checked(&self.remove, true, true)?;
        if ks != self.keep_stamp || rs != self.remove_stamp || ks.id == rs.id {
            return Err(io::Error::other("A duplicate changed since review."));
        }
        let (mut left, mut right) = ([0u8; 32768], [0u8; 32768]);
        let mut remaining = ks.bytes;
        while remaining > 0 {
            let n = remaining.min(left.len() as u64) as usize;
            keep.read_exact(&mut left[..n])?;
            remove.read_exact(&mut right[..n])?;
            if left[..n] != right[..n] {
                return Err(io::Error::other(
                    "File contents differ. Both files were kept.",
                ));
            }
            remaining -= n as u64;
        }
        native::delete(&remove)?;
        drop(remove);
        drop(keep);
        Ok(())
    }
}

#[cfg(not(windows))]
mod native {
    use super::*;
    pub(super) fn open(_: &Path, _: bool) -> io::Result<(fs::File, Stamp)> {
        Err(io::ErrorKind::Unsupported.into())
    }
    pub(super) fn delete(_: &fs::File) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
}
