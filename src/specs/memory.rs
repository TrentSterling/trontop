//! RAM section (lane: memory).
//!
//! Scope: installed vs usable capacity, per-DIMM data from SMBIOS type 17
//! (slot/bank locator, size, type DDR4/DDR5, configured and rated speed,
//! manufacturer, part number; serial is private), type 16 array (slots, max
//! capacity), channel/rank where encoded, live usage (LiveKey::MemoryUsed and
//! LiveKey::MemoryCommit). Never "Number of SPD modules: 0": SPD needs a driver,
//! SMBIOS does not.
use super::native::smbios::{self, Table};
use super::{Context, Group, LiveKey, Row, Section, SectionId, SummaryLine, Value};
use crate::format;

const NOT_SET: &str = "not set by the manufacturer";

/// One SMBIOS type 17 memory device (a slot, populated or not).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Dimm {
    locator: Option<String>,
    bank: Option<String>,
    /// None when the slot is empty.
    bytes: Option<u64>,
    kind: Option<&'static str>,
    form: Option<&'static str>,
    speed: Option<u32>,
    configured: Option<u32>,
    manufacturer: Option<String>,
    part: Option<String>,
    serial: Option<String>,
    rank: Option<u8>,
    data_width: Option<u16>,
    millivolts: Option<u16>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Array {
    slots: Option<u32>,
    max_bytes: Option<u64>,
    ecc: Option<&'static str>,
}

fn memory_type(code: u8) -> Option<&'static str> {
    Some(match code {
        0x03 => "DRAM",
        0x0F => "SDRAM",
        0x12 => "DDR",
        0x13 => "DDR2",
        0x14 => "DDR2 FB-DIMM",
        0x18 => "DDR3",
        0x1A => "DDR4",
        0x1B => "LPDDR",
        0x1C => "LPDDR2",
        0x1D => "LPDDR3",
        0x1E => "LPDDR4",
        0x1F => "Logical non-volatile",
        0x20 => "HBM",
        0x21 => "HBM2",
        0x22 => "DDR5",
        0x23 => "LPDDR5",
        0x24 => "HBM3",
        _ => return None,
    })
}

fn form_factor(code: u8) -> Option<&'static str> {
    Some(match code {
        0x03 => "SIMM",
        0x09 => "DIMM",
        0x0C => "RIMM",
        0x0D => "SODIMM",
        0x0F => "FB-DIMM",
        0x10 => "Die",
        0x11 => "CAMM",
        _ => return None,
    })
}

fn ecc_type(code: u8) -> Option<&'static str> {
    Some(match code {
        0x03 => "None",
        0x04 => "Parity",
        0x05 => "Single-bit ECC",
        0x06 => "Multi-bit ECC",
        0x07 => "CRC",
        _ => return None,
    })
}

fn text(s: &smbios::Structure<'_>, offset: usize) -> Option<String> {
    s.string(offset).filter(|t| !smbios::is_placeholder(t))
}

/// Size per DMTF DSP0134 7.18.5: 0 empty, 0xFFFF unknown, 0x7FFF extended
/// (MB at 0x1C), bit 15 set means KB units.
fn dimm_bytes(s: &smbios::Structure<'_>) -> Option<u64> {
    const MB: u64 = 1024 * 1024;
    match s.word(0x0C)? {
        0 | 0xFFFF => None,
        0x7FFF => s
            .dword(0x1C)
            .map(|mb| u64::from(mb & 0x7FFF_FFFF) * MB)
            .filter(|b| *b > 0),
        size if size & 0x8000 != 0 => Some(u64::from(size & 0x7FFF) * 1024),
        size => Some(u64::from(size) * MB),
    }
}

/// Word speed with the SMBIOS 3.3 extended dword for 0xFFFF.
fn speed(s: &smbios::Structure<'_>, word: usize, extended: usize) -> Option<u32> {
    match s.word(word)? {
        0 => None,
        0xFFFF => s
            .dword(extended)
            .map(|v| v & 0x7FFF_FFFF)
            .filter(|v| *v > 0),
        value => Some(u32::from(value)),
    }
}

fn dimms(table: &Table) -> Vec<Dimm> {
    table
        .of_type(17)
        .map(|s| Dimm {
            locator: text(&s, 0x10),
            bank: text(&s, 0x11),
            bytes: dimm_bytes(&s),
            kind: s.byte(0x12).and_then(memory_type),
            form: s.byte(0x0E).and_then(form_factor),
            speed: speed(&s, 0x15, 0x54),
            configured: speed(&s, 0x20, 0x58),
            manufacturer: text(&s, 0x17),
            part: text(&s, 0x1A),
            serial: text(&s, 0x18),
            rank: s.byte(0x1B).map(|b| b & 0x0F).filter(|r| *r > 0),
            data_width: s.word(0x0A).filter(|w| *w != 0 && *w != 0xFFFF),
            millivolts: s.word(0x26).filter(|v| *v != 0),
        })
        .collect()
}

fn array(table: &Table) -> Option<Array> {
    // Use 03h (system memory) arrays only; video or flash arrays are not RAM.
    let arrays = table
        .of_type(16)
        .filter(|s| s.byte(0x05) == Some(0x03))
        .collect::<Vec<_>>();
    if arrays.is_empty() {
        return None;
    }
    // Firmware words and qwords: add in a wider type or checked, never wrap.
    let slots = arrays
        .iter()
        .filter_map(|s| s.word(0x0D))
        .map(u32::from)
        .sum::<u32>();
    let max_bytes = arrays.iter().try_fold(0u64, |total, s| {
        let bytes = match s.dword(0x07)? {
            0x8000_0000 => s.qword(0x0F)?,
            kb => u64::from(kb) * 1024,
        };
        total.checked_add(bytes)
    });
    Some(Array {
        slots: (slots > 0).then_some(slots),
        max_bytes: max_bytes.filter(|b| *b > 0),
        ecc: arrays.first().and_then(|s| s.byte(0x06)).and_then(ecc_type),
    })
}

/// "Controller0-ChannelB-DIMM1" as "B". Only the firmware's own slot label.
fn channel(locator: &str) -> Option<String> {
    let at = locator.to_ascii_lowercase().find("channel")?;
    let rest = &locator[at + "channel".len()..];
    let name = rest
        .trim_start_matches([' ', '_'])
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect::<String>();
    (!name.is_empty()).then_some(name)
}

fn mts(value: u32) -> String {
    format!("{value} MT/s ({} MHz clock)", value / 2)
}

/// Pure mapping from SMBIOS and Windows totals to the section.
fn build(
    table: Option<&Table>,
    installed: Option<u64>,
    usable: Option<u64>,
    issues: Vec<String>,
) -> Section {
    let mut section = Section::new(SectionId::Memory);
    for issue in issues {
        section.push_issue(issue);
    }
    let dimms = table.map(dimms).unwrap_or_default();
    let array = table.and_then(array);
    let populated = dimms
        .iter()
        .filter(|d| d.bytes.is_some())
        .collect::<Vec<_>>();
    let smbios_total = populated.iter().filter_map(|d| d.bytes).sum::<u64>();
    let installed = installed.or((smbios_total > 0).then_some(smbios_total));
    let mut kinds = populated.iter().filter_map(|d| d.kind).collect::<Vec<_>>();
    kinds.dedup();
    let kind = (kinds.len() == 1).then(|| kinds[0]);
    let configured = populated.iter().filter_map(|d| d.configured).min();

    let mut headline = installed.map_or_else(|| "Memory".to_string(), format::bytes);
    if let Some(kind) = kind {
        headline.push_str(&format!(" {kind}"));
    }
    if let Some(speed) = configured {
        headline.push_str(&format!(" @ {speed} MT/s"));
    }
    if !dimms.is_empty() {
        headline.push_str(&format!(" ({} of {} slots)", populated.len(), dimms.len()));
    }
    section.push_summary(SummaryLine::known(headline).live(LiveKey::MemoryUsed));

    let mut overview = Group::new("Memory")
        .row(
            Row::new(
                "Installed",
                Value::from_option(
                    installed.map(format::bytes),
                    "Windows did not report installed memory",
                ),
            )
            .note("GetPhysicallyInstalledSystemMemory (from the firmware's SMBIOS table)"),
        )
        .row(
            Row::new(
                "Usable by Windows",
                Value::from_option(usable.map(format::bytes), "GlobalMemoryStatusEx failed"),
            )
            .note("GlobalMemoryStatusEx total physical memory"),
        );
    if let (Some(installed), Some(usable)) = (installed, usable)
        && installed >= usable
    {
        overview.push_row(
            Row::known("Reserved by hardware", format::bytes(installed - usable))
                .note("Installed minus usable: firmware, integrated graphics and device memory"),
        );
    }
    overview.push_row(Row::new(
        "Type",
        Value::from_option(
            kind.map(str::to_string)
                .or_else(|| (kinds.len() > 1).then(|| kinds.join(", "))),
            NOT_SET,
        ),
    ));
    overview.push_row(Row::new(
        "Slots used",
        if dimms.is_empty() {
            Value::unavailable("the firmware lists no SMBIOS memory devices")
        } else {
            let slots = array
                .as_ref()
                .and_then(|a| a.slots)
                .map_or(dimms.len(), |n| n as usize);
            Value::known(format!("{} of {slots}", populated.len()))
        },
    ));
    if let Some(array) = &array {
        overview.push_row(Row::new(
            "Maximum capacity",
            Value::from_option(array.max_bytes.map(format::bytes), NOT_SET),
        ));
        overview.push_row(Row::new(
            "Error correction",
            Value::from_option(array.ecc, NOT_SET),
        ));
    }
    let mut channels = populated
        .iter()
        .map(|d| d.locator.as_deref().and_then(channel))
        .collect::<Option<Vec<_>>>()
        .unwrap_or_default();
    channels.sort();
    channels.dedup();
    overview.push_row(
        Row::new(
            "Channels populated",
            if channels.is_empty() {
                Value::unavailable("the slot labels do not name channels")
            } else {
                let mode = match channels.len() {
                    1 => "Single",
                    2 => "Dual",
                    3 => "Triple",
                    4 => "Quad",
                    _ => "Multi",
                };
                Value::known(format!("{mode} ({})", channels.join(", ")))
            },
        )
        .note("From the firmware's slot labels (SMBIOS type 17 device locator)"),
    );
    overview.push_row(Row::new(
        "Configured speed",
        Value::from_option(configured.map(mts), NOT_SET),
    ));
    overview.push_row(Row::live("In use", LiveKey::MemoryUsed));
    overview.push_row(Row::live("Commit charge", LiveKey::MemoryCommit));
    overview.push_row(
        Row::unavailable(
            "Timings (CL-tRCD-tRP-tRAS)",
            "memory controller registers and SPD need a kernel driver",
        )
        .note("SMBIOS does not carry timings; Trontop never installs a driver"),
    );
    overview.push_row(
        Row::known(
            "Module data source",
            "SMBIOS type 17 (no SPD or driver needed)",
        )
        .note("Speccy's 'SPD modules: 0' comes from SMBus access; SMBIOS lists each module anyway"),
    );
    section.push_group(overview);

    for (index, dimm) in dimms.iter().enumerate() {
        let name = dimm
            .locator
            .clone()
            .unwrap_or_else(|| format!("Slot {}", index + 1));
        let Some(bytes) = dimm.bytes else {
            section.push_group(
                Group::new(format!("{name}: empty"))
                    .collapsed()
                    .kv("Bank", Value::from_option(dimm.bank.clone(), NOT_SET)),
            );
            continue;
        };
        let mut group = Group::new(format!("{name}: {}", format::bytes(bytes)));
        group.push_row(Row::known("Size", format::bytes(bytes)));
        group.push_row(Row::new("Type", Value::from_option(dimm.kind, NOT_SET)));
        group.push_row(Row::new(
            "Form factor",
            Value::from_option(dimm.form, NOT_SET),
        ));
        group.push_row(Row::new(
            "Manufacturer",
            Value::from_option(dimm.manufacturer.clone(), NOT_SET),
        ));
        group.push_row(Row::new(
            "Part number",
            Value::from_option(dimm.part.clone(), NOT_SET),
        ));
        group.push_row(
            Row::new(
                "Maximum speed",
                Value::from_option(dimm.speed.map(mts), NOT_SET),
            )
            .note("SMBIOS type 17 Speed: the module's rated maximum as the firmware lists it"),
        );
        group.push_row(Row::new(
            "Configured speed",
            Value::from_option(dimm.configured.map(mts), NOT_SET),
        ));
        group.push_row(Row::new(
            "Ranks",
            Value::from_option(dimm.rank.map(|r| r.to_string()), NOT_SET),
        ));
        group.push_row(Row::new(
            "Data width",
            Value::from_option(dimm.data_width.map(|w| format!("{w} bits")), NOT_SET),
        ));
        group.push_row(
            Row::new(
                "Configured voltage",
                Value::from_option(
                    dimm.millivolts
                        .map(|mv| format!("{:.3} V", f64::from(mv) / 1000.0)),
                    NOT_SET,
                ),
            )
            .note("As the firmware records it in SMBIOS; not a live measurement"),
        );
        group.push_row(Row::new(
            "Bank",
            Value::from_option(dimm.bank.clone(), NOT_SET),
        ));
        group.push_row(
            Row::new(
                "Serial number",
                Value::from_option(dimm.serial.clone(), NOT_SET),
            )
            .private(),
        );
        section.push_group(group);
    }
    section
}

pub fn collect(ctx: &Context) -> Section {
    let _ = ctx;
    #[cfg(windows)]
    {
        let mut issues = Vec::new();
        let table = match smbios::read() {
            Ok(table) => Some(table),
            Err(error) => {
                issues.push(format!("SMBIOS: {error}"));
                None
            }
        };
        let (installed, usable) = native_totals();
        build(table.as_ref(), installed, usable, issues)
    }
    #[cfg(not(windows))]
    {
        Section::unavailable(SectionId::Memory, "read on Windows only")
    }
}

/// Installed (firmware) and usable (Windows) physical memory in bytes.
#[cfg(windows)]
fn native_totals() -> (Option<u64>, Option<u64>) {
    use windows::Win32::System::SystemInformation::{
        GetPhysicallyInstalledSystemMemory, GlobalMemoryStatusEx, MEMORYSTATUSEX,
    };
    let mut kilobytes = 0u64;
    // SAFETY: writable output value.
    let installed = unsafe { GetPhysicallyInstalledSystemMemory(&mut kilobytes) }
        .ok()
        .map(|()| kilobytes * 1024)
        .filter(|b| *b > 0);
    let mut status = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    // SAFETY: sized, writable structure.
    let usable = unsafe { GlobalMemoryStatusEx(&mut status) }
        .ok()
        .map(|()| status.ullTotalPhys)
        .filter(|b| *b > 0);
    (installed, usable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::specs::native::smbios::tests::structure;

    fn dimm_body(size: u16, extended_mb: u32) -> Vec<u8> {
        // Body starts at offset 4; index = offset - 4.
        let mut body = vec![0u8; 0x5C - 4];
        let put16 = |body: &mut Vec<u8>, offset: usize, value: u16| {
            body[offset - 4..offset - 2].copy_from_slice(&value.to_le_bytes())
        };
        put16(&mut body, 0x0A, 64);
        put16(&mut body, 0x0C, size);
        body[0x0E - 4] = 0x09;
        body[0x10 - 4] = 1;
        body[0x11 - 4] = 2;
        body[0x12 - 4] = 0x22;
        put16(&mut body, 0x15, 6400);
        body[0x17 - 4] = 3;
        body[0x18 - 4] = 4;
        body[0x1A - 4] = 5;
        body[0x1B - 4] = 2;
        body[0x1C - 4..0x20 - 4].copy_from_slice(&extended_mb.to_le_bytes());
        put16(&mut body, 0x20, 6000);
        put16(&mut body, 0x26, 1350);
        body
    }

    fn table() -> Table {
        let mut array = vec![0u8; 0x17 - 4];
        array[0x05 - 4] = 0x03;
        array[0x06 - 4] = 0x03;
        array[0x07 - 4..0x0B - 4].copy_from_slice(&(256u32 * 1024 * 1024).to_le_bytes());
        array[0x0D - 4..0x0F - 4].copy_from_slice(&4u16.to_le_bytes());
        let mut data = structure(16, 0x10, &array, &[]);
        let strings = [
            "Controller0-ChannelA-DIMM1",
            "BANK 0",
            "Fixture RAM Co.",
            "FIXTURE-DIMM-SERIAL",
            "FX5-6400-32G",
        ];
        data.extend(structure(17, 0x11, &dimm_body(0x7FFF, 32 * 1024), &strings));
        let mut empty = dimm_body(0, 0);
        empty[0x17 - 4] = 0;
        empty[0x18 - 4] = 0;
        empty[0x1A - 4] = 0;
        data.extend(structure(
            17,
            0x12,
            &empty,
            &["Controller0-ChannelA-DIMM0", "BANK 0"],
        ));
        data.extend(structure(127, 0xFFFF, &[], &[]));
        Table::from_structures(3, 7, data)
    }

    #[test]
    fn type_17_extended_size_speeds_and_empty_slots_decode() {
        let table = table();
        let dimms = dimms(&table);
        assert_eq!(dimms.len(), 2);
        assert_eq!(dimms[0].bytes, Some(32 * 1024 * 1024 * 1024));
        assert_eq!(dimms[0].kind, Some("DDR5"));
        assert_eq!(dimms[0].speed, Some(6400));
        assert_eq!(dimms[0].configured, Some(6000));
        assert_eq!(dimms[0].rank, Some(2));
        assert_eq!(dimms[1].bytes, None);
        let array = array(&table).unwrap();
        assert_eq!(array.slots, Some(4));
        assert_eq!(array.max_bytes, Some(256 * 1024 * 1024 * 1024));
        assert_eq!(array.ecc, Some("None"));
    }

    #[test]
    fn type_16_totals_from_hostile_firmware_do_not_overflow() {
        let mut body = vec![0u8; 0x17 - 4];
        body[0x05 - 4] = 0x03;
        body[0x07 - 4..0x0B - 4].copy_from_slice(&0x8000_0000u32.to_le_bytes());
        body[0x0D - 4..0x0F - 4].copy_from_slice(&0x8000u16.to_le_bytes());
        body[0x0F - 4..0x17 - 4].copy_from_slice(&u64::MAX.to_le_bytes());
        let mut data = structure(16, 0x10, &body, &[]);
        data.extend(structure(16, 0x11, &body, &[]));
        data.extend(structure(127, 0xFFFF, &[], &[]));
        let array = array(&Table::from_structures(3, 7, data)).unwrap();
        assert_eq!(array.slots, Some(0x10000));
        assert_eq!(array.max_bytes, None, "an overflowing total is unavailable");
    }

    #[test]
    fn section_reports_installed_usable_and_masks_the_serial() {
        let table = table();
        let section = build(
            Some(&table),
            Some(64 * 1024 * 1024 * 1024),
            Some(63 * 1024 * 1024 * 1024),
            Vec::new(),
        );
        let text = crate::specs::probe_text(std::slice::from_ref(&section));
        assert!(
            text.contains("64.0 GB DDR5 @ 6000 MT/s (1 of 2 slots)"),
            "{text}"
        );
        assert!(text.contains("Reserved by hardware: 1.00 GB"), "{text}");
        assert!(
            text.contains("Controller0-ChannelA-DIMM1: 32.0 GB"),
            "{text}"
        );
        assert!(text.contains("Controller0-ChannelA-DIMM0: empty"), "{text}");
        assert!(
            text.contains("Maximum speed: 6400 MT/s (3200 MHz clock)"),
            "{text}"
        );
        assert!(text.contains("Channels populated: Single (A)"), "{text}");
        assert_eq!(channel("Controller0-ChannelB-DIMM1").as_deref(), Some("B"));
        assert_eq!(channel("DIMM_A1"), None);
        assert!(!text.contains("FIXTURE-DIMM-SERIAL"));
        assert!(text.contains("Timings (CL-tRCD-tRP-tRAS): Unavailable"));
        let missing = build(None, None, None, vec!["SMBIOS: fixture".into()]);
        assert!(missing.issues[0].contains("fixture"));
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "Read-only RAM specs probe; no driver, elevation, window or input"]
    fn native_specs_memory_read_only_probe() {
        let started = std::time::Instant::now();
        let sections = [collect(&Context::probe())];
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        println!("{}", crate::specs::probe_text(&sections));
        println!("collected in {elapsed:.3} ms (private values masked)");
    }
}
