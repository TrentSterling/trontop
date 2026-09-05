//! Small, local-only failure records. Never accepts a panic/error payload or snapshot.
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub const LOG_NAME: &str = "failures-v1.jsonl";
pub const LOCATION_HINT: &str = "%LOCALAPPDATA%/Trontop/failures-v1.jsonl";
const HEADER: &str = "{\"format\":\"trontop-failure-log-v1\"}\n";
const MAX_RECORDS: usize = 32;
const MAX_RECORD_BYTES: usize = 2048;
const MAX_FILE_BYTES: usize = HEADER.len() + MAX_RECORDS * MAX_RECORD_BYTES;

#[derive(Clone, Copy)]
pub enum Kind {
    RustPanic,
    AppCreation,
    WindowSystem,
    EventLoop,
    Graphics,
}

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Self::RustPanic => "rust_panic",
            Self::AppCreation => "app_creation",
            Self::WindowSystem => "window_system",
            Self::EventLoop => "event_loop",
            Self::Graphics => "graphics",
        }
    }
}

pub struct Recorder {
    path: Option<PathBuf>,
    writing: AtomicBool,
}

impl Recorder {
    pub fn new(path: Option<PathBuf>) -> Self {
        Self {
            path,
            writing: AtomicBool::new(false),
        }
    }

    // Hook installation itself does not create/read a file or start a worker.
    // The previous hook still runs, preserving stderr/debugger behavior.
    pub fn install(self: Arc<Self>) {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let location = info
                .location()
                .map(|at| (at.file(), at.line(), at.column()));
            self.record(Kind::RustPanic, location);
            previous(info);
        }));
    }

    pub fn record(&self, kind: Kind, location: Option<(&str, u32, u32)>) {
        let Some(path) = &self.path else { return };
        if self.writing.swap(true, Ordering::AcqRel) {
            return;
        }
        struct Reset<'a>(&'a AtomicBool);
        impl Drop for Reset<'_> {
            fn drop(&mut self) {
                self.0.store(false, Ordering::Release);
            }
        }
        let _reset = Reset(&self.writing);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|value| u64::try_from(value.as_millis()).ok());
        let current = std::thread::current();
        if let Ok(record) = encode(kind, location, current.name(), now) {
            // Reporting cannot replace the original error. File I/O is best effort;
            // try_lock never waits on another instance. Disk I/O has no time guarantee.
            let _ = append(path, &record);
        }
    }
}

fn thread_role(name: Option<&str>) -> &'static str {
    // Dependency/task thread names could contain user data; retain only our known roles.
    match name {
        Some("main") => "main",
        Some("trontop-sampler") => "sampler",
        Some("trontop-tray") => "tray",
        Some("trontop-icons") => "process_icons",
        Some("trontop-service-control") => "service_control",
        Some("trontop-export") => "export",
        Some("trontop-startup-inventory") => "startup_inventory",
        Some("trontop-service-inventory") => "service_inventory",
        Some("trontop-drive-sensor") => "drive_sensor",
        Some("trontop-storage") => "storage_inventory",
        _ => "other",
    }
}

fn source_basename(path: &str) -> String {
    // This is a compiled source filename, never a process or user document path.
    path.rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
        .chars()
        .take(80)
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn encode(
    kind: Kind,
    location: Option<(&str, u32, u32)>,
    thread: Option<&str>,
    unix_ms: Option<u64>,
) -> io::Result<Vec<u8>> {
    let record = serde_json::json!({
        "schema": "trontop-failure-v1",
        "version": env!("CARGO_PKG_VERSION"),
        "build": env!("TRONTOP_BUILD_ID"),
        "target": env!("TRONTOP_BUILD_TARGET"),
        "profile": if cfg!(debug_assertions) { "debug" } else { "optimized_release" },
        "kind": kind.label(),
        "utc_unix_ms": unix_ms,
        "thread_role": thread_role(thread),
        "source_file": location.map(|(path, _, _)| source_basename(path)),
        "source_line": location.map(|(_, line, _)| line),
        "source_column": location.map(|(_, _, column)| column),
    });
    let mut bytes = serde_json::to_vec(&record)?;
    bytes.push(b'\n');
    if bytes.len() > MAX_RECORD_BYTES {
        return Err(io::Error::other("Failure record exceeds limit"));
    }
    Ok(bytes)
}

fn invalid_log() -> io::Error {
    io::Error::other("Unrecognized or oversized failure log; preserved unchanged")
}

fn local_absolute(path: &Path) -> bool {
    if !path.is_absolute() {
        return false;
    }
    #[cfg(windows)]
    {
        use std::path::{Component, Prefix};
        matches!(path.components().next(), Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)))
    }
    #[cfg(not(windows))]
    {
        true
    }
}

fn open_log(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // Open the final reparse point itself, not a substituted target.
        options.custom_flags(0x0020_0000); // FILE_FLAG_OPEN_REPARSE_POINT
    }
    let file = options.open(path)?;
    let meta = file.metadata()?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return Err(invalid_log());
        }
    }
    if !meta.is_file() {
        return Err(invalid_log());
    }
    Ok(file)
}

fn append(path: &Path, record: &[u8]) -> io::Result<()> {
    if record.len() > MAX_RECORD_BYTES || record.last() != Some(&b'\n') {
        return Err(invalid_log());
    }
    if !local_absolute(path) {
        return Err(io::Error::other("Failure log needs a local absolute path"));
    }
    fs::create_dir_all(path.parent().ok_or_else(invalid_log)?)?;
    let mut file = open_log(path)?;
    file.try_lock().map_err(io::Error::from)?;
    // The file handle owns the OS lock, including across processes. Closing after
    // success/error/panic releases it. Never truncate before ownership and validation.
    if file.metadata()?.len() > MAX_FILE_BYTES as u64 {
        return Err(invalid_log());
    }
    let mut existing = Vec::new();
    (&mut file)
        .take(MAX_FILE_BYTES as u64 + 1)
        .read_to_end(&mut existing)?;
    if existing.len() > MAX_FILE_BYTES {
        return Err(invalid_log());
    }
    let existing = if existing.is_empty() {
        ""
    } else {
        std::str::from_utf8(&existing)
            .map_err(|_| invalid_log())?
            .strip_prefix(HEADER)
            .ok_or_else(invalid_log)?
    };
    let mut lines = Vec::new();
    for line in existing.split_inclusive('\n') {
        // An interrupted append may leave an incomplete final line. Preserve all
        // complete records and recover at the next write; do not accept corrupt lines.
        if !line.ends_with('\n') {
            break;
        }
        let value: serde_json::Value = serde_json::from_str(line).map_err(|_| invalid_log())?;
        if line.len() > MAX_RECORD_BYTES
            || value.get("schema").and_then(|v| v.as_str()) != Some("trontop-failure-v1")
        {
            return Err(invalid_log());
        }
        lines.push(line);
    }
    if lines.len() > MAX_RECORDS {
        return Err(invalid_log());
    }
    let mut output = Vec::with_capacity(MAX_FILE_BYTES);
    output.extend_from_slice(HEADER.as_bytes());
    for line in lines
        .iter()
        .skip(lines.len().saturating_sub(MAX_RECORDS - 1))
    {
        output.extend_from_slice(line.as_bytes());
    }
    output.extend_from_slice(record);
    // Bounded rolling journal, not a transaction/dump service. A failure during this
    // write can leave a partial log; successful write is not a power-loss guarantee.
    file.seek(SeekFrom::Start(0))?;
    file.write_all(&output)?;
    file.set_len(output.len() as u64)?;
    Ok(())
}

#[cfg(test)]
mod tests;
