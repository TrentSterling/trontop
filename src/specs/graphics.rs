//! Graphics section (lane: graphics).
//!
//! Scope: every adapter (DXGI identity, crate::gpu_adapters::dxgi_inventory),
//! 64-bit dedicated VRAM (never WMI's 32-bit AdapterRAM, which caps at 4 GB),
//! vendor/device/subsystem IDs and board partner, driver version/date, PCIe link
//! where exposed, NVML static data for NVIDIA, monitors (name, native and
//! current resolution, refresh rate; EDID serials private), live values via
//! LiveKey::Gpu { adapter: GpuRef, metric }. Never invent a shader clock.
use super::native::pci_vendor;
use super::native::registry::RegValue;
use super::{
    Context, GpuMetric, GpuRef, Group, LiveKey, Row, Section, SectionId, SummaryLine, Value,
};
use crate::format;

#[cfg(windows)]
mod native;

const NOT_REPORTED_DRIVER: &str = "not reported by the driver";

/// One DXGI adapter plus what SetupDi says about the same PCI function.
#[derive(Clone, Debug, Default, PartialEq)]
struct Adapter {
    name: String,
    /// Every DXGI adapter LUID that belongs to this one PCI function.
    luids: Vec<(u32, u32)>,
    vendor_id: u32,
    device_id: u32,
    dedicated_video: u64,
    dedicated_system: u64,
    shared: u64,
    device: Option<Device>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Device {
    subsystem: Option<(u16, u16)>,
    revision: Option<u8>,
    driver_version: Option<String>,
    driver_date: Option<String>,
    driver_provider: Option<String>,
    location: Option<String>,
    link: Option<String>,
    status: Option<String>,
    /// Driver-reported physical memory (HardwareInformation.qwMemorySize).
    memory: Option<u64>,
    /// Video BIOS version string (HardwareInformation.BiosString).
    bios: Option<String>,
}

/// One active display path from QueryDisplayConfig.
#[derive(Clone, Debug, Default, PartialEq)]
struct Monitor {
    adapter: (u32, u32),
    name: Option<String>,
    manufacturer: Option<String>,
    product: Option<u16>,
    connection: Option<&'static str>,
    current: Option<(u32, u32)>,
    refresh: Option<f64>,
    native: Option<(u32, u32, Option<f64>)>,
    /// (supported, enabled)
    hdr: Option<(bool, bool)>,
    bits: Option<u32>,
}

/// DISPLAYCONFIG_VIDEO_OUTPUT_TECHNOLOGY names.
fn connection(code: i32) -> Option<&'static str> {
    Some(match code {
        0 => "VGA (HD15)",
        4 => "DVI",
        5 => "HDMI",
        6 => "LVDS",
        10 => "DisplayPort",
        11 => "Embedded DisplayPort",
        12 => "UDI",
        13 => "Embedded UDI",
        15 => "Miracast (wireless)",
        16 => "Indirect display (wired)",
        17 => "Indirect display (virtual)",
        18 => "DisplayPort over USB",
        i32::MIN => "Internal",
        _ => return None,
    })
}

/// EDID manufacturer ID: three 5-bit letters, stored big-endian in the EDID
/// and handed back byte-swapped by DisplayConfig.
fn pnp_id(raw: u16) -> Option<String> {
    let value = raw.swap_bytes();
    let letters = [(value >> 10) & 0x1F, (value >> 5) & 0x1F, value & 0x1F];
    letters
        .iter()
        .map(|l| {
            (1..=26)
                .contains(l)
                .then(|| char::from(b'A' + *l as u8 - 1))
        })
        .collect()
}

fn hz(rate: f64) -> String {
    if (rate - rate.round()).abs() < 0.005 {
        format!("{rate:.0} Hz")
    } else {
        format!("{rate:.2} Hz")
    }
}

fn mode_text(width: u32, height: u32, rate: Option<f64>) -> String {
    match rate {
        Some(rate) => format!("{width} x {height} @ {}", hz(rate)),
        None => format!("{width} x {height}"),
    }
}

fn monitor_group(monitor: &Monitor) -> Group {
    let name = monitor
        .name
        .clone()
        .unwrap_or_else(|| "Display".to_string());
    let mut group = Group::new(format!("Monitor: {name}"));
    group.push_row(Row::new(
        "Current mode",
        Value::from_option(
            monitor
                .current
                .map(|(w, h)| mode_text(w, h, monitor.refresh)),
            "no active mode reported",
        ),
    ));
    group.push_row(
        Row::new(
            "Native mode",
            Value::from_option(
                monitor.native.map(|(w, h, r)| mode_text(w, h, r)),
                "the monitor reports no preferred mode",
            ),
        )
        .note("The monitor's preferred timing, as Windows reads it from EDID"),
    );
    group.push_row(Row::new(
        "Connection",
        Value::from_option(monitor.connection, "connector type not reported"),
    ));
    group.push_row(Row::new(
        "HDR (advanced color)",
        Value::from_option(
            monitor
                .hdr
                .map(|(supported, enabled)| match (supported, enabled) {
                    (true, true) => "Supported, on",
                    (true, false) => "Supported, off",
                    _ => "Not supported",
                }),
            "not reported",
        ),
    ));
    group.push_row(Row::new(
        "Bits per color channel",
        Value::from_option(
            monitor.bits.filter(|b| *b > 0).map(|b| b.to_string()),
            "not reported",
        ),
    ));
    group.push_row(
        Row::new(
            "Manufacturer ID",
            Value::from_option(monitor.manufacturer.clone(), "not in the EDID"),
        )
        .note("EDID PNP manufacturer code"),
    );
    group.push_row(Row::new(
        "Product code",
        Value::from_option(
            monitor
                .product
                .filter(|p| *p != 0)
                .map(|p| format!("{p:04X}h")),
            "not in the EDID",
        ),
    ));
    group
}

fn adapter_group(adapter: &Adapter, ordinal: u8, monitors: &[&Monitor]) -> Group {
    let nvidia = adapter.vendor_id == 0x10DE;
    let key = |metric| LiveKey::Gpu {
        adapter: GpuRef {
            name: adapter.name.clone(),
            ordinal,
        },
        metric,
    };
    let mut group = Group::new(adapter.name.clone());
    if nvidia {
        group = group.live(key(GpuMetric::Temperature));
    }
    let vendor = u16::try_from(adapter.vendor_id).ok();
    group.push_row(Row::new(
        "Manufacturer",
        Value::from_option(
            vendor.and_then(pci_vendor),
            format!(
                "PCI vendor {:04X}h is not in Trontop's table",
                adapter.vendor_id
            ),
        ),
    ));
    let device = adapter.device.clone().unwrap_or_default();
    group.push_row(
        Row::new(
            "Board partner (subsystem vendor)",
            match device.subsystem {
                Some((vendor, _)) => Value::from_option(
                    pci_vendor(vendor),
                    format!("subsystem vendor {vendor:04X}h is not in Trontop's table"),
                ),
                None => Value::unavailable("no PCI subsystem ID (integrated or virtual adapter)"),
            },
        )
        .note("PCI subsystem vendor ID"),
    );
    let mut ids = format!("{:04X}:{:04X}", adapter.vendor_id, adapter.device_id);
    if let Some((vendor, device)) = device.subsystem {
        ids.push_str(&format!(", subsystem {vendor:04X}:{device:04X}"));
    }
    if let Some(revision) = device.revision {
        ids.push_str(&format!(", revision {revision:02X}h"));
    }
    group.push_row(Row::known("PCI IDs", ids));
    if let Some(memory) = device.memory {
        group.push_row(
            Row::known("Video memory", format::bytes(memory))
                .note("Driver-reported physical memory (64-bit HardwareInformation.qwMemorySize); WMI's 32-bit AdapterRAM caps near 4 GB"),
        );
    }
    group.push_row(
        Row::known("Dedicated to the adapter", format::bytes(adapter.dedicated_video))
            .note("DXGI DedicatedVideoMemory (64-bit): video memory not reserved by the driver or firmware"),
    );
    if adapter.dedicated_system > 0 {
        group.push_row(Row::known(
            "Dedicated system memory",
            format::bytes(adapter.dedicated_system),
        ));
    }
    group.push_row(
        Row::known("Shared system memory", format::bytes(adapter.shared))
            .note("DXGI SharedSystemMemory: the most system RAM the adapter may borrow"),
    );
    group.push_row(Row::new(
        "Video BIOS",
        Value::from_option(device.bios.clone(), NOT_REPORTED_DRIVER),
    ));
    if adapter.luids.len() > 1 {
        group.push_row(
            Row::known(
                "DXGI adapter entries",
                format!("{} (one physical device)", adapter.luids.len()),
            )
            .note("DXGI lists this PCI device under several adapter LUIDs, for example for indirect or virtual displays that render on it"),
        );
    }
    group.push_row(Row::new(
        "Driver version",
        Value::from_option(device.driver_version, NOT_REPORTED_DRIVER),
    ));
    group.push_row(Row::new(
        "Driver date",
        Value::from_option(device.driver_date, NOT_REPORTED_DRIVER),
    ));
    group.push_row(Row::new(
        "Driver provider",
        Value::from_option(device.driver_provider, NOT_REPORTED_DRIVER),
    ));
    group.push_row(
        Row::new(
            "Bus interface",
            Value::from_option(
                device.link,
                "PCIe link not reported (integrated or non-PCIe adapter)",
            ),
        )
        .note("DEVPKEY_PciDevice current and maximum link speed and width"),
    );
    group.push_row(Row::new(
        "Location",
        Value::from_option(device.location, "not reported"),
    ));
    group.push_row(Row::new(
        "Status",
        Value::from_option(device.status, "device node status not reported"),
    ));
    if nvidia {
        group.push_row(Row::live("Temperature", key(GpuMetric::Temperature)));
        group.push_row(Row::live("Core clock", key(GpuMetric::CoreClock)));
        group.push_row(Row::live("Memory clock", key(GpuMetric::MemoryClock)));
        group.push_row(Row::live("Board power", key(GpuMetric::Power)));
        group.push_row(
            Row::live("Fan (target)", key(GpuMetric::FanTarget))
                .note("NVML's intended fan speed, not a tachometer reading"),
        );
        group.push_row(Row::live("Video memory in use", key(GpuMetric::MemoryUsed)));
    } else {
        group.push_row(Row::unavailable(
            "Live sensors",
            "NVML covers NVIDIA adapters only; Windows has no driver-free sensor API for this adapter",
        ));
    }
    for monitor in monitors {
        group.push_group(monitor_group(monitor));
    }
    group
}

fn build(
    adapters: Result<Vec<Adapter>, String>,
    monitors: Result<Vec<Monitor>, String>,
    others: &[(String, Option<String>)],
) -> Section {
    let mut section = Section::new(SectionId::Graphics);
    let adapters = match adapters {
        Ok(adapters) => adapters,
        Err(reason) => return Section::unavailable(SectionId::Graphics, reason),
    };
    let monitors = match monitors {
        Ok(monitors) => monitors,
        Err(reason) => {
            section.push_issue(format!("Monitors: {reason}"));
            Vec::new()
        }
    };
    let mut seen = Vec::<String>::new();
    for adapter in &adapters {
        let ordinal = seen.iter().filter(|n| **n == adapter.name).count() as u8;
        seen.push(adapter.name.clone());
        let attached = monitors
            .iter()
            .filter(|m| adapter.luids.contains(&m.adapter))
            .collect::<Vec<_>>();
        let group = adapter_group(adapter, ordinal, &attached);
        let memory = adapter
            .device
            .as_ref()
            .and_then(|d| d.memory)
            .unwrap_or(adapter.dedicated_video);
        let mut details = vec![if memory > 0 {
            format::bytes(memory)
        } else {
            "shared memory".into()
        }];
        details.extend(
            adapter
                .device
                .as_ref()
                .filter(|d| d.link.is_some())
                .and_then(|d| d.subsystem)
                .and_then(|(vendor, _)| pci_vendor(vendor))
                .filter(|p| Some(*p) != u16::try_from(adapter.vendor_id).ok().and_then(pci_vendor))
                .map(str::to_string),
        );
        let line = format!("{} ({})", adapter.name, details.join(", "));
        let mut summary = SummaryLine::known(line);
        if let Some(key) = &group.live {
            summary = summary.live(key.clone());
        }
        section.push_summary(summary);
        section.push_group(group);
    }
    for monitor in monitors
        .iter()
        .filter(|m| !adapters.iter().any(|a| a.luids.contains(&m.adapter)))
    {
        section.push_group(monitor_group(monitor));
    }
    for monitor in &monitors {
        let name = monitor.name.clone().unwrap_or_else(|| "Display".into());
        let mode = monitor
            .current
            .map(|(w, h)| format!(" ({})", mode_text(w, h, monitor.refresh)))
            .unwrap_or_default();
        section.push_summary(SummaryLine::known(format!("{name}{mode}")));
    }
    if !others.is_empty() {
        let mut group = Group::new(format!(
            "Virtual and indirect display drivers ({})",
            others.len()
        ))
        .collapsed();
        for (name, version) in others {
            group.push_row(
                Row::new(
                    name.clone(),
                    Value::from_option(version.clone(), NOT_REPORTED_DRIVER),
                )
                .note("Display-class device without its own GPU; the value is its driver version"),
            );
        }
        section.push_group(group);
    }
    if adapters.is_empty() {
        return Section::unavailable(SectionId::Graphics, "DXGI reported no hardware adapters");
    }
    section
}

/// The video BIOS version from HardwareInformation.BiosString. NVIDIA writes
/// REG_SZ, Intel writes REG_MULTI_SZ and some drivers write UTF-16 REG_BINARY.
fn bios_text(value: RegValue) -> Option<String> {
    let text = match value {
        RegValue::Text(text) => text,
        RegValue::MultiText(parts) => parts.into_iter().find(|p| !p.trim().is_empty())?,
        RegValue::Binary(bytes) => super::native::utf16_bytes_until_nul(&bytes)?,
        _ => return None,
    };
    let text = text.trim().trim_start_matches("Version").trim();
    (!text.is_empty()).then(|| text.to_string())
}

pub fn collect(ctx: &Context) -> Section {
    #[cfg(windows)]
    {
        native::collect(ctx)
    }
    #[cfg(not(windows))]
    {
        let _ = ctx;
        Section::unavailable(SectionId::Graphics, "read on Windows only")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adapter(name: &str, vendor: u32, luid: u32, vram: u64) -> Adapter {
        Adapter {
            name: name.into(),
            luids: vec![(0, luid)],
            vendor_id: vendor,
            device_id: 0x2C05,
            dedicated_video: vram,
            shared: 32 << 30,
            device: Some(Device {
                subsystem: Some((0x196E, 0x205B)),
                link: Some("PCIe 5.0 x16".into()),
                memory: (vendor == 0x10DE).then_some(17_094_934_528),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn bios_string_decodes_from_every_driver_value_kind() {
        use crate::specs::native::registry::decode;
        let utf16 = |t: &str| -> Vec<u8> { t.encode_utf16().flat_map(u16::to_le_bytes).collect() };
        // REG_SZ (NVIDIA), REG_MULTI_SZ (Intel), UTF-16 REG_BINARY.
        let sz = decode(1, &utf16("Version98.3.58.0.23\0")).unwrap();
        assert_eq!(bios_text(sz).as_deref(), Some("98.3.58.0.23"));
        let multi = decode(7, &utf16(" \0Intel Video BIOS\0\0")).unwrap();
        assert_eq!(bios_text(multi).as_deref(), Some("Intel Video BIOS"));
        let binary = decode(3, &utf16("1.2.3\0")).unwrap();
        assert_eq!(bios_text(binary).as_deref(), Some("1.2.3"));
        assert_eq!(bios_text(decode(7, &utf16("\0\0")).unwrap()), None);
        assert_eq!(bios_text(RegValue::U32(1)), None);
    }

    #[test]
    fn vram_is_64_bit_and_live_keys_use_name_and_ordinal() {
        let adapters = vec![
            adapter("Fixture GPU", 0x10DE, 1, 16 << 30),
            adapter("Fixture GPU", 0x10DE, 2, 16 << 30),
            adapter("Fixture iGPU", 0x8086, 3, 128 << 20),
        ];
        let monitors = vec![Monitor {
            adapter: (0, 1),
            name: Some("Fixture TV".into()),
            current: Some((3840, 2160)),
            refresh: Some(59.94),
            native: Some((3840, 2160, Some(60.0))),
            connection: connection(5),
            hdr: Some((true, false)),
            bits: Some(10),
            manufacturer: pnp_id(0x6D1E),
            product: Some(0x1234),
        }];
        let section = build(
            Ok(adapters),
            Ok(monitors),
            &[("Fixture Virtual Display".into(), None)],
        );
        let text = crate::specs::probe_text(std::slice::from_ref(&section));
        assert!(text.contains("Video memory: 15.9 GiB"), "{text}");
        assert!(
            text.contains("Dedicated to the adapter: 16.0 GiB"),
            "{text}"
        );
        assert!(text.contains("  Fixture GPU (15.9 GiB, PNY)"), "{text}");
        assert!(
            text.contains("Current mode: 3840 x 2160 @ 59.94 Hz"),
            "{text}"
        );
        assert!(text.contains("Native mode: 3840 x 2160 @ 60 Hz"), "{text}");
        assert!(
            text.contains("HDR (advanced color): Supported, off"),
            "{text}"
        );
        assert!(
            !text.to_ascii_lowercase().contains("shader"),
            "no invented shader clock"
        );
        assert!(
            text.contains("Live sensors: Unavailable (NVML covers NVIDIA"),
            "{text}"
        );
        assert!(
            text.contains("Virtual and indirect display drivers (1)"),
            "{text}"
        );
        let second = &section.groups[1];
        assert_eq!(
            second.live,
            Some(LiveKey::Gpu {
                adapter: GpuRef {
                    name: "Fixture GPU".into(),
                    ordinal: 1
                },
                metric: GpuMetric::Temperature
            })
        );
        assert_eq!(section.summary.len(), 4);
    }

    #[test]
    fn pnp_ids_and_connections_decode() {
        // "GSM" = 7, 19, 13 -> 0b00111_10011_01101 = 0x1E6D, byte-swapped by DisplayConfig.
        assert_eq!(pnp_id(0x6D1E).as_deref(), Some("GSM"));
        assert_eq!(pnp_id(0), None);
        assert_eq!(connection(10), Some("DisplayPort"));
        assert_eq!(connection(i32::MIN), Some("Internal"));
        assert_eq!(hz(59.94), "59.94 Hz");
        assert_eq!(hz(60.0001), "60 Hz");
        assert!(build(Ok(Vec::new()), Ok(Vec::new()), &[]).groups.is_empty());
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "Read-only Graphics specs probe; no driver, elevation, window or input"]
    fn native_specs_graphics_read_only_probe() {
        let started = std::time::Instant::now();
        let sections = [collect(&Context::probe())];
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        println!("{}", crate::specs::probe_text(&sections));
        println!("collected in {elapsed:.3} ms (private values masked)");
    }
}
