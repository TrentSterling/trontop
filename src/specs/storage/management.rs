//! Decodes Windows Storage Management rows without native calls.
use super::*;
use crate::specs::native::wmi::{WmiRow, WmiValue};

fn number_of(row: &WmiRow, name: &str) -> Option<u32> {
    row.u64(name).and_then(|n| u32::try_from(n).ok())
}

fn string<'a>(row: &'a WmiRow, name: &str) -> Option<&'a str> {
    match row.get(name)? {
        WmiValue::Text(value) => {
            let value = value.trim();
            (!value.is_empty()).then_some(value)
        }
        _ => None,
    }
}

fn identity(row: &WmiRow) -> Option<(&str, u64)> {
    let format = row.u64("UniqueIdFormat")?;
    matches!(format, 0 | 1 | 2 | 3 | 8).then_some((string(row, "UniqueId")?, format))
}

fn letter(row: &WmiRow) -> Option<char> {
    let character = if let Some(value) = row.u64("DriveLetter") {
        char::from_u32(u32::from(u16::try_from(value).ok()?))?
    } else {
        let mut characters = string(row, "DriveLetter")?.chars();
        let character = characters.next()?;
        if characters.next().is_some() {
            return None;
        }
        character
    };
    character
        .is_ascii_alphabetic()
        .then(|| character.to_ascii_uppercase())
}

fn unique<'a>(mut rows: impl Iterator<Item = &'a WmiRow>) -> Result<Option<&'a WmiRow>, ()> {
    let first = rows.next();
    if rows.next().is_some() {
        Err(())
    } else {
        Ok(first)
    }
}

fn read_rows(
    query: &mut impl FnMut(&str) -> Result<Vec<WmiRow>, String>,
    class: &str,
    properties: &str,
    issues: &mut Vec<String>,
) -> Vec<WmiRow> {
    match query(&format!("SELECT {properties} FROM {class}")) {
        Ok(rows) => rows,
        Err(error) => {
            issues.push(format!("{class}: {error}"));
            Vec::new()
        }
    }
}

pub(super) fn collect(
    disks: &mut [Disk],
    issues: &mut Vec<String>,
    mut query: impl FnMut(&str) -> Result<Vec<WmiRow>, String>,
) -> Value {
    let physical = read_rows(
        &mut query,
        "MSFT_PhysicalDisk",
        "UniqueId, UniqueIdFormat, MediaType, SpindleSpeed, HealthStatus, Size",
        issues,
    );
    let os_disks = read_rows(
        &mut query,
        "MSFT_Disk",
        "Number, PartitionStyle, Size, UniqueId, UniqueIdFormat",
        issues,
    );
    for disk in disks.iter_mut() {
        let Some(number) = disk.number else {
            continue;
        };
        let os = match unique(
            os_disks
                .iter()
                .filter(|r| number_of(r, "Number") == Some(number)),
        ) {
            Ok(Some(row)) => row,
            Ok(None) => continue,
            Err(()) => {
                issues.push(format!(
                    "MSFT_Disk identity ambiguous for Disk {number}; metadata not joined."
                ));
                continue;
            }
        };
        disk.partition_style = os.u64("PartitionStyle");
        disk.bytes = os.u64("Size").filter(|s| *s > 0).or(disk.bytes);
        // DeviceId is a provider's address, not an OS disk number. Both classes
        // expose the VPD identifier and its format; require an unambiguous match.
        let Some(id) = identity(os) else {
            issues.push(format!(
                "Physical-media identity unavailable for Disk {number}; metadata not joined."
            ));
            continue;
        };
        match unique(physical.iter().filter(|row| identity(row) == Some(id))) {
            Ok(Some(row)) => {
                let size = row.u64("Size").filter(|s| *s > 0);
                if disk.bytes.zip(size).is_some_and(|(a, b)| a != b) {
                    issues.push(format!(
                        "Physical-media capacity disagrees for Disk {number}; metadata not joined."
                    ));
                    continue;
                }
                disk.bytes = disk.bytes.or(size);
                disk.media = row.u64("MediaType");
                disk.spindle = row.u64("SpindleSpeed");
                disk.health = row.u64("HealthStatus");
            }
            Ok(None) => {
                issues.push(format!(
                    "Physical-media identity unavailable for Disk {number}; metadata not joined."
                ));
            }
            Err(()) => {
                issues.push(format!(
                    "Physical-media identity ambiguous for Disk {number}; metadata not joined."
                ));
            }
        }
    }
    let volumes = read_rows(
        &mut query,
        "MSFT_Volume",
        "Path, FileSystem, FileSystemLabel, Size, SizeRemaining",
        issues,
    );
    let partitions = read_rows(
        &mut query,
        "MSFT_Partition",
        "DiskNumber, PartitionNumber, DriveLetter, Size, GptType, MbrType, AccessPaths",
        issues,
    );
    for row in partitions {
        let Some(number) = number_of(&row, "DiskNumber") else {
            continue;
        };
        let Some(disk) = disks.iter_mut().find(|d| d.number == Some(number)) else {
            continue;
        };
        let paths: Vec<_> = match row.get("AccessPaths") {
            Some(WmiValue::Array(values)) => values
                .iter()
                .filter_map(|v| match v {
                    WmiValue::Text(path) if !path.trim().is_empty() => Some(path.trim()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        let volume = match unique(volumes.iter().filter(|v| {
            string(v, "Path").is_some_and(|p| paths.iter().any(|a| a.eq_ignore_ascii_case(p)))
        })) {
            Ok(volume) => volume,
            Err(()) => {
                issues.push(format!(
                    "MSFT_Volume identity ambiguous for Disk {number}; volume fields not joined."
                ));
                None
            }
        };
        let kind = string(&row, "GptType")
            .filter(|g| g.trim_matches(['{', '}']) != "00000000-0000-0000-0000-000000000000")
            .map(|g| gpt_type(g).map_or_else(|| format!("GPT type {g}"), str::to_string))
            .or_else(|| {
                row.u64("MbrType")
                    .filter(|t| (1..=255).contains(t))
                    .map(|t| format!("MBR type {t:02X}h"))
            });
        let free = volume.and_then(|v| {
            let free = v.u64("SizeRemaining")?;
            if v.u64("Size").is_some_and(|size| free > size) {
                issues.push(format!("MSFT_Volume free space exceeds volume size for Disk {number}; free space unavailable."));
                None
            } else { Some(free) }
        });
        disk.partitions.push(Partition {
            number: number_of(&row, "PartitionNumber"),
            letter: letter(&row),
            bytes: row.u64("Size"),
            kind,
            file_system: volume
                .and_then(|v| string(v, "FileSystem"))
                .map(str::to_string),
            label: volume
                .and_then(|v| string(v, "FileSystemLabel"))
                .map(str::to_string),
            free,
        });
    }
    for disk in disks {
        disk.partitions
            .sort_by_key(|p| (p.number.is_none(), p.number));
    }
    match query("SELECT DeviceId FROM MSFT_StorageReliabilityCounter") {
        Ok(_) => Value::unavailable(
            "ATA SMART attribute tables need a pass-through command, which Trontop does not send",
        ),
        Err(error) => Value::unavailable(error),
    }
}

#[cfg(test)]
mod tests;
