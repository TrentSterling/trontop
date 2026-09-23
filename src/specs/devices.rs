//! Audio and Peripherals sections (lane: devices).
//!
//! Scope, Audio: sound devices (SetupDi media class) with driver details, and
//! playback/recording endpoints (MMDevice API, default device, format).
//! Peripherals: keyboards, mice/HID, USB controllers and attached USB devices
//! (bus-reported names), Bluetooth radios and paired devices, printers,
//! cameras. Instance IDs and serials are private.
use super::native::setupapi::DeviceInfo;
use super::{Context, Group, Row, Section, SectionId, SummaryLine, Value};

#[cfg(windows)]
mod native;

const NOT_REPORTED_DEVICE: &str = "not reported by the device or its driver";

/// One MMDevice endpoint.
#[derive(Clone, Debug, Default, PartialEq)]
struct Endpoint {
    name: String,
    capture: bool,
    default: bool,
    /// Shared-mode engine format: (sample rate, bits, channels, float).
    format: Option<(u32, u16, u16, bool)>,
}

/// WAVEFORMATEX / WAVEFORMATEXTENSIBLE bytes from PKEY_AudioEngine_DeviceFormat.
fn parse_format(blob: &[u8]) -> Option<(u32, u16, u16, bool)> {
    let word = |at: usize| {
        blob.get(at..at + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
    };
    let tag = word(0)?;
    let channels = word(2)?;
    let rate = u32::from_le_bytes(blob.get(4..8)?.try_into().ok()?);
    let mut bits = word(14)?;
    let mut float = tag == 3;
    if tag == 0xFFFE {
        // WAVEFORMATEXTENSIBLE: valid bits, channel mask, then the subformat GUID
        // whose first dword is the format tag (1 PCM, 3 IEEE float).
        if let Some(valid) = word(18).filter(|v| *v > 0) {
            bits = valid;
        }
        float = blob
            .get(24..28)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            == Some(3);
    }
    (rate > 0 && channels > 0).then_some((rate, bits, channels, float))
}

fn format_text((rate, bits, channels, float): (u32, u16, u16, bool)) -> String {
    let rate = if rate.is_multiple_of(1000) {
        format!("{} kHz", rate / 1000)
    } else {
        format!("{:.1} kHz", f64::from(rate) / 1000.0)
    };
    let channels = match channels {
        1 => "mono".to_string(),
        2 => "stereo".to_string(),
        6 => "5.1".to_string(),
        8 => "7.1".to_string(),
        n => format!("{n} channels"),
    };
    format!(
        "{rate}, {bits}-bit{}, {channels}",
        if float { " float" } else { "" }
    )
}

fn driver_text(device: &DeviceInfo) -> Value {
    Value::from_option(
        device.driver_version.clone().map(|version| {
            let mut text = version;
            if let Some(date) = &device.driver_date {
                text.push_str(&format!(" ({date})"));
            }
            if let Some(provider) = &device.driver_provider {
                text.push_str(&format!(", {provider}"));
            }
            text
        }),
        NOT_REPORTED_DEVICE,
    )
}

fn build_audio(
    devices: Result<Vec<DeviceInfo>, String>,
    endpoints: Result<Vec<Endpoint>, String>,
) -> Section {
    let mut section = Section::new(SectionId::Audio);
    match &endpoints {
        Ok(list) => {
            for (capture, label) in [(false, "Default playback"), (true, "Default recording")] {
                if let Some(endpoint) = list.iter().find(|e| e.capture == capture && e.default) {
                    section.push_summary(SummaryLine::known(format!("{label}: {}", endpoint.name)));
                }
            }
        }
        Err(reason) => section.push_issue(format!("Audio endpoints: {reason}")),
    }
    match devices {
        Ok(devices) => {
            if section.summary.is_empty()
                && let Some(first) = devices.first().and_then(|d| d.name())
            {
                section.push_summary(SummaryLine::known(first));
            }
            let mut group = Group::new(format!("Sound devices ({})", devices.len()));
            for device in &devices {
                let name = device.name().unwrap_or("Sound device").to_string();
                group.push_group(
                    Group::new(name)
                        .collapsed()
                        .kv(
                            "Manufacturer",
                            Value::from_option(device.manufacturer.clone(), NOT_REPORTED_DEVICE),
                        )
                        .kv("Driver", driver_text(device))
                        .kv(
                            "Status",
                            Value::from_option(
                                device
                                    .status
                                    .map(super::native::setupapi::DevNodeStatus::label),
                                NOT_REPORTED_DEVICE,
                            ),
                        ),
                );
            }
            section.push_group(group);
        }
        Err(reason) => section.push_issue(format!("Sound devices: {reason}")),
    }
    if let Ok(list) = endpoints {
        for (capture, title) in [(false, "Playback"), (true, "Recording")] {
            let matching = list
                .iter()
                .filter(|e| e.capture == capture)
                .collect::<Vec<_>>();
            let mut group = Group::new(format!("{title} endpoints ({})", matching.len()));
            if matching.is_empty() {
                group.push_row(Row::known("Active endpoints", "None"));
            }
            for endpoint in matching {
                let mut label = endpoint.name.clone();
                if endpoint.default {
                    label.push_str(" (default)");
                }
                group.push_row(
                    Row::new(
                        label,
                        Value::from_option(
                            endpoint.format.map(format_text),
                            "engine format not reported",
                        ),
                    )
                    .note("Shared-mode engine format from PKEY_AudioEngine_DeviceFormat"),
                );
            }
            section.push_group(group);
        }
    }
    if section.groups.is_empty() {
        let reason = section
            .issues
            .first()
            .cloned()
            .unwrap_or_else(|| "no audio information".into());
        return Section::unavailable(SectionId::Audio, reason);
    }
    section
}

/// How a device reaches the PC, from its ancestors' instance IDs.
fn connection(chain: &[&DeviceInfo]) -> Option<&'static str> {
    chain.iter().find_map(|d| {
        let id = d.instance_id.to_ascii_uppercase();
        if id.starts_with("BTHENUM\\")
            || id.starts_with("BTHLE\\")
            || id.starts_with("BTHLEDEVICE\\")
        {
            Some("Bluetooth")
        } else if id.starts_with("USB\\") {
            Some("USB")
        } else if id.starts_with("ACPI\\") {
            Some("Built-in (ACPI)")
        } else {
            None
        }
    })
}

/// Peripherals, from one SetupDi snapshot of every present device.
#[derive(Clone, Debug, Default)]
struct Inventory {
    devices: Vec<DeviceInfo>,
    /// Parent instance ID for each device (same order).
    parents: Vec<Option<String>>,
}

impl Inventory {
    fn chain(&self, index: usize) -> Vec<&DeviceInfo> {
        let mut chain = vec![&self.devices[index]];
        let mut current = index;
        for _ in 0..8 {
            let Some(parent) = self.parents.get(current).and_then(|p| p.as_deref()) else {
                break;
            };
            let Some(next) = self
                .devices
                .iter()
                .position(|d| d.instance_id.eq_ignore_ascii_case(parent))
            else {
                break;
            };
            chain.push(&self.devices[next]);
            current = next;
        }
        chain
    }

    fn of_class(&self, class: &str) -> Vec<usize> {
        (0..self.devices.len())
            .filter(|i| {
                self.devices[*i]
                    .class
                    .as_deref()
                    .is_some_and(|c| c.eq_ignore_ascii_case(class))
            })
            .collect()
    }

    /// The bus-reported product name nearest the device, then its own name.
    fn product(&self, index: usize) -> (String, Option<&'static str>) {
        let chain = self.chain(index);
        let name = chain
            .iter()
            .take(3)
            .find_map(|d| d.bus_description.clone())
            .or_else(|| self.devices[index].name().map(str::to_string))
            .unwrap_or_else(|| "Device".into());
        (name, connection(&chain))
    }
}

fn device_rows(inventory: &Inventory, indices: &[usize], label: &str) -> Vec<Row> {
    let mut rows = indices
        .iter()
        .map(|index| {
            let (name, via) = inventory.product(*index);
            let text = match via {
                Some(via) => format!("{name} ({via})"),
                None => name,
            };
            let row = Row::known(label, text).note(format!(
                "Windows name: {}",
                inventory.devices[*index].name().unwrap_or("not reported")
            ));
            // Bluetooth names are set by their owners and often personal.
            if via == Some("Bluetooth") {
                row.private()
            } else {
                row
            }
        })
        .collect::<Vec<_>>();
    rows.dedup_by(|a, b| a.value == b.value && !a.private);
    rows
}

fn build_peripherals(
    inventory: Result<Inventory, String>,
    printers: Result<Vec<(String, Option<String>, bool)>, String>,
) -> Section {
    let inventory = match inventory {
        Ok(inventory) => inventory,
        Err(reason) => return Section::unavailable(SectionId::Peripherals, reason),
    };
    let mut section = Section::new(SectionId::Peripherals);
    let keyboards = inventory.of_class("Keyboard");
    let mice = inventory.of_class("Mouse");
    section.push_summary(SummaryLine::known(format!(
        "{} keyboard{}, {} pointing device{}",
        keyboards.len(),
        if keyboards.len() == 1 { "" } else { "s" },
        mice.len(),
        if mice.len() == 1 { "" } else { "s" }
    )));
    let mut input = Group::new("Keyboards and mice");
    input.items.extend(
        device_rows(&inventory, &keyboards, "Keyboard")
            .into_iter()
            .map(super::Item::Row),
    );
    input.items.extend(
        device_rows(&inventory, &mice, "Pointing device")
            .into_iter()
            .map(super::Item::Row),
    );
    if input.items.is_empty() {
        input.push_row(Row::known("Devices", "None present"));
    }
    section.push_group(input);

    let controllers = inventory
        .of_class("HIDClass")
        .into_iter()
        .filter(|i| {
            inventory.devices[*i]
                .name()
                .is_some_and(|n| n.to_ascii_lowercase().contains("game controller"))
        })
        .collect::<Vec<_>>();
    if !controllers.is_empty() {
        section.push_group(Group::new("Game controllers").rows(device_rows(
            &inventory,
            &controllers,
            "Controller",
        )));
    }

    // USB: host controllers, then attached devices (not hubs or interfaces).
    let usb = inventory.of_class("USB");
    let hosts = usb
        .iter()
        .filter(|i| {
            inventory.devices[**i].name().is_some_and(|n| {
                let n = n.to_ascii_lowercase();
                n.contains("host controller") || n.contains("host router")
            })
        })
        .copied()
        .collect::<Vec<_>>();
    let attached = (0..inventory.devices.len())
        .filter(|i| {
            let d = &inventory.devices[*i];
            let id = d.instance_id.to_ascii_uppercase();
            id.starts_with("USB\\")
                && !id.contains("&MI_")
                && !d
                    .name()
                    .is_some_and(|n| n.to_ascii_lowercase().contains("hub"))
                && !id.starts_with("USB\\ROOT_HUB")
        })
        .collect::<Vec<_>>();
    let mut usb_group = Group::new(format!("USB ({} devices)", attached.len()));
    for host in &hosts {
        let d = &inventory.devices[*host];
        usb_group.push_row(Row::new(
            d.name().unwrap_or("USB controller").to_string(),
            driver_text(d),
        ));
    }
    let mut devices = Group::new("Attached USB devices").collapsed();
    for index in &attached {
        let (name, _) = inventory.product(*index);
        let d = &inventory.devices[*index];
        devices.push_row(
            Row::known(name, d.class.clone().unwrap_or_else(|| "Device".into())).note(format!(
                "Windows name: {}",
                d.name().unwrap_or("not reported")
            )),
        );
    }
    if !devices.items.is_empty() {
        usb_group.push_group(devices);
    }
    section.push_group(usb_group);

    let bluetooth = inventory.of_class("Bluetooth");
    let radios = bluetooth
        .iter()
        .filter(|i| {
            let id = inventory.devices[**i].instance_id.to_ascii_uppercase();
            !id.starts_with("BTH") && !id.starts_with("SWD\\")
        })
        .copied()
        .collect::<Vec<_>>();
    let paired = bluetooth
        .iter()
        .filter(|i| {
            let id = inventory.devices[**i].instance_id.to_ascii_uppercase();
            id.starts_with("BTHENUM\\DEV_") || id.starts_with("BTHLE\\DEV_")
        })
        .copied()
        .collect::<Vec<_>>();
    let mut bt = Group::new("Bluetooth");
    if radios.is_empty() {
        bt.push_row(Row::known("Radio", "None present"));
    }
    for radio in &radios {
        let d = &inventory.devices[*radio];
        bt.push_row(Row::new(
            format!("Radio: {}", d.name().unwrap_or("Bluetooth radio")),
            driver_text(d),
        ));
    }
    bt.push_row(Row::known("Paired devices", paired.len().to_string()));
    for index in &paired {
        let d = &inventory.devices[*index];
        bt.push_row(
            Row::new(
                "Paired device",
                Value::from_option(d.name().map(str::to_string), NOT_REPORTED_DEVICE),
            )
            .private()
            .note("Bluetooth device names are set by their owners"),
        );
    }
    section.push_group(bt);

    let cameras = [inventory.of_class("Camera"), inventory.of_class("Image")].concat();
    let mut camera_group = Group::new("Cameras and scanners");
    if cameras.is_empty() {
        camera_group.push_row(Row::known("Devices", "None present"));
    }
    camera_group.items.extend(
        device_rows(&inventory, &cameras, "Device")
            .into_iter()
            .map(super::Item::Row),
    );
    section.push_group(camera_group);

    let mut printer_group = Group::new("Printers");
    match printers {
        Ok(list) if list.is_empty() => {
            printer_group.push_row(Row::known("Printers", "None installed"))
        }
        Ok(list) => {
            for (name, driver, default) in list {
                let label = if default {
                    format!("{name} (default)")
                } else {
                    name
                };
                printer_group.push_row(Row::new(
                    label,
                    Value::from_option(driver, "driver not reported"),
                ));
            }
        }
        Err(reason) => printer_group.push_row(Row::unavailable("Printers", reason)),
    }
    section.push_group(printer_group);
    section
}

pub fn collect_audio(ctx: &Context) -> Section {
    #[cfg(windows)]
    {
        build_audio(native::sound_devices(ctx), native::endpoints())
    }
    #[cfg(not(windows))]
    {
        let _ = ctx;
        build_audio(
            Err("read on Windows only".into()),
            Err("read on Windows only".into()),
        )
    }
}

pub fn collect_peripherals(ctx: &Context) -> Section {
    #[cfg(windows)]
    {
        let inventory = native::inventory(ctx);
        let printers = if ctx.should_stop() {
            Err("read budget exhausted".into())
        } else {
            native::printers(ctx)
        };
        build_peripherals(inventory, printers)
    }
    #[cfg(not(windows))]
    {
        let _ = ctx;
        build_peripherals(Err("read on Windows only".into()), Ok(Vec::new()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(id: &str, class: &str, name: &str, bus: Option<&str>) -> DeviceInfo {
        DeviceInfo {
            instance_id: id.into(),
            class: Some(class.into()),
            friendly_name: Some(name.into()),
            bus_description: bus.map(str::to_string),
            ..Default::default()
        }
    }

    #[test]
    fn wave_formats_decode_extensible_float() {
        let mut blob = vec![0u8; 40];
        blob[0..2].copy_from_slice(&0xFFFEu16.to_le_bytes());
        blob[2..4].copy_from_slice(&2u16.to_le_bytes());
        blob[4..8].copy_from_slice(&48_000u32.to_le_bytes());
        blob[14..16].copy_from_slice(&32u16.to_le_bytes());
        blob[18..20].copy_from_slice(&32u16.to_le_bytes());
        blob[24..28].copy_from_slice(&3u32.to_le_bytes());
        let parsed = parse_format(&blob).unwrap();
        assert_eq!(format_text(parsed), "48 kHz, 32-bit float, stereo");
        assert_eq!(parse_format(&blob[..6]), None);
        let pcm = [1u8, 0, 1, 0, 0x44, 0xAC, 0, 0, 0, 0, 0, 0, 0, 0, 16, 0];
        assert_eq!(
            format_text(parse_format(&pcm).unwrap()),
            "44.1 kHz, 16-bit, mono"
        );
    }

    #[test]
    fn audio_lists_defaults_devices_and_endpoints() {
        let devices = vec![device(
            "HDAUDIO\\FIXTURE",
            "MEDIA",
            "Fixture HD Audio",
            None,
        )];
        let endpoints = vec![
            Endpoint {
                name: "Fixture Speakers".into(),
                default: true,
                format: Some((48_000, 24, 2, false)),
                ..Default::default()
            },
            Endpoint {
                name: "Fixture Mic".into(),
                capture: true,
                ..Default::default()
            },
        ];
        let section = build_audio(Ok(devices), Ok(endpoints));
        let text = crate::specs::probe_text(std::slice::from_ref(&section));
        assert!(
            text.contains("Default playback: Fixture Speakers"),
            "{text}"
        );
        assert!(
            text.contains("Fixture Speakers (default): 48 kHz, 24-bit, stereo"),
            "{text}"
        );
        assert!(
            text.contains("Fixture Mic: Unavailable (engine format not reported)"),
            "{text}"
        );
    }

    #[test]
    fn peripherals_use_bus_names_and_hide_bluetooth_names() {
        let devices = vec![
            device(
                "USB\\VID_046D&PID_C08B\\PRIVATE-SERIAL",
                "USB",
                "USB Composite Device",
                Some("Fixture G502"),
            ),
            device(
                "HID\\VID_046D&PID_C08B&MI_00\\7&1",
                "Mouse",
                "HID-compliant mouse",
                None,
            ),
            device(
                "BTHLE\\DEV_PRIVATEADDRESS\\1",
                "Bluetooth",
                "Fixture Owner's Mouse",
                None,
            ),
            device(
                "USB\\VID_8087&PID_0036\\5&1",
                "Bluetooth",
                "Fixture Bluetooth Adapter",
                None,
            ),
        ];
        let parents = vec![
            None,
            Some("USB\\VID_046D&PID_C08B\\PRIVATE-SERIAL".into()),
            None,
            None,
        ];
        let section = build_peripherals(Ok(Inventory { devices, parents }), Ok(Vec::new()));
        let text = crate::specs::probe_text(std::slice::from_ref(&section));
        assert!(
            text.contains("Pointing device: Fixture G502 (USB)"),
            "{text}"
        );
        assert!(text.contains("Radio: Fixture Bluetooth Adapter"), "{text}");
        assert!(text.contains("Paired devices: 1"), "{text}");
        assert!(
            !text.contains("Fixture Owner's Mouse") && !text.contains("PRIVATE"),
            "{text}"
        );
        assert!(text.contains("Printers: None installed"), "{text}");
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "Read-only Audio and Peripherals specs probe; no driver, elevation, window or input"]
    fn native_specs_devices_read_only_probe() {
        let started = std::time::Instant::now();
        let sections = [
            collect_audio(&Context::probe()),
            collect_peripherals(&Context::probe()),
        ];
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        println!("{}", crate::specs::probe_text(&sections));
        println!("collected in {elapsed:.3} ms (private values masked)");
    }
}
