use super::*;
use std::collections::{BTreeMap, hash_map::DefaultHasher};
use std::fs::File;
use std::hash::Hasher;
use std::io::Read;

#[derive(Default)]
pub(super) struct Report {
    pub groups: Vec<(u64, Vec<PathBuf>)>,
    pub skipped: usize,
}

fn checkpoint(stop: &AtomicBool) -> io::Result<()> {
    if stop.load(Ordering::Relaxed) {
        Err(io::ErrorKind::Interrupted.into())
    } else {
        Ok(())
    }
}

fn open(path: &Path, size: u64) -> io::Result<File> {
    let mut options = File::options();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
        use windows::Win32::{Foundation::HANDLE, Storage::FileSystem::*};
        options
            .share_mode(FILE_SHARE_READ.0)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0);
        let file = options.open(path)?;
        let mut info = BY_HANDLE_FILE_INFORMATION::default();
        unsafe {
            GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &mut info)?;
        }
        let meta = file.metadata()?;
        if info.nNumberOfLinks != 1
            || super::scan::linked(&meta)
            || !meta.is_file()
            || meta.len() != size
        {
            return Err(io::Error::other("Linked or changed file"));
        }
        Ok(file)
    }
    #[cfg(not(windows))]
    {
        let f = options.open(path)?;
        if f.metadata()?.len() != size {
            return Err(io::Error::other("Changed file"));
        }
        Ok(f)
    }
}

fn digest(path: &Path, size: u64, limit: u64, stop: &AtomicBool) -> io::Result<u64> {
    let mut file = open(path, size)?;
    let mut remaining = size.min(limit);
    let mut buf = [0; 32768];
    let mut hash = DefaultHasher::new();
    while remaining > 0 {
        checkpoint(stop)?;
        let n = remaining.min(buf.len() as u64) as usize;
        file.read_exact(&mut buf[..n])?;
        hash.write(&buf[..n]);
        remaining -= n as u64;
    }
    Ok(hash.finish())
}

fn equal(a: &Path, b: &Path, size: u64, stop: &AtomicBool) -> io::Result<bool> {
    let (mut left, mut right) = (open(a, size)?, open(b, size)?);
    let (mut lhs, mut rhs) = ([0; 32768], [0; 32768]);
    let mut remaining = size;
    while remaining > 0 {
        checkpoint(stop)?;
        let n = remaining.min(lhs.len() as u64) as usize;
        left.read_exact(&mut lhs[..n])?;
        right.read_exact(&mut rhs[..n])?;
        if lhs[..n] != rhs[..n] {
            return Ok(false);
        }
        remaining -= n as u64;
    }
    Ok(true)
}

fn buckets<'a>(
    paths: impl Iterator<Item = &'a PathBuf>,
    size: u64,
    limit: u64,
    stop: &AtomicBool,
    skipped: &mut usize,
) -> io::Result<BTreeMap<u64, Vec<&'a PathBuf>>> {
    let mut groups = BTreeMap::<_, Vec<_>>::new();
    for path in paths {
        checkpoint(stop)?;
        match digest(path, size, limit, stop) {
            Ok(hash) => groups.entry(hash).or_default().push(path),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => return Err(e),
            Err(_) => *skipped += 1,
        }
    }
    groups.retain(|_, v| v.len() > 1);
    Ok(groups)
}

pub(super) fn find(
    scan: &scan::Scan,
    stop: &AtomicBool,
    progress: &AtomicU64,
) -> io::Result<Report> {
    let mut report = Report::default();
    for (&size, paths) in &scan.large_files {
        let partial = buckets(paths.iter(), size, 4096, stop, &mut report.skipped)?;
        for group in partial.into_values() {
            let full = buckets(group.into_iter(), size, size, stop, &mut report.skipped)?;
            for matches in full.into_values() {
                let first = matches[0];
                let mut verified = vec![first.clone()];
                for path in &matches[1..] {
                    match equal(first, path, size, stop) {
                        Ok(true) => verified.push((*path).clone()),
                        Err(e) if e.kind() == io::ErrorKind::Interrupted => return Err(e),
                        _ => report.skipped += 1,
                    }
                }
                if verified.len() > 1 {
                    report.groups.push((size, verified));
                }
            }
        }
        progress.fetch_add(paths.len() as u64, Ordering::Relaxed);
    }
    report
        .groups
        .sort_by_key(|(size, paths)| std::cmp::Reverse(*size as u128 * (paths.len() - 1) as u128));
    Ok(report)
}
