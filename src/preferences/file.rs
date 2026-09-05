use super::*;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;

const MAX_BYTES: u64 = 2 * 1024 * 1024;
static NEXT: AtomicU64 = AtomicU64::new(0);

pub(super) struct Store {
    directory: PathBuf,
    expected: Option<Vec<u8>>,
    values: BTreeMap<String, String>,
    loaded: bool,
}

impl Store {
    pub fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            expected: None,
            values: BTreeMap::new(),
            loaded: false,
        }
    }
}

fn checked_file(path: &Path, write_lock: bool) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options.read(true);
    if write_lock {
        options.write(true).create(true).truncate(false);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x0020_0000); // Open final reparse point itself.
    }
    let file = options.open(path).map_err(|e| {
        format!(
            "Cannot access settings ({:?}). Existing file preserved.",
            e.kind()
        )
    })?;
    let metadata = file
        .metadata()
        .map_err(|_| "Cannot inspect settings file.")?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("Settings links are not followed; existing file preserved.".into());
        }
    }
    if !metadata.is_file() || metadata.len() > MAX_BYTES {
        return Err("Settings file is not regular or exceeds 2 MiB; preserved unchanged.".into());
    }
    Ok(file)
}

fn read(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("Cannot inspect settings; existing file preserved.".into()),
        Ok(_) => {}
    }
    let mut bytes = Vec::new();
    checked_file(path, false)?
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read settings; existing file preserved.")?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("Settings exceed 2 MiB; file preserved.".into());
    }
    Ok(Some(bytes))
}

fn decode(bytes: &[u8], legacy: bool) -> Result<BTreeMap<String, String>, String> {
    let values: BTreeMap<String, String> = if legacy {
        ron::de::from_bytes(bytes).map_err(|_| "Legacy settings are invalid; file preserved.")?
    } else {
        let value: serde_json::Value = serde_json::from_slice(bytes)
            .map_err(|_| "Settings JSON is invalid; file preserved.")?;
        if value["format"].as_str() != Some("trontop-settings")
            || value["version"].as_u64() != Some(1)
        {
            return Err(
                "Settings format is unknown or newer than this build; file preserved.".into(),
            );
        }
        serde_json::from_value(value["values"].clone())
            .map_err(|_| "Settings entries are invalid; file preserved.")?
    };
    if values.len() > 64
        || values
            .iter()
            .any(|(k, v)| k.len() > 256 || v.len() > 1024 * 1024)
    {
        return Err("Settings entry limits exceeded; file preserved.".into());
    }
    Ok(values)
}

impl Backend for Store {
    fn load(&mut self) -> Result<Loaded, String> {
        if !self.directory.is_absolute() {
            return Err("Settings need a local absolute directory.".into());
        }
        #[cfg(windows)]
        {
            use std::path::{Component, Prefix};
            if !matches!(self.directory.components().next(), Some(Component::Prefix(p)) if matches!(p.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)))
            {
                return Err("Settings need a local drive, not a network path.".into());
            }
        }
        let expected = read(&self.directory.join(FILE_NAME))?;
        let values = if let Some(bytes) = &expected {
            decode(bytes, false)?
        } else if let Some(bytes) = read(&self.directory.join(LEGACY_NAME))? {
            decode(&bytes, true)?
        } else {
            BTreeMap::new()
        };
        let loaded = values_to_loaded(&values)?;
        self.expected = expected;
        self.values = values;
        self.loaded = true;
        Ok(loaded)
    }

    fn save(&mut self, snapshot: Snapshot, stop: &AtomicBool) -> Result<(), String> {
        if !self.loaded {
            return Err("Settings have not loaded; existing files preserved.".into());
        }
        let mut values = self.values.clone();
        values.insert(crate::theme::STORAGE_KEY.into(), snapshot.theme);
        values.insert(crate::theme_studio::LIBRARY_KEY.into(), snapshot.library);
        values.insert(
            "egui".into(),
            ron::to_string(&snapshot.memory).map_err(|_| "Cannot encode UI settings.")?,
        );
        values_to_loaded(&values)?;
        let bytes = serde_json::to_vec(
            &serde_json::json!({"format":"trontop-settings", "version":1, "values":values}),
        )
        .map_err(|_| "Cannot encode settings.")?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err("Settings exceed 2 MiB; existing file preserved.".into());
        }
        // Enforce the same entry limits on output, including preserved unknown
        // keys. Never produce a file that the next launch would reject.
        decode(&bytes, false)?;
        if stop.load(Ordering::Acquire) {
            return Err("Settings save cancelled before writing.".into());
        }
        fs::create_dir_all(&self.directory).map_err(|_| "Cannot create settings directory.")?;
        let lock = checked_file(&self.directory.join("settings-v3.lock"), true)?;
        lock.try_lock()
            .map_err(|_| "Another Trontop is saving settings. Retry when it finishes.")?;
        let destination = self.directory.join(FILE_NAME);
        if read(&destination)? != self.expected {
            return Err("Settings changed in another instance. Existing file preserved; export your theme before restarting.".into());
        }
        if self.expected.as_deref() == Some(bytes.as_slice()) {
            return Ok(());
        }
        let temp_path = self.directory.join(format!(
            ".settings-{}-{}.tmp",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .map_err(|_| "Cannot create staged settings file; existing file preserved.")?;
        let temp = Temporary(temp_path);
        // Explicitly close before cleanup on both success and error (Windows).
        let result = (|| {
            output
                .write_all(&bytes)
                .map_err(|_| "Cannot write staged settings; existing file preserved.")?;
            output
                .sync_all()
                .map_err(|_| "Cannot flush staged settings; existing file preserved.")?;
            Ok::<_, String>(())
        })();
        drop(output);
        result?;
        if stop.load(Ordering::Acquire) {
            return Err("Settings save cancelled before replacement.".into());
        }
        fs::rename(&temp.0, &destination)
            .map_err(|_| "Cannot replace settings; existing file preserved.")?;
        self.expected = Some(bytes);
        self.values = values;
        Ok(())
    }
}

struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
