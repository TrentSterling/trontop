use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

#[derive(Debug)]
pub(super) struct Entry {
    pub path: PathBuf,
    pub bytes: u64,
    pub directory: bool,
}

#[derive(Debug)]
pub(super) struct Project {
    pub root: PathBuf,
    pub generated: PathBuf,
    pub kind: &'static str,
    pub bytes: u64,
}

#[derive(Default, Debug)]
pub(super) struct Scan {
    pub root: PathBuf,
    pub bytes: u64,
    pub files: u64,
    pub skipped: u64,
    pub links: u64,
    pub entries: Vec<Entry>,
    pub largest: Vec<(u64, PathBuf)>,
    pub projects: Vec<Project>,
    pub types: Vec<(String, (u64, u64))>,
    pub large_files: BTreeMap<u64, Vec<PathBuf>>,
}

pub(super) fn linked(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        use windows::Win32::Storage::FileSystem::{
            FILE_ATTRIBUTE_OFFLINE, FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS,
            FILE_ATTRIBUTE_RECALL_ON_OPEN, FILE_ATTRIBUTE_REPARSE_POINT,
        };
        meta.file_attributes()
            & (FILE_ATTRIBUTE_REPARSE_POINT.0
                | FILE_ATTRIBUTE_OFFLINE.0
                | FILE_ATTRIBUTE_RECALL_ON_OPEN.0
                | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS.0)
            != 0
    }
    #[cfg(not(windows))]
    meta.is_symlink()
}

struct Walker<'a> {
    scan: Scan,
    largest: BinaryHeap<Reverse<(u64, PathBuf)>>,
    types: BTreeMap<String, (u64, u64)>,
    cancel: &'a AtomicBool,
    progress: &'a AtomicU64,
}

#[derive(Default)]
struct Folder {
    bytes: u64,
    version_file: bool,
}

impl Walker<'_> {
    fn directory(&mut self, path: &Path, depth: usize) -> io::Result<Folder> {
        let mut folder = Folder::default();
        let mut markers = [false; 3];
        let mut outputs = [None, None, None];
        let entries = fs::read_dir(path)?;
        for item in entries {
            if self.cancel.load(Ordering::Relaxed) {
                return Err(io::ErrorKind::Interrupted.into());
            }
            let Ok(item) = item else {
                self.scan.skipped += 1;
                continue;
            };
            let child = item.path();
            let Ok(meta) = fs::symlink_metadata(&child) else {
                self.scan.skipped += 1;
                continue;
            };
            if linked(&meta) {
                self.scan.links += 1;
                continue;
            }
            let name = item.file_name();
            let is_dir = meta.is_dir();
            let bytes = if is_dir {
                if depth >= 255 {
                    self.scan.skipped += 1;
                    continue;
                }
                let nested = match self.directory(&child, depth + 1) {
                    Ok(nested) => nested,
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => return Err(e),
                    Err(_) => {
                        self.scan.skipped += 1;
                        continue;
                    }
                };
                match name.to_str() {
                    Some("ProjectSettings") => markers[0] = nested.version_file,
                    Some("Library") => outputs[0] = Some(nested.bytes),
                    Some("target") => outputs[1] = Some(nested.bytes),
                    Some("node_modules") => outputs[2] = Some(nested.bytes),
                    _ => {}
                }
                nested.bytes
            } else if meta.is_file() {
                match name.to_str() {
                    Some("ProjectVersion.txt") => folder.version_file = true,
                    Some("Cargo.toml") => markers[1] = true,
                    Some("package.json") => markers[2] = true,
                    _ => {}
                }
                self.scan.files += 1;
                if self.scan.files.is_multiple_of(128) {
                    self.progress.store(self.scan.files, Ordering::Relaxed);
                }
                let size = meta.len();
                let ext = child
                    .extension()
                    .map(|e| e.to_string_lossy().to_ascii_lowercase())
                    .unwrap_or_default();
                let (bytes, count) = self.types.entry(ext).or_default();
                *bytes += size;
                *count += 1;
                if size >= 1024 * 1024 {
                    self.scan
                        .large_files
                        .entry(size)
                        .or_default()
                        .push(child.clone());
                }
                if self.largest.len() < 50 || self.largest.peek().is_some_and(|r| size > r.0.0) {
                    if self.largest.len() == 50 {
                        self.largest.pop();
                    }
                    self.largest.push(Reverse((size, child.clone())));
                }
                size
            } else {
                continue;
            };
            folder.bytes = folder.bytes.saturating_add(bytes);
            if depth == 0 {
                self.scan.entries.push(Entry {
                    path: child,
                    bytes,
                    directory: is_dir,
                });
            }
        }
        for ((kind, output), (marker, bytes)) in [
            ("Unity", "Library"),
            ("Rust", "target"),
            ("Node", "node_modules"),
        ]
        .into_iter()
        .zip(markers.into_iter().zip(outputs))
        {
            if let (true, Some(bytes)) = (marker, bytes) {
                self.scan.projects.push(Project {
                    root: path.to_owned(),
                    generated: path.join(output),
                    kind,
                    bytes,
                });
            }
        }
        Ok(folder)
    }
}

pub(super) fn run(path: &Path, cancel: &AtomicBool, progress: &AtomicU64) -> io::Result<Scan> {
    if path.as_os_str().is_empty() {
        return Err(io::Error::other("Choose a folder to scan."));
    }
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_dir() || linked(&meta) {
        return Err(io::Error::other(
            "Choose a directory that is not a link or junction.",
        ));
    }
    let root = fs::canonicalize(path)?;
    let mut walker = Walker {
        scan: Scan {
            root: root.clone(),
            ..Default::default()
        },
        largest: Default::default(),
        types: Default::default(),
        cancel,
        progress,
    };
    walker.scan.bytes = walker.directory(&root, 0)?.bytes;
    walker
        .scan
        .entries
        .sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.path.cmp(&b.path)));
    walker.scan.projects.sort_by_key(|p| Reverse(p.bytes));
    walker.scan.large_files.retain(|_, v| v.len() > 1);
    walker.scan.types = walker.types.into_iter().collect();
    walker
        .scan
        .types
        .sort_by_key(|(_, (bytes, _))| Reverse(*bytes));
    walker.scan.largest = walker
        .largest
        .into_sorted_vec()
        .into_iter()
        .map(|r| r.0)
        .collect();
    Ok(walker.scan)
}

pub(super) fn changes(before: &Scan, after: &Scan) -> Vec<(PathBuf, i128)> {
    let mut deltas = BTreeMap::new();
    for e in &before.entries {
        deltas.insert(e.path.clone(), -i128::from(e.bytes));
    }
    for e in &after.entries {
        *deltas.entry(e.path.clone()).or_default() += i128::from(e.bytes);
    }
    let mut deltas: Vec<_> = deltas.into_iter().filter(|(_, n)| *n != 0).collect();
    deltas.sort_by_key(|(_, n)| Reverse(n.abs()));
    deltas
}
