//! Motherboard section (lane: board).
//!
//! Scope: SMBIOS type 2 baseboard (manufacturer, product, version; serial is
//! private), type 1 system (UUID and serial private), type 3 chassis, type 0
//! BIOS (vendor, version, date, ROM size, UEFI flag), chipset from the PCI host
//! bridge / LPC device via SetupDi, PCIe slot inventory (type 9), live board
//! temperature only via LiveKey::MotherboardTemperature (bridge). OEM filler
//! strings (smbios::is_placeholder) are Unavailable("not set by the manufacturer").
use super::native::smbios::{self, Table};
#[cfg(windows)]
use super::native::{PciId, pci_vendor};
use super::{Context, Group, LiveKey, Row, Section, SectionId, SummaryLine, Value};

const NOT_SET: &str = "not set by the manufacturer";

fn text(s: Option<&smbios::Structure<'_>>, offset: usize) -> Option<String> {
    s?.string(offset).filter(|t| !smbios::is_placeholder(t))
}

fn chassis_type(code: u8) -> Option<&'static str> {
    Some(match code & 0x7F {
        0x03 => "Desktop",
        0x04 => "Low profile desktop",
        0x05 => "Pizza box",
        0x06 => "Mini tower",
        0x07 => "Tower",
        0x08 => "Portable",
        0x09 => "Laptop",
        0x0A => "Notebook",
        0x0B => "Handheld",
        0x0C => "Docking station",
        0x0D => "All in one",
        0x0E => "Sub notebook",
        0x0F => "Space-saving",
        0x10 => "Lunch box",
        0x11 => "Main server chassis",
        0x17 => "Rack mount chassis",
        0x18 => "Sealed-case PC",
        0x1C => "Blade",
        0x1E => "Tablet",
        0x1F => "Convertible",
        0x20 => "Detachable",
        0x21 => "IoT gateway",
        0x22 => "Embedded PC",
        0x23 => "Mini PC",
        0x24 => "Stick PC",
        _ => return None,
    })
}

/// SMBIOS type 9 slot types (DMTF DSP0134 7.10.1), PCI Express and M.2 only.
fn slot_type(code: u8) -> Option<String> {
    let generation = |base: u8| match code - base {
        0 => None,
        1 => Some("x1"),
        2 => Some("x2"),
        3 => Some("x4"),
        4 => Some("x8"),
        5 => Some("x16"),
        _ => None,
    };
    let pcie = |version: Option<u8>, base: u8| {
        let name = match version {
            Some(version) => format!("PCI Express {version}.0"),
            None => "PCI Express".to_string(),
        };
        match generation(base) {
            Some(width) => format!("{name} {width}"),
            None => name,
        }
    };
    Some(match code {
        0x1F => "M.2 Socket 1-DP (key A)".into(),
        0x20 => "M.2 Socket 1-SD (key E)".into(),
        0x21 => "M.2 Socket 2 (key B)".into(),
        0x22 => "M.2 Socket 3 (key M)".into(),
        0xA5..=0xAA => pcie(None, 0xA5),
        0xAB..=0xB0 => pcie(Some(2), 0xAB),
        0xB1..=0xB6 => pcie(Some(3), 0xB1),
        0xB8..=0xBD => pcie(Some(4), 0xB8),
        0xBE..=0xC3 => pcie(Some(5), 0xBE),
        0xC4 => "PCI Express 6.0 or later".into(),
        _ => return None,
    })
}

fn slot_usage(code: u8) -> Option<&'static str> {
    Some(match code {
        0x03 => "In use",
        0x04 => "Available",
        0x05 => "Unavailable",
        _ => return None,
    })
}

fn slot_width(code: u8) -> Option<&'static str> {
    Some(match code {
        0x08 => "x1",
        0x09 => "x2",
        0x0A => "x4",
        0x0B => "x8",
        0x0C => "x12",
        0x0D => "x16",
        0x0E => "x32",
        _ => return None,
    })
}

/// "MM/DD/YYYY" (SMBIOS BIOS release date) as "YYYY-MM-DD".
fn bios_date(text: &str) -> Option<String> {
    let mut parts = text.trim().split('/');
    let (month, day, year) = (parts.next()?, parts.next()?, parts.next()?);
    let year = match year.len() {
        4 => year.to_string(),
        2 => format!("19{year}"),
        _ => return None,
    };
    let month = month.parse::<u8>().ok().filter(|m| (1..=12).contains(m))?;
    let day = day.parse::<u8>().ok().filter(|d| (1..=31).contains(d))?;
    Some(format!("{year}-{month:02}-{day:02}"))
}

fn rom_size(bios: &smbios::Structure<'_>) -> Option<String> {
    match bios.byte(0x09)? {
        0xFF => {
            let extended = bios.word(0x18)?;
            let size = extended & 0x3FFF;
            match extended >> 14 {
                0 => Some(format!("{size} MB")),
                1 => Some(format!("{size} GB")),
                _ => None,
            }
        }
        blocks => Some(format!("{} KB", (u32::from(blocks) + 1) * 64)),
    }
}

/// Everything besides SMBIOS: chipset device names from SetupDi.
#[derive(Clone, Debug, Default)]
struct Chipset {
    bridge: Option<(String, Option<String>)>,
    host: Option<(String, Option<String>)>,
    error: Option<String>,
}

fn build(table: Option<&Table>, chipset: Chipset, issues: Vec<String>) -> Section {
    let mut section = Section::new(SectionId::Motherboard);
    for issue in issues {
        section.push_issue(issue);
    }
    let Some(table) = table else {
        return Section::unavailable(
            SectionId::Motherboard,
            section
                .issues
                .first()
                .cloned()
                .unwrap_or_else(|| "SMBIOS unavailable".into()),
        );
    };
    let board = table.of_type(2).next();
    let system = table.of_type(1).next();
    let chassis = table.of_type(3).next();
    let bios = table.of_type(0).next();

    let maker = text(board.as_ref(), 0x04);
    let model = text(board.as_ref(), 0x05);
    let headline = match (&maker, &model) {
        (Some(maker), Some(model)) => Value::known(format!("{maker} {model}")),
        (None, Some(model)) => Value::known(model.clone()),
        _ => Value::unavailable(NOT_SET),
    };
    section.push_summary(SummaryLine::new(headline).live(LiveKey::MotherboardTemperature));

    let mut baseboard = Group::new("Motherboard")
        .live(LiveKey::MotherboardTemperature)
        .kv("Manufacturer", Value::from_option(maker, NOT_SET))
        .kv("Model", Value::from_option(model, NOT_SET))
        .kv(
            "Version",
            Value::from_option(text(board.as_ref(), 0x06), NOT_SET),
        )
        .row(
            Row::new(
                "Serial number",
                Value::from_option(text(board.as_ref(), 0x07), NOT_SET),
            )
            .private(),
        );
    baseboard.push_row(
        Row::new(
            "Chipset bridge (LPC/eSPI)",
            match &chipset.bridge {
                Some((name, _)) => Value::known(name.clone()),
                None => Value::unavailable(
                    chipset
                        .error
                        .clone()
                        .unwrap_or_else(|| "no PCI ISA/LPC bridge reported by Windows".into()),
                ),
            },
        )
        .note("PCI class 0601 function of the platform controller hub; Windows names it generically, so no chipset model is claimed"),
    );
    if let Some((name, driver)) = &chipset.host {
        let mut row = Row::known("Host bridge", name.clone());
        if let Some(driver) = driver {
            row = row.note(format!("Driver {driver}"));
        }
        baseboard.push_row(row);
    }
    baseboard.push_row(
        Row::live("Temperature", LiveKey::MotherboardTemperature)
            .note("From a running sensor provider only; Windows exposes no board sensors"),
    );
    section.push_group(baseboard);

    if let Some(bios) = bios {
        let uefi = bios.byte(0x13).map(|b| b & 0x08 != 0);
        let release = match (bios.byte(0x14), bios.byte(0x15)) {
            (Some(major), Some(minor)) if major != 0xFF => Some(format!("{major}.{minor}")),
            _ => None,
        };
        section.push_group(
            Group::new("BIOS")
                .kv(
                    "Vendor",
                    Value::from_option(text(Some(&bios), 0x04), NOT_SET),
                )
                .kv(
                    "Version",
                    Value::from_option(text(Some(&bios), 0x05), NOT_SET),
                )
                .kv(
                    "Release date",
                    Value::from_option(
                        text(Some(&bios), 0x08).and_then(|d| bios_date(&d).or(Some(d))),
                        NOT_SET,
                    ),
                )
                .kv("System BIOS release", Value::from_option(release, NOT_SET))
                .kv("ROM size", Value::from_option(rom_size(&bios), NOT_SET))
                .kv(
                    "UEFI support",
                    Value::from_option(uefi.map(|u| if u { "Yes" } else { "No" }), NOT_SET),
                )
                .kv(
                    "SMBIOS version",
                    Value::known(format!("{}.{}", table.major, table.minor)),
                ),
        );
    } else {
        section.push_issue("SMBIOS has no BIOS record (type 0)");
    }

    let mut computer = Group::new("System")
        .kv(
            "Manufacturer",
            Value::from_option(text(system.as_ref(), 0x04), NOT_SET),
        )
        .kv(
            "Product",
            Value::from_option(text(system.as_ref(), 0x05), NOT_SET),
        )
        .kv(
            "Version",
            Value::from_option(text(system.as_ref(), 0x06), NOT_SET),
        )
        .kv(
            "Family",
            Value::from_option(text(system.as_ref(), 0x1A), NOT_SET),
        )
        .kv(
            "SKU",
            Value::from_option(text(system.as_ref(), 0x19), NOT_SET),
        )
        .row(
            Row::new(
                "Serial number",
                Value::from_option(text(system.as_ref(), 0x07), NOT_SET),
            )
            .private(),
        )
        .row(
            Row::new(
                "UUID",
                Value::from_option(system.as_ref().and_then(|s| s.uuid(0x08)), NOT_SET),
            )
            .private(),
        );
    if let Some(chassis) = &chassis {
        computer.push_row(Row::new(
            "Chassis type",
            Value::from_option(chassis.byte(0x05).and_then(chassis_type), NOT_SET),
        ));
        computer.push_row(Row::new(
            "Chassis manufacturer",
            Value::from_option(text(Some(chassis), 0x04), NOT_SET),
        ));
        computer.push_row(
            Row::new(
                "Chassis serial",
                Value::from_option(text(Some(chassis), 0x07), NOT_SET),
            )
            .private(),
        );
    }
    section.push_group(computer);

    let slots = table.of_type(9).collect::<Vec<_>>();
    if !slots.is_empty() {
        let mut group = Group::new(format!("Expansion slots ({})", slots.len()));
        for slot in slots {
            let name = slot
                .string(0x04)
                .unwrap_or_else(|| format!("Slot {:04X}h", slot.handle));
            let kind = slot.byte(0x05).and_then(slot_type);
            let width = slot.byte(0x06).and_then(slot_width);
            let usage = slot.byte(0x07).and_then(slot_usage);
            let description = [
                kind,
                width.map(|w| format!("{w} electrical")),
                usage.map(str::to_string),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(", ");
            group.push_row(
                Row::new(
                    name,
                    if description.is_empty() {
                        Value::unavailable(NOT_SET)
                    } else {
                        Value::known(description)
                    },
                )
                .note("SMBIOS type 9; usage is what the firmware recorded at boot"),
            );
        }
        section.push_group(group);
    }
    if table.truncated() {
        section.push_issue("SMBIOS table ended early; later records may be missing");
    }
    section
}

pub fn collect(ctx: &Context) -> Section {
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
        let chipset = if ctx.should_stop() {
            Chipset {
                error: Some("read budget exhausted".into()),
                ..Default::default()
            }
        } else {
            native_chipset(ctx)
        };
        build(table.as_ref(), chipset, issues)
    }
    #[cfg(not(windows))]
    {
        let _ = ctx;
        Section::unavailable(SectionId::Motherboard, "read on Windows only")
    }
}

/// Windows usually names these functions generically ("PCI standard ISA
/// bridge"), so each carries its PCI vendor and device ID.
#[cfg(windows)]
fn native_chipset(ctx: &Context) -> Chipset {
    use super::native::setupapi::{Filter, Query, devices};
    let describe = |d: &super::native::setupapi::DeviceInfo| {
        let name = d.name().unwrap_or("PCI device").to_string();
        match PciId::parse(&d.hardware_ids) {
            Some(id) => format!(
                "{} {name} (PCI {:04X}:{:04X})",
                pci_vendor(id.vendor).unwrap_or("Unknown vendor"),
                id.vendor,
                id.device
            ),
            None => name,
        }
    };
    let class = |d: &super::native::setupapi::DeviceInfo, code: &str| {
        d.compatible_ids.iter().any(|c| {
            c.to_ascii_uppercase()
                .starts_with(&format!("PCI\\CC_{code}"))
        })
    };
    match devices(&Query::new(Filter::Enumerator("PCI"), ctx)) {
        Ok(list) => {
            let bridges = list
                .iter()
                .filter(|d| class(d, "0601"))
                .map(describe)
                .collect::<Vec<_>>();
            Chipset {
                bridge: (!bridges.is_empty()).then(|| (bridges.join("; "), None)),
                host: list
                    .iter()
                    .find(|d| class(d, "0600"))
                    .map(|d| (describe(d), d.driver_version.clone())),
                error: None,
            }
        }
        Err(error) => Chipset {
            error: Some(error.to_string()),
            ..Default::default()
        },
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::specs::native::smbios::tests::structure;

    #[test]
    fn slot_and_chassis_tables_follow_dsp0134() {
        assert_eq!(slot_type(0xBD).as_deref(), Some("PCI Express 4.0 x16"));
        assert_eq!(slot_type(0xC3).as_deref(), Some("PCI Express 5.0 x16"));
        assert_eq!(slot_type(0xBE).as_deref(), Some("PCI Express 5.0"));
        assert_eq!(slot_type(0x22).as_deref(), Some("M.2 Socket 3 (key M)"));
        assert_eq!(slot_type(0x01), None);
        assert_eq!(chassis_type(0x83), Some("Desktop"));
        assert_eq!(bios_date("07/15/2026").as_deref(), Some("2026-07-15"));
        assert_eq!(bios_date("13/40/2026"), None);
    }

    #[test]
    fn board_section_masks_serials_and_uuid_and_rejects_filler() {
        let mut bios = vec![0u8; 0x1A - 4];
        bios[0x05 - 4] = 2;
        bios[0x08 - 4] = 3;
        bios[0x09 - 4] = 0xFF;
        bios[0x13 - 4] = 0x08;
        bios[0x14 - 4] = 5;
        bios[0x15 - 4] = 32;
        bios[0x18 - 4..0x1A - 4].copy_from_slice(&32u16.to_le_bytes());
        bios[0] = 1;
        let mut data = structure(0, 0, &bios, &["Fixture BIOS Inc.", "3.10", "07/15/2026"]);
        let mut system = vec![0u8; 0x1B - 4];
        system[0] = 1;
        system[1] = 2;
        system[3] = 3;
        system[0x08 - 4..0x18 - 4].copy_from_slice(&[7u8; 16]);
        data.extend(structure(
            1,
            1,
            &system,
            &[
                "Fixture PC Co.",
                "To Be Filled By O.E.M.",
                "FIXTURE-SYSTEM-SERIAL",
            ],
        ));
        data.extend(structure(
            2,
            2,
            &[1, 2, 3, 4],
            &["Fixture Board Co.", "FX-890", "1.0", "FIXTURE-BOARD-SERIAL"],
        ));
        data.extend(structure(3, 3, &[0, 0x03, 0, 0], &[]));
        data.extend(structure(9, 9, &[1, 0xC3, 0x0D, 0x03], &["PCIEX16_1"]));
        let table = Table::from_structures(3, 7, data);
        let section = build(
            Some(&table),
            Chipset {
                bridge: Some(("Fixture LPC/eSPI Controller".into(), None)),
                ..Default::default()
            },
            Vec::new(),
        );
        let text = crate::specs::probe_text(std::slice::from_ref(&section));
        assert!(text.contains("  Fixture Board Co. FX-890"), "{text}");
        assert!(text.contains("Release date: 2026-07-15"), "{text}");
        assert!(text.contains("ROM size: 32 MB"), "{text}");
        assert!(text.contains("UEFI support: Yes"), "{text}");
        assert!(
            text.contains("Product: Unavailable (not set by the manufacturer)"),
            "{text}"
        );
        assert!(text.contains("Chassis type: Desktop"), "{text}");
        assert!(
            text.contains("PCIEX16_1: PCI Express 5.0 x16, x16 electrical, In use"),
            "{text}"
        );
        for secret in ["FIXTURE-SYSTEM-SERIAL", "FIXTURE-BOARD-SERIAL", "07070707"] {
            assert!(!text.contains(secret), "{secret} leaked");
        }
        let missing = build(None, Chipset::default(), vec!["SMBIOS: fixture".into()]);
        assert!(missing.groups.is_empty());
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "Read-only Motherboard specs probe; no driver, elevation, window or input"]
    fn native_specs_board_read_only_probe() {
        let started = std::time::Instant::now();
        let sections = [collect(&Context::probe())];
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        println!("{}", crate::specs::probe_text(&sections));
        println!("collected in {elapsed:.3} ms (private values masked)");
    }
}
