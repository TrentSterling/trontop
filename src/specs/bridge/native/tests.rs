//! Native reads of exclusively owned Local sections, never the real HWiNFO name.
use super::*;
use crate::specs::native::wmi::{WmiRow, WmiValue};
use std::sync::atomic::{AtomicU64, Ordering};
use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError, INVALID_HANDLE_VALUE};
use windows::Win32::System::Memory::{
    CreateFileMappingW, FILE_MAP_WRITE, MEM_RESERVE, SEC_RESERVE, VirtualAlloc,
};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
fn unique_name() -> HSTRING {
    HSTRING::from(format!(
        "Local\\Trontop.Test.Hwinfo.{}.{}.{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ))
}
struct Publisher {
    name: HSTRING,
    view: View,
    _mapping: Mapping,
    size: usize,
}
impl Publisher {
    fn new(bytes: &[u8], size: usize) -> Self {
        assert!(size >= bytes.len() && size > 0 && size <= u32::MAX as usize);
        let name = unique_name();
        // SAFETY: pagefile-backed fixture only, owned Local name and bounded size.
        let mapping = Mapping(unsafe {
            CreateFileMappingW(
                INVALID_HANDLE_VALUE,
                None,
                PAGE_READWRITE,
                0,
                size as u32,
                &name,
            )
            .unwrap()
        });
        assert_ne!(
            unsafe { GetLastError() },
            ERROR_ALREADY_EXISTS,
            "fixture must own a new section"
        );
        let view = View(unsafe { MapViewOfFile(mapping.0, FILE_MAP_WRITE, 0, 0, size) });
        assert!(!view.0.Value.is_null());
        let publisher = Self {
            name,
            view,
            _mapping: mapping,
            size,
        };
        publisher.write(0, bytes);
        publisher
    }
    fn write(&self, at: usize, bytes: &[u8]) {
        assert!(at <= self.size && bytes.len() <= self.size - at);
        // SAFETY: fixture's own writable view, bounded above; no concurrent readers.
        unsafe {
            std::ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                self.view.0.Value.cast::<u8>().add(at),
                bytes.len(),
            );
        }
    }
}
fn fixture() -> Vec<u8> {
    super::super::tests::hwinfo_fixture()
}

#[test]
fn native_owned_hwinfo_mapping_reads_changes_without_mutating_publisher_and_releases_handles() {
    let bytes = fixture();
    let publisher = Publisher::new(&bytes, 8192);
    let name = publisher.name.clone();
    let first = hwinfo_mapping(&name).unwrap();
    assert_eq!(first, parse_hwinfo(&bytes).unwrap());
    assert_eq!(first[0].value, 64.5);
    let reading_at = u32::from_le_bytes(bytes[32..36].try_into().unwrap()) as usize;
    publisher.write(reading_at + 284, &91.25f64.to_le_bytes());
    for _ in 0..40 {
        let current = hwinfo_mapping(&name).unwrap();
        assert_eq!(current[0].value, 91.25);
        assert_eq!(
            first[0].value, 64.5,
            "prior owned copy must stay independent"
        );
    }
    drop(publisher);
    assert!(
        hwinfo_mapping(&name).unwrap_err().contains("not running"),
        "all reader views and handles must be released"
    );
}

#[test]
fn native_owned_hwinfo_mapping_refuses_inactive_malformed_and_out_of_view_tables() {
    let bytes = fixture();
    let publisher = Publisher::new(&bytes, 8192);
    for (offset, replacement, expected) in [
        (0, 0x4441_4544u32, "inactive"),
        (0, 0u32, "not recognised"),
        (20, 0xffff_f000u32, "not recognised"),
        (32, 0xffff_f000u32, "not recognised"),
        (28, 1025u32, "not recognised"),
        (40, 16385u32, "not recognised"),
    ] {
        publisher.write(0, &bytes);
        publisher.write(offset, &replacement.to_le_bytes());
        assert!(
            hwinfo_mapping(&publisher.name)
                .unwrap_err()
                .contains(expected)
        );
    }
    publisher.write(0, &bytes);
    assert_eq!(hwinfo_mapping(&publisher.name).unwrap().len(), 3);
    let name = publisher.name.clone();
    drop(publisher);
    assert!(hwinfo_mapping(&name).unwrap_err().contains("not running"));
}

#[test]
fn native_owned_hwinfo_mapping_does_not_publish_or_count_nonfinite_readings() {
    let bytes = fixture();
    let publisher = Publisher::new(&bytes, 8192);
    let reading_at = u32::from_le_bytes(bytes[32..36].try_into().unwrap()) as usize;
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, f64::MAX] {
        publisher.write(reading_at + 284, &invalid.to_le_bytes());
        let raws = hwinfo_mapping(&publisher.name).unwrap();
        assert_eq!(raws.len(), 2);
        assert!(raws.iter().all(|raw| raw.value.is_finite()));
        let section = Poll {
            providers: vec![(HWINFO, Ok(raws))],
        }
        .section();
        let text = crate::specs::probe_text(&[section]);
        assert!(text.contains("Running: 2 readings"));
        assert!(!text.contains("Running: 3 readings"));
    }
}

#[test]
fn native_owned_hwinfo_mapping_caps_copy_and_refuses_data_beyond_reader_limit() {
    let mut bytes = fixture();
    bytes[32..36].copy_from_slice(&(LARGEST_VIEW as u32).to_le_bytes());
    let publisher = Publisher::new(&bytes, LARGEST_VIEW + 8192);
    assert!(
        hwinfo_mapping(&publisher.name)
            .unwrap_err()
            .contains("not recognised")
    );
}

#[test]
fn native_owned_hwinfo_reserved_mapping_is_rejected_before_reading_bytes() {
    let name = unique_name();
    let mapping = Mapping(unsafe {
        CreateFileMappingW(
            INVALID_HANDLE_VALUE,
            None,
            PAGE_READWRITE | SEC_RESERVE,
            0,
            8192,
            &name,
        )
        .unwrap()
    });
    let view = View(unsafe { MapViewOfFile(mapping.0, FILE_MAP_READ, 0, 0, 0) });
    assert!(!view.0.Value.is_null());
    let mut info = MEMORY_BASIC_INFORMATION::default();
    assert_eq!(
        unsafe {
            VirtualQuery(
                Some(view.0.Value.cast_const()),
                &mut info,
                std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
            )
        },
        std::mem::size_of::<MEMORY_BASIC_INFORMATION>()
    );
    assert_eq!(
        info.State, MEM_RESERVE,
        "fixture must be genuinely uncommitted"
    );
    // Never access these pages; the production reader must reject their state.
    assert!(hwinfo_mapping(&name).unwrap_err().contains("not readable"));
    drop(view);
    drop(mapping);
    assert!(hwinfo_mapping(&name).unwrap_err().contains("not running"));
}

#[test]
fn native_owned_hwinfo_partial_commit_never_reads_past_first_readable_region() {
    let name = unique_name();
    let mapping = Mapping(unsafe {
        CreateFileMappingW(
            INVALID_HANDLE_VALUE,
            None,
            PAGE_READWRITE | SEC_RESERVE,
            0,
            8192,
            &name,
        )
        .unwrap()
    });
    let view = View(unsafe { MapViewOfFile(mapping.0, FILE_MAP_WRITE, 0, 0, 0) });
    assert!(!view.0.Value.is_null());
    let committed = unsafe {
        VirtualAlloc(
            Some(view.0.Value.cast_const()),
            4096,
            MEM_COMMIT,
            PAGE_READWRITE,
        )
    };
    assert_eq!(committed, view.0.Value);
    let mut bytes = fixture();
    bytes[32..36].copy_from_slice(&5000u32.to_le_bytes());
    // SAFETY: only the first 4096 bytes are committed; this fixture is shorter.
    assert!(bytes.len() < 4096);
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), view.0.Value.cast::<u8>(), bytes.len());
    }
    assert!(
        hwinfo_mapping(&name)
            .unwrap_err()
            .contains("not recognised")
    );
    let valid = fixture();
    unsafe {
        std::ptr::copy_nonoverlapping(valid.as_ptr(), view.0.Value.cast::<u8>(), valid.len());
    }
    assert_eq!(hwinfo_mapping(&name).unwrap().len(), 3);
    drop(view);
    drop(mapping);
    assert!(hwinfo_mapping(&name).unwrap_err().contains("not running"));
}

#[test]
fn native_owned_hwinfo_name_collision_and_missing_mapping_are_explicit_failures() {
    assert!(
        hwinfo_mapping(&unique_name())
            .unwrap_err()
            .contains("not running")
    );
    let name = unique_name();
    // A different named kernel-object type forces a genuine open error.
    let event = Mapping(unsafe {
        windows::Win32::System::Threading::CreateEventW(None, true, false, &name).unwrap()
    });
    let error = hwinfo_mapping(&name).unwrap_err();
    assert!(error.contains("OpenFileMappingW"));
    assert!(!error.contains("not running"));
    drop(event);
}

fn row(fields: &[(&str, WmiValue)]) -> WmiRow {
    WmiRow {
        properties: fields
            .iter()
            .map(|(name, value)| (name.to_string(), value.clone()))
            .collect(),
    }
}
#[test]
fn hardware_monitor_rows_resolve_names_skip_incomplete_data_and_keep_valid_fallbacks() {
    let hardware = [
        row(&[
            ("Identifier", WmiValue::Text("/intelcpu/0".into())),
            ("Name", WmiValue::Text("Fixture CPU".into())),
        ]),
        row(&[("Identifier", WmiValue::Text("/broken".into()))]),
    ];
    let sensor = |parent: &str, id: &str, value: WmiValue| {
        row(&[
            ("Identifier", WmiValue::Text(id.into())),
            ("Parent", WmiValue::Text(parent.into())),
            ("SensorType", WmiValue::Text("Temperature".into())),
            ("Name", WmiValue::Text("CPU Package".into())),
            ("Value", value),
        ])
    };
    let mut sensors = vec![
        sensor(
            "/intelcpu/0",
            "/intelcpu/0/temperature/0",
            WmiValue::Real(62.5),
        ),
        sensor("/unknown", "/unknown/temperature/0", WmiValue::Real(39.0)),
        sensor("", "/missing-parent/temperature/0", WmiValue::Real(40.0)),
        sensor("/intelcpu/0", "/bad-value", WmiValue::Text("bad".into())),
        sensor("/intelcpu/0", "/nan", WmiValue::Real(f64::NAN)),
        sensor("/intelcpu/0", "/overflow", WmiValue::Real(f64::MAX)),
    ];
    let complete = sensors[0].clone();
    for missing in ["Identifier", "Name", "SensorType", "Value"] {
        let mut incomplete = complete.clone();
        incomplete.properties.retain(|(name, _)| name != missing);
        sensors.push(incomplete);
    }
    let raws = hardware_monitor_rows(&hardware, &sensors);
    assert_eq!(raws.len(), 3);
    assert_eq!(raws[0].hardware, "Fixture CPU");
    assert_eq!(raws[0].value, 62.5);
    assert_eq!(raws[1].hardware, "/unknown");
    assert_eq!(raws[2].hardware, "");
    assert!(raws.iter().all(|r| r.value.is_finite()));
    let mut readings = vec![];
    super::super::readings(LIBRE, &raws, &mut readings);
    assert!(
        readings
            .iter()
            .any(|r| r.key == LiveKey::CpuPackageTemperature && r.value == 62.5)
    );
}
