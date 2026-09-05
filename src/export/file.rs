use super::{Capture, encode};
use std::fs::{self, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

const LIMIT: u64 = 64 * 1024 * 1024;
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

pub(super) struct Limited<W> {
    pub inner: W,
    pub written: u64,
    pub limit: u64,
}
impl<W: Write> Write for Limited<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() as u64 > self.limit.saturating_sub(self.written) {
            return Err(io::Error::other(
                "Export exceeds the 64 MiB file limit; nothing was replaced.",
            ));
        }
        let n = self.inner.write(bytes)?;
        self.written += n as u64;
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

pub(super) fn save(path: &Path, capture: &Capture, stop: &AtomicBool) -> io::Result<u64> {
    save_limited(path, capture, stop, LIMIT)
}

pub(super) fn save_limited(
    path: &Path,
    capture: &Capture,
    stop: &AtomicBool,
    limit: u64,
) -> io::Result<u64> {
    if !path.is_absolute() || path.file_name().is_none() {
        return Err(io::Error::other("Choose an absolute file path."));
    }
    if !path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(capture.options.format.extension()))
    {
        return Err(io::Error::other(
            "The filename extension must match the selected export format.",
        ));
    }
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("Missing export directory."))?;
    // Exclusively create a sibling, never truncate the chosen destination first.
    let (temporary, file) = (0..32)
        .find_map(|_| {
            let candidate = parent.join(format!(
                ".trontop-export-{}-{}.tmp",
                std::process::id(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ));
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&candidate)
            {
                Ok(file) => Some(Ok((Temporary(candidate), file))),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => None,
                Err(e) => Some(Err(e)),
            }
        })
        .unwrap_or_else(|| {
            Err(io::Error::other(
                "Could not reserve a temporary export file.",
            ))
        })?;
    // Declare writer after the cleanup guard so Windows closes it before removal.
    let mut writer = Limited {
        inner: BufWriter::new(file),
        written: 0,
        limit,
    };
    encode::write(&mut writer, capture, stop)?;
    writer.flush()?;
    writer.inner.get_ref().sync_all()?;
    let bytes = writer.written;
    drop(writer);
    if stop.load(Ordering::Acquire) {
        return Err(io::Error::other("Export cancelled before replacement."));
    }
    fs::rename(&temporary.0, path)?;
    Ok(bytes)
}
