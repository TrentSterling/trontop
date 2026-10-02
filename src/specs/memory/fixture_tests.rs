//! Byte fixtures derived from DMTF DSP0134 3.8.0, not a live firmware fallback.
use super::*;
use crate::specs::native::smbios::tests::structure;

fn put16(body: &mut [u8], offset: usize, value: u16) {
    body[offset - 4..offset - 2].copy_from_slice(&value.to_le_bytes());
}

fn put32(body: &mut [u8], offset: usize, value: u32) {
    body[offset - 4..offset].copy_from_slice(&value.to_le_bytes());
}

fn device(handle: u16, size: u16, kind: u8, form: u8, locator: &str) -> Vec<u8> {
    let mut body = vec![0; 0x5c - 4];
    put16(&mut body, 4, 0x10);
    put16(&mut body, 0x0c, if size == 32768 { 0x7fff } else { size });
    if size == 32768 {
        put32(&mut body, 0x1c, 32768);
    }
    body[0x0e - 4] = form;
    body[0x10 - 4] = 1;
    body[0x12 - 4] = kind;
    body[0x17 - 4] = 2;
    body[0x18 - 4] = 3;
    structure(
        17,
        handle,
        &body,
        &[locator, "Fixture maker", "FIXTURE-RAM-PRIVATE"],
    )
}

fn memory_array(handle: u16, use_code: u8, slots: u16, ecc: u8) -> Vec<u8> {
    let mut body = vec![0; 0x17 - 4];
    body[5 - 4] = use_code;
    body[6 - 4] = ecc;
    put32(&mut body, 7, 128 * 1024 * 1024);
    put16(&mut body, 0x0d, slots);
    structure(16, handle, &body, &[])
}

#[test]
fn unknown_dimm_size_keeps_module_facts_without_claiming_an_empty_slot() {
    let table = Table::from_structures(3, 8, device(1, 0xffff, 0x22, 9, "ChannelA-DIMM0"));
    let text = crate::specs::probe_text(&[build(Some(&table), None, None, Vec::new())]);
    assert!(!text.contains(": empty"), "{text}");
    assert!(text.contains("Manufacturer: Fixture maker"), "{text}");
    assert!(text.contains("Size: Unavailable"), "{text}");
    assert!(!text.contains("Slots used: 0 of") && !text.contains("FIXTURE-RAM-PRIVATE"));
    assert!(
        text.contains("Channels populated: Unavailable"),
        "occupancy is unknown: {text}"
    );
}

#[test]
fn partial_dimm_capacities_do_not_become_a_complete_installed_total() {
    let mut data = device(1, 32768, 0x22, 9, "ChannelA-DIMM0");
    data.extend(device(2, 0xffff, 0x22, 9, "ChannelB-DIMM0"));
    let table = Table::from_structures(3, 8, data);
    let section = build(Some(&table), None, None, Vec::new());
    let text = crate::specs::probe_text(std::slice::from_ref(&section));
    assert!(text.contains("Installed: Unavailable"), "{text}");
    assert!(text.contains("ChannelA-DIMM0: 32.0 GiB"), "{text}");
    assert!(
        !section.issues.is_empty(),
        "partial facts need an explicit issue"
    );
}

#[test]
fn summary_and_slot_row_use_the_same_physical_array_count() {
    let mut data = memory_array(0x10, 3, 4, 3);
    data.extend(device(1, 32768, 0x22, 9, "ChannelA-DIMM0"));
    data.extend(device(2, 0, 0x22, 9, "ChannelB-DIMM0"));
    let table = Table::from_structures(3, 8, data);
    let text = crate::specs::probe_text(&[build(Some(&table), Some(1 << 35), None, Vec::new())]);
    assert!(
        text.contains("(1 of 4 slots)") && text.contains("Slots used: 1 of 4"),
        "{text}"
    );
}

#[test]
fn complete_smbios_fallback_names_its_source_and_filters_zero_windows_totals() {
    let table = Table::from_structures(3, 8, device(1, 32768, 0x22, 9, "ChannelA-DIMM0"));
    let section = build(Some(&table), Some(0), Some(0), Vec::new());
    let text = crate::specs::probe_text(std::slice::from_ref(&section));
    assert!(text.contains("Installed: 32.0 GiB"), "{text}");
    assert!(text.contains("Usable by Windows: Unavailable"), "{text}");
    let overview = &section.groups[0];
    let row = overview
        .items
        .iter()
        .find_map(|item| match item {
            super::super::Item::Row(row) if row.label == "Installed" => Some(row),
            _ => None,
        })
        .unwrap();
    assert!(
        row.note
            .as_deref()
            .is_some_and(|n| n.contains("SMBIOS type 17")),
        "{:?}",
        row.note
    );
}

#[test]
fn non_system_array_devices_do_not_inflate_ram_capacity_or_slots() {
    let mut data = memory_array(0x10, 3, 1, 5);
    data.extend(memory_array(0x20, 4, 1, 3));
    data.extend(device(1, 8192, 0x1a, 9, "ChannelA-System"));
    let mut video = device(2, 16384, 0x22, 0x10, "VIDEO-FIXTURE");
    video[4..6].copy_from_slice(&0x20_u16.to_le_bytes());
    data.extend(video);
    let table = Table::from_structures(3, 8, data);
    let text = crate::specs::probe_text(&[build(Some(&table), None, None, Vec::new())]);
    assert!(text.contains("Installed: 8.00 GiB"), "{text}");
    assert!(
        !text.contains("VIDEO-FIXTURE"),
        "video memory is not system RAM"
    );
    assert!(text.contains("Error correction: Single-bit ECC"), "{text}");
}

#[test]
fn reserved_extended_size_and_speed_bits_do_not_become_readable_facts() {
    let mut bytes = device(1, 32768, 0x22, 9, "ChannelA-DIMM0");
    bytes[0x1c..0x20].copy_from_slice(&0x8000_8000_u32.to_le_bytes());
    bytes[0x15..0x17].copy_from_slice(&0xffff_u16.to_le_bytes());
    bytes[0x54..0x58].copy_from_slice(&0x8001_0000_u32.to_le_bytes());
    let table = Table::from_structures(3, 8, bytes);
    let text = crate::specs::probe_text(&[build(Some(&table), None, None, Vec::new())]);
    assert!(text.contains("Size: Unavailable"), "{text}");
    assert!(text.contains("Maximum speed: Unavailable"), "{text}");
}

#[test]
fn dimm_units_extended_speeds_and_private_serial_survive_reports() {
    let cases = [
        (0x8100, 256 * 1024),
        (0x0100, 256 * 1024 * 1024),
        (32768, 1 << 35),
    ];
    for (size, expected) in cases {
        let mut bytes = device(1, size, 0x22, 0x11, "ChannelA-DIMM0");
        bytes[0x15..0x17].copy_from_slice(&0xffff_u16.to_le_bytes());
        bytes[0x20..0x22].copy_from_slice(&0xffff_u16.to_le_bytes());
        bytes[0x54..0x58].copy_from_slice(&100_000_u32.to_le_bytes());
        bytes[0x58..0x5c].copy_from_slice(&80_000_u32.to_le_bytes());
        let table = Table::from_structures(3, 8, bytes);
        let section = build(Some(&table), None, None, Vec::new());
        let text = crate::specs::probe_text(std::slice::from_ref(&section));
        assert!(
            text.contains(&format!("Installed: {}", format::bytes(expected))),
            "{text}"
        );
        assert!(text.contains("Maximum speed: 100000 MT/s (50000 MHz clock)"));
        assert!(text.contains("Configured speed: 80000 MT/s (40000 MHz clock)"));
        assert!(text.contains("Form factor: CAMM") && !text.contains("FIXTURE-RAM-PRIVATE"));
        let mut private = String::new();
        crate::specs::section_text(&mut private, &section, None, true);
        assert!(private.contains("FIXTURE-RAM-PRIVATE"));
    }
}

#[test]
fn firmware_memory_kinds_and_forms_resolve_through_module_reports() {
    let kinds = [
        (3, "DRAM"),
        (0x0f, "SDRAM"),
        (0x12, "DDR"),
        (0x13, "DDR2"),
        (0x14, "DDR2 FB-DIMM"),
        (0x18, "DDR3"),
        (0x1a, "DDR4"),
        (0x1b, "LPDDR"),
        (0x1c, "LPDDR2"),
        (0x1d, "LPDDR3"),
        (0x1e, "LPDDR4"),
        (0x1f, "Logical non-volatile"),
        (0x20, "HBM"),
        (0x21, "HBM2"),
        (0x22, "DDR5"),
        (0x23, "LPDDR5"),
        (0x24, "HBM3"),
    ];
    let forms = [
        (3, "SIMM"),
        (9, "DIMM"),
        (0x0c, "RIMM"),
        (0x0d, "SODIMM"),
        (0x0f, "FB-DIMM"),
        (0x10, "Die"),
        (0x11, "CAMM"),
    ];
    for (index, (kind, name)) in kinds.into_iter().enumerate() {
        let (form, form_name) = forms[index % forms.len()];
        let table = Table::from_structures(3, 8, device(1, 4096, kind, form, "ChannelA-DIMM0"));
        let text = crate::specs::probe_text(&[build(Some(&table), None, None, Vec::new())]);
        assert!(
            text.contains(&format!("Type: {name}"))
                && text.contains(&format!("Form factor: {form_name}")),
            "{text}"
        );
    }
    let table = Table::from_structures(3, 8, device(1, 4096, 0xff, 0xff, "No channel"));
    let text = crate::specs::probe_text(&[build(Some(&table), None, None, Vec::new())]);
    assert!(text.contains("Type: Unavailable") && text.contains("Form factor: Unavailable"));
    assert!(text.contains("Channels populated: Unavailable"));
}

#[test]
fn array_totals_and_ecc_do_not_assume_missing_arrays_match_the_first() {
    for (code, expected) in [
        (4, "Parity"),
        (5, "Single-bit ECC"),
        (6, "Multi-bit ECC"),
        (7, "CRC"),
    ] {
        let table = Table::from_structures(3, 8, memory_array(0x10, 3, 4, code));
        assert_eq!(array(&table).unwrap().ecc.as_deref(), Some(expected));
    }
    let mut data = memory_array(0x10, 3, 2, 3);
    data.extend(memory_array(0x11, 3, 2, 5));
    let table = Table::from_structures(3, 8, data.clone());
    let result = array(&table).unwrap();
    assert_eq!(result.slots, Some(4));
    assert_eq!(result.max_bytes, Some(1 << 38));
    assert_eq!(result.ecc.as_deref(), Some("Mixed (None, Single-bit ECC)"));
    data.extend(structure(16, 0x12, &[0, 3, 2, 0, 0, 0, 0], &[]));
    let result = array(&Table::from_structures(3, 8, data)).unwrap();
    assert_eq!(
        result.slots, None,
        "a missing count is not a complete total"
    );
    assert_eq!(
        result.max_bytes, None,
        "a missing capacity is not a complete total"
    );
    assert_eq!(
        result.ecc, None,
        "unknown array ECC cannot inherit the first array"
    );
}

#[test]
fn channel_counts_mixed_kinds_and_truncated_records_keep_partial_facts_explicit() {
    for (count, mode) in [(3, "Triple"), (4, "Quad"), (5, "Multi")] {
        let mut data = Vec::new();
        for index in 0..count {
            data.extend(device(
                index,
                4096,
                if index % 2 == 0 { 0x1a } else { 0x22 },
                9,
                &format!("Channel{}-DIMM0", char::from(b'A' + index as u8)),
            ));
        }
        let table = Table::from_structures(3, 8, data);
        let text = crate::specs::probe_text(&[build(Some(&table), None, None, Vec::new())]);
        assert!(
            text.contains(&format!("Channels populated: {mode}")),
            "{text}"
        );
        assert!(text.contains("Type: DDR4, DDR5") && !text.contains("DDR4, DDR5, DDR4"));
    }
    let mut data = device(1, 4096, 0x22, 9, "ChannelA-DIMM0");
    data.extend_from_slice(&[17, 2, 0, 0]);
    let table = Table::from_structures(3, 8, data);
    let section = build(
        Some(&table),
        None,
        None,
        vec!["Fixture provider issue".into()],
    );
    let text = crate::specs::probe_text(&[section]);
    assert!(text.contains("ChannelA-DIMM0: 4.00 GiB") && text.contains("Installed: Unavailable"));
    assert!(text.contains("truncated") && text.contains("Fixture provider issue"));
    let text = crate::specs::probe_text(&[build(None, Some(1 << 30), Some(1 << 31), Vec::new())]);
    assert!(
        !text.contains("Reserved by hardware:"),
        "inconsistent totals cannot become a negative or wrapped reservation"
    );
}

#[test]
fn contradictory_or_missing_array_slot_counts_do_not_claim_complete_occupancy() {
    for count in [0, 1] {
        let mut data = memory_array(0x10, 3, count, 3);
        data.extend(device(1, 4096, 0x22, 9, "ChannelA-DIMM0"));
        data.extend(device(2, 4096, 0x22, 9, "ChannelB-DIMM0"));
        let table = Table::from_structures(3, 8, data);
        let section = build(Some(&table), Some(1 << 33), None, Vec::new());
        let text = crate::specs::probe_text(std::slice::from_ref(&section));
        assert!(
            text.contains("Installed: 8.00 GiB"),
            "readable Windows capacity is preserved"
        );
        assert!(!text.contains("2 of 1"), "{text}");
        assert!(
            !section.issues.is_empty(),
            "slot-count uncertainty must be explicit"
        );
        if count == 0 {
            assert!(text.contains("Slots used: 2 of 2 (partial)"), "{text}");
        } else {
            assert!(text.contains("Slots used: Unavailable"), "{text}");
        }
    }
}

#[test]
fn partial_configured_speeds_keep_complete_capacities_and_report_the_reading_count() {
    let mut data = device(1, 4096, 0x22, 9, "ChannelA-DIMM0");
    data[0x20..0x22].copy_from_slice(&6000_u16.to_le_bytes());
    data.extend(device(2, 4096, 0x22, 9, "ChannelB-DIMM0"));
    let table = Table::from_structures(3, 8, data);
    let section = build(Some(&table), None, None, Vec::new());
    let text = crate::specs::probe_text(std::slice::from_ref(&section));
    assert!(
        text.contains("Installed: 8.00 GiB"),
        "a missing speed does not erase independent capacity"
    );
    assert!(
        text.contains("Configured speed: 6000 MT/s (3000 MHz clock) (partial)"),
        "{text}"
    );
    let row = section.groups[0]
        .items
        .iter()
        .find_map(|item| match item {
            super::super::Item::Row(row) if row.label == "Configured speed" => Some(row),
            _ => None,
        })
        .unwrap();
    assert!(
        row.note.as_deref().is_some_and(|n| n.contains("1 of 2")),
        "{:?}",
        row.note
    );
}
